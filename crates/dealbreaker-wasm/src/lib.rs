//! WebAssembly bindings: a small JSON-in, JSON-out API for JavaScript and TypeScript.
//!
//! ```js
//! import init, { Solver, presets } from "dealbreaker";
//! await init();
//! const solver = Solver.fromPreset("klondike");
//! const state = solver.deal(42);                 // position JSON
//! const result = JSON.parse(solver.solve(state, JSON.stringify({ max_states: 500000 })));
//! // { outcome: "solved" | "unsolvable" | "gave-up", states, steps: [{type: "move", from, to, count}, ...] }
//! ```
//!
//! Positions and steps use the formats documented in the core crate (`StateData`, `Step`).

use dealbreaker::{
    hint, maximize, microsoft_deal, presets, solve, Card, Game, HintOptions, Outcome, Rules,
    SolveOptions, State, Step,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("serializable")
}

/// Presets as JSON: `[{ "id": "klondike", "name": "Klondike" }, …]`.
#[wasm_bindgen]
pub fn presets() -> String {
    let v: Vec<_> = presets::PRESETS
        .iter()
        .map(|p| serde_json::json!({ "id": p.id, "name": (p.rules)().name }))
        .collect();
    to_json(&v)
}

/// One game's compiled rules. Create once, use for many positions.
#[wasm_bindgen]
pub struct Solver {
    game: Game,
}

#[derive(Serialize)]
struct SolveOut {
    outcome: &'static str,
    states: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<Vec<Step>>,
}

#[wasm_bindgen]
impl Solver {
    /// From rules JSON.
    #[wasm_bindgen(constructor)]
    pub fn new(rules_json: &str) -> Result<Solver, JsError> {
        let rules = Rules::from_json(rules_json).map_err(js_err)?;
        Ok(Solver {
            game: Game::new(rules).map_err(js_err)?,
        })
    }

    /// From a preset id (see `presets()`).
    #[wasm_bindgen(js_name = fromPreset)]
    pub fn from_preset(id: &str) -> Result<Solver, JsError> {
        let rules = presets::preset(id).ok_or_else(|| js_err(format!("no preset {id:?}")))?;
        Ok(Solver {
            game: Game::new(rules).map_err(js_err)?,
        })
    }

    /// The rules, as JSON.
    pub fn rules(&self) -> String {
        self.game.rules().to_json()
    }

    /// Deal from a seed (0 to 2^53). Returns position JSON.
    pub fn deal(&self, seed: f64) -> String {
        self.game.state_to_json(&self.game.deal(seed as u64))
    }

    /// Deal Microsoft FreeCell deal `n` (for games with FreeCell's layout). Returns position JSON.
    #[wasm_bindgen(js_name = dealMicrosoft)]
    pub fn deal_microsoft(&self, n: u32) -> Result<String, JsError> {
        let s = self.game.deal_ordered(&microsoft_deal(n)).map_err(js_err)?;
        Ok(self.game.state_to_json(&s))
    }

    /// Deal an explicit card order from a JSON array of card strings (e.g. `["AS", "2S", ...]`).
    /// `cards[0]` is dealt first. Returns position JSON.
    #[wasm_bindgen(js_name = dealOrdered)]
    pub fn deal_ordered(&self, cards_json: &str) -> Result<String, JsError> {
        let names: Vec<String> = serde_json::from_str(cards_json).map_err(js_err)?;
        let cards: Vec<Card> = names
            .iter()
            .map(|c| c.parse::<Card>().map_err(js_err))
            .collect::<Result<_, _>>()?;
        let s = self.game.deal_ordered(&cards).map_err(js_err)?;
        Ok(self.game.state_to_json(&s))
    }

    /// Validate a position JSON against this game's pack and pile capacities. Throws if invalid.
    pub fn validate(&self, state_json: &str) -> Result<(), JsError> {
        self.state(state_json)?;
        Ok(())
    }

    /// Solve a position. `options_json` is optional: `{ "max_states": 2000000, "strategy":
    /// "auto" | "depth_first" | "best_first", "autoplay": true }`.
    /// Returns `{ outcome, states, steps? }`.
    pub fn solve(&self, state_json: &str, options_json: Option<String>) -> Result<String, JsError> {
        let s = self.state(state_json)?;
        let opts: SolveOptions = parse_opts(options_json)?;
        let r = solve(&self.game, &s, &opts);
        let steps = match &r.outcome {
            Outcome::Solved(m) => Some(self.game.steps(&s, m)),
            _ => None,
        };
        Ok(to_json(&SolveOut {
            outcome: r.outcome.label(),
            states: r.states,
            steps,
        }))
    }

    /// For games played for points: the line of play that ends with the most points.
    /// `options_json` as for `solve`. Returns `{ score, proven, states, steps }`.
    pub fn best(&self, state_json: &str, options_json: Option<String>) -> Result<String, JsError> {
        let s = self.state(state_json)?;
        let opts: SolveOptions = parse_opts(options_json)?;
        let r = maximize(&self.game, &s, &opts);
        Ok(to_json(&serde_json::json!({
            "score": r.score,
            "proven": r.proven,
            "states": r.states,
            "steps": self.game.steps(&s, &r.moves),
        })))
    }

    /// Points for a position, or `null` for games not played for points.
    pub fn score(&self, state_json: &str) -> Result<Option<u32>, JsError> {
        Ok(self.game.score(&self.state(state_json)?))
    }

    /// A hint that doesn't peek at face-down cards. `options_json` is optional:
    /// `{ "samples": 16, "seed": 0, "solve": { "max_states": 100000 } }`.
    /// Returns `{ step, wins, samples, score? }` or `null`; `score` (games played for points)
    /// is the step's average best score over the guesses.
    pub fn hint(&self, state_json: &str, options_json: Option<String>) -> Result<String, JsError> {
        let s = self.state(state_json)?;
        let opts: HintOptions = parse_opts(options_json)?;
        Ok(to_json(&hint(&self.game, &s, &opts)))
    }

    /// Every legal step, as a JSON array.
    #[wasm_bindgen(js_name = legalSteps)]
    pub fn legal_steps(&self, state_json: &str) -> Result<String, JsError> {
        let s = self.state(state_json)?;
        Ok(to_json(&self.game.legal_steps(&s)))
    }

    /// Play one step (JSON); returns the new position JSON, or throws if it's illegal.
    #[wasm_bindgen(js_name = playStep)]
    pub fn play_step(&self, state_json: &str, step_json: &str) -> Result<String, JsError> {
        let mut s = self.state(state_json)?;
        let step: Step = serde_json::from_str(step_json).map_err(js_err)?;
        self.game.play_step(&mut s, &step).map_err(js_err)?;
        Ok(self.game.state_to_json(&s))
    }

    #[wasm_bindgen(js_name = isWon)]
    pub fn is_won(&self, state_json: &str) -> Result<bool, JsError> {
        Ok(self.game.is_won(&self.state(state_json)?))
    }

    fn state(&self, json: &str) -> Result<State, JsError> {
        self.game.state_from_json(json).map_err(js_err)
    }
}

fn parse_opts<T: serde::de::DeserializeOwned + Default>(
    json: Option<String>,
) -> Result<T, JsError> {
    match json.as_deref().map(str::trim) {
        None | Some("") => Ok(T::default()),
        Some(j) => serde_json::from_str(j).map_err(js_err),
    }
}
