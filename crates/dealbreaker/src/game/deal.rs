//! Initial dealing from a seed or an explicit card order.

use crate::card::{Card, REAL_FACES};
use crate::rng::Rng;

use super::{err, Game, RulesError, State};
use crate::rules::DealRoute;

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
        let rng = &mut Rng::new(seed);
        if self.rules.separate_decks && self.rules.pack.copies > 1 {
            let n = deck.len() / self.rules.pack.copies as usize;
            for chunk in deck.chunks_mut(n) {
                rng.shuffle(chunk);
            }
        } else {
            rng.shuffle(&mut deck);
        }
        for cr in self.rules.deck_first.iter().rev() {
            if let Some(at) = deck
                .iter()
                .position(|c| !c.is_joker() && c.suit() == cr.suit && c.rank() == cr.rank)
            {
                let c = deck.remove(at);
                deck.insert(0, c);
            }
        }
        for cr in &self.rules.deck_last {
            if let Some(at) = deck
                .iter()
                .position(|c| !c.is_joker() && c.suit() == cr.suit && c.rank() == cr.rank)
            {
                let c = deck.remove(at);
                deck.push(c);
            }
        }
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

    /// Fixed suit of a pile (via `empty_suit` or a single-suit `admits` mask), if any.
    fn pile_fixed_suit(&self, p: u16) -> Option<crate::card::Suit> {
        let r = self.rule(p);
        if r.empty_suit.is_some() {
            return r.empty_suit;
        }
        let real = r.admits & REAL_FACES;
        crate::card::Suit::ALL
            .into_iter()
            .find(|&s| real == (self.suit_faces[s as usize] & REAL_FACES))
    }

    /// If `step` diverts `c` straight to an accepting foundation, return that foundation pile.
    fn divert_target(&self, s: &State, step: &crate::rules::DealStep, c: Card) -> Option<u16> {
        let divert = step.auto_foundation
            || step
                .divert_ranks
                .as_ref()
                .is_some_and(|rs| rs.contains(&c.rank()));
        if divert {
            self.find_foundation(s, c)
        } else {
            None
        }
    }

    /// First foundation pile that currently accepts `c` (used by card-dependent deal steps).
    pub(super) fn find_foundation(&self, s: &State, c: Card) -> Option<u16> {
        self.find_foundation_from(s, c, None)
    }

    /// As `find_foundation`, for a card that comes from pile `from`.
    pub(super) fn find_foundation_from(
        &self,
        s: &State,
        c: Card,
        from: Option<u16>,
    ) -> Option<u16> {
        for g in self
            .groups
            .iter()
            .filter(|g| g.kind == crate::rules::PileKind::Foundation)
        {
            for p in g.piles() {
                let pile = &s.piles[p as usize];
                let tr = self.rule(p);
                if pile.len() >= tr.max_cards as usize || tr.complete_runs {
                    continue;
                }
                if from.is_some_and(|f| !self.pile_map_allows(g, f, p)) {
                    continue;
                }
                let ok = if !g.rank_sequence.is_empty() {
                    g.rank_sequence.get(pile.len()) == Some(&c.rank())
                        && (tr.admits >> c.face() & 1 != 0)
                        && match pile.last() {
                            None => tr.empty_suit.is_none_or(|es| c.suit() == es),
                            Some(&top) => {
                                top.is_face_up()
                                    && tr.fits[top.face() as usize] >> c.face() & 1 != 0
                            }
                        }
                } else {
                    match pile.last() {
                        None => self.empty_accepts(p, c, s),
                        Some(&top) => match g.repeat_rank {
                            Some(n) => top.is_face_up() && self.repeat_ok(pile, top, c, n),
                            None => {
                                top.is_face_up()
                                    && tr.fits[top.face() as usize] >> c.face() & 1 != 0
                            }
                        },
                    }
                };
                if ok {
                    return Some(p);
                }
            }
        }
        None
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
            let mut passed: Vec<Card> = Vec::new();
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
                let mut prev: Option<Card> = None;
                for (i, &r) in ranks.iter().enumerate() {
                    let at = deck
                        .iter()
                        .position(|c| {
                            c.rank() == r
                                && !(step.alternate_colors
                                    && prev.is_some_and(|p| p.color() == c.color()))
                        })
                        .ok_or_else(|| {
                            RulesError(format!("deal: no card of rank {r} left to take"))
                        })?;
                    let c = deck.remove(at).unwrap();
                    prev = Some(c);
                    put(&mut s, piles[i % piles.len()], c);
                }
            } else if let Some(ranks) = &step.ranks {
                if let Some(n) = step.count {
                    for _ in 0..n {
                        for &p in &piles {
                            let want_suit = self.pile_fixed_suit(p);
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
                    let limit = step.limit.map(|l| l as usize);
                    deck.retain(|c| {
                        if ranks.contains(&c.rank()) && limit.is_none_or(|m| taken.len() < m) {
                            taken.push(*c);
                            false
                        } else {
                            true
                        }
                    });
                    let has_suit = piles.iter().any(|&p| self.pile_fixed_suit(p).is_some());
                    if has_suit {
                        let mut used = vec![0usize; piles.len()];
                        for c in taken {
                            let slot = (0..piles.len())
                                .filter(|&i| self.pile_fixed_suit(piles[i]) == Some(c.suit()))
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
            } else if step.one_per_suit {
                let rounds = step.count.unwrap_or(1) as usize;
                let mut taken = Vec::new();
                for _ in 0..rounds {
                    for &suit in &self.rules.pack.suits {
                        let at = deck
                            .iter()
                            .position(|c| !c.is_joker() && c.suit() == suit)
                            .ok_or_else(|| {
                                RulesError(format!("deal: no card of suit {suit:?} left to take"))
                            })?;
                        taken.push(deck.remove(at).unwrap());
                    }
                }
                taken.sort_by_key(|c| c.suit() as u8);
                for (i, c) in taken.into_iter().enumerate() {
                    put(&mut s, piles[i % piles.len()], c);
                }
            } else if let (Some(route), true) = (&step.route, step.rest) {
                if let DealRoute::Chain { rank, to } = route {
                    let tg = self
                        .groups
                        .iter()
                        .find(|g| &g.name == to)
                        .ok_or_else(|| RulesError(format!("deal: no group named {to:?}")))?;
                    let mut started = 0u16;
                    while let Some(c) = deck.pop_front() {
                        if c.rank() == *rank {
                            if started >= tg.count {
                                return err("deal: more chain starts than piles");
                            }
                            put(&mut s, tg.start + started, c);
                            started += 1;
                        } else {
                            let at = started.saturating_sub(1) as usize;
                            if at >= piles.len() {
                                return err("deal: a routed card has no pile");
                            }
                            put(&mut s, piles[at], c);
                        }
                    }
                }
                if let DealRoute::Split { rank, to } = route {
                    let tg = self
                        .groups
                        .iter()
                        .find(|g| &g.name == to)
                        .ok_or_else(|| RulesError(format!("deal: no group named {to:?}")))?;
                    let (mut started, mut others) = (0u16, 0u16);
                    while others < step.limit.unwrap_or(u16::MAX) {
                        let Some(c) = deck.pop_front() else { break };
                        if c.rank() == *rank {
                            if started >= tg.count {
                                return err("deal: more split cards than piles");
                            }
                            put(&mut s, tg.start + started, c);
                            started += 1;
                        } else {
                            put(&mut s, piles[0], c);
                            others += 1;
                        }
                    }
                }
                let by_card = !matches!(route, DealRoute::Chain { .. } | DealRoute::Split { .. });
                while let Some(c) = by_card.then(|| deck.pop_front()).flatten() {
                    let at = match route {
                        DealRoute::Chain { .. } | DealRoute::Split { .. } => unreachable!(),
                        DealRoute::Rank { first_rank } => {
                            (c.rank() as i32 - *first_rank as i32).max(0) as usize
                        }
                        DealRoute::Suit => self
                            .rules
                            .pack
                            .suits
                            .iter()
                            .position(|&x| !c.is_joker() && x == c.suit())
                            .unwrap_or(0),
                    };
                    if at >= piles.len() {
                        return err("deal: a routed card has no pile");
                    }
                    put(&mut s, piles[at], c);
                }
            } else if step.rest {
                if let Some(split_r) = step.split_on_rank {
                    let mut idx = 0usize;
                    while let Some(c) = deck.pop_front() {
                        if let Some(f) = self.divert_target(&s, step, c) {
                            put(&mut s, f, c);
                            continue;
                        }
                        if c.rank() == split_r
                            && !s.piles[piles[idx] as usize].is_empty()
                            && idx + 1 < piles.len()
                        {
                            idx += 1;
                        }
                        put(&mut s, piles[idx], c);
                    }
                } else {
                    let mut i = 0;
                    while let Some(c) = deck.pop_front() {
                        if let Some(f) = self.divert_target(&s, step, c) {
                            put(&mut s, f, c);
                            i += 1;
                            continue;
                        }
                        put(&mut s, piles[i % piles.len()], c);
                        i += 1;
                    }
                }
            } else if step.set_base
                && step.count == Some(1)
                && step.counts.is_none()
                && piles.len() > 1
                && piles.iter().any(|&p| self.pile_fixed_suit(p).is_some())
            {
                let Some(c) = deck.pop_front() else {
                    return err("deal: ran out of cards");
                };
                let target = piles
                    .iter()
                    .copied()
                    .find(|&p| self.pile_fixed_suit(p) == Some(c.suit()))
                    .unwrap_or(piles[0]);
                put(&mut s, target, c);
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
                let allow_short = step.stop_when_out
                    || (!step.drain_foundations
                        && (step.auto_foundation || step.divert_ranks.is_some()));
                'deal_rows: for r in 0..rows {
                    for (i, &p) in piles.iter().enumerate() {
                        if counts[i] > r {
                            let mut next = deck.pop_front();
                            while let Some(c) = next {
                                if !step.skip_ranks.contains(&c.rank()) {
                                    break;
                                }
                                passed.push(c);
                                next = deck.pop_front();
                            }
                            let Some(c) = next else {
                                if allow_short {
                                    break 'deal_rows;
                                }
                                return err("deal: ran out of cards");
                            };
                            let mut c = c;
                            loop {
                                if !step.drain_foundations {
                                    if let Some(f) = self.divert_target(&s, step, c) {
                                        put(&mut s, f, c);
                                        if step.retry_after_divert {
                                            let Some(n) = deck.pop_front() else {
                                                if allow_short {
                                                    break 'deal_rows;
                                                }
                                                return err("deal: ran out of cards");
                                            };
                                            c = n;
                                            continue;
                                        }
                                        break;
                                    }
                                }
                                put(&mut s, p, c);
                                break;
                            }
                        }
                    }
                    if step.drain_foundations {
                        loop {
                            let mut moved = false;
                            for &p in &piles {
                                if let Some(&c) = s.piles[p as usize].last() {
                                    if c.is_face_up()
                                        && step
                                            .divert_ranks
                                            .as_ref()
                                            .is_none_or(|rs| rs.contains(&c.rank()))
                                    {
                                        if let Some(f) = self.find_foundation_from(&s, c, Some(p)) {
                                            s.piles[p as usize].pop();
                                            s.piles[f as usize].push(c);
                                            moved = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            if !moved {
                                break;
                            }
                        }
                    }
                }
            }
            deck.extend(passed);
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
