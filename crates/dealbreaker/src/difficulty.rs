//! Difficulty grading: is a deal easy, medium or hard?
//!
//! [`grade`] measures a deal with the solver, then compares the measurement with thresholds
//! calibrated for the game, so that roughly a third of the game's winnable deals land in each
//! grade.
//!
//! The measurement has two parts, both taken from the solver's own work:
//!
//! * **Effort**: how many distinct positions the default search visited before it found a win.
//!   Deals where steady, obvious progress wins are found almost at once; deals that need a
//!   long detour or an early, non-obvious commitment make the search back up and look wider.
//! * **Traps**: how unforgiving the deal is. At a handful of positions spread along the winning
//!   line, every legal move is tried and the solver is given a small budget to win from the
//!   position it leads to. A move from which it can't (the position is lost, or recovery takes
//!   more search than the budget) is a trap. The trap rate is the share of moves tried that
//!   are traps.
//!
//! The two are combined into one [`Difficulty::score`]:
//!
//! ```text
//! score = log2(1 + effort) + TRAP_WEIGHT * trap rate
//! ```
//!
//! A deal the solver proves unwinnable, or can't win within the budget, is graded
//! [`Grade::Hard`]; [`Difficulty::winnable`] tells those cases apart.
//!
//! Both parts see every card, face down or not (as [`crate::solve`] does), so they measure
//! the deal, not how much luck a player needs with hidden cards.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::game::{Game, Move, State};
use crate::presets::PRESETS;
use crate::solver::{solve, solve_with_cancel, Outcome, SolveOptions};

/// How much a trap rate of 1 (every move tried is a trap) adds to the score, in the same units
/// as `log2(effort)`: a trap rate of 0.1 counts as much as doubling the effort.
pub const TRAP_WEIGHT: f64 = 10.0;

/// A difficulty grade.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    Easy,
    Medium,
    Hard,
}

impl Grade {
    pub fn label(&self) -> &'static str {
        match self {
            Grade::Easy => "easy",
            Grade::Medium => "medium",
            Grade::Hard => "hard",
        }
    }
}

impl std::fmt::Display for Grade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl std::str::FromStr for Grade {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "easy" => Ok(Grade::Easy),
            "medium" => Ok(Grade::Medium),
            "hard" => Ok(Grade::Hard),
            _ => Err(format!("unknown grade {s:?} (easy, medium or hard)")),
        }
    }
}

/// Score cut-offs between grades: a winnable deal scoring below `medium` is easy, below `hard`
/// is medium, and otherwise hard.
#[derive(Copy, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct GradeThresholds {
    pub medium: f64,
    pub hard: f64,
}

impl GradeThresholds {
    pub fn new(medium: f64, hard: f64) -> GradeThresholds {
        GradeThresholds { medium, hard }
    }

    /// The grade for a winnable deal with this score.
    pub fn grade(&self, score: f64) -> Grade {
        if score < self.medium {
            Grade::Easy
        } else if score < self.hard {
            Grade::Medium
        } else {
            Grade::Hard
        }
    }

    /// Built-in thresholds for a preset id, calibrated with the default [`GradeOptions`].
    pub fn for_preset(id: &str) -> Option<GradeThresholds> {
        CALIBRATION
            .iter()
            .find(|c| c.0 == id)
            .map(|c| GradeThresholds::new(c.1, c.2))
    }

    /// Built-in thresholds for a game whose rules are a preset's (search hints aside), with the
    /// preset's id.
    pub fn for_game(game: &Game) -> Option<(&'static str, GradeThresholds)> {
        let id = preset_id(game)?;
        Some((id, GradeThresholds::for_preset(id)?))
    }

    /// Thresholds for games with no calibration of their own: the median of the presets'.
    pub fn generic() -> GradeThresholds {
        let mut m: Vec<f64> = CALIBRATION.iter().map(|c| c.1).collect();
        let mut h: Vec<f64> = CALIBRATION.iter().map(|c| c.2).collect();
        GradeThresholds::new(median(&mut m), median(&mut h))
    }
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// The id of the preset whose rules `game` has, ignoring search hints (which change only how
/// fast the solver works, never what is legal).
pub fn preset_id(game: &Game) -> Option<&'static str> {
    let mut mine = game.rules().clone();
    mine.search = Default::default();
    PRESETS
        .iter()
        .filter(|p| {
            let r = (p.rules)();
            r.name == mine.name
        })
        .find(|p| {
            let mut r = (p.rules)();
            r.search = Default::default();
            r == mine
        })
        .map(|p| p.id)
}

/// Grading settings.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
#[non_exhaustive]
pub struct GradeOptions {
    /// Budget for the main search, in distinct positions. A deal not won within it is graded
    /// hard.
    pub max_states: u64,
    /// Positions along the winning line at which every move is tried.
    pub probe_positions: u32,
    /// Budget for each move tried, in distinct positions.
    pub probe_states: u64,
    /// Cut-offs to use instead of the game's built-in ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thresholds: Option<GradeThresholds>,
}

impl Default for GradeOptions {
    fn default() -> Self {
        GradeOptions {
            max_states: 500_000,
            probe_positions: 10,
            probe_states: 3_000,
            thresholds: None,
        }
    }
}

impl GradeOptions {
    pub fn max_states(mut self, n: u64) -> Self {
        self.max_states = n;
        self
    }

    pub fn probe_positions(mut self, n: u32) -> Self {
        self.probe_positions = n;
        self
    }

    pub fn probe_states(mut self, n: u64) -> Self {
        self.probe_states = n;
        self
    }

    pub fn thresholds(mut self, t: GradeThresholds) -> Self {
        self.thresholds = Some(t);
        self
    }

    /// Are these the settings the built-in thresholds were calibrated with?
    pub fn is_calibration_default(&self) -> bool {
        let d = GradeOptions::default();
        self.max_states == d.max_states
            && self.probe_positions == d.probe_positions
            && self.probe_states == d.probe_states
    }
}

/// Where the thresholds used for a grade came from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ThresholdSource {
    /// Calibrated for this game (a preset), with the default options.
    Preset,
    /// Given in [`GradeOptions::thresholds`].
    Custom,
    /// No calibration for this game: [`GradeThresholds::generic`].
    Generic,
}

/// The difficulty of a deal, and the measurements behind it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct Difficulty {
    pub grade: Grade,
    /// `Some(true)`: the solver won it. `Some(false)`: proven unwinnable. `None`: the budget
    /// ran out first. Anything but `Some(true)` is graded hard.
    pub winnable: Option<bool>,
    /// `log2(1 + effort) + TRAP_WEIGHT * trap_rate`, for winnable deals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// Distinct positions the main search visited (to its first win, if it found one).
    pub effort: u64,
    /// Moves tried along the winning line.
    pub probes: u32,
    /// Moves tried that the solver couldn't win from within the probe budget.
    pub traps: u32,
    /// `traps / probes` (0 if no move was tried).
    pub trap_rate: f64,
    /// Length of the winning line found, in single steps (0 if none).
    pub steps: usize,
    pub thresholds: GradeThresholds,
    pub source: ThresholdSource,
}

/// The score combining effort and trap rate (see the [module docs](self)).
pub fn score(effort: u64, trap_rate: f64) -> f64 {
    (1.0 + effort as f64).log2() + TRAP_WEIGHT * trap_rate
}

/// Grade a deal (or any position) as easy, medium or hard. See the [module docs](self).
///
/// Deterministic: the same game, position and options always give the same grade.
pub fn grade(game: &Game, state: &State, opts: &GradeOptions) -> Difficulty {
    grade_inner(game, state, opts, None)
}

/// Like [`grade`], stopping early if `cancel` becomes `true`. A cancelled grading reports
/// `winnable: None` and [`Grade::Hard`] if the main search hadn't finished, and counts the
/// remaining probes as traps otherwise; don't rely on its grade.
pub fn grade_with_cancel(
    game: &Game,
    state: &State,
    opts: &GradeOptions,
    cancel: &AtomicBool,
) -> Difficulty {
    grade_inner(game, state, opts, Some(cancel))
}

fn thresholds(game: &Game, opts: &GradeOptions) -> (GradeThresholds, ThresholdSource) {
    if let Some(t) = opts.thresholds {
        return (t, ThresholdSource::Custom);
    }
    match GradeThresholds::for_game(game) {
        Some((_, t)) => (t, ThresholdSource::Preset),
        None => (GradeThresholds::generic(), ThresholdSource::Generic),
    }
}

fn run(game: &Game, s: &State, opts: &SolveOptions, cancel: Option<&AtomicBool>) -> Outcome {
    match cancel {
        Some(c) => solve_with_cancel(game, s, opts, c).outcome,
        None => solve(game, s, opts).outcome,
    }
}

fn grade_inner(
    game: &Game,
    state: &State,
    opts: &GradeOptions,
    cancel: Option<&AtomicBool>,
) -> Difficulty {
    let (thresholds, source) = thresholds(game, opts);
    // Shortening doesn't change effort, and the line only places the probes.
    let main = SolveOptions::default()
        .max_states(opts.max_states)
        .shorten(false);
    let r = match cancel {
        Some(c) => solve_with_cancel(game, state, &main, c),
        None => solve(game, state, &main),
    };
    let mut d = Difficulty {
        grade: Grade::Hard,
        winnable: None,
        score: None,
        effort: r.states,
        probes: 0,
        traps: 0,
        trap_rate: 0.0,
        steps: 0,
        thresholds,
        source,
    };
    let line = match r.outcome {
        Outcome::Solved(line) => line,
        Outcome::Unsolvable => {
            d.winnable = Some(false);
            return d;
        }
        Outcome::GaveUp => return d,
    };
    d.winnable = Some(true);
    d.steps = game.steps(state, &line).len();
    (d.probes, d.traps) = probe(game, state, &line, opts, cancel);
    if d.probes > 0 {
        d.trap_rate = d.traps as f64 / d.probes as f64;
    }
    let sc = score(d.effort, d.trap_rate);
    d.score = Some(sc);
    d.grade = thresholds.grade(sc);
    d
}

/// Try every distinct move at `probe_positions` positions spread evenly along `line`. Returns
/// (moves tried, traps).
fn probe(
    game: &Game,
    start: &State,
    line: &[Move],
    opts: &GradeOptions,
    cancel: Option<&AtomicBool>,
) -> (u32, u32) {
    let k = (opts.probe_positions as usize).min(line.len());
    if k == 0 {
        return (0, 0);
    }
    let sub = SolveOptions::default()
        .max_states(opts.probe_states.max(1))
        .shorten(false);
    let (mut probes, mut traps) = (0u32, 0u32);
    let mut next = 0usize;
    let mut s = start.clone();
    for (i, &m) in line.iter().enumerate() {
        // Positions i * len / k for i in 0..k: evenly spaced, starting with the deal.
        if next < k && i == next * line.len() / k {
            next += 1;
            for mv in game.distinct_moves(&s) {
                let mut c = s.clone();
                if game.play(&mut c, mv).is_err() {
                    continue;
                }
                probes += 1;
                let won = game.is_won(&c)
                    || (!cancel.is_some_and(|c| c.load(Ordering::Relaxed))
                        && run(game, &c, &sub, cancel).is_solved());
                if !won {
                    traps += 1;
                }
            }
        }
        if game.play(&mut s, m).is_err() {
            break;
        }
    }
    (probes, traps)
}

/// The result of [`calibrate`].
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct Calibration {
    /// Cut-offs putting a third of the sampled winnable deals in each grade.
    pub thresholds: GradeThresholds,
    /// Deals sampled.
    pub deals: u32,
    /// Deals won.
    pub winnable: u32,
    /// Deals proven unwinnable.
    pub unwinnable: u32,
    /// Deals the budget ran out on.
    pub unknown: u32,
}

/// Calibrate thresholds from graded deals: cut-offs at the 1/3 and 2/3 quantiles of the
/// winnable deals' scores. `None` if no deal was won.
pub fn calibrate(graded: &[Difficulty]) -> Option<Calibration> {
    let mut scores: Vec<f64> = graded.iter().filter_map(|d| d.score).collect();
    if scores.is_empty() {
        return None;
    }
    scores.sort_by(f64::total_cmp);
    let q = |f: f64| {
        // Midpoint between neighbouring samples, so the cut falls between deals.
        let x = f * scores.len() as f64;
        let i = (x.ceil() as usize).clamp(1, scores.len()) - 1;
        let j = (i + 1).min(scores.len() - 1);
        (scores[i] + scores[j]) / 2.0
    };
    Some(Calibration {
        thresholds: GradeThresholds::new(q(1.0 / 3.0), q(2.0 / 3.0)),
        deals: graded.len() as u32,
        winnable: scores.len() as u32,
        unwinnable: graded.iter().filter(|d| d.winnable == Some(false)).count() as u32,
        unknown: graded.iter().filter(|d| d.winnable.is_none()).count() as u32,
    })
}

/// Built-in calibration: (preset id, medium cut-off, hard cut-off). Generated by
/// `scripts/calibrate-difficulty.sh` with the default [`GradeOptions`], from deals 1001..1200
/// (1001..2000 for games where few deals are won). Accordion has no entry: almost no deal is
/// won within the default budget, so there is nothing to calibrate from.
const CALIBRATION: &[(&str, f64, f64)] = &[
    ("klondike", 9.10, 10.84),
    ("klondike-draw3", 9.24, 11.17),
    ("klondike-vegas", 13.33, 15.95),
    ("klondike-jokers", 10.98, 12.04),
    ("freecell", 9.24, 10.88),
    ("bakers-game", 9.53, 11.13),
    ("eight-off", 8.37, 10.50),
    ("seahaven-towers", 9.84, 13.12),
    ("spider", 22.28, 24.95),
    ("spider-2suit", 14.74, 18.83),
    ("spider-1suit", 9.75, 10.93),
    ("spiderette", 16.96, 21.18),
    ("scorpion", 8.10, 9.92),
    ("yukon", 8.28, 10.66),
    ("russian", 7.23, 7.56),
    ("forty-thieves", 15.91, 19.43),
    ("gypsy", 12.03, 18.00),
    ("bakers-dozen", 11.53, 19.08),
    ("beleaguered-castle", 12.84, 19.10),
    ("streets-and-alleys", 15.24, 20.73),
    ("fortress", 9.56, 10.15),
    ("canfield", 8.78, 10.88),
    ("golf", 21.79, 24.17),
    ("tripeaks", 6.60, 11.30),
    ("pyramid", 7.14, 9.79),
    ("aces-up", 13.45, 18.36),
    ("calculation", 16.61, 21.45),
    ("sir-tommy", 18.13, 21.01),
    ("bisley", 9.87, 16.07),
    ("elevens", 6.07, 6.25),
    ("nestor", 9.99, 12.36),
    ("gaps-single-deal", 21.39, 23.12),
    ("montana", 18.07, 20.90),
    ("cruel", 8.41, 8.92),
    ("monte-carlo", 6.87, 7.61),
    ("la-belle-lucie", 13.46, 20.89),
    ("poker-squares", 16.37, 23.97),
    ("cribbage-squares", 7.53, 14.05),
];
