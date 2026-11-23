//! Initial dealing from a seed or an explicit card order.

use crate::card::{Card, REAL_FACES};
use crate::rng::Rng;

use super::{err, Game, RulesError, State};

impl Game {
    // ----- Dealing --------------------------------------------------------------------------

    /// All the cards of the pack, unshuffled.
    pub fn pack_cards(&self) -> Vec<Card> {
        let p = &self.rules.pack;
        let mut v = Vec::with_capacity(p.len());
        for _ in 0..p.copies {
            for &s in &p.suits {
                for &r in &p.ranks {
                    v.push(Card::new(s, r));
                }
            }
        }
        v.extend((0..p.jokers).map(|_| Card::joker()));
        v
    }

    /// Deal a game from a seed. The same seed always gives the same deal for the same rules.
    pub fn deal(&self, seed: u64) -> State {
        self.try_deal(seed)
            .expect("rules were validated when compiled")
    }

    pub(super) fn try_deal(&self, seed: u64) -> Result<State, RulesError> {
        let mut deck = self.pack_cards();
        Rng::new(seed).shuffle(&mut deck);
        let mut s = self.deal_from(deck)?;
        s.seed = seed;
        Ok(s)
    }

    /// Deal an explicit card order. `deck[0]` is dealt first.
    pub fn deal_ordered(&self, deck: &[Card]) -> Result<State, RulesError> {
        let d = deck.to_vec();
        let mut expect = self.pack_cards();
        let mut got: Vec<Card> = d.iter().map(|c| c.up()).collect();
        expect.sort();
        got.sort();
        if expect != got {
            return err("deck is not exactly the game's pack");
        }
        self.deal_from(d)
    }

    fn deal_from(&self, deck: Vec<Card>) -> Result<State, RulesError> {
        let mut deck: std::collections::VecDeque<Card> = deck.into_iter().map(|c| c.up()).collect();
        let mut s = State {
            piles: vec![Vec::new(); self.num_piles()],
            redeals_used: 0,
            base_rank: None,
            seed: 0,
        };
        for step in &self.rules.deal {
            let gi = self
                .groups
                .iter()
                .position(|g| g.name == step.to)
                .ok_or_else(|| RulesError(format!("deal: no group named {:?}", step.to)))?;
            let g = &self.groups[gi];
            let piles: Vec<u16> = match &step.piles {
                None => g.piles().collect(),
                Some(ix) => {
                    let mut v = Vec::new();
                    for &i in ix {
                        if i >= g.count {
                            return err(format!("deal: {:?} has no pile {i}", step.to));
                        }
                        v.push(g.start + i);
                    }
                    v
                }
            };
            if piles.is_empty() {
                return err("deal: no piles selected");
            }
            let orient = |c: Card| if step.face_up { c } else { c.down() };
            let mut first: Option<Card> = None;
            let mut put = |s: &mut State, p: u16, c: Card| {
                first.get_or_insert(c);
                s.piles[p as usize].push(orient(c));
            };
            if step.of_base {
                let base = s.base_rank.ok_or_else(|| {
                    RulesError("deal: of_base requires an earlier set_base step".into())
                })?;
                let limit = step.count.map(|c| c as usize * piles.len());
                let mut taken = Vec::new();
                deck.retain(|c| {
                    if c.rank() == base && limit.is_none_or(|m| taken.len() < m) {
                        taken.push(*c);
                        false
                    } else {
                        true
                    }
                });
                for (i, c) in taken.into_iter().enumerate() {
                    put(&mut s, piles[i % piles.len()], c);
                }
            } else if let (Some(ranks), true) = (&step.ranks, step.one_each) {
                for (i, &r) in ranks.iter().enumerate() {
                    let at = deck.iter().position(|c| c.rank() == r).ok_or_else(|| {
                        RulesError(format!("deal: no card of rank {r} left to take"))
                    })?;
                    let c = deck.remove(at).unwrap();
                    put(&mut s, piles[i % piles.len()], c);
                }
            } else if let Some(ranks) = &step.ranks {
                let pile_suit = |p: u16| -> Option<crate::card::Suit> {
                    let r = self.rule(p);
                    if r.empty_suit.is_some() {
                        return r.empty_suit;
                    }
                    let real = r.admits & REAL_FACES;
                    crate::card::Suit::ALL
                        .into_iter()
                        .find(|&s| real == (self.suit_faces[s as usize] & REAL_FACES))
                };
                if let Some(n) = step.count {
                    for _ in 0..n {
                        for &p in &piles {
                            let want_suit = pile_suit(p);
                            let at = deck
                                .iter()
                                .position(|c| {
                                    ranks.contains(&c.rank())
                                        && want_suit.is_none_or(|s| c.suit() == s)
                                })
                                .ok_or_else(|| {
                                    RulesError(format!("deal: no matching card left for {p}"))
                                })?;
                            let c = deck.remove(at).unwrap();
                            put(&mut s, p, c);
                        }
                    }
                } else {
                    let mut taken = Vec::new();
                    deck.retain(|c| {
                        if ranks.contains(&c.rank()) {
                            taken.push(*c);
                            false
                        } else {
                            true
                        }
                    });
                    let has_suit = piles.iter().any(|&p| pile_suit(p).is_some());
                    if has_suit {
                        let mut used = vec![0usize; piles.len()];
                        for c in taken {
                            let slot = (0..piles.len())
                                .filter(|&i| pile_suit(piles[i]) == Some(c.suit()))
                                .min_by_key(|&i| used[i])
                                .unwrap_or(0);
                            used[slot] += 1;
                            put(&mut s, piles[slot], c);
                        }
                    } else {
                        for (i, c) in taken.into_iter().enumerate() {
                            put(&mut s, piles[i % piles.len()], c);
                        }
                    }
                }
            } else if step.rest {
                let mut i = 0;
                while let Some(c) = deck.pop_front() {
                    put(&mut s, piles[i % piles.len()], c);
                    i += 1;
                }
            } else {
                let counts: Vec<u16> = match (&step.counts, step.count) {
                    (Some(c), _) => {
                        if c.len() != piles.len() {
                            return err(format!(
                                "deal to {:?}: counts has the wrong length",
                                step.to
                            ));
                        }
                        c.clone()
                    }
                    (None, Some(n)) => vec![n; piles.len()],
                    (None, None) if step.collect.is_some() => vec![0; piles.len()],
                    (None, None) => {
                        return err(format!(
                            "deal to {:?}: give count, counts, rest or ranks",
                            step.to
                        ))
                    }
                };
                let rows = counts.iter().copied().max().unwrap_or(0);
                for r in 0..rows {
                    for (i, &p) in piles.iter().enumerate() {
                        if counts[i] > r {
                            let c = deck
                                .pop_front()
                                .ok_or_else(|| RulesError("deal: ran out of cards".into()))?;
                            put(&mut s, p, c);
                        }
                    }
                }
            }
            if step.kings_to_bottom {
                for &p in &piles {
                    let pile = &mut s.piles[p as usize];
                    let (mut k, rest): (Vec<Card>, Vec<Card>) =
                        pile.iter().partition(|c| c.rank() == 13);
                    k.extend(rest);
                    *pile = k;
                }
            }
            if let Some(ranks) = &step.collect {
                let mut taken = Vec::new();
                for (p, pile) in s.piles.iter_mut().enumerate() {
                    if piles.contains(&(p as u16)) {
                        continue;
                    }
                    pile.retain(|c| {
                        let hit = ranks.contains(&c.rank());
                        if hit {
                            taken.push(c.up());
                        }
                        !hit
                    });
                }
                for (i, c) in taken.into_iter().enumerate() {
                    s.piles[piles[i % piles.len()] as usize].push(c);
                }
            }
            if step.set_base {
                s.base_rank = first.map(|c| c.rank());
            }
        }
        if !deck.is_empty() {
            return err(format!("deal leaves {} cards undealt", deck.len()));
        }
        let mut j = Vec::new();
        self.settle(&mut s, &mut j);
        Ok(s)
    }
}
