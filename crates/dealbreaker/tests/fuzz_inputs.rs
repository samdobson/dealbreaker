//! Property and mutation-based fuzz tests for untrusted inputs:
//! `Card::from_str`, `Rules::from_json` + `Game::new` + `deal`/`legal_steps`/`solve`,
//! `state_from_json` / `hint_state_from_json`, and `Step` deserialization / `play_step`.
//!
//! Runs deterministically with zero extra dependencies so CI exercises every parser and
//! compiler boundary on every push.

use dealbreaker::rng::Rng;
use dealbreaker::*;
use serde_json::Value;

const EDGE_VALUES: &[&str] = &[
    "null",
    "true",
    "false",
    "0",
    "1",
    "2",
    "255",
    "65535",
    "-1",
    "\"\"",
    "\"nonexistent\"",
    "\"stock\"",
    "\"tableau\"",
    "\"foundation\"",
    "[]",
    "[0]",
    "[999]",
    "[0, 0]",
    "{}",
];

fn random_ascii(rng: &mut Rng, max_len: usize) -> String {
    let len = rng.below((max_len + 1) as u64) as usize;
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let b = (0x20 + rng.below(95) as u8) as char;
        s.push(b);
    }
    s
}

fn mutate_json_string(rng: &mut Rng, base: &str) -> String {
    let mut bytes = base.as_bytes().to_vec();
    if bytes.is_empty() {
        return random_ascii(rng, 16);
    }
    let ops = 1 + rng.below(4);
    for _ in 0..ops {
        match rng.below(4) {
            0 => {
                // Truncate.
                let at = rng.below(bytes.len() as u64) as usize;
                bytes.truncate(at);
                if bytes.is_empty() {
                    break;
                }
            }
            1 => {
                // Flip a byte to printable ASCII.
                let idx = rng.below(bytes.len() as u64) as usize;
                bytes[idx] = 0x20 + rng.below(95) as u8;
            }
            2 => {
                // Delete a slice.
                let start = rng.below(bytes.len() as u64) as usize;
                let len = (1 + rng.below(8) as usize).min(bytes.len() - start);
                bytes.drain(start..start + len);
                if bytes.is_empty() {
                    break;
                }
            }
            _ => {
                // Insert a random JSON punctuation or digit.
                const CHARS: &[u8] = b"{}[]\":,019-truefalsn?";
                let idx = rng.below((bytes.len() + 1) as u64) as usize;
                let ch = CHARS[rng.below(CHARS.len() as u64) as usize];
                bytes.insert(idx, ch);
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn mutate_value(rng: &mut Rng, val: &mut Value) {
    match val {
        Value::Object(map) => {
            if map.is_empty() {
                return;
            }
            let keys: Vec<String> = map.keys().cloned().collect();
            let k = &keys[rng.below(keys.len() as u64) as usize];
            if rng.below(3) == 0 {
                let replacement: Value =
                    serde_json::from_str(EDGE_VALUES[rng.below(EDGE_VALUES.len() as u64) as usize])
                        .unwrap();
                map.insert(k.clone(), replacement);
            } else if rng.below(4) == 0 {
                map.remove(k);
            } else if let Some(child) = map.get_mut(k) {
                mutate_value(rng, child);
            }
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                let v: Value =
                    serde_json::from_str(EDGE_VALUES[rng.below(EDGE_VALUES.len() as u64) as usize])
                        .unwrap();
                arr.push(v);
            } else {
                match rng.below(3) {
                    0 => {
                        let i = rng.below(arr.len() as u64) as usize;
                        mutate_value(rng, &mut arr[i]);
                    }
                    1 => {
                        let i = rng.below(arr.len() as u64) as usize;
                        arr.remove(i);
                    }
                    _ => {
                        let v = arr[rng.below(arr.len() as u64) as usize].clone();
                        arr.push(v);
                    }
                }
            }
        }
        _ => {
            *val = serde_json::from_str(EDGE_VALUES[rng.below(EDGE_VALUES.len() as u64) as usize])
                .unwrap();
        }
    }
}

#[test]
fn card_from_str_never_panics() {
    // Exhaustive 1-byte and selected 2-byte ASCII inputs.
    for b in 0u8..=127 {
        let buf = [b];
        let s = std::str::from_utf8(&buf).unwrap();
        let _ = s.parse::<Card>();
    }
    let prefixes = ["", "~", "1", "10", "J", "A", "K", "Q", "?"];
    let suffixes = ["", "S", "H", "D", "C", "K", "s", "?", "0", "9", "Z", "~"];
    for p in prefixes {
        for s in suffixes {
            let candidate = format!("{p}{s}");
            if let Ok(c) = candidate.parse::<Card>() {
                // Any successfully parsed card must round-trip cleanly.
                assert_eq!(c.to_string().parse::<Card>().unwrap(), c);
            }
        }
    }

    // Random ASCII & multi-byte UTF-8 strings.
    let mut rng = Rng::new(0xC0DE_CAFE);
    for _ in 0..5_000 {
        let s = random_ascii(&mut rng, 12);
        if let Ok(c) = s.parse::<Card>() {
            assert_eq!(c.to_string().parse::<Card>().unwrap(), c);
        }
    }
    for utf8 in ["♠", "A♠", "~♠", "🃏", "10♥", "é", "~é", "  AS  "] {
        let _ = utf8.parse::<Card>();
    }
}

#[test]
fn rules_from_json_and_compile_never_panic() {
    let seeds: Vec<Rules> = vec![
        presets::freecell(),
        presets::klondike1(),
        presets::spider(4),
        presets::pyramid(),
        presets::montana(),
        presets::la_belle_lucie(),
        presets::monte_carlo(),
        presets::poker_squares(),
        presets::cribbage_squares(),
        presets::klondike_jokers(),
    ];
    let seed_jsons: Vec<String> = seeds.iter().map(Rules::to_json).collect();
    let seed_vals: Vec<Value> = seed_jsons
        .iter()
        .map(|j| serde_json::from_str(j).unwrap())
        .collect();

    let mut rng = Rng::new(0x5EED_0001);

    // 1. Raw byte/character-level JSON mutations must never panic.
    for _ in 0..1_000 {
        let base = &seed_jsons[rng.below(seed_jsons.len() as u64) as usize];
        let mutated = mutate_json_string(&mut rng, base);
        if let Ok(rules) = Rules::from_json(&mutated) {
            let _ = Game::new(rules);
        }
    }

    // 2. Structured JSON field mutations: any rule that compiles must also deal, validate,
    //    generate legal steps, and run a bounded solve without panicking.
    for _ in 0..600 {
        let mut val = seed_vals[rng.below(seed_vals.len() as u64) as usize].clone();
        let n_muts = 1 + rng.below(3);
        for _ in 0..n_muts {
            mutate_value(&mut rng, &mut val);
        }
        let Ok(rules) = Rules::from_json(&val.to_string()) else {
            continue;
        };
        let Ok(game) = Game::new(rules) else {
            continue;
        };
        let state = game.deal(1);
        game.validate(&state)
            .expect("deal() from compiled mutated rules must be valid");
        let _ = game.legal_steps(&state);
        let _ = solve(&game, &state, &SolveOptions::default().max_states(50));
    }
}

#[test]
fn state_and_hint_json_never_panic() {
    let games = [
        Game::new(presets::freecell()).unwrap(),
        Game::new(presets::klondike1()).unwrap(),
        Game::new(presets::pyramid()).unwrap(),
        Game::new(presets::poker_squares()).unwrap(),
    ];

    let mut rng = Rng::new(0x5EED_0002);

    for game in &games {
        let base_state = game.deal(1);
        let base_json = game.state_to_json(&base_state);
        let base_val: Value = serde_json::from_str(&base_json).unwrap();

        // Raw string mutations.
        for _ in 0..300 {
            let mutated = mutate_json_string(&mut rng, &base_json);
            if let Ok(s) = game.state_from_json(&mutated) {
                game.validate(&s).unwrap();
                let _ = game.legal_steps(&s);
            }
            if let Ok(s) = game.hint_state_from_json(&mutated) {
                game.validate(&s).unwrap();
                let _ = game.legal_steps(&s);
            }
        }

        // Structured mutations including "?" and "~?" placeholders.
        for _ in 0..300 {
            let mut val = base_val.clone();
            mutate_value(&mut rng, &mut val);
            // Occasionally inject "?" / "~?" into a random card slot.
            if rng.below(2) == 0 {
                if let Some(piles_obj) = val.get_mut("piles").and_then(Value::as_object_mut) {
                    let keys: Vec<String> = piles_obj.keys().cloned().collect();
                    if !keys.is_empty() {
                        let k = &keys[rng.below(keys.len() as u64) as usize];
                        if let Some(arr) = piles_obj.get_mut(k).and_then(Value::as_array_mut) {
                            if !arr.is_empty() {
                                let pi = rng.below(arr.len() as u64) as usize;
                                if let Some(pile) = arr[pi].as_array_mut() {
                                    if !pile.is_empty() {
                                        let ci = rng.below(pile.len() as u64) as usize;
                                        pile[ci] = Value::String(
                                            if rng.below(2) == 0 { "?" } else { "~?" }.into(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let json = val.to_string();
            if let Ok(s) = game.state_from_json(&json) {
                game.validate(&s).unwrap();
                let _ = game.legal_steps(&s);
            }
            if let Ok(s) = game.hint_state_from_json(&json) {
                game.validate(&s).unwrap();
                let _ = game.legal_steps(&s);
            }
        }
    }
}

#[test]
fn step_json_and_play_step_never_panic() {
    let games = [
        Game::new(presets::freecell()).unwrap(),
        Game::new(presets::klondike1()).unwrap(),
        Game::new(presets::pyramid()).unwrap(),
    ];
    let adversarial_steps = [
        r#"{"type":"deal"}"#,
        r#"{"type":"redeal"}"#,
        r#"{"type":"move","from":{"group":"tableau","index":0},"to":{"group":"tableau","index":0},"count":0}"#,
        r#"{"type":"move","from":{"group":"tableau","index":0},"to":{"group":"tableau","index":1},"count":999}"#,
        r#"{"type":"move","from":{"group":"nope","index":0},"to":{"group":"tableau","index":1},"count":1}"#,
        r#"{"type":"move","from":{"group":"tableau","index":999},"to":{"group":"tableau","index":1},"count":1}"#,
        r#"{"type":"remove","piles":[]}"#,
        r#"{"type":"remove","piles":[{"group":"nope","index":0}]}"#,
        r#"{"type":"remove","piles":[{"group":"tableau","index":999},{"group":"tableau","index":0}]}"#,
        r#"{"type":"bogus"}"#,
        r#"{}"#,
        r#"null"#,
    ];

    for game in &games {
        let base = game.deal(1);
        for raw in adversarial_steps {
            if let Ok(step) = serde_json::from_str::<Step>(raw) {
                let mut s = base.clone();
                if game.play_step(&mut s, &step).is_ok() {
                    game.validate(&s).unwrap();
                }
            }
        }
    }
}

#[test]
fn adversarial_rules_and_options_never_panic_or_hang() {
    // 1. Cyclic refill_from (self-cycle and 2-group cycle) must be rejected.
    let mut self_cycle = presets::freecell();
    self_cycle.groups[2].refill_from = vec!["tableau".into()];
    assert!(Game::new(self_cycle).unwrap_err().0.contains("cycle"));

    let mut two_cycle = presets::freecell();
    two_cycle.groups[0].refill_from = vec!["tableau".into()];
    two_cycle.groups[2].refill_from = vec!["cell".into()];
    assert!(Game::new(two_cycle).unwrap_err().0.contains("cycle"));

    // 2. Out-of-range ranks in EmptyRule variants must be rejected.
    for bad_empty in [
        EmptyRule::Rank(0),
        EmptyRule::Rank(14),
        EmptyRule::Rank(255),
        EmptyRule::Ranks(vec![1, 14]),
        EmptyRule::AfterLeft { first: 0 },
        EmptyRule::AfterNeighbor { first: 99 },
    ] {
        let mut r = presets::freecell();
        r.groups[2].empty = Some(bad_empty);
        assert!(Game::new(r).unwrap_err().0.contains("1..=13"));
    }

    // 3. Out-of-range base_rank in StateData must be rejected.
    let fc = Game::new(presets::freecell()).unwrap();
    for bad_rank in [0u8, 14, 255] {
        let mut data = fc.state_to_data(&fc.deal(1));
        data.base_rank = Some(bad_rank);
        assert!(fc.state_from_data(&data).unwrap_err().0.contains("1..=13"));
    }

    // 4. Pyramid rows u16 overflow (512 * 513 / 2 wraps mod 65536 to 256).
    let mut pyr = presets::pyramid();
    pyr.groups[2].count = 256;
    pyr.groups[2].layout = Some(Layout::Pyramid { rows: 512 });
    assert!(Game::new(pyr).is_err());

    // 5. Excessive pile counts (and redeal scratch pile at boundary) must be rejected.
    let mut huge = presets::freecell();
    huge.groups[2].count = 2000;
    assert!(Game::new(huge).unwrap_err().0.contains("too many piles"));

    let mut boundary_redeal = presets::la_belle_lucie();
    boundary_redeal.groups[1].count = 1020; // 4 foundations + 1020 tableau + 1 scratch > 1024
    assert!(Game::new(boundary_redeal)
        .unwrap_err()
        .0
        .contains("too many piles"));

    // 6. Scored games with >2 jokers in Cribbage or non-monotone Poker must be rejected.
    let mut crib = presets::cribbage_squares();
    crib.pack.jokers = 3;
    assert!(Game::new(crib).unwrap_err().0.contains("jokers"));

    let mut poker = presets::poker_squares();
    poker.pack.jokers = 3;
    if let Some(sc) = &mut poker.score {
        let mut pts = PokerTable::Named(PokerScoring::American).points();
        pts.pair = 100;
        pts.two_pair = 1; // non-monotone
        sc.hands = Hands::Poker(PokerTable::Points(pts));
    }
    assert!(Game::new(poker).unwrap_err().0.contains("jokers"));

    // 7. Large CribbagePoints must not overflow during compile_score / score / maximize.
    let mut big_crib = presets::cribbage_squares();
    if let Some(sc) = &mut big_crib.score {
        if let Hands::Cribbage(ref mut cr) = sc.hands {
            cr.points.fifteen = u32::MAX;
            cr.points.pair = u32::MAX;
            cr.points.run_card = u32::MAX;
            cr.points.flush = u32::MAX;
            cr.points.heels = u32::MAX;
            cr.points.nobs = u32::MAX;
        }
    }
    let g_big = Game::new(big_crib).unwrap();
    let s_big = g_big.deal(1);
    let _ = g_big.score(&s_big);
    let _ = maximize(&g_big, &s_big, &SolveOptions::default().max_states(10));

    // 8. Huge timeout_ms must not panic on Instant overflow.
    let s = fc.deal(1);
    let _ = solve(
        &fc,
        &s,
        &SolveOptions::default().max_states(10).timeout_ms(u64::MAX),
    );

    // 9. Huge hint samples with pre-triggered cancel must return immediately without OOM.
    let cancel = std::sync::atomic::AtomicBool::new(true);
    let kl = Game::new(presets::klondike1()).unwrap();
    let ks = kl.deal(1);
    assert!(
        hint_with_cancel(&kl, &ks, &HintOptions::default().samples(u32::MAX), &cancel,).is_none()
    );
}
