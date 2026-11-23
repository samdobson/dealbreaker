//! PyO3 native extension module for Dealbreaker.
//!
//! All long-running search methods (`solve`, `best`, `hint`) release the Python GIL via
//! `Python::allow_threads`, allowing concurrent searches across Python threads.

use dealbreaker::{
    hint, maximize, microsoft_deal, presets, solve, Card, Game, HintOptions, Outcome, Rules,
    SolveOptions, State, Step, Strategy,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use serde::Serialize;

fn py_err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("serializable")
}

fn parse_strategy(s: &str) -> PyResult<Strategy> {
    match s {
        "auto" => Ok(Strategy::Auto),
        "depth_first" | "depth-first" | "dfs" => Ok(Strategy::DepthFirst),
        "best_first" | "best-first" | "bfs" => Ok(Strategy::BestFirst),
        other => Err(py_err(format!("unknown strategy {other:?}"))),
    }
}

fn build_solve_opts(
    max_states: u64,
    strategy: &str,
    timeout_ms: Option<u64>,
    autoplay: bool,
    shorten: bool,
) -> PyResult<SolveOptions> {
    let mut opts = SolveOptions::default()
        .max_states(max_states)
        .strategy(parse_strategy(strategy)?)
        .autoplay(autoplay)
        .shorten(shorten);
    if let Some(ms) = timeout_ms {
        opts = opts.timeout_ms(ms);
    }
    Ok(opts)
}

#[derive(Serialize)]
struct SolveOut {
    outcome: &'static str,
    states: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<Vec<Step>>,
}

/// Return all built-in presets as a JSON string: `[{"id": "...", "name": "..."}, ...]`.
#[pyfunction]
fn presets_json() -> String {
    let v: Vec<_> = presets::PRESETS
        .iter()
        .map(|p| serde_json::json!({ "id": p.id, "name": (p.rules)().name }))
        .collect();
    to_json(&v)
}

#[pyclass(name = "_Solver")]
struct NativeSolver {
    game: Game,
}

impl NativeSolver {
    fn parse_state(&self, state_json: &str) -> PyResult<State> {
        self.game.state_from_json(state_json).map_err(py_err)
    }
}

#[pymethods]
impl NativeSolver {
    #[new]
    fn new(rules_json: &str) -> PyResult<Self> {
        let rules = Rules::from_json(rules_json).map_err(py_err)?;
        Ok(NativeSolver {
            game: Game::new(rules).map_err(py_err)?,
        })
    }

    #[staticmethod]
    fn from_preset(id: &str) -> PyResult<Self> {
        let rules = presets::preset(id).ok_or_else(|| py_err(format!("no preset {id:?}")))?;
        Ok(NativeSolver {
            game: Game::new(rules).map_err(py_err)?,
        })
    }

    fn rules_json(&self) -> String {
        self.game.rules().to_json()
    }

    fn deal_json(&self, seed: u64) -> String {
        self.game.state_to_json(&self.game.deal(seed))
    }

    fn deal_microsoft_json(&self, n: u32) -> PyResult<String> {
        let s = self.game.deal_ordered(&microsoft_deal(n)).map_err(py_err)?;
        Ok(self.game.state_to_json(&s))
    }

    fn deal_ordered_json(&self, cards: Vec<String>) -> PyResult<String> {
        let parsed: Vec<Card> = cards
            .iter()
            .map(|c| c.parse::<Card>().map_err(py_err))
            .collect::<PyResult<_>>()?;
        let s = self.game.deal_ordered(&parsed).map_err(py_err)?;
        Ok(self.game.state_to_json(&s))
    }

    fn validate_json(&self, state_json: &str) -> PyResult<()> {
        self.parse_state(state_json)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (state_json, max_states=2_000_000, strategy="auto", timeout_ms=None, autoplay=true, shorten=true))]
    fn solve_json(
        &self,
        py: Python<'_>,
        state_json: &str,
        max_states: u64,
        strategy: &str,
        timeout_ms: Option<u64>,
        autoplay: bool,
        shorten: bool,
    ) -> PyResult<String> {
        let s = self.parse_state(state_json)?;
        let opts = build_solve_opts(max_states, strategy, timeout_ms, autoplay, shorten)?;
        let game = &self.game;
        let out = py.allow_threads(|| {
            let r = solve(game, &s, &opts);
            let steps = match &r.outcome {
                Outcome::Solved(m) => Some(game.steps(&s, m)),
                _ => None,
            };
            to_json(&SolveOut {
                outcome: r.outcome.label(),
                states: r.states,
                steps,
            })
        });
        Ok(out)
    }

    #[pyo3(signature = (state_json, max_states=2_000_000, strategy="auto", timeout_ms=None))]
    fn best_json(
        &self,
        py: Python<'_>,
        state_json: &str,
        max_states: u64,
        strategy: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let s = self.parse_state(state_json)?;
        let opts = build_solve_opts(max_states, strategy, timeout_ms, true, true)?;
        let game = &self.game;
        let out = py.allow_threads(|| {
            let r = maximize(game, &s, &opts);
            to_json(&serde_json::json!({
                "score": r.score,
                "proven": r.proven,
                "states": r.states,
                "steps": game.steps(&s, &r.moves),
            }))
        });
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (state_json, samples=16, seed=0, max_states=100_000, strategy="auto", timeout_ms=None, autoplay=true, shorten=true))]
    fn hint_json(
        &self,
        py: Python<'_>,
        state_json: &str,
        samples: u32,
        seed: u64,
        max_states: u64,
        strategy: &str,
        timeout_ms: Option<u64>,
        autoplay: bool,
        shorten: bool,
    ) -> PyResult<String> {
        let s = self.parse_state(state_json)?;
        let solve_opts = build_solve_opts(max_states, strategy, timeout_ms, autoplay, shorten)?;
        let opts = HintOptions::default()
            .samples(samples)
            .seed(seed)
            .solve(solve_opts);
        let game = &self.game;
        let out = py.allow_threads(|| to_json(&hint(game, &s, &opts)));
        Ok(out)
    }

    fn legal_steps_json(&self, state_json: &str) -> PyResult<String> {
        let s = self.parse_state(state_json)?;
        Ok(to_json(&self.game.legal_steps(&s)))
    }

    fn play_step_json(&self, state_json: &str, step_json: &str) -> PyResult<String> {
        let mut s = self.parse_state(state_json)?;
        let step: Step = serde_json::from_str(step_json).map_err(py_err)?;
        self.game.play_step(&mut s, &step).map_err(py_err)?;
        Ok(self.game.state_to_json(&s))
    }

    fn is_won_json(&self, state_json: &str) -> PyResult<bool> {
        Ok(self.game.is_won(&self.parse_state(state_json)?))
    }

    fn score_json(&self, state_json: &str) -> PyResult<Option<u32>> {
        Ok(self.game.score(&self.parse_state(state_json)?))
    }
}

#[pymodule]
fn _dealbreaker(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(presets_json, m)?)?;
    m.add_class::<NativeSolver>()?;
    Ok(())
}
