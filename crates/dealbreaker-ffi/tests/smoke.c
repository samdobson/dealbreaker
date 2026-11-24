#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "dealbreaker.h"

int main(void) {
    char *presets = dealbreaker_presets();
    assert(presets != NULL);
    assert(strstr(presets, "freecell") != NULL);
    dealbreaker_string_free(presets);

    DealbreakerSolver *solver = dealbreaker_solver_from_preset("freecell");
    assert(solver != NULL);

    char *deal = dealbreaker_solver_deal_microsoft(solver, 1);
    assert(deal != NULL);
    assert(dealbreaker_solver_validate(solver, deal) == 0);
    assert(dealbreaker_solver_is_won(solver, deal) == 0);

    char *res = dealbreaker_solver_solve(solver, deal, "{\"max_states\":200000}", NULL);
    assert(res != NULL);
    assert(strstr(res, "\"solved\"") != NULL);

    char *hint = dealbreaker_solver_hint(solver, deal, "{\"samples\":1}", NULL);
    assert(hint != NULL);
    assert(strstr(hint, "\"step\"") != NULL);

    DealbreakerCancel *cancel = dealbreaker_cancel_new();
    assert(cancel != NULL);
    dealbreaker_cancel_trigger(cancel);
    char *cancelled = dealbreaker_solver_solve(solver, deal, NULL, cancel);
    assert(cancelled != NULL);
    assert(strstr(cancelled, "\"gave-up\"") != NULL);
    dealbreaker_cancel_free(cancel);

    assert(dealbreaker_solver_from_preset("no-such-preset") == NULL);
    assert(dealbreaker_last_error() != NULL);

    dealbreaker_string_free(cancelled);
    dealbreaker_string_free(hint);
    dealbreaker_string_free(res);
    dealbreaker_string_free(deal);
    dealbreaker_solver_free(solver);
    dealbreaker_solver_free(NULL);
    printf("C ABI smoke test passed!\n");
    return 0;
}
