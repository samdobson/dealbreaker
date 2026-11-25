//! Hints that respect hidden cards.
//!
//! [`crate::solve`] sees every card. A player doesn't, so a hint computed from the true deal
//! could leak where a face-down card is. [`hint`] instead treats every face-down card as
//! unknown: it deals the unknown cards at random among the face-down places many times, solves
//! each guess, and recommends the first step that wins most often.
//!
//! In a game played for points, every possible next step is tried on every guess instead, and
//! the step with the highest average best score is recommended.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::game::{Game, State, Step};
use crate::rng::Rng;
use crate::solver::{
    maximize, maximize_with_cancel, solve, solve_with_cancel, Outcome, SolveOptions,
};

/// Hint settings.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default)]
#[non_exhaustive]
pub struct HintOptions {
    /// Random guesses at the hidden cards (ignored when nothing is hidden).
    pub samples: u32,
    /// Search options for each guess. Its `max_states` is the budget for each guess.
    pub solve: SolveOptions,
    pub seed: u64,
}

impl Default for HintOptions {
    fn default() -> Self {
        HintOptions {
            samples: 16,
            solve: SolveOptions {
                max_states: 100_000,
                ..SolveOptions::default()
            },
            seed: 0,
        }
    }
}

impl HintOptions {
    /// Random guesses at the hidden cards.
    pub fn samples(mut self, n: u32) -> Self {
        self.samples = n;
        self
    }

    /// Search options for each guess.
    pub fn solve(mut self, opts: SolveOptions) -> Self {
        self.solve = opts;
        self
    }

    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}

/// A recommended next step.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct Hint {
    pub step: Step,
    /// Guesses in which this step started a win.
    pub wins: u32,
    /// Guesses tried.
    pub samples: u32,
    /// Games played for points: the step's average best score over the guesses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

/// Recommend a next step without looking at face-down cards. `None` if no guess could be won
/// within the budget.
pub fn hint(game: &Game, state: &State, opts: &HintOptions) -> Option<Hint> {
    hint_inner(game, state, opts, None)
}

/// Like [`hint`], stopping early if `cancel` becomes `true`.
///
/// Checked between samples and periodically inside each underlying search. If cancelled after
/// some samples have already completed, returns the best recommendation from completed samples
/// (or `None` if none succeeded).
pub fn hint_with_cancel(
    game: &Game,
    state: &State,
    opts: &HintOptions,
    cancel: &AtomicBool,
) -> Option<Hint> {
    hint_inner(game, state, opts, Some(cancel))
}

fn hint_inner(
    game: &Game,
    state: &State,
    opts: &HintOptions,
    cancel: Option<&AtomicBool>,
) -> Option<Hint> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return None;
    }
    let hidden: Vec<(usize, usize)> = state
        .piles
        .iter()
        .enumerate()
        .flat_map(|(p, pile)| {
            pile.iter()
                .enumerate()
                .filter(|(_, c)| !c.is_face_up())
                .map(move |(i, _)| (p, i))
        })
        .collect();
    // A shuffling redeal makes the future unknown even with every card face up.
    let random_future = game.rules().redeal.as_ref().is_some_and(|r| {
        matches!(
            r.how,
            crate::rules::RedealHow::Shuffle { .. }
                | crate::rules::RedealHow::ShuffleTopUp { .. }
                | crate::rules::RedealHow::ShuffleRedeal
                | crate::rules::RedealHow::GridShuffle
                | crate::rules::RedealHow::GridShuffleRandomGaps
                | crate::rules::RedealHow::GridShuffleEndGaps
        )
    }) || game
        .rules()
        .stock
        .as_ref()
        .is_some_and(|st| st.redeal_shuffle);
    let samples = if hidden.is_empty() && !random_future {
        1
    } else {
        opts.samples.max(1)
    };
    let mut rng = Rng::new(opts.seed);
    let guess = |rng: &mut Rng| {
        let mut guess = state.clone();
        // Future shuffles are unknown too.
        if random_future {
            guess.seed = rng.next_u64();
        }
        let mut cards: Vec<_> = hidden.iter().map(|&(p, i)| state.piles[p][i]).collect();
        rng.shuffle(&mut cards);
        for (&(p, i), c) in hidden.iter().zip(cards) {
            guess.piles[p][i] = c;
        }
        guess
    };
    if game.is_scored() {
        return scored_hint(game, state, opts, samples, cancel, || guess(&mut rng));
    }
    let mut votes: HashMap<String, (Step, u32)> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut tried = 0u32;
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        const CHUNK: u32 = 256;
        let mut remaining = samples;
        while remaining > 0 {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                break;
            }
            let batch = remaining.min(CHUNK);
            remaining -= batch;
            let guesses: Vec<State> = (0..batch).map(|_| guess(&mut rng)).collect();
            let outcomes: Vec<Option<Option<Step>>> = guesses
                .par_iter()
                .map(|g| {
                    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                        return None;
                    }
                    let r = match cancel {
                        Some(c) => solve_with_cancel(game, g, &opts.solve, c),
                        None => solve(game, g, &opts.solve),
                    };
                    let step = match r.outcome {
                        Outcome::Solved(moves) => game.steps(g, &moves).into_iter().next(),
                        _ => None,
                    };
                    Some(step)
                })
                .collect();
            for item in outcomes {
                let Some(step_opt) = item else {
                    continue;
                };
                tried += 1;
                if let Some(step) = step_opt {
                    let key = format!("{step:?}");
                    let e = votes.entry(key.clone()).or_insert_with(|| {
                        order.push(key);
                        (step, 0)
                    });
                    e.1 += 1;
                }
            }
        }
    }
    #[cfg(not(feature = "parallel"))]
    for _ in 0..samples {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            break;
        }
        let guess = guess(&mut rng);
        let r = match cancel {
            Some(c) => solve_with_cancel(game, &guess, &opts.solve, c),
            None => solve(game, &guess, &opts.solve),
        };
        tried += 1;
        if let Outcome::Solved(moves) = r.outcome {
            if let Some(step) = game.steps(&guess, &moves).into_iter().next() {
                let key = format!("{step:?}");
                let e = votes.entry(key.clone()).or_insert_with(|| {
                    order.push(key);
                    (step, 0)
                });
                e.1 += 1;
            }
        }
    }
    // Most wins; earliest found breaks ties.
    let best = order.iter().max_by_key(|k| {
        (
            votes[*k].1,
            std::cmp::Reverse(order.iter().position(|o| o == *k)),
        )
    })?;
    let (step, wins) = votes.remove(best)?;
    Some(Hint {
        step,
        wins,
        samples: tried,
        score: None,
    })
}

/// Try every distinct next step on every guess; recommend the best average score. `wins`
/// counts guesses where the step's best line reached the game's target score, if it has one.
fn scored_hint(
    game: &Game,
    state: &State,
    opts: &HintOptions,
    samples: u32,
    cancel: Option<&AtomicBool>,
    mut guess: impl FnMut() -> State,
) -> Option<Hint> {
    let moves = game.distinct_moves(state);
    match moves.as_slice() {
        [] => return None,
        // Nothing to choose between.
        [m] => {
            return Some(Hint {
                step: game.steps(state, &[*m]).into_iter().next()?,
                wins: 0,
                samples: 0,
                score: None,
            })
        }
        _ => {}
    }
    let per = SolveOptions {
        max_states: (opts.solve.max_states / moves.len() as u64).max(1),
        ..opts.solve.clone()
    };
    let mut totals = vec![(0u64, 0u32); moves.len()];
    let mut completed = 0u32;
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        const CHUNK: u32 = 256;
        let mut remaining = samples;
        while remaining > 0 {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                break;
            }
            let batch = remaining.min(CHUNK);
            remaining -= batch;
            let guesses: Vec<State> = (0..batch).map(|_| guess()).collect();
            let results: Vec<Option<Vec<(u64, u32)>>> = guesses
                .par_iter()
                .map(|g| {
                    let mut scores = vec![(0u64, 0u32); moves.len()];
                    for (i, &m) in moves.iter().enumerate() {
                        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                            return None;
                        }
                        let mut s = g.clone();
                        if game.play(&mut s, m).is_err() {
                            continue;
                        }
                        let r = match cancel {
                            Some(c) => maximize_with_cancel(game, &s, &per, c),
                            None => maximize(game, &s, &per),
                        };
                        scores[i].0 = r.score as u64;
                        if game.target_score().is_some_and(|t| r.score >= t) {
                            scores[i].1 = 1;
                        }
                    }
                    Some(scores)
                })
                .collect();
            for scores in results.into_iter().flatten() {
                for (t, &(sc, w)) in totals.iter_mut().zip(&scores) {
                    t.0 += sc;
                    t.1 += w;
                }
                completed += 1;
            }
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut sample_scores = vec![(0u64, 0u32); moves.len()];
        'samples: for _ in 0..samples {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                break;
            }
            let g = guess();
            sample_scores.fill((0, 0));
            for (i, &m) in moves.iter().enumerate() {
                if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                    break 'samples;
                }
                let mut s = g.clone();
                if game.play(&mut s, m).is_err() {
                    continue;
                }
                let r = match cancel {
                    Some(c) => maximize_with_cancel(game, &s, &per, c),
                    None => maximize(game, &s, &per),
                };
                sample_scores[i].0 = r.score as u64;
                if game.target_score().is_some_and(|t| r.score >= t) {
                    sample_scores[i].1 = 1;
                }
            }
            for (t, &(sc, w)) in totals.iter_mut().zip(&sample_scores) {
                t.0 += sc;
                t.1 += w;
            }
            completed += 1;
        }
    }
    if completed == 0 {
        return None;
    }
    // Highest total; the earliest step breaks ties.
    let (i, &(total, wins)) = totals
        .iter()
        .enumerate()
        .max_by_key(|&(i, &(t, _))| (t, std::cmp::Reverse(i)))?;
    let step = game.steps(state, &[moves[i]]).into_iter().next()?;
    Some(Hint {
        step,
        wins,
        samples: completed,
        score: Some(total as f64 / completed as f64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    #[test]
    fn hint_random_future_and_budget_exhaustion() {
        // La Belle Lucie has a shuffling redeal, so even with all cards face up it samples.
        let g = Game::new(presets::la_belle_lucie()).unwrap();
        let s = g.deal(1);
        let opts = HintOptions::default()
            .samples(2)
            .seed(7)
            .solve(SolveOptions::default().max_states(5_000));
        if let Some(h) = hint(&g, &s, &opts) {
            assert_eq!(h.samples, 2);
            assert!(h.wins >= 1);
        }

        // Tiny budget on an unsolved position yields None.
        let fc = Game::new(presets::freecell()).unwrap();
        let fs = fc.deal(1);
        let tiny = HintOptions::default().solve(SolveOptions::default().max_states(1));
        assert!(hint(&fc, &fs, &tiny).is_none());
    }

    #[test]
    fn scored_hint_edge_cases() {
        let g = Game::new(presets::poker_squares()).unwrap();
        let mut s = g.deal(1);
        // Play 25 cards to leave 0 legal moves.
        let mut plays = 0;
        while plays < 25 {
            let moves = g.distinct_moves(&s);
            if moves.is_empty() {
                break;
            }
            g.play(&mut s, moves[0]).unwrap();
            plays += 1;
        }
        assert!(hint(&g, &s, &HintOptions::default()).is_none());

        // Back up by 1 card (24 placed -> only 1 empty cell left, so 1 distinct move).
        let mut s24 = g.deal(1);
        for _ in 0..24 {
            let m = g.distinct_moves(&s24)[0];
            g.play(&mut s24, m).unwrap();
        }
        let h = hint(&g, &s24, &HintOptions::default()).expect("single forced move");
        assert_eq!(h.samples, 0);
        assert_eq!(h.score, None);
    }

    #[test]
    fn hint_with_cancel_stops_immediately() {
        let cancel = AtomicBool::new(true);
        let kl = Game::new(presets::klondike1()).unwrap();
        let ks = kl.deal(1);
        assert!(hint_with_cancel(&kl, &ks, &HintOptions::default(), &cancel).is_none());

        let ps = Game::new(presets::poker_squares()).unwrap();
        let pstate = ps.deal(1);
        assert!(hint_with_cancel(&ps, &pstate, &HintOptions::default(), &cancel).is_none());
    }

    #[test]
    fn hint_is_deterministic_across_repeated_calls() {
        let kl = Game::new(presets::klondike1()).unwrap();
        let ks = kl.deal(2);
        let opts = HintOptions::default()
            .samples(4)
            .seed(42)
            .solve(SolveOptions::default().max_states(10_000));
        let h1 = hint(&kl, &ks, &opts);
        let h2 = hint(&kl, &ks, &opts);
        assert_eq!(h1, h2);
    }

    #[test]
    fn scored_hint_is_deterministic_across_repeated_calls() {
        let ps = Game::new(presets::poker_squares()).unwrap();
        let mut s = ps.deal(5);
        for i in 0..3 {
            let to = ps.pile_index("tableau", i * 6).unwrap();
            let from = ps.pile_index("waste", 0).unwrap();
            ps.play(
                &mut s,
                crate::game::Move::new(crate::game::Action::Transfer { from, to, count: 1 }),
            )
            .unwrap();
        }
        let opts = HintOptions::default()
            .samples(4)
            .seed(11)
            .solve(SolveOptions::default().max_states(2_000));
        let h1 = hint(&ps, &s, &opts);
        let h2 = hint(&ps, &s, &opts);
        assert_eq!(h1, h2);
        assert!(h1.is_some());
    }
}
