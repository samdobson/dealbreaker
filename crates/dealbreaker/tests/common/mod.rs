//! Shared test helpers.
#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};

use dealbreaker::*;

struct Semaphore {
    state: Mutex<usize>,
    cvar: Condvar,
}

impl Semaphore {
    fn new(permits: usize) -> Self {
        Self {
            state: Mutex::new(permits.max(1)),
            cvar: Condvar::new(),
        }
    }

    fn acquire(&self) -> Permit<'_> {
        let mut avail = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while *avail == 0 {
            avail = self.cvar.wait(avail).unwrap_or_else(|e| e.into_inner());
        }
        *avail -= 1;
        Permit { sem: self }
    }
}

struct Permit<'a> {
    sem: &'a Semaphore,
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        let mut avail = self.sem.state.lock().unwrap_or_else(|e| e.into_inner());
        *avail += 1;
        self.sem.cvar.notify_one();
    }
}

fn global_semaphore() -> &'static Semaphore {
    static SEM: OnceLock<Semaphore> = OnceLock::new();
    SEM.get_or_init(|| {
        let cpus = std::thread::available_parallelism().map_or(4, |n| n.get());
        Semaphore::new(cpus)
    })
}

/// Run `f` over `items` in parallel, bounded by the global CPU-count semaphore so nested
/// `#[test]` parallelism never oversubscribes cores or memory.
pub fn parallel_map<T: Sync, R: Send, F: Fn(&T) -> R + Sync>(items: &[T], f: F) -> Vec<R> {
    if items.len() <= 1 {
        return items.iter().map(f).collect();
    }
    let cpus = std::thread::available_parallelism().map_or(4, |n| n.get());
    let workers = items.len().min(cpus);
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<R>>> = (0..items.len()).map(|_| Mutex::new(None)).collect();
    let sem = global_semaphore();

    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let idx = next.fetch_add(1, Ordering::Relaxed);
                if idx >= items.len() {
                    break;
                }
                let _permit = sem.acquire();
                let val = f(&items[idx]);
                *results[idx].lock().unwrap() = Some(val);
            });
        }
    });

    results
        .into_iter()
        .map(|slot| slot.into_inner().unwrap().unwrap())
        .collect()
}

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
