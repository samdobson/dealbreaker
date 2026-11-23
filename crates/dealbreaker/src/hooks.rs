//! An escape hatch for rules the data model can't express.
//!
//! Most games are pure data. A game with one odd rule ("a king may only go to an empty pile in
//! the top row", "the game is also won when every pile is in sequence") can keep its data rules
//! and attach [`Hooks`] for the odd part. Each hook returns `None` to leave the data rules in
//! charge, so a hook only has to know about the cases it changes.
//!
//! Hooks are Rust only: they don't travel in JSON.
//!
//! The solver's pruning (interchangeable piles, safe autoplay, folding stock deals into moves)
//! relies on properties of the data rules that a hook could break, so attaching hooks turns all
//! of it off unless [`Hooks::keeps_pruning_sound`] says otherwise. That keeps "unsolvable"
//! verdicts honest at some cost in speed.

use std::fmt::Debug;

use crate::game::{Game, State};

/// Rules written as code. Every method has a default that defers to the data rules.
pub trait Hooks: Send + Sync + Debug {
    /// May the top `count` cards of pile `from` go onto pile `to`? `Some` overrides the data
    /// rules either way (after basic checks: the piles differ, the cards exist, nothing covers
    /// either pile).
    fn accepts(
        &self,
        _game: &Game,
        _state: &State,
        _from: u16,
        _to: u16,
        _count: usize,
    ) -> Option<bool> {
        None
    }

    /// How many cards from the top of `pile` may move together. `Some(0)`: none.
    fn movable(&self, _game: &Game, _state: &State, _pile: u16) -> Option<usize> {
        None
    }

    /// Has the game been won?
    fn is_won(&self, _game: &Game, _state: &State) -> Option<bool> {
        None
    }

    /// Moves made automatically after every move, as `(from, to, count)` transfers. Called
    /// repeatedly until it returns nothing, so it must eventually stop asking.
    fn auto_moves(&self, _game: &Game, _state: &State) -> Vec<(u16, u16, u16)> {
        Vec::new()
    }

    /// Return true only if these hooks never make two piles of an interchangeable group behave
    /// differently, never make a foundation play costly, and never depend on the stock or
    /// waste. Then the solver keeps its pruning on.
    fn keeps_pruning_sound(&self) -> bool {
        false
    }
}
