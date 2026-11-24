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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    #[derive(Debug)]
    struct DefaultHooks;
    impl Hooks for DefaultHooks {}

    #[test]
    fn default_hooks_defer_to_data_rules() {
        let g = Game::with_hooks(presets::freecell(), std::sync::Arc::new(DefaultHooks)).unwrap();
        let s = g.deal(1);
        let h = DefaultHooks;
        assert_eq!(h.accepts(&g, &s, 0, 1, 1), None);
        assert_eq!(h.movable(&g, &s, 0), None);
        assert_eq!(h.is_won(&g, &s), None);
        assert!(h.auto_moves(&g, &s).is_empty());
        assert!(!h.keeps_pruning_sound());
        assert!(!g.is_won(&s));
    }

    #[test]
    fn custom_hooks_override_win_accepts_movable_and_auto_moves() {
        use crate::card::Suit;
        use crate::rules::{BuildBy, DealStep, Dir, GroupRules, MoveUnit, Pack, PileKind, Rules};
        use crate::solver::{solve, Outcome, SolveOptions};

        /// Forces all transfers from tableau to foundation to happen via `auto_moves`,
        /// caps `movable` to 1, and declares a win once foundation has 2 cards.
        #[derive(Debug)]
        struct EarlyWinHooks;

        impl Hooks for EarlyWinHooks {
            fn accepts(
                &self,
                game: &Game,
                _state: &State,
                _from: u16,
                to: u16,
                _count: usize,
            ) -> Option<bool> {
                // Disallow manual transfers onto foundation; auto_moves handles them.
                (game.pile_kind(to) == PileKind::Foundation).then_some(false)
            }

            fn movable(&self, _game: &Game, state: &State, pile: u16) -> Option<usize> {
                Some(usize::from(!state.piles[pile as usize].is_empty()))
            }

            fn is_won(&self, game: &Game, state: &State) -> Option<bool> {
                let f = game.pile_index("foundation", 0).unwrap() as usize;
                Some(state.piles[f].len() >= 2)
            }

            fn auto_moves(&self, game: &Game, state: &State) -> Vec<(u16, u16, u16)> {
                let f = game.pile_index("foundation", 0).unwrap();
                // Symmetrically promote the top card of any tableau pile reaching 3 cards.
                for i in 0..2 {
                    let t = game.pile_index("tableau", i).unwrap();
                    if state.piles[t as usize].len() >= 3 {
                        return vec![(t, f, 1)];
                    }
                }
                Vec::new()
            }

            fn keeps_pruning_sound(&self) -> bool {
                true
            }
        }

        let r = Rules::new(
            "hook-test",
            Pack::new(&[Suit::Spades], &[1, 2, 3, 4]),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 1),
                GroupRules::new("tableau", PileKind::Tableau, 2)
                    .build(BuildBy::Any, Dir::Either)
                    .moves(MoveUnit::Top),
            ],
            vec![DealStep::to("tableau").each(2)],
        );
        let g = Game::with_hooks(r, std::sync::Arc::new(EarlyWinHooks)).unwrap();
        let ordered: Vec<crate::card::Card> = ["AS", "2S", "3S", "4S"]
            .iter()
            .map(|c| c.parse().unwrap())
            .collect();
        let s = g.deal_ordered(&ordered).unwrap();
        assert!(!g.is_won(&s));

        // Direct move to foundation is rejected by `accepts`.
        let f = g.pile_index("foundation", 0).unwrap();
        let t0 = g.pile_index("tableau", 0).unwrap();
        assert!(!g.is_legal(
            &s,
            crate::game::Move::new(crate::game::Action::Transfer {
                from: t0,
                to: f,
                count: 1,
            })
        ));

        // Moving between tableau piles triggers `auto_moves` and reaches `is_won`.
        let res = solve(&g, &s, &SolveOptions::default().max_states(1_000));
        let Outcome::Solved(moves) = res.outcome else {
            panic!(
                "expected EarlyWinHooks game to solve, got {:?}",
                res.outcome
            );
        };
        let mut cur = s;
        for &m in &moves {
            g.play(&mut cur, m).unwrap();
        }
        assert!(g.is_won(&cur));
    }
}
