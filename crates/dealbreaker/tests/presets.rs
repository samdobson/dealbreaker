//! Every preset compiles, round-trips through JSON, deals a valid position, and every solution
//! the solver returns replays legally to a win.

mod common;

use common::assert_wins;
use dealbreaker::*;

#[test]
fn presets_compile_and_round_trip() {
    for p in presets::PRESETS {
        let rules = (p.rules)();
        let json = rules.to_json();
        let back = Rules::from_json(&json).unwrap_or_else(|e| panic!("{}: {e}\n{json}", p.id));
        assert_eq!(rules, back, "{}", p.id);
        let g = Game::new(rules).unwrap_or_else(|e| panic!("{}: {e}", p.id));
        for seed in 0..20 {
            let s = g.deal(seed);
            g.validate(&s).unwrap();
            let again = g.state_from_json(&g.state_to_json(&s)).unwrap();
            assert_eq!(s, again, "{} state JSON round trip", p.id);
        }
        assert_eq!(
            g.deal(7),
            g.deal(7),
            "{}: deals must be deterministic",
            p.id
        );
    }
}

#[test]
fn preset_solutions_replay() {
    let mut presets_solved = 0usize;
    let must_solve = [
        "freecell",
        "klondike",
        "spider-1suit",
        "yukon",
        "golf",
        "pyramid",
        "montana",
        "canfield",
        "poker-squares",
        "cribbage-squares",
    ];
    for p in presets::PRESETS {
        let g = Game::new((p.rules)()).unwrap();
        let mut solved = 0;
        for seed in 1..=4 {
            let s = g.deal(seed);
            let r = solve(&g, &s, &SolveOptions::default().max_states(60_000));
            if let Outcome::Solved(m) = &r.outcome {
                assert_wins(&g, &s, m);
                assert!(!g.steps(&s, m).is_empty());
                solved += 1;
            }
        }
        println!("{:<20} {solved}/4 solved within budget", p.id);
        if solved > 0 {
            presets_solved += 1;
        }
        if must_solve.contains(&p.id) {
            assert!(
                solved >= 1,
                "bellwether preset {} solved 0/4 within 60k states",
                p.id
            );
        }
    }
    assert!(
        presets_solved >= 30,
        "expected at least 30 presets to solve >=1/4 seeds, got {presets_solved}"
    );
}

#[test]
fn microsoft_deal_numbering() {
    let g = Game::new(presets::freecell()).unwrap();
    let s = g.deal_ordered(&microsoft_deal(1)).unwrap();
    let first_row: Vec<String> = (0..8)
        .map(|i| s.piles[g.pile_index("tableau", i).unwrap() as usize][0].to_string())
        .collect();
    assert_eq!(first_row, ["JD", "2D", "9H", "JC", "5D", "7H", "7C", "5H"]);
    let col1: Vec<String> = s.piles[g.pile_index("tableau", 0).unwrap() as usize]
        .iter()
        .map(|c| c.to_string())
        .collect();
    assert_eq!(col1, ["JD", "KD", "2S", "4C", "3S", "6D", "6S"]);
}

#[test]
fn freecell_11982_is_unsolvable() {
    let g = Game::new(presets::freecell()).unwrap();
    let s = g.deal_ordered(&microsoft_deal(11982)).unwrap();
    let r = solve(&g, &s, &SolveOptions::default().max_states(5_000_000));
    assert_eq!(r.outcome, Outcome::Unsolvable);
}

#[test]
fn freecell_first_hundred_solve() {
    let g = Game::new(presets::freecell()).unwrap();
    for n in 1..=100 {
        let s = g.deal_ordered(&microsoft_deal(n)).unwrap();
        let r = solve(&g, &s, &SolveOptions::default());
        match &r.outcome {
            Outcome::Solved(m) => assert_wins(&g, &s, m),
            o => panic!("MS deal {n}: {o:?}"),
        }
    }
}

#[test]
fn hint_without_hidden_cards_matches_solution() {
    let g = Game::new(presets::freecell()).unwrap();
    let s = g.deal_ordered(&microsoft_deal(5)).unwrap();
    let h = hint(&g, &s, &HintOptions::default()).expect("a hint");
    assert_eq!(h.samples, 1);
    assert_eq!(h.wins, 1);
}

#[test]
fn hint_with_hidden_cards_is_legal() {
    let g = Game::new(presets::klondike1()).unwrap();
    for seed in 1..4 {
        let s = g.deal(seed);
        if let Some(h) = hint(&g, &s, &HintOptions::default().samples(6)) {
            match h.step {
                Step::Deal | Step::Redeal => assert!(g.can_deal(&s)),
                Step::Move { from, to, count } => {
                    let m = Move::new(Action::Transfer {
                        from: g.pile_index(&from.group, from.index).unwrap(),
                        to: g.pile_index(&to.group, to.index).unwrap(),
                        count,
                    });
                    assert!(g.is_legal(&s, m), "{m:?}");
                }
                Step::Remove { .. } => panic!("no removals in Klondike"),
                other => panic!("unexpected step {other:?}"),
            }
        }
    }
}

/// Best lines in games played for points finish the game with the score claimed.
#[test]
fn scored_presets_best_lines_replay() {
    for p in presets::PRESETS {
        let g = Game::new((p.rules)()).unwrap();
        if !g.is_scored() {
            continue;
        }
        for seed in 1..=3 {
            let s = g.deal(seed);
            let opts = SolveOptions::default().max_states(20_000);
            let r = maximize(&g, &s, &opts);
            let mut end = s;
            for &m in &r.moves {
                g.play(&mut end, m).unwrap();
                g.validate(&end).unwrap();
            }
            assert!(g.legal_moves(&end).is_empty(), "{} seed {seed}", p.id);
            assert_eq!(g.score(&end), Some(r.score), "{} seed {seed}", p.id);
            assert!(r.score > 0, "{} seed {seed}: no points at all", p.id);
        }
    }
}

/// A hint part way through Poker Squares is a legal step with an average score.
#[test]
fn scored_hint_is_legal() {
    let g = Game::new(presets::poker_squares()).unwrap();
    let mut s = g.deal(5);
    for i in 0..3 {
        let to = g.pile_index("tableau", i * 6).unwrap();
        let from = g.pile_index("waste", 0).unwrap();
        g.play(&mut s, Move::new(Action::Transfer { from, to, count: 1 }))
            .unwrap();
    }
    let h = hint(
        &g,
        &s,
        &HintOptions::default()
            .samples(4)
            .solve(SolveOptions::default().max_states(4_000))
            .seed(1),
    )
    .expect("a hint");
    assert!(h.score.is_some_and(|v| v > 0.0), "{h:?}");
    let mut t = s.clone();
    g.play_step(&mut t, &h.step).unwrap();
}

#[test]
fn rules_errors_are_reported() {
    let mut r = presets::freecell();
    r.deal.push(DealStep::to("nowhere").each(1));
    assert!(Game::new(r).is_err());
    let mut r = presets::klondike1();
    r.deal.pop(); // leaves the stock cards undealt
    assert!(Game::new(r).unwrap_err().0.contains("undealt"));
    assert!(Rules::from_json(r#"{"name":"x","groups":[],"bogus":1}"#).is_err());
}

#[test]
fn state_validation_catches_bad_positions() {
    let g = Game::new(presets::freecell()).unwrap();
    let mut d = g.state_to_data(&g.deal(1));
    d.piles.get_mut("tableau").unwrap()[0][0] = "AS".into(); // now two AS, no JD
    let e = g.state_from_data(&d).unwrap_err();
    assert!(
        e.0.contains("missing: [JD]") || e.0.contains("missing"),
        "{e}"
    );
}
