//! Scoring positions, for games played for points (Poker Squares, Cribbage Squares).
//!
//! A score rule names a grid group; each row and each column is a hand. The solver needs two
//! things from it: the exact score of a position, and an upper bound on the score of any
//! position reachable from it. The bound is what lets a search discard a line of play without
//! exploring it, so it must never be too low.
//!
//! Jokers are wild: in each hand, a joker counts as whichever card scores that hand best.

use std::sync::Arc;

use crate::card::{Card, FACES};
use crate::game::{Game, State};
use crate::rules::{CribbagePoints, PokerPoints};

/// Poker hands in the usual ranking order.
const PAIR: usize = 1;
const TWO_PAIR: usize = 2;
const THREE: usize = 3;
const STRAIGHT: usize = 4;
const FLUSH: usize = 5;
const FULL_HOUSE: usize = 6;
const FOUR: usize = 7;
const STRAIGHT_FLUSH: usize = 8;
const ROYAL_FLUSH: usize = 9;

/// Where the cards still to enter the scored group can come from.
#[derive(Clone, Debug)]
pub(crate) enum Pool {
    /// Cards leave the scored group, so nothing in it is settled: no bound.
    Unbounded,
    /// Any card outside the group may still arrive.
    Outside,
    /// Cards arrive only from the waste, one at a time, and the waste is fed only from the
    /// stock when empty: the next cards are known exactly (the waste card, then the stock from
    /// the top).
    NextFromStock { stock: u16, waste: u16 },
}

/// What the cribbage starter will be, as far as the bound can tell.
#[derive(Clone, Debug)]
pub(crate) enum StarterBound {
    /// No starter, ever.
    None,
    /// The top card of a pile nothing enters or leaves.
    Fixed(u16),
    /// The waste card after the grid is filled from the waste ([`Pool::NextFromStock`]).
    NextInSequence,
    /// Any card outside the grid.
    Unknown,
}

/// How a line of cards scores.
#[derive(Clone, Debug)]
pub(crate) enum HandScoring {
    Poker {
        /// Points by hand, indexed by the constants above (0: nothing).
        points: [u32; 10],
        /// Hands, most points first.
        order: [usize; 10],
        /// Points never fall as hands rise, so the highest hand a joker can make scores best.
        monotone: bool,
    },
    Cribbage {
        points: CribbagePoints,
        /// The pile whose top card is the starter.
        starter: Option<u16>,
        starter_bound: StarterBound,
        /// Best fifteens, pairs and runs reachable from each rank multiset (see [`rank_key`]).
        best: Arc<Vec<u32>>,
    },
}

/// A compiled score rule.
#[derive(Clone, Debug)]
pub(crate) struct Score {
    pub group: u16,
    /// Piles of each line (rows, then columns).
    pub lines: Vec<Vec<u16>>,
    pub hands: HandScoring,
    pub pool: Pool,
    /// Rows are interchangeable, and so are columns: permuting either gives an equally good
    /// position. Then a card going into an empty row only needs trying in the first empty row
    /// (likewise columns).
    pub symmetric: bool,
    /// Grid size.
    pub rows: u16,
    pub columns: u16,
}

/// Hands ordered by their points, most first.
pub(crate) fn points_order(points: &[u32; 10]) -> [usize; 10] {
    let mut o = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    o.sort_by_key(|&h| std::cmp::Reverse(points[h]));
    o
}

pub(crate) fn points_table(p: &PokerPoints) -> [u32; 10] {
    [
        0,
        p.pair,
        p.two_pair,
        p.three,
        p.straight,
        p.flush,
        p.full_house,
        p.four,
        p.straight_flush,
        p.royal_flush,
    ]
}

/// The ten straights, as rank bit sets (bit r for rank r; the ace is bit 1 and bit 14).
const STRAIGHTS: [u16; 10] = {
    let mut w = [0u16; 10];
    let mut lo = 1;
    while lo <= 10 {
        w[lo - 1] = 0b11111 << lo;
        lo += 1;
    }
    w
};
const ROYAL: u16 = 0b11111 << 10;

/// A rank's bits: the ace is low and high.
fn rank_bit(r: u8) -> u16 {
    if r == 1 {
        1 << 1 | 1 << 14
    } else {
        1 << r
    }
}

/// Card counts for one line (or a pool of cards). Jokers are only counted.
#[derive(Clone)]
struct Counts {
    face: [u16; FACES],
    rank: [u16; 14],
    suit: [u16; 4],
    total: u32,
    jokers: u32,
    /// Ranks present, overall and per suit (see [`rank_bit`]).
    ranks: u16,
    suit_ranks: [u16; 4],
}

impl Default for Counts {
    fn default() -> Self {
        Counts {
            face: [0; FACES],
            rank: [0; 14],
            suit: [0; 4],
            total: 0,
            jokers: 0,
            ranks: 0,
            suit_ranks: [0; 4],
        }
    }
}

impl Counts {
    fn add(&mut self, c: Card) {
        self.total += 1;
        if c.is_joker() {
            self.jokers += 1;
            return;
        }
        self.face[c.face() as usize] += 1;
        self.rank[c.rank() as usize] += 1;
        self.suit[c.suit() as usize] += 1;
        self.ranks |= rank_bit(c.rank());
        self.suit_ranks[c.suit() as usize] |= rank_bit(c.rank());
    }

    /// Real cards.
    fn real(&self) -> u32 {
        self.total - self.jokers
    }
}

/// The best (highest-ranking) poker hand among these cards, each joker standing for any card.
fn hand(c: &Counts) -> usize {
    // With nothing to draw but the jokers, which can be anything, possible means present.
    15 - (possible(c, &Counts::default(), c.jokers, c.jokers) | 1).leading_zeros() as usize
}

/// Points for a poker line. Each joker stands for the card that scores best: with points that
/// follow the hands' ranking that's the highest hand it can make, otherwise every card is tried.
fn poker_points(c: &Counts, points: &[u32; 10], monotone: bool) -> u32 {
    if c.jokers == 0 || monotone {
        return points[hand(c)];
    }
    let mut most = 0;
    for f in 0..52u8 {
        let mut d = c.clone();
        d.jokers -= 1;
        d.total -= 1;
        d.add(Card::from_face(f));
        most = most.max(poker_points(&d, points, monotone));
    }
    most
}

/// Which poker hands (as a bit set) a line holding `have` (real cards) could make with up to
/// `slots` more cards drawn from `pool`, of which up to `wild` may be any card at all. Bit 0,
/// no hand, is always set. Possible is relaxed: each hand is tested alone, as if every other
/// card could be anything, so it can say yes when the truth is no, never the reverse. With an
/// empty pool and `wild == slots` filling the line, it is exact.
#[allow(clippy::needless_range_loop)]
fn possible(have: &Counts, pool: &Counts, slots: u32, wild: u32) -> u16 {
    let mut set = 1u16;
    // Cards needed from the pool to hold `n` of rank r.
    let need_rank = |r: usize, n: u16| -> Option<u32> {
        let k = n.saturating_sub(have.rank[r]) as u32;
        (pool.rank[r] as u32 + wild >= k && k <= slots).then_some(k)
    };
    // Cheapest cost to reach 2, 3 and 4 of a rank, and the second cheapest pair (other rank).
    let (mut pair, mut pair2, mut pair_rank) = (u32::MAX, u32::MAX, 0usize);
    let (mut three, mut four) = (false, false);
    let mut full = false;
    let mut k3 = [u32::MAX; 14];
    for r in 1..14 {
        if let Some(k) = need_rank(r, 2) {
            if k < pair {
                pair2 = pair;
                pair = k;
                pair_rank = r;
            } else if k < pair2 {
                pair2 = k;
            }
        }
        if let Some(k) = need_rank(r, 3) {
            three = true;
            k3[r] = k;
        }
        four |= need_rank(r, 4).is_some();
    }
    if pair != u32::MAX {
        set |= 1 << PAIR;
    }
    if pair2 != u32::MAX && pair + pair2 <= slots {
        set |= 1 << TWO_PAIR;
    }
    if three {
        set |= 1 << THREE;
        for (r, &k) in k3.iter().enumerate() {
            let other = if r == pair_rank { pair2 } else { pair };
            if k != u32::MAX && other != u32::MAX && k + other <= slots {
                full = true;
                break;
            }
        }
    }
    if full {
        set |= 1 << FULL_HOUSE;
    }
    if four {
        set |= 1 << FOUR;
    }
    if (0..4).any(|s| {
        let k = 5u32.saturating_sub(have.suit[s] as u32);
        pool.suit[s] as u32 + wild >= k && k <= slots
    }) {
        set |= 1 << FLUSH;
    }
    let reachable = |missing: u16, from: u16| {
        missing.count_ones() <= slots && (missing & !from).count_ones() <= wild
    };
    for w in STRAIGHTS {
        if reachable(w & !have.ranks, pool.ranks) {
            set |= 1 << STRAIGHT;
            // A straight flush needs the same ranks in one suit.
            for s in 0..4 {
                if reachable(w & !have.suit_ranks[s], pool.suit_ranks[s]) {
                    set |= 1
                        << if w == ROYAL {
                            ROYAL_FLUSH
                        } else {
                            STRAIGHT_FLUSH
                        };
                }
            }
        }
    }
    set
}

// ----- Cribbage ---------------------------------------------------------------------------

/// Points from fifteens, pairs and runs: the part of a cribbage hand that depends only on
/// ranks. `count[r]` cards of rank r (1..=13).
fn rank_points(p: &CribbagePoints, count: &[u8; 14]) -> u32 {
    let mut values = [0u8; 16];
    let mut n = 0;
    for r in 1..14u8 {
        for _ in 0..count[r as usize] {
            if n < values.len() {
                values[n] = r.min(10);
                n += 1;
            }
        }
    }
    let mut fifteens = 0u32;
    if p.fifteen > 0 {
        // Subset sums by dynamic programming: ways[t] = subsets so far adding to t.
        let mut ways = [0u32; 16];
        ways[0] = 1;
        for &v in &values[..n] {
            for t in (v as usize..16).rev() {
                ways[t] += ways[t - v as usize];
            }
        }
        fifteens = ways[15];
    }
    let pairs: u32 = count
        .iter()
        .map(|&c| c as u32 * (c as u32).saturating_sub(1) / 2)
        .sum();
    let mut runs = 0u32;
    let mut r = 1;
    while r < 14 {
        if count[r] == 0 {
            r += 1;
            continue;
        }
        let start = r;
        let mut ways = 1u32;
        while r < 14 && count[r] > 0 {
            ways *= count[r] as u32;
            r += 1;
        }
        let len = (r - start) as u32;
        if len >= 3 {
            runs += len * ways;
        }
    }
    fifteens
        .saturating_mul(p.fifteen)
        .saturating_add(pairs.saturating_mul(p.pair))
        .saturating_add(runs.saturating_mul(p.run_card))
}

/// A cribbage hand's points: the line's cards plus the starter, jokers standing for whatever
/// scores best. A flush needs `full` (the line complete) and four or more cards in the line.
fn cribbage_hand(p: &CribbagePoints, line: &[Card], starter: Option<Card>, full: bool) -> u32 {
    if let Some(i) = line.iter().position(|c| c.is_joker()) {
        let mut v = line.to_vec();
        return (0..52u8)
            .map(|f| {
                v[i] = Card::from_face(f);
                cribbage_hand(p, &v, starter, full)
            })
            .max()
            .unwrap_or(0);
    }
    if starter.is_some_and(|c| c.is_joker()) {
        return (0..52u8)
            .map(|f| cribbage_hand(p, line, Some(Card::from_face(f)), full))
            .max()
            .unwrap_or(0);
    }
    let mut count = [0u8; 14];
    for c in line.iter().chain(starter.iter()) {
        count[c.rank() as usize] += 1;
    }
    let mut pts = rank_points(p, &count);
    if full && line.len() >= 4 && line.iter().all(|c| c.suit() == line[0].suit()) {
        pts = pts.saturating_add(p.flush);
        if starter.is_some_and(|s| s.suit() == line[0].suit()) {
            pts = pts.saturating_add(p.flush_starter);
        }
    }
    if let Some(st) = starter {
        if line.iter().any(|c| c.rank() == 11 && c.suit() == st.suit()) {
            pts = pts.saturating_add(p.nobs);
        }
    }
    pts
}

/// Cards in the largest hand [`best_rank_points`] covers: a four-card line and the starter.
const MAX_HAND: usize = 5;

/// A key for a multiset of up to [`MAX_HAND`] ranks: its ranks in ascending order, as base-14
/// digits.
fn rank_key(count: &[u8; 14]) -> usize {
    let mut k = 0usize;
    for (r, &n) in count.iter().enumerate().skip(1) {
        for _ in 0..n {
            k = k * 14 + r;
        }
    }
    k
}

/// For every multiset of up to five ranks, the most rank points (fifteens, pairs, runs) of any
/// five-card multiset containing it with at most `cap` cards of a rank. Adding a card never
/// loses rank points, so this bounds every such hand of five or fewer cards that contains the
/// multiset.
pub(crate) fn best_rank_points(p: &CribbagePoints, cap: u8) -> Vec<u32> {
    let mut best = vec![0u32; 14usize.pow(MAX_HAND as u32)];
    // Multisets by size, largest first, so each extension is known before it's needed.
    fn each(size: usize, min: usize, count: &mut [u8; 14], f: &mut dyn FnMut(&[u8; 14])) {
        if size == 0 {
            f(count);
            return;
        }
        for r in min..14 {
            count[r] += 1;
            each(size - 1, r, count, f);
            count[r] -= 1;
        }
    }
    let mut count = [0u8; 14];
    for size in (0..=MAX_HAND).rev() {
        each(size, 1, &mut count, &mut |c| {
            let v = if size == MAX_HAND {
                rank_points(p, c)
            } else {
                let mut c2 = *c;
                (1..14)
                    .filter(|&r| c[r] < cap)
                    .map(|r| {
                        c2[r] += 1;
                        let v = best[rank_key(&c2)];
                        c2[r] -= 1;
                        v
                    })
                    .max()
                    .unwrap_or(0)
            };
            best[rank_key(c)] = v;
        });
    }
    best
}

impl Score {
    fn line_counts(&self, s: &State, line: &[u16]) -> Counts {
        let mut c = Counts::default();
        for &p in line {
            for &card in &s.piles[p as usize] {
                c.add(card);
            }
        }
        c
    }

    fn grid_full(&self, s: &State) -> bool {
        self.lines[..self.rows as usize]
            .iter()
            .flatten()
            .all(|&p| !s.piles[p as usize].is_empty())
    }

    /// Points for a position.
    pub fn score(&self, s: &State) -> u32 {
        match &self.hands {
            HandScoring::Poker {
                points, monotone, ..
            } => self.lines.iter().fold(0u32, |acc, l| {
                acc.saturating_add(poker_points(&self.line_counts(s, l), points, *monotone))
            }),
            HandScoring::Cribbage {
                points, starter, ..
            } => {
                let starter = starter
                    .filter(|_| self.grid_full(s))
                    .and_then(|p| s.piles[p as usize].last())
                    .map(|c| c.up());
                let mut total = 0u32;
                let mut cards = Vec::new();
                for line in &self.lines {
                    cards.clear();
                    let mut full = true;
                    for &p in line {
                        let pile = &s.piles[p as usize];
                        full &= !pile.is_empty();
                        cards.extend(pile.iter().map(|c| c.up()));
                    }
                    total = total.saturating_add(cribbage_hand(points, &cards, starter, full));
                }
                if starter.is_some_and(|c| c.is_joker() || c.rank() == 11) {
                    total = total.saturating_add(points.heels);
                }
                total
            }
        }
    }

    /// An upper bound on the score of any position reachable from `s`.
    pub fn bound(&self, g: &Game, s: &State) -> u32 {
        let outside = || {
            let mut c = Counts::default();
            for (p, pile) in s.piles.iter().enumerate() {
                if g.pile_group[p] != self.group {
                    for &card in pile {
                        c.add(card);
                    }
                }
            }
            c
        };
        // The cards that may still arrive, and the card after them in sequence.
        let (pool, total, next) = match &self.pool {
            Pool::Unbounded => return u32::MAX,
            Pool::Outside => {
                let c = outside();
                let t = c.total;
                (c, t, Counts::default())
            }
            Pool::NextFromStock { stock, waste } => {
                let free: u32 = g.groups[self.group as usize]
                    .piles()
                    .map(|p| self.room(g, s, p))
                    .sum();
                let mut c = Counts::default();
                let mut next = Counts::default();
                let mut left = free;
                let sequence = s.piles[*waste as usize]
                    .iter()
                    .rev()
                    .chain(s.piles[*stock as usize].iter().rev());
                for &card in sequence {
                    if left == 0 {
                        next.add(card.up());
                        break;
                    }
                    c.add(card.up());
                    left -= 1;
                }
                let t = c.total;
                (c, t, next)
            }
        };
        match &self.hands {
            HandScoring::Poker { points, order, .. } => {
                let mut sum = 0u32;
                for line in &self.lines {
                    let have = self.line_counts(s, line);
                    let slots = self.line_room(g, s, line).min(total);
                    // Held jokers are free choices that use no slot; arriving ones, at most
                    // one per slot.
                    let held = have.jokers;
                    let set = possible(&have, &pool, slots + held, held + pool.jokers.min(slots));
                    let best = order
                        .iter()
                        .copied()
                        .find(|&h| set >> h & 1 != 0)
                        .map_or(0, |h| points[h]);
                    sum = sum.saturating_add(best);
                }
                sum
            }
            HandScoring::Cribbage {
                points,
                starter_bound,
                best,
                ..
            } => {
                let starter = match starter_bound {
                    StarterBound::None => Counts::default(),
                    StarterBound::Fixed(p) => {
                        let mut c = Counts::default();
                        if let Some(&card) = s.piles[*p as usize].last() {
                            c.add(card.up());
                        }
                        c
                    }
                    StarterBound::NextInSequence => next,
                    StarterBound::Unknown => outside(),
                };
                let mut sum = 0u32;
                for line in &self.lines {
                    let have = self.line_counts(s, line);
                    let slots = self.line_room(g, s, line).min(total);
                    sum = sum.saturating_add(self.cribbage_line_bound(
                        points,
                        best,
                        &have,
                        &pool,
                        slots,
                        &starter,
                        line.len(),
                    ));
                }
                if points.heels > 0 && (starter.jokers > 0 || starter.rank[11] > 0) {
                    sum = sum.saturating_add(points.heels);
                }
                sum
            }
        }
    }

    /// The most a cribbage line holding `have` could score, with up to `slots` more cards from
    /// `pool` and a starter from `starter` (no starter if it's empty). Fifteens, pairs and runs
    /// are bounded together; flush and nobs separately.
    #[allow(clippy::too_many_arguments)]
    fn cribbage_line_bound(
        &self,
        p: &CribbagePoints,
        best: &[u32],
        have: &Counts,
        pool: &Counts,
        slots: u32,
        starter: &Counts,
        line_len: usize,
    ) -> u32 {
        let has_starter = starter.total > 0;
        // A starter of one known real card is fixed; otherwise it's one more free choice.
        let known = (starter.total == 1 && starter.jokers == 0)
            .then(|| (1..14).find(|&r| starter.rank[r] > 0).unwrap());
        let fixed_n: usize =
            have.rank.iter().map(|&c| c as usize).sum::<usize>() + usize::from(known.is_some());
        let free = (have.jokers + slots) as usize + usize::from(has_starter && known.is_none());
        if fixed_n + free > MAX_HAND {
            // Bigger hands than the table knows: no useful bound.
            return u32::MAX / 4;
        }
        let mut fixed = [0u8; 14];
        for r in 1..14 {
            fixed[r] = have.rank[r] as u8;
        }
        if let Some(r) = known {
            fixed[r] += 1;
        }
        let rank_part = if free == 0 {
            rank_points(p, &fixed)
        } else if free <= 2 {
            // Few enough to try every choice of ranks the arriving cards could have.
            let any = have.jokers > 0 || pool.jokers > 0 || starter.jokers > 0;
            let avail: Vec<usize> = (1..14)
                .filter(|&r| any || pool.rank[r] > 0 || starter.rank[r] > 0)
                .collect();
            let mut most = rank_points(p, &fixed);
            let mut c = fixed;
            for (i, &a) in avail.iter().enumerate() {
                c[a] += 1;
                if free == 1 {
                    most = most.max(rank_points(p, &c));
                } else {
                    for &b in &avail[i..] {
                        c[b] += 1;
                        most = most.max(rank_points(p, &c));
                        c[b] -= 1;
                    }
                }
                c[a] -= 1;
            }
            most
        } else {
            best[rank_key(&fixed)]
        };
        let mut pts = rank_part;
        // Flush: every real card of the full line in one suit, the rest able to follow.
        if line_len >= 4 && (p.flush > 0 || p.flush_starter > 0) {
            let suits: Vec<usize> = (0..4)
                .filter(|&s| have.suit[s] as u32 == have.real())
                .filter(|&s| pool.suit[s] as u32 + pool.jokers >= slots)
                .collect();
            if !suits.is_empty() {
                let starter_too = suits
                    .iter()
                    .any(|&s| starter.suit[s] > 0 || starter.jokers > 0);
                pts = pts.saturating_add(p.flush).saturating_add(if starter_too {
                    p.flush_starter
                } else {
                    0
                });
            }
        }
        // Nobs: a jack in the line of the starter's suit.
        if p.nobs > 0 && has_starter {
            let can_get_jack = |s: usize| {
                have.jokers > 0
                    || (slots > 0 && (pool.jokers > 0 || pool.face[s * 13 + 10] > 0))
                    || have.face[s * 13 + 10] > 0
            };
            if (0..4).any(|s| (starter.suit[s] > 0 || starter.jokers > 0) && can_get_jack(s)) {
                pts = pts.saturating_add(p.nobs);
            }
        }
        pts
    }

    /// Cards pile `p` can still take.
    fn room(&self, g: &Game, s: &State, p: u16) -> u32 {
        (g.pile_rules[p as usize].max_cards as usize).saturating_sub(s.piles[p as usize].len())
            as u32
    }

    fn line_room(&self, g: &Game, s: &State, line: &[u16]) -> u32 {
        line.iter().map(|&p| self.room(g, s, p)).sum()
    }

    /// With symmetric rows and columns, placing a card into an empty row other than the first
    /// empty one (or an empty column other than the first) adds nothing new.
    pub fn redundant(&self, g: &Game, s: &State, to: u16) -> bool {
        if !self.symmetric {
            return false;
        }
        let gr = &g.groups[self.group as usize];
        if !gr.piles().contains(&to) {
            return false;
        }
        let i = to - gr.start;
        let (r, c) = (i / self.columns, i % self.columns);
        let empty = |p: u16| s.piles[p as usize].is_empty();
        let row_empty = |r: u16| (0..self.columns).all(|c| empty(gr.start + r * self.columns + c));
        let col_empty = |c: u16| (0..self.rows).all(|r| empty(gr.start + r * self.columns + c));
        (row_empty(r) && (0..r).any(row_empty)) || (col_empty(c) && (0..c).any(col_empty))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Suit;

    fn counts(cards: &[(Suit, u8)]) -> Counts {
        let mut c = Counts::default();
        for &(s, r) in cards {
            c.add(Card::new(s, r));
        }
        c
    }

    use Suit::*;

    #[test]
    fn classifies_hands() {
        let cases: &[(&[(Suit, u8)], usize)] = &[
            (
                &[
                    (Spades, 10),
                    (Spades, 11),
                    (Spades, 12),
                    (Spades, 13),
                    (Spades, 1),
                ],
                ROYAL_FLUSH,
            ),
            (
                &[
                    (Hearts, 1),
                    (Hearts, 2),
                    (Hearts, 3),
                    (Hearts, 4),
                    (Hearts, 5),
                ],
                STRAIGHT_FLUSH,
            ),
            (
                &[
                    (Hearts, 9),
                    (Clubs, 9),
                    (Spades, 9),
                    (Diamonds, 9),
                    (Hearts, 5),
                ],
                FOUR,
            ),
            (
                &[
                    (Hearts, 9),
                    (Clubs, 9),
                    (Spades, 9),
                    (Diamonds, 5),
                    (Hearts, 5),
                ],
                FULL_HOUSE,
            ),
            (
                &[
                    (Hearts, 9),
                    (Hearts, 2),
                    (Hearts, 7),
                    (Hearts, 5),
                    (Hearts, 12),
                ],
                FLUSH,
            ),
            (
                &[
                    (Hearts, 10),
                    (Clubs, 11),
                    (Hearts, 12),
                    (Spades, 13),
                    (Hearts, 1),
                ],
                STRAIGHT,
            ),
            (
                &[
                    (Hearts, 13),
                    (Clubs, 1),
                    (Hearts, 2),
                    (Spades, 3),
                    (Hearts, 4),
                ],
                0,
            ),
            (
                &[
                    (Hearts, 9),
                    (Clubs, 9),
                    (Spades, 9),
                    (Diamonds, 2),
                    (Hearts, 5),
                ],
                THREE,
            ),
            (
                &[
                    (Hearts, 9),
                    (Clubs, 9),
                    (Spades, 5),
                    (Diamonds, 2),
                    (Hearts, 5),
                ],
                TWO_PAIR,
            ),
            (
                &[
                    (Hearts, 9),
                    (Clubs, 9),
                    (Spades, 4),
                    (Diamonds, 2),
                    (Hearts, 5),
                ],
                PAIR,
            ),
            (&[(Hearts, 9), (Clubs, 9)], PAIR),
            (&[(Hearts, 9), (Hearts, 8), (Hearts, 7), (Hearts, 6)], 0),
        ];
        for (cards, want) in cases {
            assert_eq!(hand(&counts(cards)), *want, "{cards:?}");
        }
    }

    fn deck_with_jokers(n: usize) -> Vec<Card> {
        let mut deck: Vec<Card> = (0..52u8).map(Card::from_face).collect();
        deck.extend((0..n).map(|_| Card::joker()));
        deck
    }

    /// The best hand when each joker may be any card, by trying every card.
    fn best_by_substitution(cards: &[Card]) -> usize {
        match cards.iter().position(|c| c.is_joker()) {
            None => {
                let mut c = Counts::default();
                cards.iter().for_each(|&x| c.add(x));
                hand(&c)
            }
            Some(i) => {
                let mut v = cards.to_vec();
                (0..52u8)
                    .map(|f| {
                        v[i] = Card::from_face(f);
                        best_by_substitution(&v)
                    })
                    .max()
                    .unwrap()
            }
        }
    }

    #[test]
    fn jokers_make_the_best_hand() {
        let mut rng = crate::rng::Rng::new(3);
        for i in 0..400 {
            let mut deck = deck_with_jokers(2);
            rng.shuffle(&mut deck);
            // Make sure most lines hold a joker.
            let jokers = 1 + i % 2;
            let mut line: Vec<Card> = deck
                .iter()
                .copied()
                .filter(|c| !c.is_joker())
                .take(5 - jokers)
                .collect();
            line.extend((0..jokers).map(|_| Card::joker()));
            let n = 1 + (rng.next_u64() % 5) as usize;
            let line = &line[5 - n..];
            let mut c = Counts::default();
            line.iter().for_each(|&x| c.add(x));
            assert_eq!(hand(&c), best_by_substitution(line), "{line:?}");
        }
        // Non-monotone points: the joker picks the better-paying, lower hand.
        let english = points_table(
            &crate::rules::PokerTable::Named(crate::rules::PokerScoring::English).points(),
        );
        let mut c = Counts::default();
        for card in ["2H", "3H", "4H", "5H", "JK"] {
            c.add(card.parse().unwrap());
        }
        // A flush (5) or, with the joker as an ace or a six, a straight flush (30).
        assert_eq!(poker_points(&c, &english, false), 30);
        let mut c = Counts::default();
        for card in ["2H", "7H", "4H", "9H", "JK"] {
            c.add(card.parse().unwrap());
        }
        // Flush 5, but a pair pays only 1 and nothing better is possible.
        assert_eq!(poker_points(&c, &english, false), 5);
        let mut odd = english;
        odd[PAIR] = 9;
        assert_eq!(poker_points(&c, &odd, false), 9);
    }

    /// The relaxed test must say yes whenever some real completion makes the hand, held and
    /// arriving jokers included.
    #[test]
    fn possible_hands_cover_every_completion() {
        let mut rng = crate::rng::Rng::new(7);
        for _ in 0..3000 {
            let mut deck = deck_with_jokers(2);
            rng.shuffle(&mut deck);
            let k = (rng.next_u64() % 5) as usize;
            let pool_n = 1 + (rng.next_u64() % 8) as usize;
            let have_cards = &deck[..k];
            let pool_cards = &deck[k..k + pool_n];
            let slots = (5 - k).min(pool_n);
            let mut have = Counts::default();
            have_cards.iter().for_each(|&c| have.add(c));
            let mut pool = Counts::default();
            pool_cards.iter().for_each(|&c| pool.add(c));
            // Every way to fill the slots from the pool.
            let n = pool_cards.len();
            for mask in 0u32..1 << n {
                if mask.count_ones() as usize != slots {
                    continue;
                }
                let mut c = Counts::default();
                have_cards.iter().for_each(|&x| c.add(x));
                (0..n)
                    .filter(|i| mask >> i & 1 != 0)
                    .for_each(|i| c.add(pool_cards[i]));
                let mut cards: Vec<Card> = have_cards.to_vec();
                (0..n)
                    .filter(|i| mask >> i & 1 != 0)
                    .for_each(|i| cards.push(pool_cards[i]));
                let h = if cards.iter().filter(|c| c.is_joker()).count() == 2 {
                    hand(&c)
                } else {
                    best_by_substitution(&cards)
                };
                let slots = slots as u32;
                let set = possible(
                    &have,
                    &pool,
                    slots + have.jokers,
                    have.jokers + pool.jokers.min(slots),
                );
                assert!(
                    set >> h & 1 != 0,
                    "{have_cards:?} + {pool_cards:?}: hand {h} not judged possible"
                );
            }
        }
    }

    // ----- Cribbage ---------------------------------------------------------------------

    /// Cribbage points the slow way, straight from the definitions.
    fn reference_rank_points(p: &CribbagePoints, ranks: &[u8]) -> u32 {
        let n = ranks.len();
        let subsets = || {
            (1u32..1 << n).map(|m| {
                (0..n)
                    .filter(move |i| m >> i & 1 != 0)
                    .map(|i| ranks[i])
                    .collect::<Vec<u8>>()
            })
        };
        let fifteens = subsets()
            .filter(|s| s.iter().map(|&r| r.min(10) as u32).sum::<u32>() == 15)
            .count() as u32;
        let pairs = subsets().filter(|s| s.len() == 2 && s[0] == s[1]).count() as u32;
        let is_run = |s: &Vec<u8>| {
            let mut v = s.clone();
            v.sort();
            v.len() >= 3 && v.windows(2).all(|w| w[1] == w[0] + 1)
        };
        // Runs: every run of cards not inside a longer run.
        let runs: Vec<Vec<u8>> = subsets().filter(is_run).collect();
        let longest = runs.iter().map(|r| r.len()).max().unwrap_or(0);
        let run_cards: u32 = runs
            .iter()
            .filter(|r| r.len() == longest)
            .map(|r| r.len() as u32)
            .sum();
        fifteens * p.fifteen + pairs * p.pair + run_cards * p.run_card
    }

    fn cards(s: &[&str]) -> Vec<Card> {
        s.iter().map(|c| c.parse().unwrap()).collect()
    }

    #[test]
    fn cribbage_hands() {
        let p = CribbagePoints::default();
        let hand = |line: &[&str], starter: &str| {
            cribbage_hand(&p, &cards(line), Some(starter.parse().unwrap()), true)
        };
        // The best hand there is.
        assert_eq!(hand(&["5C", "5D", "5H", "JS"], "5S"), 29);
        // Double double run: 3-4-5 four ways (12), two pairs (4), 3+3+4+5 with either 4 (4).
        assert_eq!(hand(&["3C", "3D", "4H", "4S"], "5S"), 12 + 4 + 4);
        // Nothing: even values never make 15.
        assert_eq!(hand(&["2C", "4D", "6H", "8S"], "QH"), 0);
        // Flush of four, then five with the starter, which also brings nobs.
        assert_eq!(hand(&["2H", "4H", "6H", "JH"], "KC"), 4);
        assert_eq!(hand(&["2H", "4H", "6H", "JH"], "KH"), 5 + 1);
        // A joker counts as its best card: here the fourth five.
        assert_eq!(hand(&["5C", "5D", "5H", "JS"], "JK"), 29);
        assert_eq!(hand(&["5C", "5D", "JK", "JS"], "5S"), 29);
        // No flush in an incomplete line.
        assert_eq!(
            cribbage_hand(&p, &cards(&["2H", "4H", "6H", "JH"]), None, false),
            0
        );

        let mut rng = crate::rng::Rng::new(11);
        for _ in 0..2000 {
            let mut deck = deck_with_jokers(0);
            rng.shuffle(&mut deck);
            let n = 1 + (rng.next_u64() % 5) as usize;
            let ranks: Vec<u8> = deck[..n].iter().map(|c| c.rank()).collect();
            let mut count = [0u8; 14];
            ranks.iter().for_each(|&r| count[r as usize] += 1);
            assert_eq!(
                rank_points(&p, &count),
                reference_rank_points(&p, &ranks),
                "{ranks:?}"
            );
        }
    }

    #[test]
    fn best_rank_points_bounds_every_extension() {
        let p = CribbagePoints::default();
        let best = best_rank_points(&p, 4);
        assert_eq!(best[rank_key(&[0; 14])], 28);
        assert_eq!(best_rank_points(&p, 5)[0], 40);
        let mut rng = crate::rng::Rng::new(5);
        for _ in 0..2000 {
            let mut deck = deck_with_jokers(0);
            rng.shuffle(&mut deck);
            let k = (rng.next_u64() % 5) as usize;
            let mut count = [0u8; 14];
            deck[..k].iter().for_each(|c| count[c.rank() as usize] += 1);
            let b = best[rank_key(&count)];
            let mut full = count;
            deck[k..5].iter().for_each(|c| full[c.rank() as usize] += 1);
            assert!(rank_points(&p, &full) <= b);
            assert!(rank_points(&p, &count) <= b);
        }
    }

    /// The cribbage line bound is never below a real completion, jokers and all.
    #[test]
    fn cribbage_bound_covers_every_completion() {
        let p = CribbagePoints {
            heels: 0,
            ..CribbagePoints::default()
        };
        let score = Score {
            group: 0,
            lines: Vec::new(),
            hands: HandScoring::Poker {
                points: [0; 10],
                order: [0; 10],
                monotone: true,
            },
            pool: Pool::Outside,
            symmetric: false,
            rows: 0,
            columns: 0,
        };
        let best = best_rank_points(&p, 5);
        let mut rng = crate::rng::Rng::new(9);
        for _ in 0..1500 {
            let mut deck = deck_with_jokers(2);
            rng.shuffle(&mut deck);
            let k = (rng.next_u64() % 5) as usize;
            let pool_n = 1 + (rng.next_u64() % 6) as usize;
            let st_n = (rng.next_u64() % 3) as usize;
            let have_cards = &deck[..k];
            let pool_cards = &deck[k..k + pool_n];
            let st_cards = &deck[k + pool_n..k + pool_n + st_n];
            let slots = (4 - k).min(pool_n);
            let mut have = Counts::default();
            have_cards.iter().for_each(|&c| have.add(c));
            let mut pool = Counts::default();
            pool_cards.iter().for_each(|&c| pool.add(c));
            let mut starter = Counts::default();
            st_cards.iter().for_each(|&c| starter.add(c));
            let bound =
                score.cribbage_line_bound(&p, &best, &have, &pool, slots as u32, &starter, 4);
            let n = pool_cards.len();
            for mask in 0u32..1 << n {
                if mask.count_ones() as usize != slots {
                    continue;
                }
                let mut line = have_cards.to_vec();
                (0..n)
                    .filter(|i| mask >> i & 1 != 0)
                    .for_each(|i| line.push(pool_cards[i]));
                let full = line.len() == 4;
                let starters: Vec<Option<Card>> = if st_cards.is_empty() {
                    vec![None]
                } else {
                    st_cards.iter().map(|&c| Some(c)).collect()
                };
                for st in starters {
                    let v = cribbage_hand(&p, &line, st, full);
                    assert!(
                        v <= bound,
                        "{have_cards:?} + {line:?} / {st:?}: {v} > bound {bound}"
                    );
                }
            }
        }
    }
}
