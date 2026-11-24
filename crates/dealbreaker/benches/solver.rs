//! Reproducible solver benchmarks across representative presets.
//!
//! Run with:
//! ```sh
//! cargo bench -p dealbreaker
//! ```

use std::time::Instant;

use dealbreaker::{maximize, microsoft_deal, presets, solve, Game, Outcome, SolveOptions};

fn bench_freecell_ms(deals: std::ops::RangeInclusive<u32>, max_states: u64) {
    let game = Game::new(presets::freecell()).expect("freecell preset");
    let opts = SolveOptions::default().max_states(max_states);
    let t0 = Instant::now();
    let mut solved = 0u32;
    let mut unsolvable = 0u32;
    let mut total_states = 0u64;
    let count = deals.end() - deals.start() + 1;
    for n in deals {
        let state = game
            .deal_ordered(&microsoft_deal(n))
            .expect("microsoft deal");
        let res = solve(&game, &state, &opts);
        total_states += res.states;
        match res.outcome {
            Outcome::Solved(_) => solved += 1,
            Outcome::Unsolvable => unsolvable += 1,
            Outcome::GaveUp => {}
        }
    }
    let elapsed = t0.elapsed();
    let mean_ms = elapsed.as_secs_f64() * 1000.0 / count as f64;
    println!(
        "{:<24} {:>4} deals | {:>3} solved {:>2} unsolvable | {:>7.2} ms/deal | {:>9} states",
        "freecell (ms #1..#20)", count, solved, unsolvable, mean_ms, total_states
    );
}

fn bench_freecell_11982() {
    let game = Game::new(presets::freecell()).expect("freecell preset");
    let state = game
        .deal_ordered(&microsoft_deal(11982))
        .expect("microsoft deal #11982");
    let opts = SolveOptions::default().max_states(2_000_000);
    let t0 = Instant::now();
    let res = solve(&game, &state, &opts);
    let elapsed = t0.elapsed();
    assert_eq!(res.outcome, Outcome::Unsolvable);
    println!(
        "{:<24} {:>4} deal  | {:>18} | {:>7.2} ms      | {:>9} states",
        "freecell (#11982 proof)",
        1,
        res.outcome.label(),
        elapsed.as_secs_f64() * 1000.0,
        res.states
    );
}

fn bench_preset(label: &str, game: &Game, seeds: u64, max_states: u64) {
    let opts = SolveOptions::default().max_states(max_states);
    let t0 = Instant::now();
    let mut solved = 0u64;
    let mut unsolvable = 0u64;
    let mut total_states = 0u64;
    for seed in 1..=seeds {
        let state = game.deal(seed);
        let res = solve(game, &state, &opts);
        total_states += res.states;
        match res.outcome {
            Outcome::Solved(_) => solved += 1,
            Outcome::Unsolvable => unsolvable += 1,
            Outcome::GaveUp => {}
        }
    }
    let elapsed = t0.elapsed();
    let mean_ms = elapsed.as_secs_f64() * 1000.0 / seeds as f64;
    println!(
        "{:<24} {:>4} deals | {:>3} solved {:>2} unsolvable | {:>7.2} ms/deal | {:>9} states",
        label, seeds, solved, unsolvable, mean_ms, total_states
    );
}

fn bench_poker_squares(seeds: u64, max_states: u64) {
    let game = Game::new(presets::poker_squares()).expect("poker_squares preset");
    let opts = SolveOptions::default().max_states(max_states);
    let t0 = Instant::now();
    let mut total_score = 0u64;
    let mut total_states = 0u64;
    for seed in 1..=seeds {
        let state = game.deal(seed);
        let res = maximize(&game, &state, &opts);
        total_score += u64::from(res.score);
        total_states += res.states;
    }
    let elapsed = t0.elapsed();
    let mean_ms = elapsed.as_secs_f64() * 1000.0 / seeds as f64;
    println!(
        "{:<24} {:>4} deals | mean score {:>7.1} | {:>7.2} ms/deal | {:>9} states",
        "poker_squares (best)",
        seeds,
        total_score as f64 / seeds as f64,
        mean_ms,
        total_states
    );
}

fn main() {
    println!("Dealbreaker solver benchmark (release mode recommended)");
    println!("{}", "-".repeat(86));
    let total_start = Instant::now();

    bench_freecell_ms(1..=20, 500_000);
    bench_freecell_11982();
    bench_preset(
        "klondike1",
        &Game::new(presets::klondike1()).expect("klondike1"),
        10,
        200_000,
    );
    bench_preset(
        "yukon",
        &Game::new(presets::yukon()).expect("yukon"),
        10,
        200_000,
    );
    bench_preset(
        "tripeaks",
        &Game::new(presets::tripeaks()).expect("tripeaks"),
        20,
        200_000,
    );
    bench_poker_squares(5, 50_000);

    println!("{}", "-".repeat(86));
    println!(
        "Total elapsed: {:.2} s",
        total_start.elapsed().as_secs_f64()
    );
}
