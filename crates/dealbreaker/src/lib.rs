//! # dealbreaker
//!
//! A solver for patience (solitaire) card games, where a game is **data**, not code.
//!
//! Describe a game with [`Rules`]: its pack, its groups of piles and what each accepts and lets
//! go, how the stock deals, the opening deal, any card-removal mechanic, and the win condition.
//! [`Game::new`] compiles the rules; [`solve`] searches a position for a win, or proves there
//! is none.
//!
//! ```
//! use dealbreaker::{presets, solve, Game, Outcome, SolveOptions};
//!
//! let game = Game::new(presets::freecell()).unwrap();
//! let deal = game.deal(42);
//! let result = solve(&game, &deal, &SolveOptions::default());
//! if let Outcome::Solved(moves) = &result.outcome {
//!     for step in game.steps(&deal, moves) {
//!         println!("{step}");
//!     }
//! }
//! ```
//!
//! The [`presets`] module has 50 built-in games written in the same model, and
//! every preset round-trips through JSON, so rules can live in files.
//!
//! ## What the solver assumes
//!
//! * It sees every card, including face-down ones: it solves the deal ("thoughtful"
//!   solitaire). Use [`hint()`] for advice that doesn't peek.
//! * "Unsolvable" means every distinct reachable position was searched. Positions are compared
//!   by 128-bit hash, so a false "unsolvable" needs a hash collision.
//! * Automatic actions (turning up exposed cards, refilling from a reserve) happen after every
//!   move, as the rules say.

pub mod card;
pub mod game;
pub mod hint;
pub mod hooks;
pub mod io;
pub mod presets;
pub mod rng;
pub mod rules;
#[cfg(feature = "schema")]
pub mod schema;
mod score;
pub mod solver;

pub use card::{Card, Color, Suit};
pub use game::{Action, Game, IllegalMove, IllegalStep, Move, PileRef, RulesError, State, Step};
pub use hint::{hint, hint_with_cancel, Hint, HintOptions};
pub use hooks::Hooks;
pub use io::{microsoft_deal, StateData, StateError};
pub use rules::*;
pub use solver::{
    maximize, maximize_with_cancel, solve, solve_with_cancel, MaxResult, Outcome, SolveOptions,
    SolveResult, Strategy,
};
