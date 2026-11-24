//! Positions as data: a JSON-friendly form, validation, and well-known deal numbering.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::card::{Card, Suit};
use crate::game::{Game, State};

/// A position keyed by group name. Each group is a list of piles, each pile a list of cards
/// bottom first, written like `"AS"`, `"10H"` or `"~KC"` (face down).
///
/// ```json
/// { "piles": { "tableau": [["~5H", "KS"], ["QD"]], "foundation": [[], [], [], []] } }
/// ```
///
/// Groups left out are empty.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct StateData {
    pub piles: BTreeMap<String, Vec<Vec<String>>>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub redeals_used: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_rank: Option<u8>,
    /// Seed for shuffles during play (shuffling redeals). Part of the deal.
    #[serde(default, skip_serializing_if = "is_zero64")]
    pub seed: u64,
}

fn is_zero64(n: &u64) -> bool {
    *n == 0
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// A position that doesn't fit the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateError(pub String);

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid position: {}", self.0)
    }
}

impl std::error::Error for StateError {}

impl Game {
    pub fn state_to_data(&self, s: &State) -> StateData {
        let mut piles = BTreeMap::new();
        for g in self.groups.iter().filter(|g| !g.hidden) {
            let v: Vec<Vec<String>> = g
                .piles()
                .map(|p| s.piles[p as usize].iter().map(|c| c.to_string()).collect())
                .collect();
            piles.insert(g.name.clone(), v);
        }
        StateData {
            piles,
            redeals_used: s.redeals_used,
            base_rank: s.base_rank,
            seed: s.seed,
        }
    }

    pub fn state_from_data(&self, d: &StateData) -> Result<State, StateError> {
        let mut s = State {
            piles: vec![Vec::new(); self.num_piles()],
            redeals_used: d.redeals_used,
            base_rank: d.base_rank,
            seed: d.seed,
        };
        for (name, piles) in &d.piles {
            let g = self
                .groups
                .iter()
                .find(|g| &g.name == name && !g.hidden)
                .ok_or_else(|| StateError(format!("no group named {name:?}")))?;
            if piles.len() > g.count as usize {
                return Err(StateError(format!(
                    "{name:?} has {} piles, not {}",
                    g.count,
                    piles.len()
                )));
            }
            for (i, pile) in piles.iter().enumerate() {
                for c in pile {
                    let card: Card = c.parse().map_err(|e| StateError(format!("{e}")))?;
                    s.piles[g.start as usize + i].push(card);
                }
            }
        }
        self.validate(&s)?;
        Ok(s)
    }

    /// Parse a position for [`crate::hint()`], allowing `"?"` or `"~?"` for unknown face-down cards.
    ///
    /// Concrete cards must form a valid subset of the game's pack and the number of `"?"` slots
    /// must match the remaining unseen cards. Unknown slots are filled face down with the
    /// remaining unseen cards in deterministic order so [`crate::hint()`] can shuffle them across
    /// all face-down slots. Use [`Game::state_from_data`] when every card must be known (for
    /// [`crate::solve`] and [`crate::maximize`]).
    pub fn hint_state_from_data(&self, d: &StateData) -> Result<State, StateError> {
        let mut s = State {
            piles: vec![Vec::new(); self.num_piles()],
            redeals_used: d.redeals_used,
            base_rank: d.base_rank,
            seed: d.seed,
        };
        let mut unknown_slots: Vec<(usize, usize)> = Vec::new();
        let mut concrete: Vec<Card> = Vec::new();
        for (name, piles) in &d.piles {
            let g = self
                .groups
                .iter()
                .find(|g| &g.name == name && !g.hidden)
                .ok_or_else(|| StateError(format!("no group named {name:?}")))?;
            if piles.len() > g.count as usize {
                return Err(StateError(format!(
                    "{name:?} has {} piles, not {}",
                    g.count,
                    piles.len()
                )));
            }
            for (i, pile) in piles.iter().enumerate() {
                let p = g.start as usize + i;
                for c in pile {
                    let t = c.trim();
                    if t == "?" || t == "~?" {
                        let idx = s.piles[p].len();
                        unknown_slots.push((p, idx));
                        s.piles[p].push(Card::from_face(0).down());
                    } else {
                        let card: Card = c.parse().map_err(|e| StateError(format!("{e}")))?;
                        concrete.push(card.up());
                        s.piles[p].push(card);
                    }
                }
            }
        }
        if !unknown_slots.is_empty() {
            let mut want = self.pack_cards();
            want.sort();
            concrete.sort();
            let extra = diff(&concrete, &want);
            if !extra.is_empty() {
                let names: Vec<String> = extra.iter().map(|c| c.to_string()).collect();
                return Err(StateError(format!(
                    "cards don't match the pack (extra: [{}])",
                    names.join(" ")
                )));
            }
            let remaining = diff(&want, &concrete);
            if remaining.len() != unknown_slots.len() {
                return Err(StateError(format!(
                    "found {} unknown \"?\" card(s), expected {} to complete the pack",
                    unknown_slots.len(),
                    remaining.len()
                )));
            }
            for ((p, idx), card) in unknown_slots.into_iter().zip(remaining) {
                s.piles[p][idx] = card.down();
            }
        }
        self.validate(&s)?;
        Ok(s)
    }

    /// Check a position holds exactly the game's pack, in piles of legal sizes.
    pub fn validate(&self, s: &State) -> Result<(), StateError> {
        if s.piles.len() != self.num_piles() {
            return Err(StateError(format!(
                "{} piles, expected {}",
                s.piles.len(),
                self.num_piles()
            )));
        }
        let mut want = self.pack_cards();
        let mut got: Vec<Card> = s.piles.iter().flatten().map(|c| c.up()).collect();
        want.sort();
        got.sort();
        if want != got {
            let missing: Vec<String> = diff(&want, &got).iter().map(|c| c.to_string()).collect();
            let extra: Vec<String> = diff(&got, &want).iter().map(|c| c.to_string()).collect();
            return Err(StateError(format!(
                "cards don't match the pack (missing: [{}], extra: [{}])",
                missing.join(" "),
                extra.join(" ")
            )));
        }
        for (p, pile) in s.piles.iter().enumerate() {
            if pile.len() > self.rule(p as u16).max_cards as usize || pile.len() > 255 {
                return Err(StateError(format!(
                    "{} holds too many cards",
                    self.pile_ref(p as u16)
                )));
            }
        }
        Ok(())
    }

    #[cfg(feature = "json")]
    pub fn state_from_json(&self, json: &str) -> Result<State, StateError> {
        let d: StateData = serde_json::from_str(json).map_err(|e| StateError(e.to_string()))?;
        self.state_from_data(&d)
    }

    /// JSON form of [`Game::hint_state_from_data`].
    #[cfg(feature = "json")]
    pub fn hint_state_from_json(&self, json: &str) -> Result<State, StateError> {
        let d: StateData = serde_json::from_str(json).map_err(|e| StateError(e.to_string()))?;
        self.hint_state_from_data(&d)
    }

    #[cfg(feature = "json")]
    pub fn state_to_json(&self, s: &State) -> String {
        serde_json::to_string_pretty(&self.state_to_data(s)).expect("state serializes")
    }
}

/// Cards in `a` not matched in `b` (both sorted).
fn diff(a: &[Card], b: &[Card]) -> Vec<Card> {
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() {
        if j < b.len() && a[i] == b[j] {
            i += 1;
            j += 1;
        } else if j < b.len() && b[j] < a[i] {
            j += 1;
        } else {
            out.push(a[i]);
            i += 1;
        }
    }
    out
}

/// The card order of Microsoft FreeCell deal `n` (1 to 2³¹−1, the numbering every FreeCell
/// program shares). Dealt in rows of eight, as [`crate::presets::freecell`] deals with
/// [`Game::deal_ordered`].
pub fn microsoft_deal(n: u32) -> Vec<Card> {
    let mut seed = n;
    let mut rnd = || {
        seed = seed.wrapping_mul(214_013).wrapping_add(2_531_011) & 0x7fff_ffff;
        seed >> 16
    };
    let mut t: Vec<u32> = (0..52).map(|i| 51 - i).collect();
    for i in 0..51 {
        let j = 51 - (rnd() % (52 - i as u32)) as usize;
        t.swap(i, j);
    }
    t.into_iter()
        .map(|c| Card::new(Suit::from_index((c % 4) as u8), (c / 4 + 1) as u8))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    #[test]
    fn state_validation_errors() {
        let g = Game::new(presets::freecell()).unwrap();
        let mut s = g.deal(1);
        s.piles.pop();
        let err = g.validate(&s).unwrap_err();
        assert!(err.to_string().contains("expected"), "{err}");

        let mut d = g.state_to_data(&g.deal(1));
        d.piles.insert("bogus".into(), vec![]);
        let err = g.state_from_data(&d).unwrap_err();
        assert!(err.0.contains("no group"), "{err}");

        let mut d = g.state_to_data(&g.deal(1));
        d.piles.get_mut("cell").unwrap().push(vec![]);
        let err = g.state_from_data(&d).unwrap_err();
        assert!(err.0.contains("4 piles"), "{err}");

        let g_klondike = Game::new(presets::klondike1()).unwrap();
        let mut s_k = g_klondike.deal(1);
        let top = s_k.piles[g_klondike.pile_index("stock", 0).unwrap() as usize]
            .pop()
            .unwrap();
        s_k.piles[g_klondike.pile_index("foundation", 0).unwrap() as usize].push(top);
        // Move all 24 stock cards? Easier: check freecell cell max_cards = 1.
        let mut s_fc = g.deal(1);
        let c0 = s_fc.piles[g.pile_index("tableau", 0).unwrap() as usize]
            .pop()
            .unwrap();
        let c1 = s_fc.piles[g.pile_index("tableau", 0).unwrap() as usize]
            .pop()
            .unwrap();
        let cell0 = g.pile_index("cell", 0).unwrap() as usize;
        s_fc.piles[cell0].push(c0);
        s_fc.piles[cell0].push(c1);
        let err = g.validate(&s_fc).unwrap_err();
        assert!(err.0.contains("too many cards"), "{err}");
    }

    #[cfg(feature = "json")]
    #[test]
    fn state_from_json_rejects_bad_json() {
        let g = Game::new(presets::freecell()).unwrap();
        assert!(g.state_from_json("{bad").is_err());
    }

    #[test]
    fn hint_state_accepts_unknown_placeholders_and_state_rejects_them() {
        let g = Game::new(presets::klondike1()).unwrap();
        let deal = g.deal(2);
        let mut data = g.state_to_data(&deal);
        // Replace every face-down card with "?" or "~?".
        let mut alt = false;
        for piles in data.piles.values_mut() {
            for pile in piles {
                for card in pile {
                    if card.starts_with('~') {
                        *card = if alt { "~?".into() } else { "?".into() };
                        alt = !alt;
                    }
                }
            }
        }
        // Strict state_from_data rejects "?" so solve()/maximize() never fabricate hidden cards.
        assert!(g.state_from_data(&data).is_err());

        // hint_state_from_data fills unseen cards deterministically and passes validate().
        let s1 = g.hint_state_from_data(&data).unwrap();
        let s2 = g.hint_state_from_data(&data).unwrap();
        assert_eq!(s1, s2);
        assert!(g.validate(&s1).is_ok());

        // Wrong number of "?" slots is rejected.
        let mut bad_count = data.clone();
        bad_count.piles.get_mut("stock").unwrap()[0].pop();
        let err = g.hint_state_from_data(&bad_count).unwrap_err();
        assert!(err.0.contains("unknown \"?\""), "{err}");

        // Duplicate visible card alongside "?" is rejected.
        let mut dup = data.clone();
        let first_vis = dup.piles["tableau"][0][0].clone();
        dup.piles.get_mut("stock").unwrap()[0][0] = first_vis;
        let err = g.hint_state_from_data(&dup).unwrap_err();
        assert!(err.0.contains("extra"), "{err}");
    }
}
