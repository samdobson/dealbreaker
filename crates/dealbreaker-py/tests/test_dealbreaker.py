from concurrent.futures import ThreadPoolExecutor

import pytest

from dealbreaker import CancelToken, Solver, format_step, presets


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


def test_grade():
    fc = Solver.from_preset("freecell")
    state = fc.deal_microsoft(1)
    d = fc.grade(state)
    assert d["grade"] in ("easy", "medium", "hard")
    assert d["winnable"] is True
    assert d["source"] == "preset"
    assert d["score"] > 0 and d["probes"] > 0
    assert fc.grade(state, thresholds=(1000.0, 2000.0))["grade"] == "easy"
    assert fc.grade(state, thresholds=(0.0, 0.0))["grade"] == "hard"
    # A budget too small to win: graded hard, winnability unknown.
    tiny = fc.grade(state, max_states=1)
    assert tiny["grade"] == "hard" and tiny["winnable"] is None and "score" not in tiny


def test_concurrent_threads_release_gil():
    fc = Solver.from_preset("freecell")
    deals = [fc.deal_microsoft(n) for n in (1, 2, 3, 4)]
    with ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(lambda d: fc.solve(d, max_states=200_000), deals))
    assert all(r["outcome"] == "solved" for r in results)


def test_deal_ordered_and_json_string_inputs():
    import json

    fc = Solver.from_preset("freecell")
    cards = [
        f"{r}{s}"
        for s in ("C", "D", "H", "S")
        for r in ("A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K")
    ]
    state = fc.deal_ordered(cards)
    fc.validate(state)
    fc.validate(json.dumps(state))

    with pytest.raises(ValueError):
        fc.deal_ordered(cards[:10])
    with pytest.raises(ValueError):
        fc.deal_ordered(["ZZ"])

    # Solver accepts raw JSON string rules as well as dicts.
    fc_from_str = Solver(json.dumps(fc.rules()))
    assert fc_from_str.deal(1) == fc.deal(1)


def test_options_strategies_and_error_paths():
    fc = Solver.from_preset("freecell")
    deal = fc.deal_microsoft(1)

    for strat in ("dfs", "best-first"):
        res = fc.solve(deal, max_states=200_000, strategy=strat, autoplay=False, shorten=False)
        assert res["outcome"] == "solved"

    with pytest.raises(ValueError):
        fc.solve(deal, strategy="bogus")
    with pytest.raises(ValueError):
        Solver("{not json")
    with pytest.raises(ValueError):
        fc.play_step(deal, {"type": "deal"})

    # deal_microsoft rejects non-52-card games.
    sp = Solver.from_preset("spider")
    with pytest.raises(ValueError):
        sp.deal_microsoft(1)


def test_hint_unknown_cards_and_none_on_tiny_budget():
    fc = Solver.from_preset("freecell")
    assert fc.hint(fc.deal_microsoft(1), max_states=1) is None

    kl = Solver.from_preset("klondike")
    state = kl.deal(2)
    for piles in state["piles"].values():
        for pile in piles:
            for i, card in enumerate(pile):
                if card.startswith("~"):
                    pile[i] = "~?" if i % 2 else "?"

    # Strict validate rejects "?", while hint accepts it.
    with pytest.raises(ValueError):
        kl.validate(state)
    h = kl.hint(state, samples=2, seed=1, max_states=5_000)
    assert h is None or isinstance(h["step"]["type"], str)


def test_cancel_token_and_render_and_format_step():
    fc = Solver.from_preset("freecell")
    deal = fc.deal_microsoft(11982)

    board = fc.render(deal)
    assert "tableau" in board
    assert "cell" in board

    step = fc.legal_steps(deal)[0]
    formatted = format_step(step)
    assert "->" in formatted

    tok = CancelToken()
    assert not tok.is_cancelled
    tok.cancel()
    assert tok.is_cancelled
    res = fc.solve(deal, cancel=tok)
    assert res["outcome"] == "gave-up"
    assert res["states"] == 0

    tok.reset()
    assert not tok.is_cancelled
    solved = fc.solve(fc.deal_microsoft(1), max_states=200_000, cancel=tok)
    assert solved["outcome"] == "solved"
