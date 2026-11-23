//! Shared test helpers.
#![allow(dead_code)]

use std::collections::HashSet;

use dealbreaker::*;

/// Replay a solution move by move through the public, fully-checked `play`, and check it wins.
pub fn assert_wins(g: &Game, start: &State, moves: &[Move]) {
    let mut s = start.clone();
    for (i, &m) in moves.iter().enumerate() {
        g.play(&mut s, m)
            .unwrap_or_else(|e| panic!("{}: move {i} illegal: {e}\n{}", g.name(), g.render(&s)));
        g.validate(&s).unwrap();
    }
    assert!(
        g.is_won(&s),
        "{}: solution doesn't win:\n{}",
        g.name(),
        g.render(&s)
    );
}

/// Exhaustive search over `legal_moves` with exact state equality: no symmetry, no autoplay,
/// no talon compression. `None` if more than `cap` states.
pub fn brute_force(g: &Game, start: &State, cap: usize) -> Option<bool> {
    let mut seen = HashSet::new();
    let mut stack = vec![start.clone()];
    seen.insert(start.clone());
    while let Some(s) = stack.pop() {
        if g.is_won(&s) {
            return Some(true);
        }
        for m in g.legal_moves(&s) {
            let mut t = s.clone();
            g.play(&mut t, m).unwrap();
            if seen.insert(t.clone()) {
                stack.push(t);
            }
        }
        if seen.len() > cap {
            return None;
        }
    }
    Some(false)
}

/// The highest score of any position where the game is over (no legal moves), by exhaustive
/// search with exact state equality. `None` if more than `cap` states.
pub fn brute_force_max(g: &Game, start: &State, cap: usize) -> Option<u32> {
    let mut seen = HashSet::new();
    let mut stack = vec![start.clone()];
    seen.insert(start.clone());
    let mut best = 0;
    while let Some(s) = stack.pop() {
        let moves = g.legal_moves(&s);
        if moves.is_empty() {
            best = best.max(g.score(&s).unwrap());
        }
        for m in moves {
            let mut t = s.clone();
            g.play(&mut t, m).unwrap();
            if seen.insert(t.clone()) {
                stack.push(t);
            }
        }
        if seen.len() > cap {
            return None;
        }
    }
    Some(best)
}
