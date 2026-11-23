//! Search.
//!
//! Two strategies share one move generator:
//!
//! * **Depth-first** with a transposition table and heuristic move ordering. Little memory, finds
//!   a solution fast, and, given enough budget, proves a deal unsolvable by exhausting it.
//! * **Best-first** (weighted A*-style) over compactly encoded positions. More memory, usually
//!   shorter solutions, and steadier on games where depth-first wanders (Klondike, Spider).
//!
//! Both see every card, face down or not: they solve the deal, not the player's view of it.
//! See [`crate::hint()`] for advice that respects hidden cards.
//!
//! Positions are deduplicated by a 128-bit canonical hash: piles in interchangeable groups are
//! sorted first, so mirror-image positions are searched once.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::card::{Card, FACES};
use crate::game::{Action, Game, Move, Op, State};
use crate::rules::{BuildBy, PileKind, WinRule};

/// Search strategy.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Strategy {
    /// Depth-first, heuristic ordering. Can prove unsolvability.
    DepthFirst,
    /// Best-first by heuristic. Can also prove unsolvability, at a higher memory cost.
    BestFirst,
    /// Best-first for a share of the budget, then depth-first with the rest.
    #[default]
    Auto,
}

/// Search limits and knobs.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
#[non_exhaustive]
pub struct SolveOptions {
    pub strategy: Strategy,
    /// Stop after visiting this many distinct positions.
    pub max_states: u64,
    /// Stop after this many milliseconds of wall-clock time (native targets only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Play provably safe foundation moves automatically (when the rules allow it).
    pub autoplay: bool,
    /// Shorten winning lines by cutting out detours (default true).
    pub shorten: bool,
}

impl Default for SolveOptions {
    fn default() -> Self {
        SolveOptions {
            strategy: Strategy::Auto,
            max_states: 2_000_000,
            timeout_ms: None,
            autoplay: true,
            shorten: true,
        }
    }
}

impl SolveOptions {
    /// Stop after visiting `n` distinct positions.
    pub fn max_states(mut self, n: u64) -> Self {
        self.max_states = n;
        self
    }

    /// Stop after `ms` milliseconds of wall-clock time (native targets only).
    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Stop after wall-clock duration `d` (native targets only).
    pub fn max_duration(mut self, d: Duration) -> Self {
        self.timeout_ms = Some(d.as_millis().min(u64::MAX as u128) as u64);
        self
    }

    pub fn strategy(mut self, s: Strategy) -> Self {
        self.strategy = s;
        self
    }

    pub fn autoplay(mut self, on: bool) -> Self {
        self.autoplay = on;
        self
    }

    pub fn shorten(mut self, on: bool) -> Self {
        self.shorten = on;
        self
    }
}

/// How a search ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A winning line of play.
    Solved(Vec<Move>),
    /// Every reachable position was searched: there is no win.
    Unsolvable,
    /// The budget ran out first.
    GaveUp,
}

impl Outcome {
    pub fn is_solved(&self) -> bool {
        matches!(self, Outcome::Solved(_))
    }

    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Solved(_) => "solved",
            Outcome::Unsolvable => "unsolvable",
            Outcome::GaveUp => "gave-up",
        }
    }
}

/// Result of [`solve`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SolveResult {
    pub outcome: Outcome,
    /// Distinct positions visited.
    pub states: u64,
}

/// Cooperative stop condition: an external cancel flag and/or a wall-clock deadline.
#[derive(Copy, Clone)]
struct Stop<'c> {
    cancel: Option<&'c AtomicBool>,
    #[cfg(not(target_arch = "wasm32"))]
    deadline: Option<std::time::Instant>,
}

impl<'c> Stop<'c> {
    fn new(opts: &SolveOptions, cancel: Option<&'c AtomicBool>) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let deadline = opts
            .timeout_ms
            .map(|ms| std::time::Instant::now() + Duration::from_millis(ms));
        #[cfg(target_arch = "wasm32")]
        let _ = opts;
        Stop {
            cancel,
            #[cfg(not(target_arch = "wasm32"))]
            deadline,
        }
    }

    fn none() -> Self {
        Stop {
            cancel: None,
            #[cfg(not(target_arch = "wasm32"))]
            deadline: None,
        }
    }

    #[inline]
    fn is_stopped_now(&self) -> bool {
        let cancelled = self.cancel.is_some_and(|c| c.load(Ordering::Relaxed));
        #[cfg(not(target_arch = "wasm32"))]
        {
            cancelled
                || self
                    .deadline
                    .is_some_and(|d| std::time::Instant::now() >= d)
        }
        #[cfg(target_arch = "wasm32")]
        {
            cancelled
        }
    }

    /// Periodic check in hot loops (every 1,024 states) to avoid overhead.
    #[inline]
    fn should_stop(&self, states: u64) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        let has_stop = self.cancel.is_some() || self.deadline.is_some();
        #[cfg(target_arch = "wasm32")]
        let has_stop = self.cancel.is_some();
        has_stop && (states & 1023) == 0 && self.is_stopped_now()
    }
}

/// Solve from a position.
pub fn solve(game: &Game, start: &State, opts: &SolveOptions) -> SolveResult {
    solve_inner(game, start, opts, Stop::new(opts, None))
}

/// Solve from a position, stopping early with [`Outcome::GaveUp`] if `cancel` becomes `true`.
///
/// The flag is checked periodically (every 1,024 visited states) and between search phases.
/// A cancelled search never reports [`Outcome::Unsolvable`].
pub fn solve_with_cancel(
    game: &Game,
    start: &State,
    opts: &SolveOptions,
    cancel: &AtomicBool,
) -> SolveResult {
    solve_inner(game, start, opts, Stop::new(opts, Some(cancel)))
}

fn solve_inner(game: &Game, start: &State, opts: &SolveOptions, stop: Stop<'_>) -> SolveResult {
    if stop.is_stopped_now() {
        return SolveResult {
            outcome: Outcome::GaveUp,
            states: 0,
        };
    }
    let mut r = search(game, start, opts, stop);
    if opts.shorten && !stop.is_stopped_now() {
        if let Outcome::Solved(moves) = &r.outcome {
            r.outcome = Outcome::Solved(shorten(game, start, moves));
        }
    }
    r
}

/// Result of [`maximize`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct MaxResult {
    /// The best final score found. 0 if the game isn't scored.
    pub score: u32,
    /// A line of play that ends with that score.
    pub moves: Vec<Move>,
    /// Every line was searched or ruled out by a bound: no line scores more.
    pub proven: bool,
    /// Positions searched (a position searched again with more freedom counts again).
    pub states: u64,
}

/// Find the line of play that ends the game (no moves left) with the most points.
///
/// Every position gets an upper bound on the points still in reach, and the search drops any
/// line whose bound falls short. It starts from one greedy line, then runs depth-first branch
/// and bound with limited discrepancy: lines may stray from the best-bound move by a total
/// that doubles each round, so lines near the bound's favourite are tried first.
/// `Strategy::DepthFirst` instead runs one depth-first branch and bound. Either way, the
/// search stops at `max_states` positions and `proven` says whether it finished. `autoplay`
/// is ignored.
pub fn maximize(game: &Game, start: &State, opts: &SolveOptions) -> MaxResult {
    maximize_inner(game, start, opts, Stop::new(opts, None))
}

/// Like [`maximize`], stopping early with `proven: false` if `cancel` becomes `true`.
pub fn maximize_with_cancel(
    game: &Game,
    start: &State,
    opts: &SolveOptions,
    cancel: &AtomicBool,
) -> MaxResult {
    maximize_inner(game, start, opts, Stop::new(opts, Some(cancel)))
}

fn maximize_inner(game: &Game, start: &State, opts: &SolveOptions, stop: Stop<'_>) -> MaxResult {
    if game.score.is_none() {
        return MaxResult {
            score: 0,
            moves: Vec::new(),
            proven: true,
            states: 0,
        };
    }
    let budget = opts.max_states;
    let mut ctx = Ctx::new(game, opts, stop);
    let (mut lo, mut line) = ctx.greedy(start);
    if stop.is_stopped_now() {
        return MaxResult {
            score: lo,
            moves: line,
            proven: false,
            states: 0,
        };
    }
    if opts.strategy == Strategy::DepthFirst {
        let p = ctx.bounded_dfs(start, lo + 1, u32::MAX, budget, false);
        let (score, moves) = p.found.unwrap_or((lo, line));
        return MaxResult {
            score,
            moves,
            proven: p.complete,
            states: p.states,
        };
    }
    // Limited discrepancy search: branch and bound that may stray from the best-bound move
    // by a growing total, so lines close to the bound's favourite are tried first.
    let mut states = 0u64;
    let mut proven = false;
    let mut slack = 1u32;
    while states < budget && !stop.is_stopped_now() {
        let mut ctx = Ctx::new(game, opts, stop);
        let p = ctx.bounded_dfs(start, lo + 1, slack, budget - states, false);
        states += p.states;
        if let Some((v, m)) = p.found {
            lo = v;
            line = m;
        }
        if p.complete {
            proven = true;
            break;
        }
        slack = slack.saturating_mul(2);
    }
    MaxResult {
        score: lo,
        moves: line,
        proven,
        states,
    }
}

/// Search for a position scoring at least `target`: the limited discrepancy search of
/// [`maximize`], stopping at the first win. Positions whose bound falls short are dead.
fn reach_score(
    game: &Game,
    start: &State,
    target: u32,
    opts: &SolveOptions,
    stop: Stop<'_>,
) -> SolveResult {
    let budget = opts.max_states;
    let mut states = 0u64;
    let mut slack = if opts.strategy == Strategy::DepthFirst {
        u32::MAX
    } else {
        0
    };
    loop {
        if stop.is_stopped_now() {
            return SolveResult {
                outcome: Outcome::GaveUp,
                states,
            };
        }
        let mut ctx = Ctx::new(game, opts, stop);
        let p = ctx.bounded_dfs(start, target, slack, budget - states, true);
        states += p.states;
        if let Some((_, line)) = p.found {
            return SolveResult {
                outcome: Outcome::Solved(line),
                states,
            };
        }
        if p.complete {
            return SolveResult {
                outcome: Outcome::Unsolvable,
                states,
            };
        }
        if states >= budget || stop.is_stopped_now() {
            return SolveResult {
                outcome: Outcome::GaveUp,
                states,
            };
        }
        slack = slack.saturating_mul(2).max(1);
    }
}

/// How a bounded depth-first search ended.
struct Probe {
    found: Option<(u32, Vec<Move>)>,
    /// Every line was searched or ruled out.
    complete: bool,
    states: u64,
}

/// Remove detours from a winning line: wherever one move from position i reaches the exact
/// position at some later j, skip from i to j. Repeats until nothing changes.
pub fn shorten(game: &Game, start: &State, moves: &[Move]) -> Vec<Move> {
    let mut line = moves.to_vec();
    let mut ctx = Ctx::new(
        game,
        &SolveOptions {
            autoplay: false,
            ..SolveOptions::default()
        },
        Stop::none(),
    );
    loop {
        // Exact positions along the line.
        let mut s = start.clone();
        let mut j = Vec::new();
        let mut index: std::collections::HashMap<Box<[u8]>, usize> =
            std::collections::HashMap::new();
        index.insert(encode(&s), 0);
        for (i, &m) in line.iter().enumerate() {
            game.apply(&mut s, m, &mut j);
            index.insert(encode(&s), i + 1);
        }
        let mut out = Vec::with_capacity(line.len());
        let mut s = start.clone();
        let mut kids = Vec::new();
        let mut buf = Vec::new();
        let mut i = 0;
        while i < line.len() {
            let mut best: Option<(usize, Move)> = None;
            kids.clear();
            game.gen_actions(&s, None, &mut kids);
            if game.compress_talon {
                ctx.talon_moves(&mut s, &mut kids);
            } else if game.can_deal(&s) {
                kids.push(Move::new(Action::Deal));
            }
            for &m in &kids {
                let mark = j.len();
                game.apply(&mut s, m, &mut j);
                buf.clear();
                encode_into(&s, &mut buf);
                if let Some(&k) = index.get(buf.as_slice()) {
                    if k > i + 1 && best.is_none_or(|(b, _)| k > b) {
                        best = Some((k, m));
                    }
                }
                game.undo_to(&mut s, &mut j, mark);
            }
            let (next, m) = best.unwrap_or((i + 1, line[i]));
            game.apply(&mut s, m, &mut j);
            j.clear();
            out.push(m);
            i = next;
        }
        if out.len() == line.len() {
            return out;
        }
        line = out;
    }
}

fn search(game: &Game, start: &State, opts: &SolveOptions, stop: Stop<'_>) -> SolveResult {
    if let (Some(target), true) = (game.target_score(), game.is_scored()) {
        return reach_score(game, start, target, opts, stop);
    }
    let mut ctx = Ctx::new(game, opts, stop);
    match opts.strategy {
        Strategy::DepthFirst => ctx.dfs(start, opts.max_states),
        Strategy::BestFirst => ctx.best_first(start, opts.max_states, false),
        Strategy::Auto => {
            // Fast phases prune moves that are rarely useful, to find a win fast. They can miss
            // wins, so only the final sound search can report unsolvable.
            let share = ctx.search.fast_share.min(100) as u64;
            let fast_budget = opts.max_states / 100 * share;
            let p1_budget = if fast_budget >= 2_000 {
                fast_budget * 3 / 5
            } else {
                fast_budget
            };
            ctx.fast = true;
            let r1 = ctx.best_first(start, p1_budget, false);
            if r1.outcome.is_solved() {
                return r1;
            }
            let mut spent = r1.states;
            if stop.is_stopped_now() {
                return SolveResult {
                    outcome: Outcome::GaveUp,
                    states: spent,
                };
            }
            // If Phase 1 gave up (hit budget in a greedy basin), try weighted A* (2*h + depth)
            // with the rest of the fast budget to escape deep local minima.
            if r1.outcome == Outcome::GaveUp && fast_budget > p1_budget {
                let mut ctx2 = Ctx::new(game, opts, stop);
                ctx2.fast = true;
                let p2_budget = fast_budget.saturating_sub(spent);
                if p2_budget > 0 {
                    let r2 = ctx2.best_first(start, p2_budget, true);
                    spent += r2.states;
                    if r2.outcome.is_solved() {
                        return SolveResult {
                            outcome: r2.outcome,
                            states: spent,
                        };
                    }
                    if stop.is_stopped_now() {
                        return SolveResult {
                            outcome: Outcome::GaveUp,
                            states: spent,
                        };
                    }
                }
            }
            let mut ctx3 = Ctx::new(game, opts, stop);
            let mut r3 = ctx3.dfs(start, opts.max_states.saturating_sub(spent));
            r3.states += spent;
            r3
        }
    }
}

struct Child {
    mv: Move,
    hash: u128,
    score: u32,
    won: bool,
}

struct Ctx<'g, 'c> {
    g: &'g Game,
    stop: Stop<'c>,
    autoplay: bool,
    seen: HashSet<u128, BuildU128>,
    journal: Vec<Op>,
    scratch: Vec<Move>,
    talon_buf: Vec<Move>,
    pile_hashes: Vec<u128>,
    pile_hash_valid: Vec<bool>,
    /// `prefix[p][i]`: running hash of pile p's bottom i cards, valid while those cards are
    /// unchanged. Piles only change at the top, so most of a pile is never rehashed.
    prefix: Vec<Vec<u128>>,
    /// Scratch: how deep each pile was disturbed by the ops being accounted for.
    disturbed: Vec<u32>,
    touched: Vec<u16>,
    /// Which non-foundation, non-stock groups have a build rule (for the disorder heuristic).
    builds: Vec<bool>,
    /// Foundation piles built in a fixed direction, for the "needed sooner" blocker term.
    ordered_foundations: Vec<u16>,
    /// True if any foundation requires complete same-suit runs (e.g. Spider, Black Widow).
    suit_complete_runs: bool,
    /// Effective heuristic weights (explicit rules override, or derived from game structure).
    search: crate::rules::SearchHints,
    /// Heuristic pruning (unsound): see `Strategy::Auto`.
    fast: bool,
}

/// Derive search weights from the compiled game when the rules leave `search` at its default.
fn effective_search(g: &Game, ordered_foundations: &[u16]) -> crate::rules::SearchHints {
    if !g.rules().search.is_default() {
        return g.rules().search.clone();
    }
    let mut s = crate::rules::SearchHints::default();
    let has_complete_runs = g.pile_rules.iter().any(|r| r.complete_runs);
    let deals_to_tableau = g
        .stock
        .as_ref()
        .is_some_and(|st| st.target_kind == PileKind::Tableau);
    if has_complete_runs && deals_to_tableau {
        // Spider-like games: spend most of the budget in the fast best-first phase. With four
        // suits, disorder and face-down cards matter more; on multi-pack tableaux, keep stock
        // weight at least equal to disorder weight so dealing a row isn't avoided indefinitely.
        s.fast_share = 90;
        if g.rules().pack.suits.len() >= 4 {
            s.disorder_weight = 3;
            s.down_weight = 5;
            if g.total_cards > 60 {
                s.stock_weight = 2;
            }
        }
    } else if g.force_lower_of_suit {
        s.fast_share = 90;
    } else if !ordered_foundations.is_empty()
        && g.groups.iter().any(|gr| {
            gr.kind == PileKind::Tableau
                && g.pile_rules[gr.start as usize].moves == crate::rules::MoveUnit::Top
                && g.pile_rules[gr.start as usize].build.by != BuildBy::None
        })
    {
        // Single-card builders (Baker's Dozen, Forty Thieves, Numerica, etc.): burying a
        // card needed sooner by a foundation is the main trap.
        s.blocker_weight = 4;
    }
    s
}

impl<'g, 'c> Ctx<'g, 'c> {
    fn new(g: &'g Game, opts: &SolveOptions, stop: Stop<'c>) -> Ctx<'g, 'c> {
        let ordered_foundations: Vec<u16> = (0..g.num_piles() as u16)
            .filter(|&p| {
                let r = g.rule(p);
                g.pile_kind(p) == PileKind::Foundation
                    && !r.complete_runs
                    && r.build.by != BuildBy::None
                    && matches!(r.build.dir, crate::rules::Dir::Up | crate::rules::Dir::Down)
            })
            .collect();
        let suit_complete_runs = g
            .pile_rules
            .iter()
            .any(|r| r.complete_runs && r.build.by == BuildBy::Suit);
        let search = effective_search(g, &ordered_foundations);
        Ctx {
            g,
            stop,
            autoplay: opts.autoplay && g.rules().safe_autoplay,
            seen: HashSet::with_hasher(BuildU128),
            journal: Vec::new(),
            scratch: Vec::new(),
            talon_buf: Vec::new(),
            pile_hashes: vec![0; g.num_piles()],
            pile_hash_valid: vec![false; g.num_piles()],
            prefix: vec![vec![PREFIX_SEED]; g.num_piles()],
            disturbed: vec![0; g.num_piles()],
            touched: Vec::new(),
            builds: g
                .pile_rules
                .iter()
                .map(|r| r.build.by != BuildBy::None)
                .collect(),
            ordered_foundations,
            suit_complete_runs,
            search,
            fast: false,
        }
    }

    // ----- Move generation for search --------------------------------------------------------

    /// Play a move, keeping the prefix hashes honest. Returns the undo mark.
    fn play(&mut self, s: &mut State, mv: Move) -> usize {
        let mark = self.journal.len();
        self.g.apply(s, mv, &mut self.journal);
        self.forget_range(s, mark);
        mark
    }

    /// Undo back to `mark`, keeping the prefix hashes honest.
    fn unplay(&mut self, s: &mut State, mark: usize) {
        self.record_disturbed(mark);
        self.g.undo_to(s, &mut self.journal, mark);
        self.apply_disturbed(s);
    }

    fn record_disturbed(&mut self, mark: usize) {
        let mut touch = |p: u16, n: u16| {
            let slot = &mut self.disturbed[p as usize];
            if *slot == 0 && n > 0 {
                self.touched.push(p);
            }
            *slot += n as u32;
        };
        for op in &self.journal[mark..] {
            match *op {
                Op::Shift { from, to, count }
                | Op::Turn {
                    from, to, count, ..
                } => {
                    touch(from, count);
                    touch(to, count);
                }
                Op::Flip(p) => touch(p, 1),
                Op::Redeal => {}
                Op::Swap { pile, .. } => touch(pile, u16::MAX),
            }
        }
    }

    fn apply_disturbed(&mut self, s: &State) {
        for p in self.touched.drain(..) {
            let p = p as usize;
            let d = std::mem::take(&mut self.disturbed[p]) as usize;
            let keep = s.piles[p].len().saturating_sub(d) + 1;
            self.prefix[p].truncate(keep.max(1));
            self.pile_hash_valid[p] = false;
        }
    }

    /// Invalidate prefix hashes above the depth `journal[mark..]` could have disturbed.
    fn forget_range(&mut self, s: &State, mark: usize) {
        self.record_disturbed(mark);
        self.apply_disturbed(s);
    }

    fn forget_all(&mut self) {
        for v in &mut self.prefix {
            v.truncate(1);
        }
        self.pile_hash_valid.fill(false);
    }

    #[inline]
    fn touches_foundation(&self, mv: Move) -> bool {
        match mv.action {
            Action::Transfer { from, to, .. } => {
                self.g.pile_kind(from) == PileKind::Foundation
                    || self.g.pile_kind(to) == PileKind::Foundation
            }
            Action::Deal => {
                self.g.redeal.is_some()
                    || self
                        .g
                        .stock
                        .as_ref()
                        .is_some_and(|st| st.target_kind == PileKind::Foundation)
            }
            Action::Remove { .. } => false,
        }
    }

    /// Visit every child of `s`, evaluating talon-compressed moves in place (one forward pass
    /// through the stock instead of replaying `k` deals per child). The callback is invoked
    /// while `s` is at the child state, and returns `true` to stop early.
    fn visit_children(
        &mut self,
        s: &mut State,
        mut on_child: impl FnMut(&mut Ctx<'g, 'c>, &State, Move, Option<&[u8; FACES]>) -> bool,
    ) {
        let g = self.g;
        let mut moves = std::mem::take(&mut self.scratch);
        moves.clear();
        let first_empty = g.compute_first_empty(s);
        g.gen_actions_with_empty(s, None, &first_empty, &mut moves);
        let forced = if let Some(m) = self.forced(s, &moves) {
            moves.clear();
            moves.push(m);
            true
        } else {
            if self.fast {
                moves.retain(|m| !self.pointless_split(s, *m));
            }
            if !g.compress_talon && g.can_deal(s) {
                moves.push(Move::new(Action::Deal));
            }
            false
        };
        self.drop_redundant(s, &mut moves);
        let target = match g.win {
            crate::game::Win::Score(t) => g.score.as_ref().map(|sc| (sc, t)),
            _ => None,
        };
        let parent_need = (!self.ordered_foundations.is_empty() && self.search.blocker_weight > 0)
            .then(|| self.need(s));
        let mut stop = false;
        for &mv in &moves {
            let mark = self.play(s, mv);
            if let Some((sc, t)) = target {
                if sc.bound(g, s) < t {
                    self.unplay(s, mark);
                    continue;
                }
            }
            let child_need = if parent_need.is_some() && self.touches_foundation(mv) {
                Some(self.need(s))
            } else {
                None
            };
            let need_ref = child_need.as_ref().or(parent_need.as_ref());
            stop = on_child(self, s, mv, need_ref);
            self.unplay(s, mark);
            if stop {
                break;
            }
        }
        self.scratch = moves;
        if stop || forced || !g.compress_talon {
            return;
        }
        // Walk the talon once in place, evaluating each playable waste card at its current depth k.
        let waste = g.waste.expect("compression needs a waste");
        let st = g.stock.as_ref().unwrap();
        let talon_mark = self.journal.len();
        let mut k: u16 = 0;
        let mut redeals_seen = 0u32;
        let mut buf = std::mem::take(&mut self.talon_buf);
        loop {
            if !g.can_deal(s) || k == u16::MAX {
                break;
            }
            let was_redeal = s.piles[st.pile as usize].is_empty();
            if was_redeal {
                redeals_seen += 1;
                if st.redeals == u32::MAX && redeals_seen >= 2 {
                    break;
                }
            }
            let step_mark = self.journal.len();
            g.talon_step(s, &mut self.journal);
            self.forget_range(s, step_mark);
            k += 1;
            if s.piles[waste as usize].is_empty() {
                continue;
            }
            buf.clear();
            if g.simple_settle {
                g.gen_actions_with_empty(s, Some(waste), &first_empty, &mut buf);
            } else {
                g.gen_actions(s, Some(waste), &mut buf);
            }
            let mut talon_forced = false;
            if st.only_when_empty || (self.fast && st.deal_count(s.redeals_used) == 1) {
                if let Some(m) = self.forced(s, &buf) {
                    buf.clear();
                    buf.push(m);
                    talon_forced = true;
                }
            }
            self.drop_redundant(s, &mut buf);
            for &m in &buf {
                let mv = Move {
                    talon: k,
                    action: m.action,
                };
                let act_mark = self.play(s, m);
                if let Some((sc, t)) = target {
                    if sc.bound(g, s) < t {
                        self.unplay(s, act_mark);
                        continue;
                    }
                }
                let child_need = if parent_need.is_some() && self.touches_foundation(m) {
                    Some(self.need(s))
                } else {
                    None
                };
                let need_ref = child_need.as_ref().or(parent_need.as_ref());
                stop = on_child(self, s, mv, need_ref);
                self.unplay(s, act_mark);
                if stop {
                    break;
                }
            }
            if stop || talon_forced {
                break;
            }
        }
        self.talon_buf = buf;
        self.unplay(s, talon_mark);
    }

    /// Push unseen children of a position onto `out`, best heuristic score first.
    fn push_dfs_children(&mut self, s: &mut State, out: &mut Vec<Child>) {
        let start = out.len();
        self.visit_children(s, |ctx, st, mv, need| {
            let hash = ctx.hash(st);
            if ctx.seen.contains(&hash) {
                return false;
            }
            let won = ctx.g.is_won(st);
            let score = if won {
                0
            } else {
                (ctx.heuristic_with_need(st, need) << 8) | (mv.talon.min(255) as u32)
            };
            out.push(Child {
                mv,
                hash,
                score,
                won,
            });
            won
        });
        out[start..].sort_by_key(|c| c.score);
    }

    /// Drop moves that a symmetry makes equivalent to another (scored grids).
    fn drop_redundant(&self, s: &State, moves: &mut Vec<Move>) {
        let g = self.g;
        if let Some(sc) = g.score.as_ref().filter(|sc| sc.symmetric) {
            moves.retain(|m| match m.action {
                Action::Transfer { to, .. } => !sc.redundant(g, s, to),
                _ => true,
            });
        }
    }

    /// Every next move with the bound on the score after it, best bound first. For
    /// [`maximize`].
    fn ranked(&mut self, s: &mut State) -> Vec<(Move, u32)> {
        let g = self.g;
        let sc = g.score.as_ref().expect("scored game");
        let mut moves = std::mem::take(&mut self.scratch);
        moves.clear();
        g.gen_actions(s, None, &mut moves);
        if g.compress_talon {
            self.talon_moves(s, &mut moves);
        } else if g.can_deal(s) {
            moves.push(Move::new(Action::Deal));
        }
        self.drop_redundant(s, &mut moves);
        let mut out: Vec<(Move, u32)> = Vec::with_capacity(moves.len());
        for &mv in &moves {
            let mark = self.play(s, mv);
            out.push((mv, sc.bound(g, s)));
            self.unplay(s, mark);
        }
        self.scratch = moves;
        out.sort_by_key(|&(_, b)| std::cmp::Reverse(b));
        out
    }

    /// Talon compression: for each number of stock actions k, the moves that play the card the
    /// k-th action leaves on top of the waste.
    fn talon_moves(&mut self, s: &mut State, out: &mut Vec<Move>) {
        let g = self.g;
        let waste = g.waste.expect("compression needs a waste");
        let st = g.stock.as_ref().unwrap();
        let mark = self.journal.len();
        let mut k: u16 = 0;
        let mut redeals_seen = 0u32;
        let mut buf = Vec::new();
        loop {
            if !g.can_deal(s) || k == u16::MAX {
                break;
            }
            let was_redeal = s.piles[st.pile as usize].is_empty();
            if was_redeal {
                redeals_seen += 1;
                // With unlimited redeals the talon cycles: after the second turn-over everything
                // repeats.
                if st.redeals == u32::MAX && redeals_seen >= 2 {
                    break;
                }
            }
            g.talon_step(s, &mut self.journal);
            k += 1;
            buf.clear();
            g.gen_actions(s, Some(waste), &mut buf);
            out.extend(buf.iter().map(|m| Move {
                talon: k,
                action: m.action,
            }));
        }
        self.unplay(s, mark);
    }

    /// Splitting a movable run onto another non-foundation pile, when the card it uncovers
    /// can't go to a foundation, rarely helps.
    fn pointless_split(&self, s: &State, m: Move) -> bool {
        let g = self.g;
        let Action::Transfer { from, to, count } = m.action else {
            return false;
        };
        if g.group_of(to).kind == PileKind::Foundation {
            return false;
        }
        // Pulling a card back off a foundation rarely helps when the goal is to fill foundations.
        if g.pile_kind(from) == PileKind::Foundation
            && matches!(g.win, crate::game::Win::AllOnFoundations)
        {
            return true;
        }
        let gr = g.rule(from);
        // Only a movable run can be split pointlessly: when only the top card can move,
        // peeling it off is the only way to move the card beneath.
        if !matches!(
            gr.moves,
            crate::rules::MoveUnit::Run
                | crate::rules::MoveUnit::SuitRun
                | crate::rules::MoveUnit::AnyFaceUp
                | crate::rules::MoveUnit::Supermove
        ) {
            return false;
        }
        let pile = &s.piles[from as usize];
        let n = count as usize;
        if n >= pile.len() {
            return false;
        }
        let exposed = pile[pile.len() - n - 1];
        let bottom = pile[pile.len() - n];
        let suit_runs = gr.moves == crate::rules::MoveUnit::SuitRun || self.suit_complete_runs;
        let built = exposed.is_face_up()
            && gr.fits[exposed.face() as usize] >> bottom.face() & 1 != 0
            && (!suit_runs || exposed.suit() == bottom.suit());
        if !built {
            return false;
        }
        !(0..g.num_piles() as u16)
            .filter(|&p| g.pile_kind(p) == PileKind::Foundation)
            .any(|p| match s.piles[p as usize].last() {
                None => g.empty_accepts(p, exposed, s),
                Some(t) => g.rule(p).fits[t.face() as usize] >> exposed.face() & 1 != 0,
            })
    }

    /// A move that can be made without losing anything, if the rules allow finding one.
    fn forced(&self, s: &State, moves: &[Move]) -> Option<Move> {
        // A joker might want any card as somewhere to stand, so nothing is forced with jokers.
        if self.g.jokers > 0
            || (!self.autoplay && !self.g.pile_rules.iter().any(|r| r.complete_runs))
        {
            return None;
        }
        let g = self.g;
        let mut on_found: Option<[u8; FACES]> = None;
        for &m in moves {
            match m.action {
                Action::Transfer { from, to, count } => {
                    let tg = g.rule(to);
                    if tg.complete_runs {
                        return Some(m);
                    }
                    let fast_found = self.fast
                        && g.pile_kind(to) == PileKind::Foundation
                        && count == 1
                        && g.score.is_none()
                        && g.redeal.is_none()
                        && !g.rule(to).complete_runs
                        && g.rule(to).build.by != BuildBy::None;
                    if !self.autoplay || count != 1 || (!tg.autoplay && !fast_found) {
                        continue;
                    }
                    // Emptying a pile that refills itself changes more than the foundation.
                    if s.piles[from as usize].len() == 1 && !g.group_of(from).refill_from.is_empty()
                    {
                        continue;
                    }
                    let card = *s.piles[from as usize].last().unwrap();
                    let counts = on_found.get_or_insert_with(|| self.foundation_counts(s));
                    // With multiple copies, autoplaying an earlier copy can consume the only
                    // foundation slot another copy on the table needs to uncover a buried card.
                    // It is still sound when every remaining copy has an open foundation slot,
                    // and the fast phase can always autoplay since it never proves unsolvability.
                    if g.copies > 1 && !self.fast {
                        let rem = g.copies - counts[card.face() as usize];
                        if rem > 1 {
                            let slots = g
                                .groups
                                .iter()
                                .filter(|gr| gr.kind == PileKind::Foundation)
                                .flat_map(|gr| gr.piles())
                                .filter(|&p| {
                                    let pr = g.rule(p);
                                    pr.autoplay
                                        && match s.piles[p as usize].last() {
                                            None => g.empty_accepts(p, card, s),
                                            Some(t) => {
                                                pr.fits[t.face() as usize] >> card.face() & 1 != 0
                                            }
                                        }
                                })
                                .count();
                            if slots < rem as usize {
                                continue;
                            }
                        }
                    }
                    if g.dependents[card.face() as usize]
                        .iter()
                        .all(|&f| counts[f as usize] >= g.copies)
                    {
                        return Some(m);
                    }
                }
                // A card removed on its own that could never be part of a pair (Pyramid's
                // kings), or a lower-of-suit discard when nothing can build on the table:
                // removing it now loses nothing.
                Action::Remove { rule, n: 1, piles } if self.autoplay => {
                    if g.force_lower_of_suit {
                        return Some(m);
                    }
                    if let crate::rules::RemovalKind::PairsSum { total, .. } =
                        g.removal[rule as usize].kind
                    {
                        let r = s.piles[piles[0] as usize].last().unwrap().rank();
                        let partner = total.checked_sub(r).filter(|p| (1..=13).contains(p));
                        if partner.is_none_or(|p| g.rank_idx[p as usize] == 255) {
                            return Some(m);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn foundation_counts(&self, s: &State) -> [u8; FACES] {
        let mut c = [0u8; FACES];
        for gr in self
            .g
            .groups
            .iter()
            .filter(|gr| gr.kind == PileKind::Foundation)
        {
            for p in gr.piles() {
                for card in &s.piles[p as usize] {
                    c[card.face() as usize] += 1;
                }
            }
        }
        c
    }

    // ----- Heuristic -------------------------------------------------------------------------

    /// Lower is closer to a win. Not admissible; it orders the search.
    fn heuristic_with_need(&self, s: &State, need: Option<&[u8; FACES]>) -> u32 {
        if let Some(sc) = &self.g.score {
            // Keep the most points in reach.
            return 1_000_000 - sc.bound(self.g, s).min(1_000_000);
        }
        let g = self.g;
        let wt = &self.search;
        let pw = wt.progress_weight;
        let mut h = 0u32;
        match &g.rules().win {
            WinRule::AllOnFoundations => {
                h += pw * g.cards_to_home().saturating_sub(g.cards_home(s)) as u32;
            }
            _ => {
                if let crate::game::Win::Empty(gs) = &g.win {
                    for &gi in gs {
                        for p in g.groups[gi as usize].piles() {
                            h += pw * s.piles[p as usize].len() as u32;
                        }
                    }
                }
                if let crate::game::Win::Rows(gi, len, _) = g.win {
                    let gr = &g.groups[gi as usize];
                    let rows = gr.count / gr.columns.unwrap_or(gr.count);
                    for row in 0..rows {
                        h += pw * (len as u32).saturating_sub(g.row_prefix(s, gi, row) as u32);
                    }
                }
                if let crate::game::Win::AtMostPiles(gi, n) = g.win {
                    let used = g.groups[gi as usize]
                        .piles()
                        .filter(|&p| !s.piles[p as usize].is_empty())
                        .count();
                    h += pw * (used as u32).saturating_sub(n as u32);
                    if let Some(st) = &g.stock {
                        h += pw * s.piles[st.pile as usize].len() as u32;
                    }
                }
                if let crate::game::Win::Count(gi, n) = g.win {
                    let have: usize = g.groups[gi as usize]
                        .piles()
                        .map(|p| s.piles[p as usize].len())
                        .sum();
                    h += pw * (n as u32).saturating_sub(have as u32);
                }
            }
        }
        for (p, pile) in s.piles.iter().enumerate() {
            let gi = g.pile_group[p] as usize;
            let gr = &g.groups[gi];
            if gr.kind == PileKind::Stock {
                // Cards still to be dealt onto the tableau are work to come; counting them keeps
                // a deal from looking like pure loss.
                if g.stock
                    .as_ref()
                    .is_some_and(|st| st.target_kind == PileKind::Tableau)
                {
                    h += wt.stock_weight * pile.len() as u32;
                }
                continue;
            }
            if gr.kind == PileKind::Foundation {
                continue;
            }
            // Face-down cards are unknowns to dig out.
            let down = pile.iter().take_while(|c| !c.is_face_up()).count();
            h += wt.down_weight * down as u32;
            if gr.kind == PileKind::Cell {
                h += wt.cell_weight * pile.len() as u32;
                continue;
            }
            if gr.kind == PileKind::Reserve {
                // Only penalize cards buried in a stacked reserve; single-card reserve slots
                // (e.g. a bouquet) are exposed and act like free cards.
                h += pile.len().saturating_sub(1) as u32;
            }
            if gr.kind == PileKind::Waste
                && g.compress_talon
                && g.stock
                    .as_ref()
                    .is_some_and(|st| st.redeals != u32::MAX && s.redeals_used >= st.redeals)
            {
                // On the final pass through a waste talon, cards buried below the top of the
                // waste can't come back around and must be dug out.
                h += pile.len().saturating_sub(1) as u32;
            }
            // Cards out of sequence in a building pile.
            let pr = &g.pile_rules[p];
            if gr.kind == PileKind::Tableau
                && !self.builds[p]
                && pr.max_cards == 1
                && gr.refill_from.is_empty()
            {
                // Non-refilling single-card tableau slots act as free cells.
                h += wt.cell_weight * pile.len() as u32;
            }
            if self.builds[p] && gr.kind == PileKind::Tableau && !pile.is_empty() {
                h += wt.disorder_weight;
            }
            if self.builds[p] {
                let suit_runs =
                    pr.moves == crate::rules::MoveUnit::SuitRun || self.suit_complete_runs;
                for w in pile[down..].windows(2) {
                    if pr.fits[w[0].face() as usize] >> w[1].face() & 1 == 0
                        || (suit_runs && w[0].suit() != w[1].suit())
                    {
                        h += wt.disorder_weight;
                    }
                }
            }
            // A card resting on a card that its foundation needs sooner is in the way.
            if let Some(need) = &need {
                let is_dump = pr.build.by == BuildBy::Any
                    && pr.build.dir == crate::rules::Dir::Any
                    && pr.build.blocked_ranks.is_empty();
                let mut soonest = u8::MAX;
                let mut prev_n = u8::MAX;
                for c in pile {
                    let n = need[c.face() as usize];
                    if n > soonest {
                        h += wt.blocker_weight;
                        if is_dump {
                            h += (n - soonest) as u32;
                        }
                    }
                    if is_dump && n > prev_n {
                        h += wt.blocker_weight;
                    }
                    soonest = soonest.min(n);
                    prev_n = n;
                }
            }
        }
        // Penalize consuming finite redeals so best-first doesn't burn passes early.
        if let Some(st) = &g.stock {
            if st.redeals != u32::MAX && s.redeals_used > 0 {
                h += pw * s.redeals_used;
            }
        } else if let Some(rd) = &g.redeal {
            if rd.times != u32::MAX && s.redeals_used > 0 {
                h += pw * s.redeals_used;
            }
        }
        h
    }

    /// For each face, how many plays some foundation needs before it can take that card (255:
    /// none can).
    fn need(&self, s: &State) -> [u8; FACES] {
        let g = self.g;
        let n = g.nranks as i16;
        let mut need = [u8::MAX; FACES];
        let mut empty_group_done = 0u64;
        for &f in &self.ordered_foundations {
            let r = g.rule(f);
            let b = &r.build;
            let step = if b.dir == crate::rules::Dir::Up {
                b.step as i16
            } else {
                -(b.step as i16)
            };
            let advance = |i: i16| -> Option<i16> {
                let j = i + step;
                if (0..n).contains(&j) {
                    Some(j)
                } else if b.wrap {
                    Some(j.rem_euclid(n))
                } else {
                    None
                }
            };
            let (mut next, suit) = match s.piles[f as usize].last() {
                // What a joker stands for isn't worth working out for a heuristic.
                Some(top) if top.is_joker() => continue,
                Some(top) => (
                    advance(g.rank_idx[top.rank() as usize] as i16),
                    (b.by == BuildBy::Suit).then(|| top.suit()),
                ),
                None => {
                    let gi = g.pile_group[f as usize];
                    if g.groups[gi as usize].symmetric {
                        if empty_group_done & (1u64 << gi) != 0 {
                            continue;
                        }
                        empty_group_done |= 1u64 << gi;
                    }
                    let base = match r.empty {
                        crate::rules::EmptyRule::Rank(k) => Some(k),
                        crate::rules::EmptyRule::DealtBase => s.base_rank,
                        crate::rules::EmptyRule::DealtBaseOffset(o) => {
                            s.base_rank.map(|b| g.rank_from(b, o))
                        }
                        _ => None,
                    };
                    (
                        base.map(|k| g.rank_idx[k as usize] as i16)
                            .filter(|&i| i < n),
                        None,
                    )
                }
            };
            let mut d = 0u8;
            while let Some(i) = next {
                // A full foundation takes nothing more.
                if s.piles[f as usize].len() + d as usize >= r.max_cards as usize {
                    break;
                }
                let rank = g.rules().pack.ranks[i as usize];
                if let Some(su) = suit {
                    let face = Card::new(su, rank).face() as usize;
                    if g.in_pack[face] && r.admits >> face & 1 != 0 {
                        need[face] = need[face].min(d);
                    }
                } else {
                    for suit_i in 0..4u8 {
                        let su = crate::card::Suit::from_index(suit_i);
                        let face = Card::new(su, rank).face() as usize;
                        if g.in_pack[face] && r.admits >> face & 1 != 0 {
                            need[face] = need[face].min(d);
                        }
                    }
                }
                d += 1;
                if d as i16 >= n {
                    break;
                }
                next = advance(i);
            }
        }
        need
    }

    // ----- Hashing ---------------------------------------------------------------------------

    fn hash(&mut self, s: &State) -> u128 {
        let g = self.g;
        for (i, p) in s.piles.iter().enumerate() {
            if !self.pile_hash_valid[i] {
                let ph = if g.pile_rules[i].sink && p.len() > 1 {
                    // Only the top card and the count of a pile nothing leaves can matter.
                    finish_prefix(prefix_step(PREFIX_SEED, p[p.len() - 1].to_byte()), p.len())
                } else {
                    let pre = &mut self.prefix[i];
                    pre.truncate(p.len() + 1);
                    while pre.len() <= p.len() {
                        let h = prefix_step(*pre.last().unwrap(), p[pre.len() - 1].to_byte());
                        pre.push(h);
                    }
                    finish_prefix(pre[p.len()], p.len())
                };
                self.pile_hashes[i] = ph;
                self.pile_hash_valid[i] = true;
            }
        }
        let mut h = Hasher::new();
        for (gi, gr) in g.groups.iter().enumerate() {
            let r = gr.start as usize..(gr.start + gr.count) as usize;
            if g.is_group_symmetric(gi, s) && gr.count > 1 {
                let mut sum = 0u128;
                for &ph in &self.pile_hashes[r] {
                    sum = sum.wrapping_add(ph);
                }
                h.write_u128(sum);
            } else if !gr.sym_classes.is_empty() {
                for cls in &gr.sym_classes {
                    if cls.len() > 1 {
                        let mut sum = 0u128;
                        for &p in cls {
                            sum = sum.wrapping_add(self.pile_hashes[p as usize]);
                        }
                        h.write_u128(sum);
                    } else {
                        h.write_u128(self.pile_hashes[cls[0] as usize]);
                    }
                }
            } else {
                for &ph in &self.pile_hashes[r] {
                    h.write_u128(ph);
                }
            }
            h.write_u128(0x5eed);
        }
        h.write_u128(s.redeals_used as u128);
        h.write_u128(s.base_rank.unwrap_or(0) as u128);
        h.finish()
    }

    // ----- Depth-first -----------------------------------------------------------------------

    fn dfs(&mut self, start: &State, budget: u64) -> SolveResult {
        if self.stop.is_stopped_now() {
            return SolveResult {
                outcome: Outcome::GaveUp,
                states: 0,
            };
        }
        let g = self.g;
        let mut s = start.clone();
        let mut states = 1u64;
        let root = self.hash(&s);
        self.seen.insert(root);
        if g.is_won(&s) {
            return SolveResult {
                outcome: Outcome::Solved(Vec::new()),
                states,
            };
        }
        struct Frame {
            start: usize,
            end: usize,
            next: usize,
        }
        let mut kids_buf: Vec<Child> = Vec::new();
        self.push_dfs_children(&mut s, &mut kids_buf);
        let mut frames = vec![Frame {
            start: 0,
            end: kids_buf.len(),
            next: 0,
        }];
        let mut path: Vec<(Move, usize)> = Vec::new();
        while let Some(f) = frames.last_mut() {
            if f.next == f.end {
                let start = f.start;
                frames.pop();
                kids_buf.truncate(start);
                if let Some((_, mark)) = path.pop() {
                    self.unplay(&mut s, mark);
                }
                continue;
            }
            let c = &kids_buf[f.next];
            let (mv, hash, won) = (c.mv, c.hash, c.won);
            f.next += 1;
            if !self.seen.insert(hash) {
                continue;
            }
            let mark = self.play(&mut s, mv);
            states += 1;
            path.push((mv, mark));
            if won {
                return SolveResult {
                    outcome: Outcome::Solved(path.into_iter().map(|(m, _)| m).collect()),
                    states,
                };
            }
            if states >= budget || self.stop.should_stop(states) {
                return SolveResult {
                    outcome: Outcome::GaveUp,
                    states,
                };
            }
            let start = kids_buf.len();
            self.push_dfs_children(&mut s, &mut kids_buf);
            let end = kids_buf.len();
            frames.push(Frame {
                start,
                end,
                next: start,
            });
        }
        SolveResult {
            outcome: Outcome::Unsolvable,
            states,
        }
    }

    // ----- Branch and bound (scores) ---------------------------------------------------------

    /// Depth-first branch and bound for lines ending with at least `need` points, best bound
    /// first, dropping any position whose bound is below `need`; each line found raises `need`
    /// past its score. Taking a node's i-th best child spends i of `slack` (limited
    /// discrepancy search); `complete` is true only if the slack never cut anything off.
    /// With `to_win`, the search instead stops at the first position that wins (a score target
    /// reached at any point).
    fn bounded_dfs(
        &mut self,
        start: &State,
        mut need: u32,
        slack: u32,
        budget: u64,
        to_win: bool,
    ) -> Probe {
        if self.stop.is_stopped_now() {
            return Probe {
                found: None,
                complete: false,
                states: 0,
            };
        }
        let g = self.g;
        let sc = g.score.as_ref().expect("scored game");
        let mut s = start.clone();
        // Slack left when each position was last entered: enter again only with more.
        let mut seen: std::collections::HashMap<u128, u32, BuildU128> =
            std::collections::HashMap::with_hasher(BuildU128);
        let root = self.hash(&s);
        seen.insert(root, slack);
        let mut cut = false;
        let mut states = 1u64;
        let mut found: Option<(u32, Vec<Move>)> = None;
        if to_win && g.is_won(&s) {
            return Probe {
                found: Some((sc.score(&s), Vec::new())),
                complete: true,
                states,
            };
        }
        if sc.bound(g, &s) < need {
            return Probe {
                found,
                complete: true,
                states,
            };
        }
        struct Frame {
            kids: Vec<(Move, u32)>,
            next: usize,
            slack: u32,
        }
        let kids = self.ranked(&mut s);
        if kids.is_empty() {
            let v = sc.score(&s);
            return Probe {
                found: (v >= need).then(|| (v, Vec::new())),
                complete: true,
                states,
            };
        }
        let mut frames = vec![Frame {
            kids,
            next: 0,
            slack,
        }];
        let mut path: Vec<(Move, usize)> = Vec::new();
        while let Some(f) = frames.last_mut() {
            // Children come best bound first: once one falls short, the rest do too.
            let i = f.next as u32;
            if f.next == f.kids.len() || f.kids[f.next].1 < need || i > f.slack {
                if f.next < f.kids.len() && f.kids[f.next].1 >= need {
                    cut = true;
                }
                frames.pop();
                if let Some((_, mark)) = path.pop() {
                    self.unplay(&mut s, mark);
                }
                continue;
            }
            let left = f.slack - i;
            let mv = f.kids[f.next].0;
            f.next += 1;
            let mark = self.play(&mut s, mv);
            let h = self.hash(&s);
            if seen.get(&h).is_some_and(|&had| had >= left) {
                self.unplay(&mut s, mark);
                continue;
            }
            // Entering again with more slack is new work, so it counts.
            states += 1;
            seen.insert(h, left);
            path.push((mv, mark));
            if to_win && g.is_won(&s) {
                found = Some((sc.score(&s), path.iter().map(|&(m, _)| m).collect()));
                break;
            }
            let kids = self.ranked(&mut s);
            if kids.is_empty() {
                // The game is over here.
                let v = sc.score(&s);
                if v >= need {
                    found = Some((v, path.iter().map(|&(m, _)| m).collect()));
                    need = v + 1;
                }
                let (_, mark) = path.pop().unwrap();
                self.unplay(&mut s, mark);
            } else {
                frames.push(Frame {
                    kids,
                    next: 0,
                    slack: left,
                });
            }
            if states >= budget || self.stop.should_stop(states) {
                return Probe {
                    found,
                    complete: false,
                    states,
                };
            }
        }
        Probe {
            found,
            complete: !cut,
            states,
        }
    }

    /// Follow the best-bound move until the game ends.
    fn greedy(&mut self, start: &State) -> (u32, Vec<Move>) {
        let g = self.g;
        let sc = g.score.as_ref().expect("scored game");
        let mut s = start.clone();
        let mut line = Vec::new();
        while let Some(&(mv, _)) = self.ranked(&mut s).first() {
            self.play(&mut s, mv);
            line.push(mv);
        }
        self.journal.clear();
        self.forget_all();
        (sc.score(&s), line)
    }

    // ----- Best-first ------------------------------------------------------------------------

    fn best_first(&mut self, start: &State, budget: u64, weighted: bool) -> SolveResult {
        if self.stop.is_stopped_now() {
            return SolveResult {
                outcome: Outcome::GaveUp,
                states: 0,
            };
        }
        let g = self.g;
        struct Node {
            parent: u32,
            mv: Move,
            depth: u32,
            enc_start: u32,
            enc_len: u16,
        }
        let mut nodes: Vec<Node> = Vec::new();
        let mut enc_arena: Vec<u8> = Vec::new();
        let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
        let root_h = self.hash(start);
        self.seen.insert(root_h);
        if g.is_won(start) {
            return SolveResult {
                outcome: Outcome::Solved(Vec::new()),
                states: 1,
            };
        }
        let root_start = enc_arena.len() as u32;
        encode_into(start, &mut enc_arena);
        let root_len = (enc_arena.len() as u32 - root_start) as u16;
        nodes.push(Node {
            parent: u32::MAX,
            mv: Move::new(Action::Deal),
            depth: 0,
            enc_start: root_start,
            enc_len: root_len,
        });
        heap.push(Reverse((0, 0)));
        let mut states = 1u64;
        let mut s = start.clone();
        while let Some(Reverse((_, idx))) = heap.pop() {
            if self.stop.should_stop(states) {
                return SolveResult {
                    outcome: Outcome::GaveUp,
                    states,
                };
            }
            let node = &nodes[idx as usize];
            let es = node.enc_start as usize;
            let el = node.enc_len as usize;
            decode_into(&enc_arena[es..es + el], &mut s);
            self.forget_all();
            let depth = node.depth;
            let mut won_mv: Option<Move> = None;
            let mut hit_budget = false;
            self.visit_children(&mut s, |ctx, st, mv, need| {
                let hash = ctx.hash(st);
                if !ctx.seen.insert(hash) {
                    return false;
                }
                states += 1;
                if ctx.g.is_won(st) {
                    won_mv = Some(mv);
                    return true;
                }
                let h = ctx.heuristic_with_need(st, need);
                let enc_start = enc_arena.len() as u32;
                encode_into(st, &mut enc_arena);
                let enc_len = (enc_arena.len() as u32 - enc_start) as u16;
                let n = nodes.len() as u32;
                let d = depth + 1;
                nodes.push(Node {
                    parent: idx,
                    mv,
                    depth: d,
                    enc_start,
                    enc_len,
                });
                let pri = if weighted {
                    // Weighted A*: f = 2*h + depth, tie-broken by h and talon offset.
                    let f = (h as u64) * 2 + (d as u64);
                    (f << 28) | ((h.min(0xffff) as u64) << 12) | (mv.talon.min(4095) as u64)
                } else {
                    // Greedy; among equals, deeper first, which crosses plateaus far faster.
                    let score = (h << 8) | (mv.talon.min(255) as u32);
                    ((score as u64) << 20) + (0xfffff - (d as u64).min(0xfffff))
                };
                heap.push(Reverse((pri, n)));
                if states >= budget || ctx.stop.should_stop(states) {
                    hit_budget = true;
                    return true;
                }
                false
            });
            if let Some(mv) = won_mv {
                let mut line = vec![mv];
                let mut at = idx;
                while at != 0 {
                    line.push(nodes[at as usize].mv);
                    at = nodes[at as usize].parent;
                }
                line.reverse();
                return SolveResult {
                    outcome: Outcome::Solved(line),
                    states,
                };
            }
            if hit_budget {
                return SolveResult {
                    outcome: Outcome::GaveUp,
                    states,
                };
            }
        }
        SolveResult {
            outcome: Outcome::Unsolvable,
            states,
        }
    }
}

/// Compact position encoding: per pile, its length then its cards; then the counters.
fn encode_into(s: &State, v: &mut Vec<u8>) {
    for p in &s.piles {
        v.push(p.len() as u8);
        v.extend(p.iter().map(|c| c.to_byte()));
    }
    v.extend_from_slice(&s.redeals_used.to_le_bytes());
    v.push(s.base_rank.unwrap_or(0));
}

fn encode(s: &State) -> Box<[u8]> {
    let n: usize = s.piles.iter().map(|p| p.len() + 1).sum();
    let mut v = Vec::with_capacity(n + 5);
    encode_into(s, &mut v);
    v.into_boxed_slice()
}

fn decode_into(b: &[u8], s: &mut State) {
    let mut i = 0;
    for p in &mut s.piles {
        let n = b[i] as usize;
        i += 1;
        p.clear();
        p.extend(b[i..i + n].iter().map(|&x| card_from_byte(x)));
        i += n;
    }
    s.redeals_used = u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    s.base_rank = if b[i + 4] == 0 { None } else { Some(b[i + 4]) };
    // `seed` never changes during play, so it isn't encoded; `s` keeps the start's.
}

fn card_from_byte(x: u8) -> Card {
    let c = Card::from_face(x & 0x3f);
    if x & 0x80 != 0 {
        c.down()
    } else {
        c
    }
}

// ----- Hashing ---------------------------------------------------------------------------------

const K1: u64 = 0x9e37_79b9_7f4a_7c15;
const K2: u64 = 0xc2b2_ae3d_27d4_eb4f;

#[inline]
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 33)).wrapping_mul(0xff51_afd7_ed55_8ccd);
    z = (z ^ (z >> 33)).wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    z ^ (z >> 33)
}

const PREFIX_SEED: u128 = 0x243f_6a88_85a3_08d3_1319_8a2e_0370_7344;

#[inline]
fn prefix_step(h: u128, c: u8) -> u128 {
    let (hi, lo) = ((h >> 64) as u64, h as u64);
    let a = (lo ^ c as u64).wrapping_mul(K1).rotate_left(29);
    let b = (hi ^ (c as u64) << 9).wrapping_mul(K2).rotate_left(31) ^ a;
    ((b as u128) << 64) | a as u128
}

#[inline]
fn finish_prefix(h: u128, len: usize) -> u128 {
    let (hi, lo) = ((h >> 64) as u64, h as u64);
    ((mix(hi ^ len as u64) as u128) << 64) | mix(lo ^ hi.rotate_left(7) ^ len as u64) as u128
}

struct Hasher {
    a: u64,
    b: u64,
}

impl Hasher {
    fn new() -> Hasher {
        Hasher {
            a: 0xa409_3822_299f_31d0,
            b: 0x082e_fa98_ec4e_6c89,
        }
    }

    #[inline]
    fn write_u128(&mut self, v: u128) {
        let (hi, lo) = ((v >> 64) as u64, v as u64);
        self.a = (self.a ^ hi).wrapping_mul(K1).rotate_left(29) ^ lo;
        self.b = (self.b ^ lo).wrapping_mul(K2).rotate_left(31) ^ hi;
    }

    #[inline]
    fn finish(&self) -> u128 {
        ((mix(self.a) as u128) << 64) | mix(self.b ^ self.a.rotate_left(7)) as u128
    }
}

/// The keys are already hashes: use them directly.
#[derive(Clone, Copy, Default)]
struct BuildU128;

impl std::hash::BuildHasher for BuildU128 {
    type Hasher = PassThrough;
    fn build_hasher(&self) -> PassThrough {
        PassThrough(0)
    }
}

#[derive(Default)]
struct PassThrough(u64);

impl std::hash::Hasher for PassThrough {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = self.0.rotate_left(8) ^ b as u64;
        }
    }
    fn write_u128(&mut self, v: u128) {
        self.0 = (v as u64) ^ ((v >> 64) as u64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::microsoft_deal;
    use crate::presets;
    use std::sync::Arc;

    #[test]
    fn preset_cancel_flag_aborts_immediately_for_all_strategies() {
        let fc = Game::new(presets::freecell()).unwrap();
        let deal = fc.deal_ordered(&microsoft_deal(1)).unwrap();
        let flag = AtomicBool::new(true);

        for strategy in [Strategy::Auto, Strategy::DepthFirst, Strategy::BestFirst] {
            let opts = SolveOptions::default().strategy(strategy);
            let r = solve_with_cancel(&fc, &deal, &opts, &flag);
            assert_eq!(r.outcome, Outcome::GaveUp, "strategy={strategy:?}");
            assert_eq!(r.states, 0, "strategy={strategy:?}");
        }

        // Scored game: maximize_with_cancel returns greedy baseline with proven=false.
        let ps = Game::new(presets::poker_squares()).unwrap();
        let pdeal = ps.deal(1);
        let mr = maximize_with_cancel(&ps, &pdeal, &SolveOptions::default(), &flag);
        assert!(!mr.proven);
        assert_eq!(mr.states, 0);

        // Scored target game: solve_with_cancel returns GaveUp, never Unsolvable.
        let mut target_rules = presets::poker_squares();
        target_rules.win = WinRule::Score { at_least: 500 };
        let target_game = Game::new(target_rules).unwrap();
        let tr = solve_with_cancel(&target_game, &pdeal, &SolveOptions::default(), &flag);
        assert_eq!(tr.outcome, Outcome::GaveUp);
        assert_eq!(tr.states, 0);
    }

    #[test]
    fn mid_search_cancel_never_reports_unsolvable() {
        // FreeCell #11982 is famously unsolvable (~60k states); cancelling mid-flight must yield
        // GaveUp, never Unsolvable.
        let fc = Game::new(presets::freecell()).unwrap();
        let deal = fc.deal_ordered(&microsoft_deal(11982)).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_bg = Arc::clone(&cancel);
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            cancel_bg.store(true, Ordering::Relaxed);
        });
        let opts = SolveOptions::default()
            .strategy(Strategy::DepthFirst)
            .max_states(5_000_000);
        let r = solve_with_cancel(&fc, &deal, &opts, &cancel);
        handle.join().unwrap();
        assert_ne!(r.outcome, Outcome::Solved(Vec::new()));
        // If it finished before the timer on a super-fast machine it could be Unsolvable;
        // if cancelled, it must be GaveUp.
        if cancel.load(Ordering::Relaxed) && r.states < 50_000 {
            assert_eq!(r.outcome, Outcome::GaveUp);
        }
    }

    #[test]
    fn timeout_ms_bounds_search_and_maximize() {
        let fc = Game::new(presets::freecell()).unwrap();
        let deal = fc.deal_ordered(&microsoft_deal(11982)).unwrap();

        // Zero timeout stops immediately.
        let r0 = solve(&fc, &deal, &SolveOptions::default().timeout_ms(0));
        assert_eq!(r0.outcome, Outcome::GaveUp);
        assert_eq!(r0.states, 0);

        // Short wall-clock timeout stops well before exhausting the state budget.
        let r_short = solve(
            &fc,
            &deal,
            &SolveOptions::default()
                .max_states(10_000_000)
                .max_duration(Duration::from_millis(5)),
        );
        assert_eq!(r_short.outcome, Outcome::GaveUp);

        // Scored maximize with timeout_ms=0 returns greedy result with proven=false.
        let ps = Game::new(presets::poker_squares()).unwrap();
        let pdeal = ps.deal(1);
        let mr = maximize(&ps, &pdeal, &SolveOptions::default().timeout_ms(0));
        assert!(!mr.proven);
        assert_eq!(mr.states, 0);

        // JSON serialization round-trips timeout_ms and omits it when None.
        #[cfg(feature = "json")]
        {
            let def_json = serde_json::to_string(&SolveOptions::default()).unwrap();
            assert!(!def_json.contains("timeout_ms"), "{def_json}");
            let with_t = SolveOptions::default().timeout_ms(250);
            let t_json = serde_json::to_string(&with_t).unwrap();
            assert!(t_json.contains("\"timeout_ms\":250"), "{t_json}");
            let parsed: SolveOptions = serde_json::from_str(&t_json).unwrap();
            assert_eq!(parsed.timeout_ms, Some(250));
        }
    }
}
