//! Card relations, legal move generation, and move legality checks.

use crate::card::{Card, JOKER, REAL_FACES};
use crate::rules::*;

use super::{Action, Game, Move, PileRules, State};

/// Is `over`'s suit the one after `under`'s in `cycle`?
fn follows_in_cycle(
    cycle: &[crate::card::Suit],
    under: crate::card::Suit,
    over: crate::card::Suit,
) -> bool {
    cycle
        .iter()
        .position(|&x| x == under)
        .is_some_and(|i| cycle[(i + 1) % cycle.len()] == over)
}

impl Game {
    // ----- Card relations -------------------------------------------------------------------

    /// Can `over` be placed on `under` under build rule `b`?
    pub(crate) fn fits(&self, b: &Build, under: Card, over: Card) -> bool {
        if b.by == BuildBy::SuitOrSameRank {
            if b.blocked_ranks.contains(&under.rank()) {
                return false;
            }
            if under.rank() == over.rank() {
                return true;
            }
            if under.suit() != over.suit() {
                return false;
            }
        }
        let ok_suit = match b.by {
            BuildBy::None => return false,
            BuildBy::Any => true,
            BuildBy::Suit | BuildBy::SuitOrSameRank => under.suit() == over.suit(),
            BuildBy::Alternate => under.color() != over.color(),
            BuildBy::SameColor => under.color() == over.color(),
            BuildBy::OtherSuit => under.suit() != over.suit(),
            BuildBy::SuitOrRank => under.suit() == over.suit() || under.rank() == over.rank(),
            BuildBy::SuitCycle => follows_in_cycle(&b.suit_cycle, under.suit(), over.suit()),
            BuildBy::RankSuitColor => {
                let (u, o) = (
                    self.rank_idx[under.rank() as usize] as i16,
                    self.rank_idx[over.rank() as usize] as i16,
                );
                let n = self.nranks as i16;
                return if u == o {
                    under.color() != over.color()
                } else if under.suit() == over.suit() {
                    (o - u).rem_euclid(n) == 1
                } else {
                    under.color() == over.color() && (u - o).rem_euclid(n) == 1
                };
            }
        };
        if !ok_suit || b.blocked_ranks.contains(&under.rank()) {
            return false;
        }
        if b.extra_over.contains(&(under.rank(), over.rank())) {
            return true;
        }
        if b.extra_pairs.iter().any(|&(a, c)| {
            (under.rank() == a && over.rank() == c) || (under.rank() == c && over.rank() == a)
        }) {
            return true;
        }
        let (u, o) = (
            self.rank_idx[under.rank() as usize],
            self.rank_idx[over.rank() as usize],
        );
        if u == 255 || o == 255 {
            return false;
        }
        let n = self.nranks as i16;
        let step = b.step as i16;
        let up = |from: u8, to: u8, to_rank: u8| -> bool {
            let d = to as i16 - from as i16;
            (d == step && !(step == 1 && b.gap_ranks.contains(&to_rank)))
                || (b.wrap && d == step - n)
        };
        let (ur, or) = (under.rank(), over.rank());
        match b.dir {
            Dir::Up => up(u, o, or),
            Dir::UpAny => o > u,
            Dir::Down => up(o, u, ur),
            Dir::DownAny => o < u,
            Dir::DownOrSame => u == o || up(o, u, ur),
            Dir::DoubleUp => o as i16 == (2 * u as i16 + 1) % n,
            Dir::Mirror => o as i16 + u as i16 == n - 1,
            Dir::Either => up(u, o, or) || up(o, u, ur),
            Dir::Same => u == o,
            Dir::Any => true,
        }
    }

    #[inline]
    pub(crate) fn rule(&self, pile: u16) -> &PileRules {
        &self.pile_rules[pile as usize]
    }

    pub(crate) fn empty_accepts(&self, pile: u16, card: Card, s: &State) -> bool {
        let r = self.rule(pile);
        if r.admits >> card.face() & 1 == 0 {
            return false;
        }
        if card.is_joker() {
            return (r.empty == EmptyRule::Any && r.empty_suit.is_none())
                || (r.wild && self.empty_set(pile, s) != 0);
        }
        if let Some(es) = r.empty_suit {
            if card.suit() != es {
                return false;
            }
        }
        let g = self.group_of(pile);
        if g.unique_suits
            && g.piles().any(|p| {
                p != pile
                    && s.piles[p as usize]
                        .first()
                        .is_some_and(|c| c.suit() == card.suit())
            })
        {
            return false;
        }
        if (g.unique_ranks || g.same_suit_group || g.same_rank_group)
            && g.piles().any(|p| {
                p != pile
                    && s.piles[p as usize].first().is_some_and(|c| {
                        (g.unique_ranks && c.rank() == card.rank())
                            || (g.same_suit_group && c.suit() != card.suit())
                            || (g.same_rank_group && c.rank() != card.rank())
                    })
            })
        {
            return false;
        }
        if g.cyclic_ranks && !self.cyclic_fits(g, pile, card.rank(), s) {
            return false;
        }
        if g.empty_needs_first_full && !self.first_pile_full(self.pile_group[pile as usize], s) {
            return false;
        }
        if self.empty_is_open(pile, s) {
            return true;
        }
        match &r.empty {
            EmptyRule::None => false,
            EmptyRule::Any => true,
            EmptyRule::Rank(r) => card.rank() == *r,
            EmptyRule::Ranks(ranks) => ranks.contains(&card.rank()),
            EmptyRule::DealtBase => s.base_rank.is_none_or(|b| card.rank() == b),
            EmptyRule::DealtBaseOffset(o) => s
                .base_rank
                .is_some_and(|b| card.rank() == self.rank_from(b, *o)),
            EmptyRule::AfterLeft { .. }
            | EmptyRule::AfterNeighbor { .. }
            | EmptyRule::AfterPile { .. }
            | EmptyRule::SameBase => self.empty_set(pile, s) >> card.face() & 1 != 0,
        }
    }

    /// Whether `empty_open_after_bases` has opened the empty piles of `pile`'s group to any card.
    fn first_pile_full(&self, gi: u16, s: &State) -> bool {
        let first = self.groups[gi as usize].start;
        s.piles[first as usize].len() >= self.rule(first).max_cards as usize
    }

    fn empty_is_open(&self, pile: u16, s: &State) -> bool {
        let g = self.group_of(pile);
        if g.empty_open_when_full
            .is_some_and(|gi| self.first_pile_full(gi, s))
        {
            return true;
        }
        let (Some(need), EmptyRule::Rank(k)) = (g.empty_open_after_bases, &self.rule(pile).empty)
        else {
            return false;
        };
        g.piles()
            .filter(|&p| s.piles[p as usize].first().is_some_and(|c| c.rank() == *k))
            .count()
            >= need as usize
    }

    /// The rank `offset` places after `base` in the pack's sequence of ranks, going round.
    pub(crate) fn rank_from(&self, base: u8, offset: i8) -> u8 {
        let n = self.nranks as i16;
        let at = self.rank_idx[base as usize];
        if at == 255 {
            return base;
        }
        let to = (at as i16 + offset as i16).rem_euclid(n);
        self.rules.pack.ranks[to as usize]
    }

    /// The real faces an empty `pile` would accept.
    pub(crate) fn empty_set(&self, pile: u16, s: &State) -> u64 {
        let r = self.rule(pile);
        let mut real = r.admits & REAL_FACES;
        if let Some(es) = r.empty_suit {
            real &= self.suit_faces[es as usize];
        }
        let g = self.group_of(pile);
        if g.unique_suits {
            for p in g.piles() {
                if p != pile {
                    if let Some(&c) = s.piles[p as usize].first() {
                        if !c.is_joker() {
                            real &= !self.suit_faces[c.suit() as usize];
                        }
                    }
                }
            }
        }
        if g.empty_needs_first_full && !self.first_pile_full(self.pile_group[pile as usize], s) {
            return 0;
        }
        if g.cyclic_ranks {
            for r in 1..14u8 {
                if self.rank_faces[r as usize] != 0 && !self.cyclic_fits(g, pile, r, s) {
                    real &= !self.rank_faces[r as usize];
                }
            }
        }
        if g.unique_ranks || g.same_suit_group || g.same_rank_group {
            for p in g.piles() {
                if p == pile {
                    continue;
                }
                if let Some(&c) = s.piles[p as usize].first() {
                    if c.is_joker() {
                        continue;
                    }
                    if g.unique_ranks {
                        real &= !self.rank_faces[c.rank() as usize];
                    }
                    if g.same_suit_group {
                        real &= self.suit_faces[c.suit() as usize];
                    }
                    if g.same_rank_group {
                        real &= self.rank_faces[c.rank() as usize];
                    }
                }
            }
        }
        if self.empty_is_open(pile, s) {
            return real;
        }
        match &r.empty {
            EmptyRule::None => 0,
            EmptyRule::Any => real,
            EmptyRule::Rank(k) => real & self.rank_faces[*k as usize],
            EmptyRule::Ranks(ranks) => ranks
                .iter()
                .fold(0u64, |m, &k| m | (real & self.rank_faces[k as usize])),
            EmptyRule::DealtBase => s
                .base_rank
                .map_or(real, |b| real & self.rank_faces[b as usize]),
            EmptyRule::DealtBaseOffset(o) => s.base_rank.map_or(0, |b| {
                real & self.rank_faces[self.rank_from(b, *o) as usize]
            }),
            EmptyRule::AfterLeft { first } => {
                let first = *first;
                let g = self.group_of(pile);
                let cols = g.columns.unwrap_or(g.count);
                if (pile - g.start) % cols == 0 {
                    return real & self.rank_faces[first as usize];
                }
                let left = &s.piles[pile as usize - 1];
                if left.is_empty() {
                    return 0;
                }
                // Only a left pile of nothing but jokers needs to know what it was built from.
                let base = if left.iter().all(|c| c.is_joker()) {
                    self.empty_set(pile - 1, s)
                } else {
                    0
                };
                real & self.accept_set(r, left, base, false)
            }
            EmptyRule::AfterNeighbor { first } => {
                let first = *first;
                let g = self.group_of(pile);
                let cols = g.columns.unwrap_or(g.count);
                if (pile - g.start) % cols == 0 {
                    return real & self.rank_faces[first as usize];
                }
                let mut out = 0u64;
                let left = &s.piles[pile as usize - 1];
                if !left.is_empty() {
                    let base = if left.iter().all(|c| c.is_joker()) {
                        self.empty_set(pile - 1, s)
                    } else {
                        0
                    };
                    out |= real & self.accept_set(r, left, base, false);
                }
                if pile + 1 < g.start + g.count {
                    let right = &s.piles[pile as usize + 1];
                    if let Some(&rc) = right.last() {
                        if rc.is_face_up() && !rc.is_joker() {
                            let mut m = real;
                            while m != 0 {
                                let f = m.trailing_zeros() as usize;
                                if r.fits[f] >> rc.face() & 1 != 0 {
                                    out |= 1 << f;
                                }
                                m &= m - 1;
                            }
                        }
                    }
                }
                out
            }
            EmptyRule::AfterPile { .. } => {
                let (ref_gi, offset) = r.after_pile.unwrap();
                let idx = pile - self.group_of(pile).start;
                let ref_pile = self.groups[ref_gi as usize].start + idx;
                match s.piles[ref_pile as usize].last() {
                    Some(&c) if c.is_face_up() && !c.is_joker() => {
                        let want = self.rank_from(c.rank(), offset);
                        real & self.rank_faces[want as usize]
                    }
                    _ => 0,
                }
            }
            EmptyRule::SameBase => {
                let base = g
                    .piles()
                    .find_map(|q| s.piles[q as usize].first().filter(|c| !c.is_joker()))
                    .map(|c| c.rank());
                base.map_or(real, |b| real & self.rank_faces[b as usize])
            }
        }
    }

    /// Real faces that may be built, under rule `r`, on a card that could be any face in `set`.
    /// With `suit_only`, only cards of the same suit (Spider's runs).
    #[inline]
    pub(crate) fn step_set(&self, r: &PileRules, set: u64, suit_only: bool) -> u64 {
        let mut out = 0u64;
        let mut m = set & REAL_FACES;
        while m != 0 {
            let f = m.trailing_zeros() as usize;
            let mut fit = r.fits[f];
            if suit_only {
                fit &= self.suit_faces[f / 13];
            }
            out |= fit;
            m &= m - 1;
        }
        out & REAL_FACES
    }

    /// Real faces that may be built, under rule `r`, on top of `cards`, where an empty pile
    /// would take `base`. A joker on top stands for any card the cards beneath allow; one on a
    /// face-down card, for any card at all.
    pub(crate) fn accept_set(
        &self,
        r: &PileRules,
        cards: &[Card],
        base: u64,
        suit_only: bool,
    ) -> u64 {
        let Some(&top) = cards.last() else {
            return base;
        };
        if !top.is_face_up() {
            return 0;
        }
        if !top.is_joker() {
            let f = top.face() as usize;
            let mut fit = r.fits[f];
            if suit_only {
                fit &= self.suit_faces[f / 13];
            }
            return fit & REAL_FACES;
        }
        if !r.wild {
            return 0;
        }
        let k = cards
            .iter()
            .rev()
            .take_while(|c| c.is_joker() && c.is_face_up())
            .count();
        let below = cards.len() - k;
        // What the lowest joker of the top run of jokers can stand for.
        let mut id = if below == 0 {
            base
        } else {
            let c = cards[below - 1];
            if c.is_face_up() {
                self.step_set(r, 1 << c.face(), suit_only)
            } else {
                r.admits & REAL_FACES
            }
        };
        for _ in 1..k {
            id = self.step_set(r, id, suit_only);
        }
        self.step_set(r, id, suit_only)
    }

    /// Could a unit (bottom card first) be placed where `allowed` real faces may go, under
    /// rule `r`? `empty_any` says the target is an empty pile taking any card. A built unit
    /// (`run`) must stay built through any jokers at its bottom, whatever they now stand for.
    fn unit_fits(
        &self,
        r: &PileRules,
        allowed: u64,
        empty_any: bool,
        unit: &[Card],
        run: bool,
    ) -> bool {
        let bottom = unit[0];
        if !bottom.is_joker() {
            return allowed >> bottom.face() & 1 != 0;
        }
        if r.admits >> JOKER & 1 == 0 {
            return false;
        }
        if !r.wild {
            // Nothing can be built on a joker that isn't wild.
            return empty_any && (unit.len() == 1 || !run);
        }
        let mut id = allowed;
        if id == 0 && !empty_any {
            return false;
        }
        if empty_any && id == 0 {
            id = r.admits & REAL_FACES;
        }
        if !run {
            return true;
        }
        for &c in &unit[1..] {
            let next = self.step_set(r, id, false);
            if !c.is_joker() {
                return next >> c.face() & 1 != 0;
            }
            if next == 0 {
                return false;
            }
            id = next;
        }
        true
    }

    /// Every pile covering this one is empty.
    #[inline]
    pub(crate) fn available(&self, s: &State, pile: u16) -> bool {
        !self.has_covers
            || self.covered_by[pile as usize]
                .iter()
                .all(|&c| s.piles[c as usize].is_empty())
    }

    /// Interlock's drop-down: the whole of pile `from`, the only pile still covering `to`, moves
    /// onto `to`.
    fn dropdown_ok(&self, s: &State, from: u16, to: u16, n: usize) -> bool {
        self.group_of(to).dropdown
            && self.covered_by[to as usize].contains(&from)
            && s.piles[from as usize].len() == n
            && s.piles[to as usize].first().is_some_and(|c| c.is_face_up())
            && self.covered_by[to as usize]
                .iter()
                .all(|&c| c == from || s.piles[c as usize].is_empty())
    }

    /// How many cards from the top of `pile` form a unit that may move together (0 if none).
    pub(crate) fn movable(&self, s: &State, pile: u16) -> usize {
        let cards = &s.piles[pile as usize];
        let Some(&top) = cards.last() else { return 0 };
        if s.base_rank.is_none()
            && self
                .choose_base
                .as_ref()
                .is_some_and(|cb| cb.lock_mask >> self.pile_group[pile as usize] & 1 != 0)
        {
            return 0;
        }
        if !top.is_face_up() {
            return usize::from(
                self.group_of(pile).move_face_down_top && self.rule(pile).moves != MoveUnit::None,
            );
        }
        if let Some(n) = self.hooks.as_ref().and_then(|h| h.movable(self, s, pile)) {
            return n.min(cards.len());
        }
        let grp = self.group_of(pile);
        if grp.moves_when_stock_empty
            && self
                .stock_pile
                .is_some_and(|sp| !s.piles[sp as usize].is_empty())
        {
            return 0;
        }
        if let Some(ref_gi) = grp.moves_when_any_empty {
            if !self.groups[ref_gi as usize]
                .piles()
                .any(|p| s.piles[p as usize].is_empty())
            {
                return 0;
            }
        }
        if let Some(fr) = grp.move_from_frontier {
            let mut rest = grp.piles().filter(|&q| !s.piles[q as usize].is_empty());
            let edge = match fr {
                crate::rules::Frontier::Last => rest.next_back(),
                crate::rules::Frontier::First => rest.next(),
            };
            if edge != Some(pile) {
                return 0;
            }
        }
        if let Some(ref_gi) = grp.moves_when_same_index_empty {
            let ref_pile = self.groups[ref_gi as usize].start + (pile - grp.start);
            if !s.piles[ref_pile as usize].is_empty() {
                return 0;
            }
        }
        let g = self.rule(pile);
        if grp.freeze_when_built
            && cards.len() >= g.max_cards as usize
            && cards.windows(2).all(|w| self.fits(&g.build, w[0], w[1]))
        {
            return 0;
        }
        let n = match g.moves {
            MoveUnit::None => 0,
            MoveUnit::Top => 1,
            MoveUnit::Whole => {
                if cards.iter().all(|c| c.is_face_up()) {
                    cards.len()
                } else {
                    0
                }
            }
            MoveUnit::AnyFaceUp => cards.iter().rev().take_while(|c| c.is_face_up()).count(),
            MoveUnit::SuitSequence => {
                let mut n = 1;
                while n < cards.len() {
                    let (under, over) = (cards[cards.len() - n - 1], cards[cards.len() - n]);
                    if !under.is_face_up()
                        || under.suit() != over.suit()
                        || self.rank_idx[under.rank() as usize] == 255
                        || self.rank_idx[under.rank() as usize]
                            != self.rank_idx[over.rank() as usize].wrapping_add(1)
                    {
                        break;
                    }
                    n += 1;
                }
                n
            }
            MoveUnit::SuitRunOrSameRank => {
                let mut n_suit = 1;
                while n_suit < cards.len() {
                    let (under, over) =
                        (cards[cards.len() - n_suit - 1], cards[cards.len() - n_suit]);
                    if !under.is_face_up()
                        || under.suit() != over.suit()
                        || under.rank() == over.rank()
                        || g.fits[under.face() as usize] >> over.face() & 1 == 0
                    {
                        break;
                    }
                    n_suit += 1;
                }
                let mut n_same = 1;
                while n_same < cards.len() {
                    let (under, over) =
                        (cards[cards.len() - n_same - 1], cards[cards.len() - n_same]);
                    if !under.is_face_up() || under.rank() != over.rank() {
                        break;
                    }
                    n_same += 1;
                }
                n_suit.max(n_same)
            }
            MoveUnit::Run
            | MoveUnit::Supermove
            | MoveUnit::SuitRun
            | MoveUnit::AltColorRun
            | MoveUnit::SameColorRun
            | MoveUnit::TopOrWhole
                if self.jokers > 0 =>
            {
                let filter = match g.moves {
                    MoveUnit::SuitRun => RunFilter::SameSuit,
                    MoveUnit::AltColorRun => RunFilter::AltColor,
                    MoveUnit::SameColorRun => RunFilter::SameColor,
                    _ => RunFilter::Any,
                };
                let n = self.movable_run_wild(g, cards, filter);
                if g.moves == MoveUnit::TopOrWhole && n != cards.len() {
                    1
                } else {
                    n
                }
            }
            MoveUnit::Run
            | MoveUnit::Supermove
            | MoveUnit::SuitRun
            | MoveUnit::AltColorRun
            | MoveUnit::SameColorRun
            | MoveUnit::TopOrWhole => {
                let mut n = 1;
                while n < cards.len() {
                    let (under, over) = (cards[cards.len() - n - 1], cards[cards.len() - n]);
                    let extra_ok = match g.moves {
                        MoveUnit::SuitRun => under.suit() == over.suit(),
                        MoveUnit::AltColorRun => under.color() != over.color(),
                        MoveUnit::SameColorRun => under.color() == over.color(),
                        _ => true,
                    };
                    if !under.is_face_up()
                        || g.fits[under.face() as usize] >> over.face() & 1 == 0
                        || !extra_ok
                    {
                        break;
                    }
                    n += 1;
                }
                if g.moves == MoveUnit::TopOrWhole && n != cards.len() {
                    1
                } else {
                    n
                }
            }
        };
        n.min(cards.len().saturating_sub(g.min_cards as usize))
    }

    fn step_set_filtered(&self, r: &PileRules, set: u64, filter: RunFilter) -> u64 {
        let mut out = 0u64;
        let mut m = set & REAL_FACES;
        while m != 0 {
            let f = m.trailing_zeros() as usize;
            let mut fit = r.fits[f];
            match filter {
                RunFilter::Any => {}
                RunFilter::SameSuit => fit &= self.suit_faces[f / 13],
                RunFilter::SameColor => {
                    let s = f / 13;
                    let mask = if s == 0 || s == 3 {
                        self.suit_faces[0] | self.suit_faces[3]
                    } else {
                        self.suit_faces[1] | self.suit_faces[2]
                    };
                    fit &= mask;
                }
                RunFilter::AltColor => {
                    let s = f / 13;
                    let mask = if s == 0 || s == 3 {
                        self.suit_faces[1] | self.suit_faces[2]
                    } else {
                        self.suit_faces[0] | self.suit_faces[3]
                    };
                    fit &= mask;
                }
            }
            out |= fit;
            m &= m - 1;
        }
        out & REAL_FACES
    }

    /// The longest built run on top of `cards` when jokers may be among them. Between two real
    /// cards, the jokers must be able to stand for a built sequence joining them; jokers at
    /// the bottom of the run for one leading up to the cards above.
    fn movable_run_wild(&self, r: &PileRules, cards: &[Card], filter: RunFilter) -> usize {
        let len = cards.len();
        let top = cards[len - 1];
        let steps = |mut set: u64, k: usize| {
            for _ in 0..k {
                set = self.step_set_filtered(r, set, filter);
            }
            set
        };
        let any = r.admits & REAL_FACES;
        // The lowest real card of the run so far, and the jokers beneath it in the run.
        let (mut b, mut j) = if top.is_joker() {
            if !r.wild {
                return 1;
            }
            (None, 1usize)
        } else {
            (Some(top.face()), 0usize)
        };
        let mut n = 1;
        while n < len {
            let under = cards[len - n - 1];
            if !under.is_face_up() {
                break;
            }
            let ok = if under.is_joker() {
                r.wild
                    && match b {
                        Some(bf) => steps(any, j + 1) >> bf & 1 != 0,
                        None => steps(any, j) != 0,
                    }
            } else {
                let a = 1u64 << under.face();
                match b {
                    Some(bf) => steps(a, j + 1) >> bf & 1 != 0,
                    None => steps(a, j) != 0,
                }
            };
            if !ok {
                break;
            }
            if under.is_joker() {
                j += 1;
            } else {
                b = Some(under.face());
                j = 0;
            }
            n += 1;
        }
        n
    }

    /// Largest unit a supermove from `from` to `to` may carry.
    fn supermove_limit(&self, s: &State, from: u16, to: u16) -> usize {
        let fg = self.pile_group[from as usize];
        let mut cells = 0usize;
        let mut empties = 0u32;
        for (p, pile) in s.piles.iter().enumerate() {
            if !pile.is_empty() || p as u16 == to || p as u16 == from {
                continue;
            }
            if self.group_of(p as u16).kind == PileKind::Cell {
                cells += 1;
            } else if self.pile_group[p] == fg && self.pile_rules[p].empty == EmptyRule::Any {
                empties += 1;
            }
        }
        (cells + 1) << empties.min(16)
    }

    /// Is pile `p` a whole run built under its own rule that starts at rank `from`?
    pub(crate) fn is_built_from(&self, p: u16, cards: &[Card], from: u8) -> bool {
        let fits = &self.rule(p).fits;
        cards
            .first()
            .is_some_and(|c| c.is_face_up() && c.rank() == from)
            && cards
                .windows(2)
                .all(|w| w[1].is_face_up() && fits[w[0].face() as usize] >> w[1].face() & 1 != 0)
    }

    /// Does the run on top of pile `p` (face up, each card fitting on the one below) include a
    /// card of rank `rank`?
    pub(crate) fn top_run_has_rank(&self, p: u16, cards: &[Card], rank: u8) -> bool {
        let fits = &self.rule(p).fits;
        let mut i = cards.len();
        while i > 0 {
            let c = cards[i - 1];
            if !c.is_face_up()
                || (i < cards.len() && fits[c.face() as usize] >> cards[i].face() & 1 == 0)
            {
                return false;
            }
            if c.rank() == rank {
                return true;
            }
            i -= 1;
        }
        false
    }

    /// Is the top `n` cards of `cards` a complete sequence matching `by`, ready for a complete-run foundation?
    fn is_complete_run(
        &self,
        cards: &[Card],
        n: usize,
        want: usize,
        by: BuildBy,
        wrap: bool,
        same: bool,
    ) -> bool {
        if n != want || cards.len() < n {
            return false;
        }
        let run = &cards[cards.len() - n..];
        let b = Build {
            by,
            dir: Dir::Down,
            wrap,
            ..Build::default()
        };
        let b_up = Build {
            by,
            dir: Dir::Up,
            wrap,
            ..Build::default()
        };
        let all = |b: &Build| {
            run.windows(2)
                .all(|w| w[0].is_face_up() && self.fits(b, w[0], w[1]))
        };
        let b_same = Build {
            by,
            dir: Dir::Same,
            ..Build::default()
        };
        run[0].is_face_up()
            && run[n - 1].is_face_up()
            && if same {
                all(&b_same)
            } else {
                all(&b) || all(&b_up)
            }
    }

    // ----- Moves ----------------------------------------------------------------------------

    /// True if moving from `from` onto `to` swaps the two piles under `GroupRules::swap_bases`.
    pub(crate) fn can_swap(&self, s: &State, from: u16, to: u16) -> bool {
        if from == to {
            return false;
        }
        let fgi = self.pile_group[from as usize];
        let Some(mode) = self.groups[fgi as usize].swap_bases else {
            return false;
        };
        if fgi != self.pile_group[to as usize]
            && !(mode == SwapBases::Crossing
                && self.groups[self.pile_group[to as usize] as usize].swap_bases == Some(mode))
        {
            return false;
        }
        let src = &s.piles[from as usize];
        let dst = &s.piles[to as usize];
        if src.is_empty() || dst.is_empty() {
            return false;
        }
        let c0 = src[0];
        let c1 = dst[0];
        if !c0.is_face_up() || !c1.is_face_up() {
            return false;
        }
        match mode {
            SwapBases::BothSingle => {
                src.len() == 1
                    && dst.len() == 1
                    && self.empty_accepts(to, c0, s)
                    && self.empty_accepts(from, c1, s)
            }
            SwapBases::Crossing => {
                let (af, at) = (self.rule(from).admits, self.rule(to).admits);
                src.len() == 1
                    && dst.len() == 1
                    && af != at
                    && at >> c0.face() & 1 != 0
                    && af >> c1.face() & 1 != 0
                    && at >> c1.face() & 1 == 0
            }
            SwapBases::EitherPile => {
                self.rule(from).empty != self.rule(to).empty
                    && c0.rank() != c1.rank()
                    && (self.empty_accepts(to, c0, s) || self.empty_accepts(from, c1, s))
            }
        }
    }

    /// Four by Four: may `card` go on `pile` (whose top is `top`) when each rank comes `n` times?
    pub(crate) fn repeat_ok(&self, pile: &[Card], top: Card, card: Card, n: u16) -> bool {
        let run = pile
            .iter()
            .rev()
            .take_while(|c| c.rank() == top.rank())
            .count();
        if run < n as usize {
            card.rank() == top.rank()
        } else {
            card.rank() == self.rank_from(top.rank(), 1)
        }
    }

    /// Does `tg`'s `from_pile_map` let pile `from` send cards to pile `to`?
    pub(crate) fn pile_map_allows(&self, tg: &crate::game::Group, from: u16, to: u16) -> bool {
        match &tg.from_pile_map {
            Some((fg, map)) if self.pile_group[from as usize] == *fg => {
                let want = map
                    .get((from - self.groups[*fg as usize].start) as usize)
                    .copied()
                    .unwrap_or(-1);
                want < 0 || want as u16 == to - tg.start
            }
            _ => true,
        }
    }

    /// Would the bottom cards of `g`'s other piles and a card of `rank` still form one run of
    /// consecutive, different ranks (wrapping round the pack)?
    fn cyclic_fits(&self, g: &crate::game::Group, pile: u16, rank: u8, s: &State) -> bool {
        let n = self.nranks as i16;
        let mut idx: Vec<i16> = g
            .piles()
            .filter(|&p| p != pile)
            .filter_map(|p| s.piles[p as usize].first())
            .filter(|c| !c.is_joker())
            .map(|c| self.rank_idx[c.rank() as usize] as i16)
            .collect();
        idx.push(self.rank_idx[rank as usize] as i16);
        idx.sort_unstable();
        let k = idx.len() as i16;
        if idx.windows(2).any(|w| w[0] == w[1]) {
            return false;
        }
        if k <= 1 {
            return true;
        }
        let mut max_gap = idx[0] + n - idx[k as usize - 1];
        for w in idx.windows(2) {
            max_gap = max_gap.max(w[1] - w[0]);
        }
        max_gap == n - k + 1
    }

    /// Could `n` cards from the top of `from` go to `to`? Doesn't check the unit is movable.
    pub(crate) fn can_place(&self, s: &State, from: u16, to: u16, n: usize) -> bool {
        self.can_place_with_limit(s, from, to, n, None)
    }

    fn can_place_with_limit(
        &self,
        s: &State,
        from: u16,
        to: u16,
        n: usize,
        supermove_cap: Option<usize>,
    ) -> bool {
        if from == to {
            return false;
        }
        if let Some(h) = &self.hooks {
            let ok_shape = n > 0
                && n <= s.piles[from as usize].len()
                && self.available(s, to)
                && self.group_of(to).kind != PileKind::Stock;
            if ok_shape {
                if let Some(b) = h.accepts(self, s, from, to, n) {
                    return b;
                }
            }
        }
        let tg = self.group_of(to);
        let tr = self.rule(to);
        let fgi = self.pile_group[from as usize];
        if let Some(cb) = &self.choose_base {
            if s.base_rank.is_none()
                && ((fgi != cb.from_group && Some(fgi) != cb.also_from_group)
                    || tg.kind != PileKind::Foundation
                    || n != 1)
                && (!cb.free_moves || tg.kind == PileKind::Foundation)
            {
                return false;
            }
        }
        let dst = &s.piles[to as usize];
        if matches!(tg.kind, PileKind::Stock)
            && tr.empty == EmptyRule::None
            && tr.build.by == BuildBy::None
        {
            return false;
        }
        if self.can_swap(s, from, to) {
            return n == 1 && self.available(s, to);
        }
        let unrestricted = tg.unrestricted_from & (1 << fgi) != 0;
        let is_final_same_index =
            tg.final_from_same_index.is_some() && dst.len() + n == tr.max_cards as usize;
        if let Some(ref_gi) = tg.final_from_same_index.filter(|_| is_final_same_index) {
            let expected_from = self.groups[ref_gi as usize].start + (to - tg.start);
            if from != expected_from {
                return false;
            }
        } else {
            let mask = if dst.is_empty() {
                tg.empty_accepts_from
            } else {
                tg.accepts_from
            };
            if (mask | tg.unrestricted_from) & (1 << fgi) == 0 {
                return false;
            }
        }
        if let Some(reach) = &tg.reach {
            let same = self.pile_group[to as usize] as usize == fgi as usize;
            if !same {
                return false;
            }
            let dist = if tg.reach_both_ways {
                from.abs_diff(to)
            } else if from >= to {
                from - to
            } else {
                return false;
            };
            if !reach.contains(&dist) {
                return false;
            }
            if tg.reach_grid && dist == 1 {
                let cols = tg.columns.unwrap_or(tg.count);
                if (from - tg.start) / cols != (to - tg.start) / cols {
                    return false;
                }
            }
        }
        let src = &s.piles[from as usize];
        if tg.accept_only_when_empty && !dst.is_empty() {
            return false;
        }
        if s.redeals_used == 0
            && tg.first_round_bans.iter().any(|&(t, fg2, f)| {
                to - tg.start == t
                    && self.pile_group[from as usize] == fg2
                    && from - self.groups[fg2 as usize].start == f
            })
        {
            return false;
        }
        if dst.is_empty() && self.waste == Some(from) {
            if let Some(gi) = tg.empty_needs_group_empty {
                if self.groups[gi as usize]
                    .piles()
                    .any(|q| !s.piles[q as usize].is_empty())
                {
                    return false;
                }
            }
        }
        if tg.accept_whole_from_group
            && self.pile_group[from as usize] == self.pile_group[to as usize]
            && n != src.len()
        {
            return false;
        }
        if !self.pile_map_allows(tg, from, to) {
            return false;
        }
        if let Some(k) = tg.one_way_split {
            if self.pile_group[from as usize] == self.pile_group[to as usize]
                && to - tg.start >= k
                && from - tg.start < k
            {
                return false;
            }
        }
        if tg.require_valid_base && !dst.is_empty() && !self.empty_accepts(to, dst[0], s) {
            return false;
        }
        if tg.accept_when_waste_empty && self.waste.is_some_and(|w| !s.piles[w as usize].is_empty())
        {
            return false;
        }
        if tg.accept_when_stock_empty
            && self
                .stock_pile
                .is_some_and(|sp| !s.piles[sp as usize].is_empty())
        {
            return false;
        }
        if tg.no_build_single_while_stock
            && !dst.is_empty()
            && self
                .stock_pile
                .is_some_and(|sp| !s.piles[sp as usize].is_empty())
            && (dst.len() == 1 || (fgi == self.pile_group[to as usize] && src.len() == 1))
        {
            return false;
        }
        if n == 0
            || n > src.len()
            || n > tr.max_accept as usize
            || (dst.is_empty() && n > tr.empty_max_accept as usize)
            || (!dst.is_empty() && n > tr.non_empty_max_accept as usize)
            || (if tg.replace_on_drop.is_some() {
                0
            } else {
                dst.len()
            }) + n
                > tr.max_cards as usize
        {
            return false;
        }
        if !self.available(s, to) && !self.dropdown_ok(s, from, to, n) {
            return false;
        }
        let card = src[src.len() - n];
        if tr.complete_runs {
            let want = tg.run_len.map_or(self.nranks as usize, |l| l as usize);
            return (dst.is_empty() || tg.run_len.is_some())
                && self.is_complete_run(
                    src,
                    n,
                    want,
                    tr.build.by,
                    tr.build.wrap,
                    tr.build.dir == Dir::Same,
                );
        }
        let ok = if unrestricted
            || (tg.accept_on_face_down && dst.last().is_some_and(|c| !c.is_face_up()))
        {
            true
        } else if let Some(n) = tg.repeat_rank {
            match dst.last() {
                None => self.empty_accepts(to, card, s),
                Some(&top) => top.is_face_up() && self.repeat_ok(dst, top, card, n),
            }
        } else if !tg.rank_sequence.is_empty() {
            let Some(&want_rank) = tg.rank_sequence.get(dst.len()) else {
                return false;
            };
            let ok_admit = tr.admits >> card.face() & 1 != 0;
            let ok_rank = card.is_joker() || card.rank() == want_rank;
            if !ok_admit || !ok_rank {
                false
            } else if let Some(&top) = dst.last() {
                if !top.is_face_up() {
                    false
                } else if card.is_joker() || top.is_joker() {
                    tr.wild
                } else {
                    match tr.build.by {
                        BuildBy::None => false,
                        BuildBy::Any => true,
                        BuildBy::Suit => top.suit() == card.suit(),
                        BuildBy::Alternate => top.color() != card.color(),
                        BuildBy::SameColor => top.color() == card.color(),
                        BuildBy::OtherSuit => top.suit() != card.suit(),
                        BuildBy::SuitOrRank | BuildBy::SuitOrSameRank => {
                            top.suit() == card.suit() || top.rank() == card.rank()
                        }
                        BuildBy::RankSuitColor => self.fits(&tr.build, top, card),
                        BuildBy::SuitCycle => {
                            follows_in_cycle(&tr.build.suit_cycle, top.suit(), card.suit())
                        }
                    }
                }
            } else {
                card.is_joker() || tr.empty_suit.is_none_or(|es| card.suit() == es)
            }
        } else if self.jokers == 0 {
            match dst.last() {
                None => self.empty_accepts(to, card, s),
                Some(&top) => {
                    top.is_face_up() && tr.fits[top.face() as usize] >> card.face() & 1 != 0
                }
            }
        } else {
            let base = if dst.iter().all(|c| c.is_joker()) {
                self.empty_set(to, s)
            } else {
                0
            };
            let allowed = self.accept_set(tr, dst, base, false);
            let run = is_run_unit(self.rule(from).moves);
            let empty_any = dst.is_empty() && tr.empty == EmptyRule::Any;
            self.unit_fits(tr, allowed, empty_any, &src[src.len() - n..], run)
        };
        if !ok {
            return false;
        }
        if (tg.lock_build_dir || tg.lock_pile_dir)
            && !unrestricted
            && !dst.is_empty()
            && !card.is_joker()
        {
            let n = self.nranks as i16;
            let step = tr.build.step as i16;
            let is_up = |a: Card, b: Card| -> bool {
                let d = self.rank_idx[b.rank() as usize] as i16
                    - self.rank_idx[a.rank() as usize] as i16;
                d == step || (tr.build.wrap && d == step - n)
            };
            let locked = if tg.lock_build_dir {
                tg.piles().find_map(|p| {
                    let pile = &s.piles[p as usize];
                    if pile.len() >= 2 && !pile[0].is_joker() && !pile[1].is_joker() {
                        Some(is_up(pile[0], pile[1]))
                    } else {
                        None
                    }
                })
            } else if dst.len() >= 2 && !dst[0].is_joker() && !dst[1].is_joker() {
                Some(is_up(dst[0], dst[1]))
            } else {
                None
            };
            if let Some(want_up) = locked {
                let top = *dst.last().unwrap();
                if !top.is_joker() && is_up(top, card) != want_up {
                    return false;
                }
            }
        }
        if let Some(ref_gi) = tg.requires_same_index_non_empty {
            let ref_pile = self.groups[ref_gi as usize].start + (to - tg.start);
            if s.piles[ref_pile as usize].is_empty() {
                return false;
            }
        }
        if let Some(ref_gi) = tg.accept_when_same_index_empty {
            let ref_pile = self.groups[ref_gi as usize].start + (to - tg.start);
            if from != ref_pile && !s.piles[ref_pile as usize].is_empty() {
                return false;
            }
        }
        if tg.requires_prev_non_empty
            && dst.is_empty()
            && to > tg.start
            && s.piles[to as usize - 1].is_empty()
        {
            return false;
        }
        if let Some(limit) = tg.max_same_rank {
            if !card.is_joker() {
                let r = card.rank();
                let existing = dst
                    .iter()
                    .rev()
                    .take_while(|c| c.is_face_up() && !c.is_joker() && c.rank() == r)
                    .count();
                let incoming = src[src.len() - n..]
                    .iter()
                    .take_while(|c| !c.is_joker() && c.rank() == r)
                    .count();
                if existing + incoming > limit as usize {
                    return false;
                }
            }
        }
        if tg.requires_rank_on_prev && to > tg.start {
            let prev = &s.piles[to as usize - 1];
            if prev.is_empty() {
                return false;
            }
            if !card.is_joker() {
                let rank_ok = if dst.is_empty() {
                    prev[0].is_joker() || prev[0].rank() == card.rank()
                } else {
                    prev.iter().any(|c| c.is_joker() || c.rank() == card.rank())
                };
                if !rank_ok {
                    return false;
                }
            }
        }
        if tg.match_longest_rank && !card.is_joker() {
            let mut max_p = None;
            let mut max_len = 0usize;
            for p in tg.piles() {
                let len = s.piles[p as usize].len();
                if len > max_len {
                    max_len = len;
                    max_p = Some(p);
                }
            }
            if dst.len() < max_len {
                let ref_card = s.piles[max_p.unwrap() as usize][dst.len()];
                if !ref_card.is_joker() && ref_card.rank() != card.rank() {
                    return false;
                }
            }
        }
        if self.rule(from).moves == MoveUnit::Supermove
            && n > 1
            && !(self.groups[fgi as usize].supermove_complete_run && n == self.nranks as usize)
        {
            let cap = supermove_cap.unwrap_or_else(|| self.supermove_limit(s, from, to));
            return n <= cap;
        }
        true
    }

    /// True if all piles of group `gi` are interchangeable in state `s` (either always, or
    /// because the stock / tableau redeals that broke symmetry can no longer fire).
    #[inline]
    pub(crate) fn is_group_symmetric(&self, gi: usize, s: &State) -> bool {
        let g = &self.groups[gi];
        if g.symmetric {
            return true;
        }
        if g.symmetric_when_no_stock {
            if let Some(st) = &self.stock {
                if s.piles[st.pile as usize].is_empty()
                    && st.redeals != u32::MAX
                    && s.redeals_used >= st.redeals
                {
                    return true;
                }
            }
        }
        if g.symmetric_when_no_redeal {
            if let Some(rd) = &self.redeal {
                if rd.times != u32::MAX && s.redeals_used >= rd.times {
                    return true;
                }
            }
        }
        false
    }

    /// First empty pile of each symmetric group (first 64) and each symmetric sub-class (next 64):
    /// moving to another empty pile of the same group/class is equivalent.
    pub(crate) fn compute_first_empty(&self, s: &State) -> ([u16; 64], [u16; 64]) {
        let mut first_empty = [u16::MAX; 64];
        let mut class_empty = [u16::MAX; 64];
        for (gi, g) in self.groups.iter().enumerate() {
            if self.is_group_symmetric(gi, s) {
                if let Some(p) = g.piles().find(|&p| s.piles[p as usize].is_empty()) {
                    first_empty[gi] = p;
                }
            } else if !g.sym_classes.is_empty() {
                for cls in &g.sym_classes {
                    if cls.len() > 1 {
                        let cid = self.pile_sym_class[cls[0] as usize];
                        if cid != u16::MAX {
                            if let Some(&p) = cls.iter().find(|&&p| s.piles[p as usize].is_empty())
                            {
                                class_empty[cid as usize] = p;
                            }
                        }
                    }
                }
            }
        }
        (first_empty, class_empty)
    }

    /// Generate transfers and removals (no stock actions). With `only`, just the moves that use
    /// that pile's top card.
    pub(crate) fn gen_actions(&self, s: &State, only: Option<u16>, out: &mut Vec<Move>) {
        if let Some(p) = only {
            if s.piles[p as usize].is_empty() {
                return;
            }
        }
        let first_empty = self.compute_first_empty(s);
        self.gen_actions_with_empty(s, only, &first_empty, out);
    }

    pub(crate) fn gen_actions_with_empty(
        &self,
        s: &State,
        only: Option<u16>,
        first_empty: &([u16; 64], [u16; 64]),
        out: &mut Vec<Move>,
    ) {
        let npiles = s.piles.len() as u16;
        // Precompute empty cell and empty-any-tableau counts once when supermoves are in play.
        let (empty_cells, empty_any_group) = if self.has_supermove {
            let mut cells = 0usize;
            let mut by_group = [0u16; 64];
            for (p, pile) in s.piles.iter().enumerate() {
                if !pile.is_empty() {
                    continue;
                }
                let gi = self.pile_group[p] as usize;
                if self.groups[gi].kind == PileKind::Cell {
                    cells += 1;
                } else if self.pile_rules[p].empty == EmptyRule::Any {
                    by_group[gi] += 1;
                }
            }
            (cells, by_group)
        } else {
            (0, [0u16; 64])
        };

        let no_hooks = self.hooks.is_none();
        let mut single = [0u16; 1];
        let sources: &[u16] = match only {
            Some(p) => {
                single[0] = p;
                &single
            }
            None if no_hooks => &self.movable_piles,
            None => &[],
        };
        let all_piles_iter = if only.is_none() && !no_hooks {
            0..npiles
        } else {
            0..0
        };

        // When scanning many source piles without hooks, pre-filter destination piles once in
        // O(npiles) so full single-card piles, dead gaps, and non-first empty symmetric piles
        // are never visited in the inner loop. Each entry is `(to, accept_mask, from_mask)`.
        let use_active = only.is_none() && no_hooks && self.movable_piles.len() >= 8;
        let mut active_targets: Vec<(u16, u64, u64)> = Vec::new();
        let mut union_accept_mask = 0u64;
        if use_active {
            active_targets.reserve((npiles as usize).min(32));
            for to in 0..npiles {
                let tgi = self.pile_group[to as usize] as usize;
                let tg = &self.groups[tgi];
                let tr = self.rule(to);
                if tg.kind == PileKind::Stock
                    && tr.empty == EmptyRule::None
                    && tr.build.by == BuildBy::None
                {
                    continue;
                }
                let dst = &s.piles[to as usize];
                let dst_empty = dst.is_empty();
                if dst_empty {
                    if self.is_group_symmetric(tgi, s) {
                        if first_empty.0[tgi] != to {
                            continue;
                        }
                    } else {
                        let cid = self.pile_sym_class[to as usize];
                        if cid != u16::MAX && first_empty.1[cid as usize] != to {
                            continue;
                        }
                    }
                    if !tr.complete_runs
                        && tg.unrestricted_from == 0
                        && tg.rank_sequence.is_empty()
                        && tr.empty == EmptyRule::None
                        && tg.final_from_same_index.is_none()
                    {
                        continue;
                    }
                } else {
                    if tg.replace_on_drop.is_none()
                        && tg.swap_bases.is_none()
                        && dst.len() >= tr.max_cards as usize
                    {
                        continue;
                    }
                    if tg.accept_only_when_empty && tg.swap_bases.is_none() {
                        continue;
                    }
                    if !tr.complete_runs
                        && tg.unrestricted_from == 0
                        && tg.rank_sequence.is_empty()
                        && tr.build.by == BuildBy::None
                        && tg.swap_bases.is_none()
                        && tg.final_from_same_index.is_none()
                    {
                        continue;
                    }
                }
                if !self.available(s, to) && !tg.dropdown {
                    continue;
                }
                let swap_possible = match tg.swap_bases {
                    None => false,
                    Some(SwapBases::BothSingle) => dst.len() == 1,
                    Some(SwapBases::EitherPile) => !dst_empty,
                    Some(SwapBases::Crossing) => dst.len() == 1,
                };
                let mut from_mask = if dst_empty {
                    tg.empty_accepts_from
                } else {
                    tg.accepts_from
                } | tg.unrestricted_from;
                if swap_possible {
                    from_mask |= 1u64 << tgi;
                    if tg.swap_bases == Some(SwapBases::Crossing) {
                        for (gi, og) in self.groups.iter().enumerate() {
                            if og.swap_bases == Some(SwapBases::Crossing) {
                                from_mask |= 1u64 << gi;
                            }
                        }
                    }
                }
                if let Some(ref_gi) = tg.final_from_same_index {
                    from_mask |= 1u64 << ref_gi;
                }
                if tg.reach.is_some() {
                    from_mask &= 1u64 << tgi;
                }
                if from_mask == 0 {
                    continue;
                }
                let accept_mask = if self.jokers > 0
                    || tr.complete_runs
                    || tg.unrestricted_from != 0
                    || !tg.rank_sequence.is_empty()
                    || tg.final_from_same_index.is_some()
                {
                    u64::MAX
                } else if dst_empty {
                    let m = self.empty_set(to, s);
                    if m == 0 {
                        continue;
                    }
                    m
                } else {
                    let top = *dst.last().unwrap();
                    if !top.is_face_up() {
                        continue;
                    }
                    let m = tr.fits[top.face() as usize];
                    if m == 0 && !swap_possible {
                        continue;
                    }
                    m
                };
                union_accept_mask |= if swap_possible { u64::MAX } else { accept_mask };
                active_targets.push((to, accept_mask, from_mask));
            }
            if active_targets.is_empty() {
                if !self.removal.is_empty() {
                    self.gen_removals(s, only, out);
                }
                return;
            }
        }

        let mut emit_from = |from: u16| {
            if !self.available(s, from) {
                return;
            }
            let unit = self.movable(s, from);
            if unit == 0 {
                return;
            }
            let fg = self.pile_group[from as usize];
            let src = &s.piles[from as usize];
            let src_len = src.len();
            if use_active
                && unit == 1
                && self.jokers == 0
                && (union_accept_mask >> src[src_len - 1].face()) & 1 == 0
            {
                return;
            }
            let from_rule = self.rule(from);
            let from_kind = self.groups[fg as usize].kind;
            let from_moves = from_rule.moves;
            let targets: &[u16] = if no_hooks && !use_active {
                &self.targets_for_group[fg as usize]
            } else {
                &[]
            };
            let target_count = if use_active {
                active_targets.len()
            } else if no_hooks {
                targets.len()
            } else {
                npiles as usize
            };
            #[allow(clippy::needless_range_loop)]
            for ti in 0..target_count {
                let (to, pre_accept_mask) = if use_active {
                    let (t, am, fm) = active_targets[ti];
                    if fm & (1u64 << fg) == 0 {
                        continue;
                    }
                    (t, Some(am))
                } else {
                    (targets.get(ti).copied().unwrap_or(ti as u16), None)
                };
                if to == from {
                    continue;
                }
                let tgi = self.pile_group[to as usize] as usize;
                let tg = &self.groups[tgi];
                if from_kind == PileKind::Foundation && tg.kind == PileKind::Foundation {
                    continue;
                }
                let dst = &s.piles[to as usize];
                let dst_empty = dst.is_empty();
                let tg_sym = self.is_group_symmetric(tgi, s);
                if !use_active && dst_empty {
                    if tg_sym {
                        if first_empty.0[tgi] != to {
                            continue;
                        }
                    } else {
                        let cid = self.pile_sym_class[to as usize];
                        if cid != u16::MAX && first_empty.1[cid as usize] != to {
                            continue;
                        }
                    }
                }
                let tr = self.rule(to);
                let unrestricted = tg.unrestricted_from & (1 << fg) != 0;
                if no_hooks && !use_active {
                    if tg.replace_on_drop.is_none()
                        && tg.swap_bases.is_none()
                        && dst.len() >= tr.max_cards as usize
                    {
                        continue;
                    }
                    if !tr.complete_runs && !unrestricted && tg.rank_sequence.is_empty() {
                        if dst_empty {
                            if tr.empty == EmptyRule::None {
                                continue;
                            }
                        } else if tr.build.by == BuildBy::None && tg.swap_bases.is_none() {
                            continue;
                        }
                    }
                }
                let (lo, hi) = if tr.complete_runs {
                    let n = tg.run_len.map_or(self.nranks as usize, |l| l as usize);
                    if n > unit {
                        continue;
                    }
                    (n, n)
                } else if from_moves == MoveUnit::Whole {
                    (unit, unit)
                } else {
                    let max_n = if no_hooks {
                        let room = if tg.replace_on_drop.is_some() || tg.swap_bases.is_some() {
                            tr.max_cards as usize
                        } else {
                            (tr.max_cards as usize).saturating_sub(dst.len())
                        };
                        let cap = if dst_empty {
                            tr.empty_max_accept.min(tr.max_accept) as usize
                        } else {
                            tr.non_empty_max_accept.min(tr.max_accept) as usize
                        };
                        unit.min(cap).min(room)
                    } else {
                        unit.min(tr.max_accept as usize)
                    };
                    if max_n == 0 {
                        continue;
                    }
                    // On a non-empty pile, at most one depth can fit for most build rules,
                    // but either-way building can allow two; check them all.
                    (1, max_n)
                };
                // Fast bitmask pre-filter for non-joker destinations.
                let fit_mask = if let Some(am) = pre_accept_mask {
                    (am != u64::MAX).then_some(am)
                } else if no_hooks
                    && self.jokers == 0
                    && !tr.complete_runs
                    && !dst_empty
                    && !unrestricted
                    && tg.rank_sequence.is_empty()
                    && tg.swap_bases.is_none()
                {
                    let top = *dst.last().unwrap();
                    if !top.is_face_up() {
                        continue;
                    }
                    Some(tr.fits[top.face() as usize])
                } else {
                    None
                };
                let supermove_cap = if from_moves == MoveUnit::Supermove && hi > 1 {
                    let mut cells = empty_cells;
                    let mut empties = empty_any_group[fg as usize] as u32;
                    if dst_empty {
                        if tg.kind == PileKind::Cell {
                            cells = cells.saturating_sub(1);
                        } else if tgi == fg as usize && tr.empty == EmptyRule::Any {
                            empties = empties.saturating_sub(1);
                        }
                    }
                    Some((cells + 1) << empties.min(16))
                } else {
                    None
                };
                for n in lo..=hi {
                    if !move_count_allowed(from_moves, n, unit, src_len) {
                        continue;
                    }
                    // Moving a whole pile into an empty pile of the same symmetric group/class is a no-op.
                    if dst_empty && n == src_len && tgi == fg as usize {
                        let same_sym = tg_sym
                            || (self.pile_sym_class[from as usize] != u16::MAX
                                && self.pile_sym_class[from as usize]
                                    == self.pile_sym_class[to as usize]);
                        if same_sym {
                            continue;
                        }
                    }
                    if let Some(mask) = fit_mask {
                        if mask >> src[src_len - n].face() & 1 == 0 {
                            if n == 1
                                && tgi == fg as usize
                                && tg.swap_bases.is_some()
                                && from < to
                                && self.can_swap(s, from, to)
                            {
                                out.push(Move::new(Action::Transfer { from, to, count: 1 }));
                            }
                            continue;
                        }
                    }
                    // Swapping two piles is symmetric (`from <-> to`); emit each pair once.
                    if n == 1 && from > to && tg.swap_bases.is_some() && self.can_swap(s, from, to)
                    {
                        continue;
                    }
                    if self.can_place_with_limit(s, from, to, n, supermove_cap) {
                        out.push(Move::new(Action::Transfer {
                            from,
                            to,
                            count: n as u16,
                        }));
                    }
                }
            }
        };

        if only.is_some() || no_hooks {
            for &from in sources {
                emit_from(from);
            }
        } else {
            for from in all_piles_iter {
                emit_from(from);
            }
        }
        if !self.removal.is_empty() {
            self.gen_removals(s, only, out);
        }
    }

    pub(super) fn gen_removals(&self, s: &State, only: Option<u16>, out: &mut Vec<Move>) {
        for (ri, r) in self.removal.iter().enumerate() {
            let mut tops: Vec<(u16, Card)> = Vec::new();
            for (p, pile) in s.piles.iter().enumerate() {
                let p = p as u16;
                if r.from & (1 << self.pile_group[p as usize]) == 0 || !self.available(s, p) {
                    continue;
                }
                if let Some(&c) = pile.last() {
                    if c.is_face_up() && !r.exclude_ranks.contains(&c.rank()) {
                        tops.push((p, c));
                    }
                }
            }
            let neighbours = |piles: &[u16]| -> bool {
                let Some((adj, gi)) = r.adjacent else {
                    return true;
                };
                let g = &self.groups[gi as usize];
                let cols = g.columns.unwrap() as i32;
                let pos = |p: u16| {
                    let i = (p - g.start) as i32;
                    (i / cols, i % cols)
                };
                piles.windows(2).all(|w| {
                    let ((r1, c1), (r2, c2)) = (pos(w[0]), pos(w[1]));
                    let (dr, dc) = ((r1 - r2).abs(), (c1 - c2).abs());
                    match adj {
                        Adjacency::Eight => dr.max(dc) == 1,
                        Adjacency::Four => dr + dc == 1,
                        Adjacency::SameLine => dr == 0 || dc == 0,
                    }
                })
            };
            let mut emit = |piles: &[u16]| {
                if only.is_none_or(|o| piles.contains(&o)) && neighbours(piles) {
                    out.push(Move::new(removal_action(ri, piles)));
                }
            };
            match &r.kind {
                RemovalKind::PairsSum { total, alone } => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        if alone.contains(&c.rank()) {
                            emit(&[p]);
                            continue;
                        }
                        for &(q, d) in &tops[i + 1..] {
                            if c.is_joker() || d.is_joker() || c.rank() + d.rank() == *total {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::RankDistance { distances } => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        for &(q, d) in &tops[i + 1..] {
                            let gap = (c.rank() as i16 - d.rank() as i16).unsigned_abs() as u8;
                            if c.is_joker() || d.is_joker() || distances.contains(&gap) {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::PairsSumOtherSuit { total } => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        for &(q, d) in &tops[i + 1..] {
                            let other = c.is_joker() || d.is_joker() || c.suit() != d.suit();
                            if other
                                && (c.is_joker() || d.is_joker() || c.rank() + d.rank() == *total)
                            {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::PairsSumSameSuit { total } => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        for &(q, d) in &tops[i + 1..] {
                            let suited = c.is_joker() || d.is_joker() || c.suit() == d.suit();
                            if suited
                                && (c.is_joker() || d.is_joker() || c.rank() + d.rank() == *total)
                            {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::SameRankIn { ranks } => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        for &(q, d) in &tops[i + 1..] {
                            let ok = if c.is_joker() && d.is_joker() {
                                true
                            } else if c.is_joker() {
                                ranks.contains(&d.rank())
                            } else if d.is_joker() {
                                ranks.contains(&c.rank())
                            } else {
                                c.rank() == d.rank() && ranks.contains(&c.rank())
                            };
                            if ok {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::SameRank => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        for &(q, d) in &tops[i + 1..] {
                            if c.rank() == d.rank() || c.is_joker() || d.is_joker() {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::SameSuit => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        for &(q, d) in &tops[i + 1..] {
                            if c.suit() == d.suit() || c.is_joker() || d.is_joker() {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::LowerOfSuit => {
                    for &(p, c) in tops.iter().filter(|(_, c)| !c.is_joker()) {
                        let ci = self.rank_idx[c.rank() as usize];
                        if tops.iter().any(|&(q, d)| {
                            q != p
                                && !d.is_joker()
                                && d.suit() == c.suit()
                                && self.rank_idx[d.rank() as usize] > ci
                        }) {
                            emit(&[p]);
                        }
                    }
                }
                RemovalKind::RankSet { ranks } => {
                    // Every way of picking one playable card per listed rank, from distinct piles.
                    let mut sets: Vec<Vec<u16>> = Vec::new();
                    let mut pick: Vec<u16> = Vec::new();
                    fn rec(
                        ranks: &[u8],
                        tops: &[(u16, Card)],
                        pick: &mut Vec<u16>,
                        sets: &mut Vec<Vec<u16>>,
                    ) {
                        let Some((&r, rest)) = ranks.split_first() else {
                            let mut v = pick.clone();
                            v.sort_unstable();
                            if !sets.contains(&v) {
                                sets.push(v);
                            }
                            return;
                        };
                        for &(p, c) in tops {
                            if (c.rank() == r || c.is_joker()) && !pick.contains(&p) {
                                pick.push(p);
                                rec(rest, tops, pick, sets);
                                pick.pop();
                            }
                        }
                    }
                    rec(ranks, &tops, &mut pick, &mut sets);
                    for set in sets {
                        emit(&set);
                    }
                }
                RemovalKind::ReachSuitOrRank { distances } => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        let pg = self.pile_group[p as usize];
                        for &(q, d) in &tops[i + 1..] {
                            if self.pile_group[q as usize] != pg {
                                continue;
                            }
                            if distances.contains(&q.abs_diff(p))
                                && (c.is_joker()
                                    || d.is_joker()
                                    || c.suit() == d.suit()
                                    || c.rank() == d.rank())
                            {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::BetweenMatch { max_len } => {
                    let max_l = *max_len as usize;
                    for (gi, g) in self.groups.iter().enumerate() {
                        if r.from & (1 << gi) == 0 {
                            continue;
                        }
                        let start = g.start;
                        let end = g.start + g.count;
                        for len in 1..=max_l {
                            for p in start + 1..end {
                                let last = p as usize + len - 1;
                                let right = p as usize + len;
                                if right >= end as usize {
                                    break;
                                }
                                let (Some(&lc), Some(&rc)) =
                                    (s.piles[p as usize - 1].last(), s.piles[right].last())
                                else {
                                    continue;
                                };
                                if !lc.is_face_up() || !rc.is_face_up() {
                                    continue;
                                }
                                let match_lr = lc.is_joker()
                                    || rc.is_joker()
                                    || lc.suit() == rc.suit()
                                    || lc.rank() == rc.rank();
                                if !match_lr {
                                    continue;
                                }
                                let mut buf = [0u16; 4];
                                let mut ok = true;
                                for (k, slot) in buf.iter_mut().enumerate().take(len) {
                                    let q = p + k as u16;
                                    if !self.available(s, q)
                                        || !s.piles[q as usize]
                                            .last()
                                            .is_some_and(|c| c.is_face_up())
                                    {
                                        ok = false;
                                        break;
                                    }
                                    *slot = q;
                                }
                                let _ = last;
                                if ok {
                                    emit(&buf[..len]);
                                }
                            }
                        }
                    }
                }
                RemovalKind::ContiguousSumMultiple { divisor, max_len } => {
                    let div = *divisor as u32;
                    let max_l = *max_len as usize;
                    for (gi, g) in self.groups.iter().enumerate() {
                        if r.from & (1 << gi) == 0 {
                            continue;
                        }
                        let start = g.start;
                        let end = g.start + g.count;
                        for p in start..end {
                            let mut sum = 0u32;
                            let mut buf = [0u16; 4];
                            for k in 0..max_l {
                                let q = p + k as u16;
                                if q >= end || !self.available(s, q) {
                                    break;
                                }
                                let Some(&c) = s.piles[q as usize].last() else {
                                    break;
                                };
                                if !c.is_face_up() || c.is_joker() {
                                    break;
                                }
                                sum += c.rank() as u32;
                                buf[k] = q;
                                if sum % div == 0 {
                                    emit(&buf[..=k]);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Can the stock deal (or the waste be turned back) now?
    pub fn can_deal(&self, s: &State) -> bool {
        if s.base_rank.is_none()
            && self.choose_base.as_ref().is_some_and(|cb| {
                !cb.free_moves
                    || self.stock.as_ref().is_some_and(|st| {
                        cb.lock_mask >> self.pile_group[st.pile as usize] & 1 != 0
                    })
            })
        {
            return false;
        }
        if let Some(rd) = &self.redeal {
            if s.redeals_used >= rd.times {
                return false;
            }
            if matches!(rd.how, RedealHow::Compact { .. }) {
                let g = &self.groups[rd.group as usize];
                let mut saw_empty = false;
                let mut can_shift = false;
                for p in g.piles() {
                    if s.piles[p as usize].is_empty() {
                        saw_empty = true;
                    } else if saw_empty {
                        can_shift = true;
                        break;
                    }
                }
                let can_refill = saw_empty
                    && rd.refill.is_some_and(|rf| {
                        self.groups[rf as usize]
                            .piles()
                            .any(|p| !s.piles[p as usize].is_empty())
                    });
                return can_shift || can_refill;
            }
            if rd.how == RedealHow::RotateBottom {
                let g = &self.groups[rd.group as usize];
                return g.piles().any(|p| s.piles[p as usize].len() > 1);
            }
            if matches!(
                rd.how,
                RedealHow::DealRows | RedealHow::DealStaircase | RedealHow::ShuffleRedeal
            ) {
                let g = &self.groups[rd.group as usize];
                return g.piles().any(|p| !s.piles[p as usize].is_empty());
            }
            return true;
        }
        let Some(st) = &self.stock else { return false };
        if !s.piles[st.pile as usize].is_empty() {
            if st.redeal_gather_only {
                return false;
            }
            if !st.face_up
                && s.piles[st.pile as usize]
                    .last()
                    .is_some_and(|c| c.is_face_up())
            {
                return false;
            }
            let stock_len = s.piles[st.pile as usize].len() as u16;
            if st.final_split_to.is_some() && stock_len < 2 {
                return false;
            }
            let rtargets = st.round_targets(s.redeals_used);
            let any_non_empty = || rtargets.iter().any(|&p| !s.piles[p as usize].is_empty());
            let any_empty = || rtargets.iter().any(|&p| s.piles[p as usize].is_empty());
            let final_to_active = st
                .final_to
                .is_some_and(|ft| stock_len <= self.groups[ft as usize].count);
            if st.only_when_empty && any_non_empty() {
                return false;
            }
            if st.first_empty && !any_empty() {
                return false;
            }
            if st.require_no_empty && !final_to_active && any_empty() {
                return false;
            }
            if let Some(req_gi) = st.require_no_empty_group {
                if self.groups[req_gi as usize]
                    .piles()
                    .any(|p| s.piles[p as usize].is_empty())
                {
                    return false;
                }
            }
            if st.max_deals.is_some_and(|m| s.redeals_used >= m) {
                return false;
            }
            if let Some(r) = st.skip_run_with_rank {
                if st
                    .targets
                    .iter()
                    .all(|&p| self.top_run_has_rank(p, &s.piles[p as usize], r))
                {
                    return false;
                }
            }
            if let Some(from) = st.skip_built_from {
                if st
                    .targets
                    .iter()
                    .all(|&p| self.is_built_from(p, &s.piles[p as usize], from))
                {
                    return false;
                }
            }
            if let Some(req_gi) = st.require_group_empty {
                if self.groups[req_gi as usize]
                    .piles()
                    .any(|p| !s.piles[p as usize].is_empty())
                {
                    return false;
                }
            }
            if matches!(st.target_kind, PileKind::Waste | PileKind::Foundation) {
                let dest = st.targets[0];
                let n = st
                    .deal_count(s.redeals_used)
                    .min(s.piles[st.pile as usize].len() as u16);
                if s.piles[dest as usize].len() + n as usize > self.rule(dest).max_cards as usize {
                    return false;
                }
            }
            return true;
        }
        s.redeals_used < st.redeals
            && self.groups[st.target as usize]
                .piles()
                .any(|p| !s.piles[p as usize].is_empty())
    }

    /// Every legal move in this position, as single steps (stock deals are separate moves).
    pub fn legal_moves(&self, s: &State) -> Vec<Move> {
        let mut out = Vec::new();
        self.gen_actions(s, None, &mut out);
        if self.can_deal(s) {
            out.push(Move::new(Action::Deal));
        }
        out
    }

    /// Is `mv` legal in `s`? (Including any talon steps it starts with.)
    pub fn is_legal(&self, s: &State, mv: Move) -> bool {
        let mut st = s.clone();
        let mut j = Vec::new();
        for _ in 0..mv.talon {
            if !self.talon_step(&mut st, &mut j) {
                return false;
            }
        }
        let mut moves = Vec::new();
        match mv.action {
            Action::Deal => return self.can_deal(&st),
            _ => self.gen_actions_unpruned(&st, &mut moves),
        }
        moves.contains(&Move::new(mv.action))
    }

    /// Like `gen_actions` but without the symmetry pruning, for checking moves made by people.
    pub(super) fn gen_actions_unpruned(&self, s: &State, out: &mut Vec<Move>) {
        let n = s.piles.len() as u16;
        for from in 0..n {
            if !self.available(s, from) {
                continue;
            }
            let unit = self.movable(s, from);
            let from_moves = self.rule(from).moves;
            let src_len = s.piles[from as usize].len();
            for to in 0..n {
                for k in 1..=unit {
                    if !move_count_allowed(from_moves, k, unit, src_len) {
                        continue;
                    }
                    if self.can_place(s, from, to, k) {
                        out.push(Move::new(Action::Transfer {
                            from,
                            to,
                            count: k as u16,
                        }));
                    }
                }
            }
        }
        self.gen_removals(s, None, out);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RunFilter {
    Any,
    SameSuit,
    AltColor,
    SameColor,
}

/// Whether `moves` moves built runs (used for joker run checks).
#[inline]
fn is_run_unit(moves: MoveUnit) -> bool {
    matches!(
        moves,
        MoveUnit::Run
            | MoveUnit::SuitRun
            | MoveUnit::AltColorRun
            | MoveUnit::SameColorRun
            | MoveUnit::SuitRunOrSameRank
            | MoveUnit::Supermove
            | MoveUnit::TopOrWhole
    )
}

/// Whether transferring `n` cards (out of `unit` movable from a pile of length `src_len`) is
/// permitted by `moves`. Shared between `gen_actions_with_empty` and `gen_actions_unpruned` so
/// move generation and `is_legal` stay in lockstep.
#[inline]
fn move_count_allowed(moves: MoveUnit, n: usize, unit: usize, src_len: usize) -> bool {
    match moves {
        MoveUnit::Whole => n == unit,
        MoveUnit::TopOrWhole => n == 1 || n == src_len,
        _ => true,
    }
}

/// A removal of the top cards of `piles` under rule `rule`.
fn removal_action(rule: usize, piles: &[u16]) -> Action {
    let mut a = [u16::MAX; 4];
    let mut v = piles.to_vec();
    v.sort_unstable();
    a[..v.len()].copy_from_slice(&v);
    Action::Remove {
        rule: rule as u8,
        n: v.len() as u8,
        piles: a,
    }
}
