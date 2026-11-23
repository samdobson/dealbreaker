//! Minimal C ABI bindings for Dealbreaker.
//!
//! See `include/dealbreaker.h` for the C header and ownership rules. Every `extern "C"` entry
//! point catches Rust panics and reports errors via thread-local [`dealbreaker_last_error`].

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use dealbreaker::{
    hint, hint_with_cancel, maximize, maximize_with_cancel, microsoft_deal, presets, solve,
    solve_with_cancel, Card, Game, HintOptions, Outcome, Rules, SolveOptions, State, Step,
};
use serde::Serialize;

/// Opaque compiled game handle.
pub struct DealbreakerSolver {
    game: Game,
}

/// Opaque cooperative cancellation flag.
pub struct DealbreakerCancel {
    flag: AtomicBool,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn clear_error() {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
}

fn set_error(msg: impl Into<String>) {
    let s = msg.into().replace('\0', " ");
    let c = CString::new(s).unwrap_or_else(|_| CString::new("unknown error").unwrap());
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(c));
}

fn into_c_string(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(c) => c.into_raw(),
        Err(_) => {
            set_error("output contained interior NUL byte");
            ptr::null_mut()
        }
    }
}

unsafe fn read_str<'a>(p: *const c_char, name: &str) -> Result<&'a str, String> {
    if p.is_null() {
        return Err(format!("{name} must not be NULL"));
    }
    CStr::from_ptr(p)
        .to_str()
        .map_err(|e| format!("{name} is not valid UTF-8: {e}"))
}

unsafe fn read_opt_str<'a>(p: *const c_char, name: &str) -> Result<Option<&'a str>, String> {
    if p.is_null() {
        return Ok(None);
    }
    let s = CStr::from_ptr(p)
        .to_str()
        .map_err(|e| format!("{name} is not valid UTF-8: {e}"))?
        .trim();
    Ok((!s.is_empty()).then_some(s))
}

unsafe fn solver_ref<'a>(p: *const DealbreakerSolver) -> Result<&'a Game, String> {
    p.as_ref()
        .map(|s| &s.game)
        .ok_or_else(|| "solver must not be NULL".to_string())
}

fn parse_opts<T: serde::de::DeserializeOwned + Default>(raw: Option<&str>) -> Result<T, String> {
    match raw {
        None => Ok(T::default()),
        Some(j) => serde_json::from_str(j).map_err(|e| e.to_string()),
    }
}

fn parse_state(game: &Game, json: &str) -> Result<State, String> {
    game.state_from_json(json).map_err(|e| e.to_string())
}

fn to_json<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("serializable")
}

fn run_ptr<T>(f: impl FnOnce() -> Result<*mut T, String>) -> *mut T {
    clear_error();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            set_error(e);
            ptr::null_mut()
        }
        Err(_) => {
            set_error("internal panic");
            ptr::null_mut()
        }
    }
}

#[derive(Serialize)]
struct SolveOut {
    outcome: &'static str,
    states: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<Vec<Step>>,
}

/// Free a string returned by any `dealbreaker_*` function.
///
/// # Safety
/// `s` must be NULL or a pointer previously returned by this library and not yet freed.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_string_free(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

/// Return the last error message on the current thread, or NULL if none.
#[no_mangle]
pub extern "C" fn dealbreaker_last_error() -> *const c_char {
    LAST_ERROR.with(|e| match &*e.borrow() {
        Some(c) => c.as_ptr(),
        None => ptr::null(),
    })
}

/// Create a new cancellation flag.
#[no_mangle]
pub extern "C" fn dealbreaker_cancel_new() -> *mut DealbreakerCancel {
    Box::into_raw(Box::new(DealbreakerCancel {
        flag: AtomicBool::new(false),
    }))
}

/// Trigger cancellation on `cancel`. Safe to call from another thread.
///
/// # Safety
/// `cancel` must be NULL or a valid pointer from [`dealbreaker_cancel_new`].
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_cancel_trigger(cancel: *const DealbreakerCancel) {
    if let Some(c) = cancel.as_ref() {
        c.flag.store(true, Ordering::Relaxed);
    }
}

/// Free a cancellation flag.
///
/// # Safety
/// `cancel` must be NULL or a pointer from [`dealbreaker_cancel_new`] not yet freed.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_cancel_free(cancel: *mut DealbreakerCancel) {
    if !cancel.is_null() {
        let _ = Box::from_raw(cancel);
    }
}

/// Return built-in presets as JSON.
#[no_mangle]
pub extern "C" fn dealbreaker_presets() -> *mut c_char {
    run_ptr(|| {
        let v: Vec<_> = presets::PRESETS
            .iter()
            .map(|p| serde_json::json!({ "id": p.id, "name": (p.rules)().name }))
            .collect();
        Ok(into_c_string(to_json(&v)))
    })
}

/// Compile a solver from rules JSON.
///
/// # Safety
/// `rules_json` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_new(
    rules_json: *const c_char,
) -> *mut DealbreakerSolver {
    run_ptr(|| {
        let raw = read_str(rules_json, "rules_json")?;
        let rules = Rules::from_json(raw).map_err(|e| e.to_string())?;
        let game = Game::new(rules).map_err(|e| e.to_string())?;
        Ok(Box::into_raw(Box::new(DealbreakerSolver { game })))
    })
}

/// Compile a solver from a built-in preset id.
///
/// # Safety
/// `preset_id` must be a valid NUL-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_from_preset(
    preset_id: *const c_char,
) -> *mut DealbreakerSolver {
    run_ptr(|| {
        let id = read_str(preset_id, "preset_id")?;
        let rules = presets::preset(id).ok_or_else(|| format!("no preset {id:?}"))?;
        let game = Game::new(rules).map_err(|e| e.to_string())?;
        Ok(Box::into_raw(Box::new(DealbreakerSolver { game })))
    })
}

/// Free a solver handle.
///
/// # Safety
/// `solver` must be NULL or a pointer from `dealbreaker_solver_*` not yet freed.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_free(solver: *mut DealbreakerSolver) {
    if !solver.is_null() {
        let _ = Box::from_raw(solver);
    }
}

/// Return this game's rules as JSON.
///
/// # Safety
/// `solver` must be a valid pointer from `dealbreaker_solver_*`.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_rules(solver: *const DealbreakerSolver) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        Ok(into_c_string(game.rules().to_json()))
    })
}

/// Deal a seeded position as JSON.
///
/// # Safety
/// `solver` must be a valid pointer from `dealbreaker_solver_*`.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_deal(
    solver: *const DealbreakerSolver,
    seed: u64,
) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        Ok(into_c_string(game.state_to_json(&game.deal(seed))))
    })
}

/// Deal Microsoft FreeCell deal `n` as JSON.
///
/// # Safety
/// `solver` must be a valid pointer from `dealbreaker_solver_*`.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_deal_microsoft(
    solver: *const DealbreakerSolver,
    n: u32,
) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        let s = game
            .deal_ordered(&microsoft_deal(n))
            .map_err(|e| e.to_string())?;
        Ok(into_c_string(game.state_to_json(&s)))
    })
}

/// Deal an explicit card order from a JSON array of card strings.
///
/// # Safety
/// `solver` and `cards_json` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_deal_ordered(
    solver: *const DealbreakerSolver,
    cards_json: *const c_char,
) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        let raw = read_str(cards_json, "cards_json")?;
        let names: Vec<String> = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        let cards: Vec<Card> = names
            .iter()
            .map(|c| c.parse::<Card>().map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        let s = game.deal_ordered(&cards).map_err(|e| e.to_string())?;
        Ok(into_c_string(game.state_to_json(&s)))
    })
}

/// Validate a position JSON. Returns 0 if valid, -1 on error.
///
/// # Safety
/// `solver` and `state_json` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_validate(
    solver: *const DealbreakerSolver,
    state_json: *const c_char,
) -> i32 {
    clear_error();
    match catch_unwind(AssertUnwindSafe(|| {
        let game = solver_ref(solver)?;
        let raw = read_str(state_json, "state_json")?;
        parse_state(game, raw)?;
        Ok::<(), String>(())
    })) {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => {
            set_error(e);
            -1
        }
        Err(_) => {
            set_error("internal panic");
            -1
        }
    }
}

/// Solve a position. `options_json` and `cancel` may be NULL.
///
/// # Safety
/// `solver` and `state_json` must be valid; `options_json` and `cancel` must be NULL or valid.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_solve(
    solver: *const DealbreakerSolver,
    state_json: *const c_char,
    options_json: *const c_char,
    cancel: *const DealbreakerCancel,
) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        let s = parse_state(game, read_str(state_json, "state_json")?)?;
        let opts: SolveOptions = parse_opts(read_opt_str(options_json, "options_json")?)?;
        let r = match cancel.as_ref() {
            Some(c) => solve_with_cancel(game, &s, &opts, &c.flag),
            None => solve(game, &s, &opts),
        };
        let steps = match &r.outcome {
            Outcome::Solved(m) => Some(game.steps(&s, m)),
            _ => None,
        };
        Ok(into_c_string(to_json(&SolveOut {
            outcome: r.outcome.label(),
            states: r.states,
            steps,
        })))
    })
}

/// Maximize score in a points game. `options_json` and `cancel` may be NULL.
///
/// # Safety
/// `solver` and `state_json` must be valid; `options_json` and `cancel` must be NULL or valid.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_best(
    solver: *const DealbreakerSolver,
    state_json: *const c_char,
    options_json: *const c_char,
    cancel: *const DealbreakerCancel,
) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        let s = parse_state(game, read_str(state_json, "state_json")?)?;
        let opts: SolveOptions = parse_opts(read_opt_str(options_json, "options_json")?)?;
        let r = match cancel.as_ref() {
            Some(c) => maximize_with_cancel(game, &s, &opts, &c.flag),
            None => maximize(game, &s, &opts),
        };
        Ok(into_c_string(to_json(&serde_json::json!({
            "score": r.score,
            "proven": r.proven,
            "states": r.states,
            "steps": game.steps(&s, &r.moves),
        }))))
    })
}

/// Compute a fair hint without peeking at face-down cards. `options_json` and `cancel` may be NULL.
///
/// # Safety
/// `solver` and `state_json` must be valid; `options_json` and `cancel` must be NULL or valid.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_hint(
    solver: *const DealbreakerSolver,
    state_json: *const c_char,
    options_json: *const c_char,
    cancel: *const DealbreakerCancel,
) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        let s = parse_state(game, read_str(state_json, "state_json")?)?;
        let opts: HintOptions = parse_opts(read_opt_str(options_json, "options_json")?)?;
        let h = match cancel.as_ref() {
            Some(c) => hint_with_cancel(game, &s, &opts, &c.flag),
            None => hint(game, &s, &opts),
        };
        Ok(into_c_string(to_json(&h)))
    })
}

/// Return all legal steps as a JSON array.
///
/// # Safety
/// `solver` and `state_json` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_legal_steps(
    solver: *const DealbreakerSolver,
    state_json: *const c_char,
) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        let s = parse_state(game, read_str(state_json, "state_json")?)?;
        Ok(into_c_string(to_json(&game.legal_steps(&s))))
    })
}

/// Play one step (JSON) and return the new position JSON.
///
/// # Safety
/// `solver`, `state_json`, and `step_json` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_play_step(
    solver: *const DealbreakerSolver,
    state_json: *const c_char,
    step_json: *const c_char,
) -> *mut c_char {
    run_ptr(|| {
        let game = solver_ref(solver)?;
        let mut s = parse_state(game, read_str(state_json, "state_json")?)?;
        let step: Step =
            serde_json::from_str(read_str(step_json, "step_json")?).map_err(|e| e.to_string())?;
        game.play_step(&mut s, &step).map_err(|e| e.to_string())?;
        Ok(into_c_string(game.state_to_json(&s)))
    })
}

/// Return 1 if won, 0 if not won, -1 on error.
///
/// # Safety
/// `solver` and `state_json` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_is_won(
    solver: *const DealbreakerSolver,
    state_json: *const c_char,
) -> i32 {
    clear_error();
    match catch_unwind(AssertUnwindSafe(|| {
        let game = solver_ref(solver)?;
        let s = parse_state(game, read_str(state_json, "state_json")?)?;
        Ok::<bool, String>(game.is_won(&s))
    })) {
        Ok(Ok(true)) => 1,
        Ok(Ok(false)) => 0,
        Ok(Err(e)) => {
            set_error(e);
            -1
        }
        Err(_) => {
            set_error("internal panic");
            -1
        }
    }
}

/// Return score (>= 0), -1 if game is not scored, or -2 on error.
///
/// # Safety
/// `solver` and `state_json` must be valid pointers.
#[no_mangle]
pub unsafe extern "C" fn dealbreaker_solver_score(
    solver: *const DealbreakerSolver,
    state_json: *const c_char,
) -> i64 {
    clear_error();
    match catch_unwind(AssertUnwindSafe(|| {
        let game = solver_ref(solver)?;
        let s = parse_state(game, read_str(state_json, "state_json")?)?;
        Ok::<Option<u32>, String>(game.score(&s))
    })) {
        Ok(Ok(Some(v))) => v as i64,
        Ok(Ok(None)) => -1,
        Ok(Err(e)) => {
            set_error(e);
            -2
        }
        Err(_) => {
            set_error("internal panic");
            -2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn take_str(p: *mut c_char) -> String {
        assert!(!p.is_null(), "unexpected NULL; err={:?}", last_err());
        let s = CStr::from_ptr(p).to_str().unwrap().to_string();
        dealbreaker_string_free(p);
        s
    }

    unsafe fn last_err() -> Option<String> {
        let p = dealbreaker_last_error();
        (!p.is_null()).then(|| CStr::from_ptr(p).to_str().unwrap().to_string())
    }

    #[test]
    fn ffi_end_to_end_and_cancellation() {
        unsafe {
            let presets_s = take_str(dealbreaker_presets());
            assert!(presets_s.contains("\"freecell\""));

            let fc_id = CString::new("freecell").unwrap();
            let solver = dealbreaker_solver_from_preset(fc_id.as_ptr());
            assert!(!solver.is_null());

            let rules_s = take_str(dealbreaker_solver_rules(solver));
            let rules_c = CString::new(rules_s).unwrap();
            let solver2 = dealbreaker_solver_new(rules_c.as_ptr());
            assert!(!solver2.is_null());
            dealbreaker_solver_free(solver2);

            let deal_s = take_str(dealbreaker_solver_deal_microsoft(solver, 1));
            let deal_c = CString::new(deal_s).unwrap();
            assert_eq!(dealbreaker_solver_validate(solver, deal_c.as_ptr()), 0);
            assert_eq!(dealbreaker_solver_is_won(solver, deal_c.as_ptr()), 0);
            assert_eq!(dealbreaker_solver_score(solver, deal_c.as_ptr()), -1);

            let legal_s = take_str(dealbreaker_solver_legal_steps(solver, deal_c.as_ptr()));
            let legal: Vec<serde_json::Value> = serde_json::from_str(&legal_s).unwrap();
            assert!(!legal.is_empty());

            // Pre-triggered cancellation returns gave-up with 0 states.
            let cancel = dealbreaker_cancel_new();
            dealbreaker_cancel_trigger(cancel);
            let cancelled_s = take_str(dealbreaker_solver_solve(
                solver,
                deal_c.as_ptr(),
                ptr::null(),
                cancel,
            ));
            let cancelled: serde_json::Value = serde_json::from_str(&cancelled_s).unwrap();
            assert_eq!(cancelled["outcome"], "gave-up");
            assert_eq!(cancelled["states"], 0);
            dealbreaker_cancel_free(cancel);

            // Solve and replay every step.
            let opts_c = CString::new(r#"{"max_states":200000}"#).unwrap();
            let solved_s = take_str(dealbreaker_solver_solve(
                solver,
                deal_c.as_ptr(),
                opts_c.as_ptr(),
                ptr::null(),
            ));
            let solved: serde_json::Value = serde_json::from_str(&solved_s).unwrap();
            assert_eq!(solved["outcome"], "solved");

            let mut cur = deal_c;
            for step in solved["steps"].as_array().unwrap() {
                let step_c = CString::new(step.to_string()).unwrap();
                let next_s = take_str(dealbreaker_solver_play_step(
                    solver,
                    cur.as_ptr(),
                    step_c.as_ptr(),
                ));
                cur = CString::new(next_s).unwrap();
            }
            assert_eq!(dealbreaker_solver_is_won(solver, cur.as_ptr()), 1);

            // Invalid preset sets last_error.
            let bad = CString::new("nope").unwrap();
            assert!(dealbreaker_solver_from_preset(bad.as_ptr()).is_null());
            assert!(last_err().unwrap().contains("no preset"));

            dealbreaker_solver_free(solver);

            // Scored game best & score.
            let ps_id = CString::new("poker-squares").unwrap();
            let ps = dealbreaker_solver_from_preset(ps_id.as_ptr());
            let ps_deal_c = CString::new(take_str(dealbreaker_solver_deal(ps, 1))).unwrap();
            assert_eq!(dealbreaker_solver_score(ps, ps_deal_c.as_ptr()), 0);
            let ps_opts = CString::new(r#"{"max_states":2000}"#).unwrap();
            let best_s = take_str(dealbreaker_solver_best(
                ps,
                ps_deal_c.as_ptr(),
                ps_opts.as_ptr(),
                ptr::null(),
            ));
            let best: serde_json::Value = serde_json::from_str(&best_s).unwrap();
            assert!(best["score"].as_u64().unwrap() > 0);
            dealbreaker_solver_free(ps);
        }
    }
}
