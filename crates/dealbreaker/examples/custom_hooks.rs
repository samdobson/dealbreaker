//! Attaching custom Rust [`Hooks`] to a data-driven game.
//!
//! Run with:
//! ```sh
//! cargo run --release --example custom_hooks
//! ```

use std::sync::Arc;

use dealbreaker::{presets, solve, Game, Hooks, Outcome, PileKind, SolveOptions, State};

/// A house-rule hook for FreeCell: only Kings may be moved into an empty tableau column.
#[derive(Debug)]
struct KingsOnlyEmptyTableau;

impl Hooks for KingsOnlyEmptyTableau {
    fn accepts(
        &self,
        game: &Game,
        state: &State,
        from: u16,
        to: u16,
        count: usize,
    ) -> Option<bool> {
        if game.pile_kind(to) == PileKind::Tableau && state.piles[to as usize].is_empty() {
            let src = &state.piles[from as usize];
            let bottom = src[src.len() - count];
            if bottom.rank() != 13 {
                return Some(false);
            }
        }
        None
    }

    fn keeps_pruning_sound(&self) -> bool {
        // Every tableau pile still behaves identically, foundation plays remain safe,
        // and FreeCell has no stock/waste.
        true
    }
}

fn main() {
    let game = Game::with_hooks(presets::freecell(), Arc::new(KingsOnlyEmptyTableau))
        .expect("freecell rules with hooks should compile");
    let deal = game.deal(1);
    let result = solve(&game, &deal, &SolveOptions::default().max_states(500_000));
    println!(
        "FreeCell (Kings-only empty columns), seed 1: {} ({} states)",
        result.outcome.label(),
        result.states
    );
    if let Outcome::Solved(moves) = &result.outcome {
        println!("Found a {}-move solution.", moves.len());
    }
}
