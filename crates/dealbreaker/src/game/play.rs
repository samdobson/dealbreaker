//! Reversible state transitions, talon/redeal execution, settling, and win checks.

use crate::card::Card;
use crate::rng::Rng;
use crate::rules::*;

use super::{Action, Game, IllegalMove, Move, Op, State, Win};

impl Game {
    // ----- Playing --------------------------------------------------------------------------

    pub(crate) fn do_op(&self, s: &mut State, op: Op) {
        match op {
            Op::Shift { from, to, count } => {
                let (from, to) = (from as usize, to as usize);
                if count == 1 {
                    let c = s.piles[from].pop().expect("shift from empty pile");
                    s.piles[to].push(c);
                } else {
                    let at = s.piles[from].len() - count as usize;
                    if from < to {
                        let (left, right) = s.piles.split_at_mut(to);
                        let src = &mut left[from];
                        right[0].extend_from_slice(&src[at..]);
                        src.truncate(at);
                    } else {
                        let (left, right) = s.piles.split_at_mut(from);
                        let src = &mut right[0];
                        left[to].extend_from_slice(&src[at..]);
                        src.truncate(at);
                    }
                }
            }
            Op::Turn {
                from,
                to,
                count,
                up,
            } => {
                for _ in 0..count {
                    let c = s.piles[from as usize].pop().expect("turn from empty pile");
                    s.piles[to as usize].push(if up { c.up() } else { c.down() });
                }
            }
            Op::Flip(p) => {
                let c = s.piles[p as usize].last_mut().expect("flip empty pile");
                *c = c.flipped();
            }
            Op::Redeal => s.redeals_used += 1,
            Op::SetBaseRank(r) => s.base_rank = Some(r),
            Op::Swap { pile, i, j } => s.piles[pile as usize].swap(i as usize, j as usize),
            Op::SwapPiles(a, b) => s.piles.swap(a as usize, b as usize),
        }
    }

    fn undo_op(&self, s: &mut State, op: Op) {
        match op {
            Op::Shift { from, to, count } => self.do_op(
                s,
                Op::Shift {
                    from: to,
                    to: from,
                    count,
                },
            ),
            Op::Turn {
                from,
                to,
                count,
                up,
            } => self.do_op(
                s,
                Op::Turn {
                    from: to,
                    to: from,
                    count,
                    up: !up,
                },
            ),
            Op::Flip(p) => self.do_op(s, Op::Flip(p)),
            Op::Redeal => s.redeals_used -= 1,
            Op::SetBaseRank(_) => s.base_rank = None,
            Op::Swap { .. } => self.do_op(s, op),
            Op::SwapPiles(a, b) => s.piles.swap(a as usize, b as usize),
        }
    }

    fn push(&self, s: &mut State, j: &mut Vec<Op>, op: Op) {
        self.do_op(s, op);
        j.push(op);
    }

    /// Undo journal entries back to `mark`.
    pub(crate) fn undo_to(&self, s: &mut State, j: &mut Vec<Op>, mark: usize) {
        while j.len() > mark {
            let op = j.pop().unwrap();
            self.undo_op(s, op);
        }
    }

    /// Deal `count` cards face up from `from` (a stock pile) onto `to`, preserving exact undo
    /// even when the top of the stock is already face up.
    fn deal_from_stock(&self, s: &mut State, j: &mut Vec<Op>, from: u16, to: u16, mut count: u16) {
        while count > 0
            && s.piles[from as usize]
                .last()
                .is_some_and(|c| c.is_face_up())
        {
            self.push(s, j, Op::Shift { from, to, count: 1 });
            count -= 1;
        }
        if count > 0 {
            self.push(
                s,
                j,
                Op::Turn {
                    from,
                    to,
                    count,
                    up: true,
                },
            );
        }
    }

    /// Deal one card from `st.pile` to `dst`, first diverting any matching `divert_ranks` cards
    /// onto foundations. If `flip` is false, the card is shifted without flipping face up.
    fn deal_one_with_divert(
        &self,
        s: &mut State,
        j: &mut Vec<Op>,
        st: &super::Stock,
        dst: u16,
        flip: bool,
    ) {
        let place = |s: &mut State, j: &mut Vec<Op>| {
            if flip {
                self.deal_from_stock(s, j, st.pile, dst, 1);
            } else {
                self.push(
                    s,
                    j,
                    Op::Shift {
                        from: st.pile,
                        to: dst,
                        count: 1,
                    },
                );
            }
        };
        if st.divert_ranks.is_empty() {
            place(s, j);
            return;
        }
        while let Some(&top) = s.piles[st.pile as usize].last() {
            let c = top.up();
            if st.divert_ranks.contains(&c.rank()) {
                if let Some(fp) = self.find_foundation(s, c) {
                    self.deal_from_stock(s, j, st.pile, fp, 1);
                    continue;
                }
            }
            place(s, j);
            break;
        }
    }

    /// Deal the entire stock across `round_targets` in alternating face-down / face-up rows,
    /// finishing with a face-up row.
    fn deal_all_alternating(&self, s: &mut State, j: &mut Vec<Op>, st: &super::Stock) {
        let targets = st.round_targets(s.redeals_used);
        let n = targets.len();
        if let Some(q) = s.piles[st.pile as usize].len().checked_div(n) {
            let mut flip = (q & 1) != 0;
            while !s.piles[st.pile as usize].is_empty() {
                if s.piles[st.pile as usize].len() <= n {
                    flip = true;
                }
                for &p in targets {
                    if s.piles[st.pile as usize].is_empty() {
                        break;
                    }
                    self.deal_one_with_divert(s, j, st, p, flip);
                }
                flip = !flip;
            }
        }
    }

    /// Gather cards back into an empty stock. Returns `true` if the talon step is complete
    /// (already settled) and should not also deal a round.
    fn redeal_empty_stock(&self, s: &mut State, j: &mut Vec<Op>, st: &super::Stock) -> bool {
        let tg = &self.groups[st.target as usize];
        if st.target_kind == PileKind::Waste {
            let n = s.piles[tg.start as usize].len() as u16;
            self.push(
                s,
                j,
                Op::Turn {
                    from: tg.start,
                    to: st.pile,
                    count: n,
                    up: false,
                },
            );
            // With unlimited redeals the count can't matter, and tracking it would make every
            // pass through the stock a new position.
            if st.redeals != u32::MAX || (s.redeals_used as usize) + 1 < st.count_per_round.len() {
                self.push(s, j, Op::Redeal);
            }
            if !self.simple_settle || st.face_up || self.has_covers {
                self.settle(s, j);
            }
            return true;
        }
        let gather: Vec<u16> = if st.redeal_reverse {
            tg.piles().rev().collect()
        } else {
            tg.piles().collect()
        };
        for p in gather {
            while let Some(&top) = s.piles[p as usize].last() {
                if top.is_face_up() {
                    self.push(
                        s,
                        j,
                        Op::Turn {
                            from: p,
                            to: st.pile,
                            count: 1,
                            up: false,
                        },
                    );
                } else {
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from: p,
                            to: st.pile,
                            count: 1,
                        },
                    );
                }
            }
        }
        if st.redeal_shuffle {
            self.shuffle_pile(s, j, st.pile);
        }
        if st.redeals != u32::MAX
            || (s.redeals_used as usize) + 1 < st.count_per_round.len()
            || (s.redeals_used as usize) + 1 < st.piles_per_round.len()
        {
            self.push(s, j, Op::Redeal);
        }
        if st.redeal_deal_all_alternating {
            self.deal_all_alternating(s, j, st);
            self.settle(s, j);
            return true;
        }
        false
    }

    /// Deal one regular (or final) round from a non-empty stock.
    fn deal_stock_round(&self, s: &mut State, j: &mut Vec<Op>, st: &super::Stock, stock_len: u16) {
        let count = st.deal_count(s.redeals_used);
        let round_targets = st.round_targets(s.redeals_used);
        if let Some(ft) = st
            .final_to
            .filter(|&ft| stock_len <= self.groups[ft as usize].count)
        {
            let fg = &self.groups[ft as usize];
            for p in fg.piles() {
                if s.piles[st.pile as usize].is_empty() {
                    break;
                }
                self.deal_from_stock(s, j, st.pile, p, 1);
            }
            return;
        }
        if let Some(fp) = st
            .final_piles
            .as_ref()
            .filter(|fp| stock_len as usize <= fp.len())
        {
            for &p in fp {
                if s.piles[st.pile as usize].is_empty() {
                    break;
                }
                self.deal_from_stock(s, j, st.pile, p, 1);
            }
            return;
        }
        match st.target_kind {
            PileKind::Waste | PileKind::Foundation => {
                let n = count.min(stock_len);
                self.deal_from_stock(s, j, st.pile, round_targets[0], n);
            }
            _ if st.first_empty => {
                if let Some(p) = round_targets
                    .iter()
                    .copied()
                    .find(|&p| s.piles[p as usize].is_empty())
                {
                    let n = count.min(stock_len);
                    self.deal_from_stock(s, j, st.pile, p, n);
                }
            }
            _ if st.skip_empty => {
                let mut targets: Vec<u16> = round_targets
                    .iter()
                    .copied()
                    .filter(|&p| !s.piles[p as usize].is_empty())
                    .collect();
                if targets.is_empty() {
                    targets.push(round_targets[0]);
                }
                for _ in 0..count {
                    for &p in &targets {
                        if s.piles[st.pile as usize].is_empty() {
                            break;
                        }
                        self.deal_from_stock(s, j, st.pile, p, 1);
                    }
                }
            }
            _ if st.deal_empty_first => {
                let empties: Vec<u16> = round_targets
                    .iter()
                    .copied()
                    .filter(|&p| s.piles[p as usize].is_empty())
                    .collect();
                let targets: &[u16] = if empties.is_empty() {
                    round_targets
                } else {
                    &empties
                };
                for _ in 0..count {
                    for &p in targets {
                        if s.piles[st.pile as usize].is_empty() {
                            break;
                        }
                        self.deal_from_stock(s, j, st.pile, p, 1);
                    }
                }
            }
            _ if st.deal_per_pile => {
                for &p in round_targets {
                    let rem = s.piles[st.pile as usize].len() as u16;
                    if rem == 0 {
                        break;
                    }
                    let n = count.min(rem);
                    self.deal_from_stock(s, j, st.pile, p, n);
                }
            }
            _ => {
                for _ in 0..count {
                    for &p in round_targets {
                        if s.piles[st.pile as usize].is_empty() {
                            break;
                        }
                        self.deal_one_with_divert(s, j, st, p, true);
                    }
                }
            }
        }
    }

    /// One stock action: deal, or turn the waste back. Returns false if neither is possible.
    pub(crate) fn talon_step(&self, s: &mut State, j: &mut Vec<Op>) -> bool {
        if !self.can_deal(s) {
            return false;
        }
        if self.redeal.is_some() {
            self.tableau_redeal(s, j);
            self.settle(s, j);
            return true;
        }
        let st = self.stock.as_ref().expect("can_deal verified stock exists");
        if s.piles[st.pile as usize].is_empty() && self.redeal_empty_stock(s, j, st) {
            return true;
        }
        let stock_len = s.piles[st.pile as usize].len() as u16;
        if stock_len > 0 && !(st.final_split_to.is_some() && stock_len <= 2) {
            self.deal_stock_round(s, j, st, stock_len);
        }
        if let Some(split_dest) = st.final_split_to {
            if s.piles[st.pile as usize].len() == 2 {
                self.deal_from_stock(s, j, st.pile, split_dest, 1);
                if s.piles[st.pile as usize]
                    .last()
                    .is_some_and(|c| !c.is_face_up())
                {
                    self.push(s, j, Op::Flip(st.pile));
                }
            }
        }
        if !self.simple_settle || st.target_kind != PileKind::Waste || st.face_up || self.has_covers
        {
            self.settle(s, j);
        }
        true
    }

    fn redeal_regroup_or_shuffle(
        &self,
        s: &mut State,
        j: &mut Vec<Op>,
        g: &super::Group,
        gp: u16,
        per_pile: u16,
        shuffle: bool,
    ) {
        // Last pile first, so the first pile's cards end on top of the gathered stack.
        for p in g.piles().rev() {
            let n = s.piles[p as usize].len() as u16;
            if n > 0 {
                self.push(
                    s,
                    j,
                    Op::Shift {
                        from: p,
                        to: gp,
                        count: n,
                    },
                );
            }
        }
        if shuffle {
            self.shuffle_pile(s, j, gp);
        }
        // More cards than `per_pile` for every pile (a card came back from a foundation):
        // share the surplus out, an equal part to each pile and one more to the first few.
        let per = per_pile as usize;
        let count = g.count as usize;
        let surplus = s.piles[gp as usize].len().saturating_sub(per * count);
        let share = |i: usize| per + surplus / count + usize::from(i < surplus % count);
        if surplus > 0 {
            let sizes: Vec<usize> = (0..count).map(share).collect();
            self.arrange_surplus(s, j, gp, per, &sizes);
        }
        for (i, p) in g.piles().enumerate() {
            let n = share(i).min(s.piles[gp as usize].len()) as u16;
            if n == 0 {
                break;
            }
            self.push(
                s,
                j,
                Op::Shift {
                    from: gp,
                    to: p,
                    count: n,
                },
            );
        }
    }

    fn redeal_compact(
        &self,
        s: &mut State,
        j: &mut Vec<Op>,
        g: &super::Group,
        refill: Option<u16>,
    ) {
        let mut write = g.start;
        for p in g.piles() {
            let n = s.piles[p as usize].len() as u16;
            if n == 0 {
                continue;
            }
            if write != p {
                self.push(
                    s,
                    j,
                    Op::Shift {
                        from: p,
                        to: write,
                        count: n,
                    },
                );
            }
            write += 1;
        }
        if let Some(src) = refill {
            let src_kind = self.groups[src as usize].kind;
            for p in write..g.start + g.count {
                let from = self.groups[src as usize]
                    .piles()
                    .find(|&q| !s.piles[q as usize].is_empty());
                let Some(from) = from else { break };
                if src_kind == PileKind::Stock {
                    self.deal_from_stock(s, j, from, p, 1);
                } else {
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from,
                            to: p,
                            count: 1,
                        },
                    );
                }
            }
        }
    }

    fn redeal_shuffle_redeal(&self, s: &mut State, j: &mut Vec<Op>, g: &super::Group, gp: u16) {
        for p in g.piles() {
            while let Some(&top) = s.piles[p as usize].last() {
                if top.is_face_up() {
                    self.push(
                        s,
                        j,
                        Op::Turn {
                            from: p,
                            to: gp,
                            count: 1,
                            up: false,
                        },
                    );
                } else {
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from: p,
                            to: gp,
                            count: 1,
                        },
                    );
                }
            }
        }
        self.shuffle_pile(s, j, gp);
        for step in &self.rules.deal {
            if step.to != g.name {
                continue;
            }
            let piles: Vec<u16> = match &step.piles {
                None => g.piles().collect(),
                Some(ix) => ix.iter().map(|&i| g.start + i).collect(),
            };
            let deal_one = |s: &mut State, j: &mut Vec<Op>, dst: u16| {
                if step.face_up {
                    self.push(
                        s,
                        j,
                        Op::Turn {
                            from: gp,
                            to: dst,
                            count: 1,
                            up: true,
                        },
                    );
                } else {
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from: gp,
                            to: dst,
                            count: 1,
                        },
                    );
                }
            };
            if let Some(c) = step.count {
                for _ in 0..c {
                    for &dst in &piles {
                        if s.piles[gp as usize].is_empty() {
                            break;
                        }
                        deal_one(s, j, dst);
                    }
                }
            } else if let Some(cs) = &step.counts {
                let max_c = cs.iter().copied().max().unwrap_or(0);
                for r in 0..max_c {
                    for (&dst, &c) in piles.iter().zip(cs.iter()) {
                        if r < c {
                            if s.piles[gp as usize].is_empty() {
                                break;
                            }
                            deal_one(s, j, dst);
                        }
                    }
                }
            }
        }
    }

    fn redeal_grid_shuffle(
        &self,
        s: &mut State,
        j: &mut Vec<Op>,
        rd: &super::Redeal,
        g: &super::Group,
        gp: u16,
    ) {
        let cols = g.columns.expect("grid_shuffle validated grid layout");
        let prefix: Vec<u16> = (0..g.count / cols)
            .map(|row| self.row_prefix(s, rd.group, row) as u16)
            .collect();
        for (row, &k) in prefix.iter().enumerate() {
            for col in k..cols {
                let p = g.start + row as u16 * cols + col;
                let n = s.piles[p as usize].len() as u16;
                if n > 0 {
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from: p,
                            to: gp,
                            count: n,
                        },
                    );
                }
            }
        }
        self.shuffle_pile(s, j, gp);
        if rd.how == RedealHow::GridShuffle {
            for (row, &k) in prefix.iter().enumerate() {
                for col in k + 1..cols {
                    if s.piles[gp as usize].is_empty() {
                        break;
                    }
                    let p = g.start + row as u16 * cols + col;
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from: gp,
                            to: p,
                            count: 1,
                        },
                    );
                }
            }
        } else {
            let mut slots: Vec<u16> = Vec::new();
            for (row, &k) in prefix.iter().enumerate() {
                for col in k..cols {
                    slots.push(g.start + row as u16 * cols + col);
                }
            }
            let mut rng =
                Rng::new(s.seed ^ (s.redeals_used as u64 + 1).wrapping_mul(0x517c_c1b7_2722_0a95));
            for i in (1..slots.len()).rev() {
                let k = rng.below(i as u64 + 1) as usize;
                slots.swap(i, k);
            }
            let n_cards = s.piles[gp as usize].len();
            let mut chosen = slots[..n_cards.min(slots.len())].to_vec();
            chosen.sort_unstable();
            for p in chosen {
                if s.piles[gp as usize].is_empty() {
                    break;
                }
                self.push(
                    s,
                    j,
                    Op::Shift {
                        from: gp,
                        to: p,
                        count: 1,
                    },
                );
            }
        }
    }

    /// Gather and redeal the tableau (see [`RedealHow`]).
    fn tableau_redeal(&self, s: &mut State, j: &mut Vec<Op>) {
        let rd = self.redeal.as_ref().expect("redeal checked by caller");
        let g = &self.groups[rd.group as usize];
        let gp = rd.gather;
        match rd.how {
            RedealHow::RotateBottom => {
                for p in g.piles() {
                    let n = s.piles[p as usize].len() as u16;
                    if n > 1 {
                        for i in 0..n - 1 {
                            self.push(
                                s,
                                j,
                                Op::Swap {
                                    pile: p,
                                    i,
                                    j: i + 1,
                                },
                            );
                        }
                    }
                }
            }
            RedealHow::Regroup { per_pile } => {
                self.redeal_regroup_or_shuffle(s, j, g, gp, per_pile, false);
            }
            RedealHow::Shuffle { per_pile } => {
                self.redeal_regroup_or_shuffle(s, j, g, gp, per_pile, true);
            }
            RedealHow::Compact { .. } => {
                self.redeal_compact(s, j, g, rd.refill);
            }
            RedealHow::DealRows => {
                for p in g.piles() {
                    let n = s.piles[p as usize].len();
                    for _ in 0..n {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: p,
                                to: gp,
                                count: 1,
                            },
                        );
                    }
                }
                while !s.piles[gp as usize].is_empty() {
                    for p in g.piles() {
                        if s.piles[gp as usize].is_empty() {
                            break;
                        }
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: gp,
                                to: p,
                                count: 1,
                            },
                        );
                    }
                }
            }
            RedealHow::RedistributePile => {
                let count = g.count as usize;
                let k = (s.redeals_used as usize) % count;
                let src = g.start + k as u16;
                let n = s.piles[src as usize].len();
                if n > 0 {
                    for _ in 0..n {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: src,
                                to: gp,
                                count: 1,
                            },
                        );
                    }
                    for i in 0..n {
                        let dst = g.start + ((k + i) % count) as u16;
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: gp,
                                to: dst,
                                count: 1,
                            },
                        );
                    }
                }
            }
            RedealHow::ShuffleRedeal => {
                self.redeal_shuffle_redeal(s, j, g, gp);
            }
            RedealHow::GridShuffle | RedealHow::GridShuffleRandomGaps => {
                self.redeal_grid_shuffle(s, j, rd, g, gp);
            }
        }
        if rd.times != u32::MAX {
            self.push(s, j, Op::Redeal);
        }
    }

    /// Shuffle a pile with the game's play seed, as a series of swaps.
    /// Reorder the gathered cards of a redeal that has more cards than `per_pile` for every pile,
    /// so that dealing `sizes[i]` cards from the top to each pile in turn (as blocks) gives every
    /// pile a run of the cards dealt in order: blocks of `per_pile` taken from the top, each kept
    /// in its own order, then cut into the piles' shares. Dealing `per_pile` to each pile in turn
    /// is already this order, so only a surplus needs it.
    fn arrange_surplus(
        &self,
        s: &mut State,
        j: &mut Vec<Op>,
        pile: u16,
        per_pile: usize,
        sizes: &[usize],
    ) {
        let cards = s.piles[pile as usize].clone();
        let n = cards.len();
        // The order the cards are dealt in.
        let mut dealt: Vec<Card> = Vec::with_capacity(n);
        let mut end = n;
        while end > 0 {
            let start = end.saturating_sub(per_pile);
            dealt.extend_from_slice(&cards[start..end]);
            end = start;
        }
        // The pile, bottom first, that block moves from the top will cut into those shares.
        let mut want: Vec<Card> = Vec::with_capacity(n);
        let mut cuts = Vec::with_capacity(sizes.len());
        let mut at = 0;
        for &size in sizes {
            let size = size.min(n - at);
            cuts.push(&dealt[at..at + size]);
            at += size;
        }
        for chunk in cuts.iter().rev() {
            want.extend_from_slice(chunk);
        }
        let mut have = cards;
        for t in 0..n {
            if have[t] == want[t] {
                continue;
            }
            let k = (t + 1..n)
                .find(|&k| have[k] == want[t])
                .expect("the same cards, rearranged");
            have.swap(t, k);
            self.push(
                s,
                j,
                Op::Swap {
                    pile,
                    i: t as u16,
                    j: k as u16,
                },
            );
        }
    }

    fn shuffle_pile(&self, s: &mut State, j: &mut Vec<Op>, pile: u16) {
        let mut rng =
            Rng::new(s.seed ^ (s.redeals_used as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let n = s.piles[pile as usize].len();
        for i in (1..n).rev() {
            let k = rng.below(i as u64 + 1) as usize;
            if k != i {
                self.push(
                    s,
                    j,
                    Op::Swap {
                        pile,
                        i: i as u16,
                        j: k as u16,
                    },
                );
            }
        }
    }

    /// Automatic follow-ups: turn up exposed cards, refill emptied piles, deal to an empty
    /// waste when the stock deals by itself.
    pub(crate) fn settle(&self, s: &mut State, j: &mut Vec<Op>) {
        self.settle_with(s, j, &[]);
    }

    #[inline]
    fn should_auto_flip(&self, s: &State, p: u16, check_available: bool) -> bool {
        let g = self.group_of(p);
        s.piles[p as usize].last().is_some_and(|c| !c.is_face_up())
            && !g.no_flip
            && (g.kind != PileKind::Stock || self.stock.as_ref().is_some_and(|st| st.face_up))
            && (!g.flip_only_last || s.piles[p as usize].len() == 1)
            && (!check_available || self.available(s, p))
    }

    fn settle_compact(&self, s: &mut State, j: &mut Vec<Op>) -> bool {
        let mut changed = false;
        for g in self.groups.iter().filter(|g| g.compact) {
            let mut write = g.start;
            for p in g.piles() {
                let n = s.piles[p as usize].len() as u16;
                if n == 0 {
                    continue;
                }
                if write != p {
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from: p,
                            to: write,
                            count: n,
                        },
                    );
                    changed = true;
                }
                write += 1;
            }
        }
        for g in self.groups.iter().filter(|g| g.compact_face_up) {
            for p in g.start..g.start + g.count.saturating_sub(1) {
                if s.piles[p as usize].is_empty() && !s.piles[p as usize + 1].is_empty() {
                    let n = self.movable(s, p + 1) as u16;
                    if n > 0 {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: p + 1,
                                to: p,
                                count: n,
                            },
                        );
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// The first group in `g.refill_from` with a card to give; within it, the pile chosen by
    /// `refill_same_index`, `refill_nearest`, `refill_largest`, or lowest index.
    fn find_refill_source(&self, s: &State, g: &super::Group, dst: u16) -> Option<u16> {
        g.refill_from.iter().find_map(|&rg| {
            let rg = &self.groups[rg as usize];
            let eligible = |q: u16| {
                s.piles[q as usize].last().is_some_and(|c| {
                    c.is_face_up() || rg.kind == PileKind::Stock || rg.flip_only_last || rg.no_flip
                })
            };
            if g.refill_same_index {
                let idx = dst - g.start;
                if idx < rg.count {
                    let q = rg.start + idx;
                    eligible(q).then_some(q)
                } else {
                    None
                }
            } else if g.refill_nearest {
                if rg.count == 0 {
                    return None;
                }
                let si = (dst - g.start) % rg.count;
                let q0 = rg.start + si;
                if eligible(q0) {
                    return Some(q0);
                }
                for d in 1..rg.count {
                    if si + d < rg.count {
                        let q = rg.start + si + d;
                        if eligible(q) {
                            return Some(q);
                        }
                    }
                    if si >= d {
                        let q = rg.start + si - d;
                        if eligible(q) {
                            return Some(q);
                        }
                    }
                }
                None
            } else if g.refill_largest {
                let mut best: Option<(u16, usize)> = None;
                for q in rg.piles() {
                    if eligible(q) {
                        let len = s.piles[q as usize].len();
                        if best.is_none_or(|(_, bl)| len > bl) {
                            best = Some((q, len));
                        }
                    }
                }
                best.map(|(q, _)| q)
            } else {
                rg.piles().find(|&q| eligible(q))
            }
        })
    }

    fn refill_pile(
        &self,
        s: &mut State,
        j: &mut Vec<Op>,
        g: &super::Group,
        dst: u16,
        next_emptied: &mut Vec<u16>,
    ) -> bool {
        let mut changed = false;
        let mut moved = 0u16;
        while moved < g.refill_count {
            let Some(q) = self.find_refill_source(s, g, dst) else {
                break;
            };
            let need = g.refill_count - moved;
            if self.group_of(q).kind == PileKind::Stock {
                let stock_gi = self.pile_group[q as usize];
                let via_waste = self.stock.as_ref().and_then(|st| {
                    let waste_before_stock = g
                        .refill_from
                        .iter()
                        .position(|&x| x == st.target)
                        .zip(g.refill_from.iter().position(|&x| x == stock_gi))
                        .is_some_and(|(wi, si)| wi < si);
                    if st.pile == q && st.target_kind == PileKind::Waste && waste_before_stock {
                        let c = st.deal_count(s.redeals_used);
                        (c > 1).then_some((self.groups[st.target as usize].start, c))
                    } else {
                        None
                    }
                });
                if let Some((waste, c)) = via_waste {
                    let n = c.min(s.piles[q as usize].len() as u16);
                    self.deal_from_stock(s, j, q, waste, n);
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from: waste,
                            to: dst,
                            count: 1,
                        },
                    );
                    moved += 1;
                } else if g.refill_first_face_down && moved == 0 {
                    if s.piles[q as usize].len() > 1 {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: q,
                                to: dst,
                                count: 1,
                            },
                        );
                    } else {
                        self.deal_from_stock(s, j, q, dst, 1);
                    }
                    moved += 1;
                } else {
                    let n = need.min(s.piles[q as usize].len() as u16);
                    self.deal_from_stock(s, j, q, dst, n);
                    moved += n;
                }
            } else {
                let rg = self.group_of(q);
                let mut sub = 0u16;
                while moved < g.refill_count {
                    let Some(&top) = s.piles[q as usize].last() else {
                        break;
                    };
                    if top.is_face_up() {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: q,
                                to: dst,
                                count: 1,
                            },
                        );
                    } else if rg.flip_only_last || rg.no_flip {
                        self.push(
                            s,
                            j,
                            Op::Turn {
                                from: q,
                                to: dst,
                                count: 1,
                                up: true,
                            },
                        );
                    } else {
                        break;
                    }
                    moved += 1;
                    sub += 1;
                }
                if s.piles[q as usize].is_empty() {
                    next_emptied.push(q);
                }
                if sub == 0 {
                    break;
                }
            }
            changed = true;
        }
        changed
    }

    fn settle_refills(
        &self,
        s: &mut State,
        j: &mut Vec<Op>,
        emptied: &[u16],
        next_emptied: &mut Vec<u16>,
    ) -> bool {
        let mut changed = false;
        for g in &self.groups {
            if g.refill_from.is_empty() {
                continue;
            }
            for p in g.piles() {
                if !s.piles[p as usize].is_empty()
                    || !self.available(s, p)
                    || (g.refill_on_empty && !emptied.contains(&p))
                {
                    continue;
                }
                if self.refill_pile(s, j, g, p, next_emptied) {
                    changed = true;
                }
            }
        }
        changed
    }

    pub(crate) fn settle_with(&self, s: &mut State, j: &mut Vec<Op>, initial_emptied: &[u16]) {
        if self.simple_settle {
            if !self.rules.auto_flip {
                return;
            }
            if !self.has_covers && !initial_emptied.is_empty() {
                for &p in initial_emptied {
                    if self.should_auto_flip(s, p, false) {
                        self.push(s, j, Op::Flip(p));
                    }
                }
                return;
            }
            for p in 0..s.piles.len() as u16 {
                if self.should_auto_flip(s, p, true) {
                    self.push(s, j, Op::Flip(p));
                }
            }
            return;
        }
        let mut emptied: Vec<u16> = initial_emptied.to_vec();
        let max_iters = s.piles.len().saturating_mul(16).max(256);
        for _ in 0..max_iters {
            let mut changed = self.settle_compact(s, j);
            let mut next_emptied: Vec<u16> = Vec::new();
            if self.settle_refills(s, j, &emptied, &mut next_emptied) {
                changed = true;
            }
            if !(self.choose_base.is_some() && s.base_rank.is_none()) {
                if let Some(st) = self.stock.as_ref().filter(|st| st.auto) {
                    let target = self.groups[st.target as usize].start;
                    if st.target_kind == PileKind::Waste
                        && s.piles[target as usize].is_empty()
                        && !s.piles[st.pile as usize].is_empty()
                    {
                        let n = st
                            .deal_count(s.redeals_used)
                            .min(s.piles[st.pile as usize].len() as u16);
                        self.deal_from_stock(s, j, st.pile, target, n);
                        changed = true;
                    }
                }
            }
            if self.rules.auto_flip {
                for p in 0..s.piles.len() as u16 {
                    if self.should_auto_flip(s, p, true) {
                        self.push(s, j, Op::Flip(p));
                        changed = true;
                    }
                }
            }
            if let Some(h) = &self.hooks {
                for (from, to, count) in h.auto_moves(self, s) {
                    if count > 0 && s.piles[from as usize].len() >= count as usize {
                        self.push(s, j, Op::Shift { from, to, count });
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
            emptied = next_emptied;
        }
    }

    /// Play a move, recording how to undo it in `j`. The move must be legal.
    pub(crate) fn apply(&self, s: &mut State, mv: Move, j: &mut Vec<Op>) {
        for _ in 0..mv.talon {
            let ok = self.talon_step(s, j);
            debug_assert!(ok, "talon step not possible");
        }
        match mv.action {
            Action::Transfer { from, to, count } => {
                if self.can_swap(s, from, to) {
                    self.push(s, j, Op::SwapPiles(from, to));
                    self.settle_with(s, j, &[]);
                } else {
                    if let Some(discard) = self.group_of(to).replace_on_drop {
                        let existing = s.piles[to as usize].len() as u16;
                        if existing > 0 {
                            self.push(
                                s,
                                j,
                                Op::Shift {
                                    from: to,
                                    to: discard,
                                    count: existing,
                                },
                            );
                        }
                    }
                    self.push(s, j, Op::Shift { from, to, count });
                    if let Some(cb) = &self.choose_base {
                        if s.base_rank.is_none() {
                            let r = s.piles[to as usize].last().unwrap().rank();
                            self.push(s, j, Op::SetBaseRank(r));
                            if cb.refill_from_stock {
                                if let Some(sp) = self.stock_pile {
                                    for p in self.groups[cb.from_group as usize].piles() {
                                        if s.piles[p as usize].is_empty()
                                            && !s.piles[sp as usize].is_empty()
                                        {
                                            self.deal_from_stock(s, j, sp, p, 1);
                                        }
                                    }
                                }
                            }
                            if cb.deal_waste {
                                if let Some(st) = &self.stock {
                                    if st.target_kind == PileKind::Waste
                                        && !s.piles[st.pile as usize].is_empty()
                                    {
                                        let dest = self.groups[st.target as usize].start;
                                        self.deal_from_stock(s, j, st.pile, dest, 1);
                                    }
                                }
                            }
                        }
                    }
                    self.settle_with(s, j, &[from]);
                }
            }
            Action::Deal => {
                let ok = self.talon_step(s, j);
                debug_assert!(ok);
            }
            Action::Remove { rule, n, piles } => {
                let to = self.removal[rule as usize].to;
                for &from in &piles[..n as usize] {
                    self.push(s, j, Op::Shift { from, to, count: 1 });
                }
                self.settle_with(s, j, &piles[..n as usize]);
            }
        }
    }

    /// Play a move on a state. Returns an error if it isn't legal.
    pub fn play(&self, s: &mut State, mv: Move) -> Result<(), IllegalMove> {
        if !self.is_legal(s, mv) {
            return Err(IllegalMove(mv));
        }
        let mut j = Vec::new();
        self.apply(s, mv, &mut j);
        Ok(())
    }

    /// Has the game been won?
    pub fn is_won(&self, s: &State) -> bool {
        if let Some(b) = self.hooks.as_ref().and_then(|h| h.is_won(self, s)) {
            return b;
        }
        match &self.win {
            Win::AllOnFoundations => self.cards_home(s) == self.cards_to_home(),
            Win::Empty(gs) => gs.iter().all(|&g| {
                self.groups[g as usize]
                    .piles()
                    .all(|p| s.piles[p as usize].is_empty())
            }),
            Win::AtMostPiles(gi, n) => {
                self.groups[*gi as usize]
                    .piles()
                    .filter(|&p| !s.piles[p as usize].is_empty())
                    .count()
                    <= *n as usize
                    && self
                        .stock
                        .as_ref()
                        .is_none_or(|st| s.piles[st.pile as usize].is_empty())
            }
            Win::Rows(gi, len, first) => {
                let g = &self.groups[*gi as usize];
                let cols = g.columns.unwrap();
                let _ = first;
                (0..g.count / cols).all(|row| self.row_prefix(s, *gi, row) >= *len as usize)
            }
            Win::Score(n) => self.score(s).is_some_and(|v| v >= *n),
            Win::AllFull(gi) => self.groups[*gi as usize].piles().all(|p| {
                let pile = &s.piles[p as usize];
                pile.len() == self.rule(p).max_cards as usize
                    && pile.first().is_some_and(|&c| self.empty_accepts(p, c, s))
            }),
            Win::Count(g, n) => {
                self.groups[*g as usize]
                    .piles()
                    .map(|p| s.piles[p as usize].len())
                    .sum::<usize>()
                    >= *n as usize
            }
            Win::CountAndSpread(g, n, spread) => {
                let f_cards: usize = self.groups[*g as usize]
                    .piles()
                    .map(|p| s.piles[p as usize].len())
                    .sum();
                let s_cards: usize = self.groups[*spread as usize]
                    .piles()
                    .map(|p| s.piles[p as usize].len())
                    .sum();
                let total: usize = s.piles.iter().map(|p| p.len()).sum();
                f_cards >= *n as usize
                    && f_cards + s_cards == total
                    && self.groups[*spread as usize]
                        .piles()
                        .all(|p| s.piles[p as usize].len() <= 1)
            }
            Win::PilesBuilt {
                group,
                length,
                has_build,
                empty,
                non_empty,
                pile,
            } => {
                if !empty.iter().all(|&g| {
                    self.groups[g as usize]
                        .piles()
                        .all(|p| s.piles[p as usize].is_empty())
                }) {
                    return false;
                }
                let check_pile = |cards: &[Card]| -> bool {
                    if let Some(l) = length {
                        if cards.len() != *l as usize {
                            return false;
                        }
                    }
                    if *has_build {
                        let fits = self.row_win_fits.as_ref().unwrap();
                        if cards.iter().any(|c| !c.is_face_up()) {
                            return false;
                        }
                        if !cards
                            .windows(2)
                            .all(|w| fits[w[0].face() as usize] >> w[1].face() & 1 != 0)
                        {
                            return false;
                        }
                    }
                    true
                };
                if let Some(pi) = pile {
                    let p = (self.groups[*group as usize].start + *pi) as usize;
                    let cards = &s.piles[p];
                    if (*non_empty || length.is_some()) && cards.is_empty() {
                        return false;
                    }
                    return check_pile(cards);
                }
                let mut any = false;
                for p in self.groups[*group as usize].piles() {
                    let cards = &s.piles[p as usize];
                    if cards.is_empty() {
                        continue;
                    }
                    any = true;
                    if !check_pile(cards) {
                        return false;
                    }
                }
                !*non_empty || any
            }
        }
    }

    /// Cards on foundations, jokers left out.
    pub(crate) fn cards_home(&self, s: &State) -> usize {
        let piles = self
            .groups
            .iter()
            .filter(|g| g.kind == PileKind::Foundation)
            .flat_map(|g| g.piles());
        if self.jokers == 0 {
            piles.map(|p| s.piles[p as usize].len()).sum()
        } else {
            piles
                .flat_map(|p| &s.piles[p as usize])
                .filter(|c| !c.is_joker())
                .count()
        }
    }

    /// Cards that must reach the foundations to win "all on foundations": every card but the
    /// jokers, which are helpers and may end anywhere.
    pub(crate) fn cards_to_home(&self) -> usize {
        self.total_cards as usize - self.jokers as usize
    }

    /// How many piles from the start of a grid row hold a built sequence (one card each,
    /// starting with the first-column rank).
    pub(crate) fn row_prefix(&self, s: &State, gi: u16, row: u16) -> usize {
        let g = &self.groups[gi as usize];
        let cols = g.columns.unwrap_or(g.count);
        let base = g.start + row * cols;
        let mut n = 0;
        // What the previous card could be (a joker: anything that keeps the row built).
        let mut prev: Option<u64> = None;
        for p in base..base + cols {
            let pile = &s.piles[p as usize];
            let [c] = pile.as_slice() else { break };
            if !c.is_face_up() {
                break;
            }
            let r = self.rule(p);
            let allowed = match prev {
                None => match r.empty {
                    EmptyRule::AfterLeft { first } | EmptyRule::AfterNeighbor { first } => {
                        self.rank_faces[first as usize]
                    }
                    _ => 0,
                },
                Some(set) => match &self.row_win_fits {
                    Some(fits) => {
                        let mut out = 0u64;
                        let mut m = set;
                        while m != 0 {
                            let f = m.trailing_zeros() as usize;
                            m &= m - 1;
                            out |= fits[f];
                        }
                        out
                    }
                    None => self.step_set(r, set, false),
                },
            };
            let id = if c.is_joker() {
                if !r.wild || allowed == 0 {
                    break;
                }
                allowed
            } else {
                if allowed >> c.face() & 1 == 0 {
                    break;
                }
                1 << c.face()
            };
            prev = Some(id);
            n += 1;
        }
        n
    }
}
