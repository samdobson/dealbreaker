//! Card relations, legal move generation, and move legality checks.

use crate::card::{Card, JOKER, REAL_FACES};
use crate::rules::*;

use super::{Action, Game, Move, PileRules, State};

impl Game {
    // ----- Card relations -------------------------------------------------------------------

    /// Can `over` be placed on `under` under build rule `b`?
    pub(crate) fn fits(&self, b: &Build, under: Card, over: Card) -> bool {
        let ok_suit = match b.by {
            BuildBy::None => return false,
            BuildBy::Any => true,
            BuildBy::Suit => under.suit() == over.suit(),
            BuildBy::Alternate => under.color() != over.color(),
            BuildBy::SameColor => under.color() == over.color(),
            BuildBy::OtherSuit => under.suit() != over.suit(),
            BuildBy::SuitOrRank => under.suit() == over.suit() || under.rank() == over.rank(),
        };
        if !ok_suit || b.blocked_ranks.contains(&under.rank()) {
            return false;
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
        let up = |from: u8, to: u8| -> bool {
            let d = to as i16 - from as i16;
            d == step || (b.wrap && d == step - n)
        };
        match b.dir {
            Dir::Up => up(u, o),
            Dir::Down => up(o, u),
            Dir::Either => up(u, o) || up(o, u),
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
        match &r.empty {
            EmptyRule::None => false,
            EmptyRule::Any => true,
            EmptyRule::Rank(r) => card.rank() == *r,
            EmptyRule::Ranks(ranks) => ranks.contains(&card.rank()),
            EmptyRule::DealtBase => s.base_rank.is_none_or(|b| card.rank() == b),
            EmptyRule::DealtBaseOffset(o) => s
                .base_rank
                .is_none_or(|b| card.rank() == self.rank_from(b, *o)),
            EmptyRule::AfterLeft { .. }
            | EmptyRule::AfterNeighbor { .. }
            | EmptyRule::AfterPile { .. } => self.empty_set(pile, s) >> card.face() & 1 != 0,
        }
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
            EmptyRule::DealtBaseOffset(o) => s.base_rank.map_or(real, |b| {
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

    /// How many cards from the top of `pile` form a unit that may move together (0 if none).
    pub(crate) fn movable(&self, s: &State, pile: u16) -> usize {
        let cards = &s.piles[pile as usize];
        let Some(&top) = cards.last() else { return 0 };
        if !top.is_face_up() {
            return 0;
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
        if let Some(ref_gi) = grp.moves_when_same_index_empty {
            let ref_pile = self.groups[ref_gi as usize].start + (pile - grp.start);
            if !s.piles[ref_pile as usize].is_empty() {
                return 0;
            }
        }
        let g = self.rule(pile);
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

    /// Is the top `n` cards of `cards` a complete sequence matching `by`, ready for a complete-run foundation?
    fn is_complete_run(&self, cards: &[Card], n: usize, by: BuildBy, wrap: bool) -> bool {
        if n != self.nranks as usize || cards.len() < n {
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
        run[0].is_face_up() && run[n - 1].is_face_up() && (all(&b) || all(&b_up))
    }

    // ----- Moves ----------------------------------------------------------------------------

    /// True if moving from `from` onto `to` swaps the two piles under `GroupRules::swap_bases`.
    pub(crate) fn can_swap(&self, s: &State, from: u16, to: u16) -> bool {
        if from == to {
            return false;
        }
        let fgi = self.pile_group[from as usize];
        if fgi != self.pile_group[to as usize] {
            return false;
        }
        let Some(mode) = self.groups[fgi as usize].swap_bases else {
            return false;
        };
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
            SwapBases::EitherPile => {
                self.rule(from).empty != self.rule(to).empty
                    && c0.rank() != c1.rank()
                    && (self.empty_accepts(to, c0, s) || self.empty_accepts(from, c1, s))
            }
        }
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
                && (fgi != cb.from_group || tg.kind != PileKind::Foundation || n != 1)
            {
                return false;
            }
        }
        let dst = &s.piles[to as usize];
        if matches!(tg.kind, PileKind::Stock) {
            return false;
        }
        if self.can_swap(s, from, to) {
            return n == 1 && self.available(s, to);
        }
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
            if mask & (1 << fgi) == 0 {
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
        }
        let src = &s.piles[from as usize];
        if tg.accept_only_when_empty && !dst.is_empty() {
            return false;
        }
        if tg.require_valid_base && !dst.is_empty() && !self.empty_accepts(to, dst[0], s) {
            return false;
        }
        if tg.accept_when_stock_empty
            && self
                .stock_pile
                .is_some_and(|sp| !s.piles[sp as usize].is_empty())
        {
            return false;
        }
        if n == 0
            || n > src.len()
            || n > tr.max_accept as usize
            || (dst.is_empty() && n > tr.empty_max_accept as usize)
            || (if tg.replace_on_drop.is_some() {
                0
            } else {
                dst.len()
            }) + n
                > tr.max_cards as usize
        {
            return false;
        }
        if !self.available(s, to) {
            return false;
        }
        let card = src[src.len() - n];
        if tr.complete_runs {
            return dst.is_empty() && self.is_complete_run(src, n, tr.build.by, tr.build.wrap);
        }
        let ok = if self.jokers == 0 {
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
        if tg.lock_build_dir && !dst.is_empty() && !card.is_joker() {
            let n = self.nranks as i16;
            let step = tr.build.step as i16;
            let is_up = |a: Card, b: Card| -> bool {
                let d = self.rank_idx[b.rank() as usize] as i16
                    - self.rank_idx[a.rank() as usize] as i16;
                d == step || (tr.build.wrap && d == step - n)
            };
            let locked = tg.piles().find_map(|p| {
                let pile = &s.piles[p as usize];
                if pile.len() >= 2 && !pile[0].is_joker() && !pile[1].is_joker() {
                    Some(is_up(pile[0], pile[1]))
                } else {
                    None
                }
            });
            if let Some(want_up) = locked {
                let top = *dst.last().unwrap();
                if !top.is_joker() && is_up(top, card) != want_up {
                    return false;
                }
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
        if self.rule(from).moves == MoveUnit::Supermove && n > 1 {
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
            let from_rule = self.rule(from);
            let from_kind = self.groups[fg as usize].kind;
            let from_moves = from_rule.moves;
            let targets: &[u16] = if no_hooks {
                &self.targets_for_group[fg as usize]
            } else {
                &[]
            };
            let target_count = if no_hooks {
                targets.len()
            } else {
                npiles as usize
            };
            for ti in 0..target_count {
                let to = targets.get(ti).copied().unwrap_or(ti as u16);
                if to == from {
                    continue;
                }
                let tgi = self.pile_group[to as usize] as usize;
                let tg = &self.groups[tgi];
                let dst = &s.piles[to as usize];
                let dst_empty = dst.is_empty();
                let tg_sym = self.is_group_symmetric(tgi, s);
                if dst_empty {
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
                if !no_hooks && from_kind == PileKind::Foundation && tg.kind == PileKind::Foundation
                {
                    continue;
                }
                let tr = self.rule(to);
                if no_hooks {
                    if tg.replace_on_drop.is_none()
                        && tg.swap_bases.is_none()
                        && dst.len() >= tr.max_cards as usize
                    {
                        continue;
                    }
                    if !tr.complete_runs {
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
                    let n = self.nranks as usize;
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
                            tr.max_accept as usize
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
                // Fast bitmask pre-filter for non-empty, non-joker destinations.
                let fit_mask = if no_hooks
                    && self.jokers == 0
                    && !tr.complete_runs
                    && !dst_empty
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
                            continue;
                        }
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
                    if c.is_face_up() {
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
                RemovalKind::SameRank => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        for &(q, d) in &tops[i + 1..] {
                            if c.rank() == d.rank() || c.is_joker() || d.is_joker() {
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
        if self.choose_base.is_some() && s.base_rank.is_none() {
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
            return true;
        }
        let Some(st) = &self.stock else { return false };
        if !s.piles[st.pile as usize].is_empty() {
            if st.final_split_to.is_some() && s.piles[st.pile as usize].len() < 2 {
                return false;
            }
            let rtargets = st.round_targets(s.redeals_used);
            let any_non_empty = || rtargets.iter().any(|&p| !s.piles[p as usize].is_empty());
            let any_empty = || rtargets.iter().any(|&p| s.piles[p as usize].is_empty());
            if st.only_when_empty && any_non_empty() {
                return false;
            }
            if st.first_empty && !any_empty() {
                return false;
            }
            if st.require_no_empty && any_empty() {
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
