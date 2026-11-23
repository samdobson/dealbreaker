from concurrent.futures import ThreadPoolExecutor

import pytest

from dealbreaker import Solver, presets


def test_presets_and_rules_roundtrip():
    items = presets()
    ids = {p["id"] for p in items}
    assert "freecell" in ids
    assert "klondike" in ids

    fc = Solver.from_preset("freecell")
    rules = fc.rules()
    assert rules["name"] == "FreeCell"

    fc2 = Solver(rules)
    assert fc2.rules() == rules


def test_freecell_solve_and_replay():
    fc = Solver.from_preset("freecell")
    deal = fc.deal_microsoft(1)
    fc.validate(deal)
    assert not fc.is_won(deal)
    assert fc.score(deal) is None
    assert len(fc.legal_steps(deal)) > 0

    res = fc.solve(deal, max_states=200_000)
    assert res["outcome"] == "solved"
    cur = deal
    for step in res["steps"]:
        cur = fc.play_step(cur, step)
    assert fc.is_won(cur)


def test_timeout_and_validation_errors():
    fc = Solver.from_preset("freecell")
    deal = fc.deal_microsoft(11982)
    res = fc.solve(deal, timeout_ms=0)
    assert res["outcome"] == "gave-up"
    assert res["states"] == 0

    with pytest.raises(ValueError):
        fc.validate({"piles": {}})

    with pytest.raises(ValueError):
        Solver.from_preset("does-not-exist")


def test_hint_and_scored_best():
    fc = Solver.from_preset("freecell")
    h = fc.hint(fc.deal_microsoft(5), samples=1)
    assert h is not None
    assert h["wins"] == 1

    ps = Solver.from_preset("poker-squares")
    pstate = ps.deal(1)
    assert ps.score(pstate) == 0
    best = ps.best(pstate, max_states=2_000)
    assert best["score"] > 0
    assert len(best["steps"]) > 0


def test_concurrent_threads_release_gil():
    fc = Solver.from_preset("freecell")
    deals = [fc.deal_microsoft(n) for n in (1, 2, 3, 4)]
    with ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(lambda d: fc.solve(d, max_states=200_000), deals))
    assert all(r["outcome"] == "solved" for r in results)
