//! Fair hints that respect hidden face-down cards.
//!
//! Run with:
//! ```sh
//! cargo run --release --example fair_hint
//! ```

use dealbreaker::{hint, presets, Game, HintOptions, SolveOptions};

fn main() {
    let game = Game::new(presets::klondike1()).expect("klondike rules should compile");
    let deal = game.deal(7);

    let opts = HintOptions::default()
        .samples(16)
        .seed(42)
        .solve(SolveOptions::default().max_states(50_000));

    match hint(&game, &deal, &opts) {
        Some(h) => {
            println!(
                "Recommended step: {} (won {}/{} hidden-card samples)",
                h.step, h.wins, h.samples
            );
        }
        None => {
            println!("No winning hint found within the sample budget.");
        }
    }
}
