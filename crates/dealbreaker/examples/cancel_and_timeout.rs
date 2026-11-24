//! Cooperative cancellation and wall-clock timeouts.
//!
//! Run with:
//! ```sh
//! cargo run --release --example cancel_and_timeout
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use dealbreaker::{microsoft_deal, presets, solve, solve_with_cancel, Game, SolveOptions};

fn main() {
    let game = Game::new(presets::freecell()).expect("freecell rules should compile");
    // Microsoft FreeCell deal #11982 is famously unsolvable.
    let hard_deal = game
        .deal_ordered(&microsoft_deal(11982))
        .expect("valid Microsoft deal");

    // 1. Wall-clock timeout via SolveOptions::max_duration.
    let timed = solve(
        &game,
        &hard_deal,
        &SolveOptions::default()
            .max_states(10_000_000)
            .max_duration(Duration::from_millis(25)),
    );
    println!(
        "Timeout (25 ms) on FreeCell #11982: {} ({} states)",
        timed.outcome.label(),
        timed.states
    );

    // 2. Cooperative cancellation from another thread via AtomicBool.
    // A cancelled search never reports Outcome::Unsolvable.
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_bg = Arc::clone(&cancel);
    let stopper = thread::spawn(move || {
        thread::sleep(Duration::from_millis(25));
        cancel_bg.store(true, Ordering::Relaxed);
    });

    let cancelled = solve_with_cancel(
        &game,
        &hard_deal,
        &SolveOptions::default().max_states(10_000_000),
        &cancel,
    );
    stopper.join().expect("stopper thread should finish");

    println!(
        "Cancelled (25 ms) on FreeCell #11982: {} ({} states)",
        cancelled.outcome.label(),
        cancelled.states
    );
}
