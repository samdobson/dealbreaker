#ifndef DEALBREAKER_H
#define DEALBREAKER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Opaque compiled game rules handle. Thread-safe for concurrent read/search calls.
 * Must outlive all concurrent calls using this handle; do not call `dealbreaker_solver_free`
 * until every in-flight call on other threads has returned.
 */
typedef struct DealbreakerSolver DealbreakerSolver;

/**
 * Opaque cooperative cancellation flag. Safe to trigger from another thread.
 * Must outlive all concurrent searches using this flag; do not call `dealbreaker_cancel_free`
 * until every search referencing it has returned.
 */
typedef struct DealbreakerCancel DealbreakerCancel;

/** Free a string returned by any `dealbreaker_*` function. Passing NULL is a no-op. */
void dealbreaker_string_free(char *s);

/**
 * Return the last error message on the current thread, or NULL if none.
 * The returned pointer is owned by Dealbreaker and valid until the next FFI call on this thread.
 * Do NOT pass it to `dealbreaker_string_free`.
 */
const char *dealbreaker_last_error(void);

/** Create a new cancellation flag (initially false). Free with `dealbreaker_cancel_free`. */
DealbreakerCancel *dealbreaker_cancel_new(void);

/** Signal cancellation to any search sharing this flag. Safe to call from any thread. */
void dealbreaker_cancel_trigger(const DealbreakerCancel *cancel);

/** Free a cancellation flag. Passing NULL is a no-op. */
void dealbreaker_cancel_free(DealbreakerCancel *cancel);

/** Return built-in presets as JSON: `[{"id":"...","name":"..."}, ...]`. Caller frees result. */
char *dealbreaker_presets(void);

/** Compile a solver from rules JSON. Returns NULL on error (see `dealbreaker_last_error`). */
DealbreakerSolver *dealbreaker_solver_new(const char *rules_json);

/** Compile a solver from a built-in preset id. Returns NULL on error. */
DealbreakerSolver *dealbreaker_solver_from_preset(const char *preset_id);

/** Free a solver handle. Passing NULL is a no-op. */
void dealbreaker_solver_free(DealbreakerSolver *solver);

/** Return this game's rules as JSON. Caller frees result. */
char *dealbreaker_solver_rules(const DealbreakerSolver *solver);

/** Deal a seeded position as JSON. Caller frees result. */
char *dealbreaker_solver_deal(const DealbreakerSolver *solver, uint64_t seed);

/** Deal Microsoft FreeCell deal `n` as JSON. Returns NULL on error. Caller frees result. */
char *dealbreaker_solver_deal_microsoft(const DealbreakerSolver *solver, uint32_t n);

/** Deal an explicit card order from a JSON array of card strings. Returns NULL on error. */
char *dealbreaker_solver_deal_ordered(const DealbreakerSolver *solver, const char *cards_json);

/** Validate a position JSON. Returns 0 if valid, -1 on error (see `dealbreaker_last_error`). */
int32_t dealbreaker_solver_validate(const DealbreakerSolver *solver, const char *state_json);

/**
 * Solve a position. `options_json` and `cancel` may be NULL.
 * Returns JSON `{ "outcome", "states", "steps"? }`, or NULL on error. Caller frees result.
 */
char *dealbreaker_solver_solve(
    const DealbreakerSolver *solver,
    const char *state_json,
    const char *options_json,
    const DealbreakerCancel *cancel
);

/**
 * Maximize score in a points game. `options_json` and `cancel` may be NULL.
 * Returns JSON `{ "score", "proven", "states", "steps" }`, or NULL on error. Caller frees result.
 */
char *dealbreaker_solver_best(
    const DealbreakerSolver *solver,
    const char *state_json,
    const char *options_json,
    const DealbreakerCancel *cancel
);

/**
 * Compute a fair hint without peeking at face-down cards. `options_json` and `cancel` may be NULL.
 * Returns JSON `{ "step", "wins", "samples", "score"? }` or `"null"`, or NULL on error.
 */
char *dealbreaker_solver_hint(
    const DealbreakerSolver *solver,
    const char *state_json,
    const char *options_json,
    const DealbreakerCancel *cancel
);

/** Return all legal steps as a JSON array, or NULL on error. Caller frees result. */
char *dealbreaker_solver_legal_steps(const DealbreakerSolver *solver, const char *state_json);

/** Play one step (JSON) and return the new position JSON, or NULL on error. Caller frees result. */
char *dealbreaker_solver_play_step(
    const DealbreakerSolver *solver,
    const char *state_json,
    const char *step_json
);

/** Return 1 if won, 0 if not won, -1 on error (see `dealbreaker_last_error`). */
int32_t dealbreaker_solver_is_won(const DealbreakerSolver *solver, const char *state_json);

/** Return score (>= 0), -1 if game is not scored, or -2 on error (see `dealbreaker_last_error`). */
int64_t dealbreaker_solver_score(const DealbreakerSolver *solver, const char *state_json);

#ifdef __cplusplus
}
#endif

#endif /* DEALBREAKER_H */
