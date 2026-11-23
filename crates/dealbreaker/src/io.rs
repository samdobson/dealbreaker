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
