//! The solver's pruning (symmetry, safe autoplay, talon compression, sink hashing, the fast
//! phase) must never change a verdict. Small games are searched exhaustively both ways.

mod common;

use common::{assert_wins, brute_force, brute_force_max, parallel_map};
use dealbreaker::*;

fn small(ranks: std::ops::RangeInclusive<u8>, suits: &[Suit], copies: u8) -> Pack {
    Pack::new(suits, &ranks.collect::<Vec<_>>()).with_copies(copies)
}

fn mini_klondike(draw: u16, redeals: Option<u32>) -> Rules {
    let mut r = presets::klondike1();
    r.name = format!("mini klondike draw {draw}");
    r.pack = small(1..=4, &Suit::ALL, 1);
    r.groups[3].count = 3;
    r.groups[3].empty = Some(EmptyRule::Rank(4));
    r.deal = vec![
        DealStep::to("tableau").counts(&[0, 1, 2]).down(),
        DealStep::to("tableau").each(1),
        DealStep::to("stock").rest().down(),
    ];
    let st = r.stock.as_mut().unwrap();
    st.count = draw;
    st.unlimited_redeals = redeals.is_none();
    st.redeals = redeals.unwrap_or(0);
    r
}

fn mini_freecell(by: BuildBy) -> Rules {
    let mut r = presets::freecell();
    r.name = "mini freecell".into();
    r.pack = small(1..=6, &Suit::ALL, 1);
    r.groups[0].count = 1;
    r.groups[2].count = 4;
    r.groups[2].build = Some(Build::new(by, Dir::Down));
    r
}

fn mini_spider() -> Rules {
    let mut r = presets::spider(2);
    r.name = "mini spider".into();
    r.pack = small(1..=5, &[Suit::Spades, Suit::Hearts], 2);
    r.groups[1].count = 4;
    r.groups[2].count = 4;
    r.deal = vec![
        DealStep::to("tableau").each(2).down(),
        DealStep::to("tableau").each(1),
        DealStep::to("stock").rest().down(),
    ];
    r
}

fn mini_yukon() -> Rules {
    let mut r = presets::yukon();
    r.name = "mini yukon".into();
    r.pack = small(1..=6, &Suit::ALL, 1);
    r.groups[1].count = 4;
    r.groups[1].empty = Some(EmptyRule::Rank(6));
    r.deal = vec![
        DealStep::to("tableau").counts(&[0, 1, 2, 3]).down(),
        DealStep::to("tableau").counts(&[3, 5, 5, 5]),
    ];
    r
}

fn mini_golf() -> Rules {
    let mut r = presets::golf();
    r.name = "mini golf".into();
    r.pack = small(1..=13, &[Suit::Spades, Suit::Hearts], 1);
    r.groups[2].count = 4;
    r.deal = vec![
        DealStep::to("tableau").each(4),
        DealStep::to("foundation").each(1),
        DealStep::to("stock").rest().down(),
    ];
    r
}

fn mini_pyramid() -> Rules {
    let mut r = presets::pyramid();
    r.name = "mini pyramid".into();
    r.pack = small(1..=13, &[Suit::Spades, Suit::Hearts], 1);
    r.groups[3].count = 10;
    r.groups[3].layout = Some(Layout::Pyramid { rows: 4 });
    r.stock.as_mut().unwrap().redeals = 1;
    r
}

fn mini_canfield() -> Rules {
    let mut r = presets::canfield();
    r.name = "mini canfield".into();
    r.pack = small(1..=5, &Suit::ALL, 1);
    r.deal = vec![
        DealStep::to("reserve").each(3).down(),
        DealStep::to("reserve").each(1),
        DealStep::to("foundation").piles(&[0]).each(1).set_base(),
        DealStep::to("tableau").each(1),
        DealStep::to("stock").rest().down(),
    ];
    r.stock.as_mut().unwrap().count = 2;
    r
}

/// Empty tableau piles take only the card one rank below the foundations' base (Penguin), going
/// round the stripped pack: with an ace as the base that is the five.
fn mini_base_offset() -> Rules {
    let mut r = mini_canfield();
    r.name = "mini base offset".into();
    let t = r.groups.iter_mut().find(|g| g.name == "tableau").unwrap();
    t.refill_from.clear();
    t.empty = Some(EmptyRule::DealtBaseOffset(-1));
    r
}

fn mini_aces_up() -> Rules {
    let mut r = presets::aces_up();
    r.name = "mini aces up".into();
    r.pack = Pack::new(&Suit::ALL, &[10, 11, 12, 13, 1]);
    r.win = WinRule::Count {
        group: "discard".into(),
        cards: 16,
    };
    r
}

fn mini_scorpion() -> Rules {
    let mut r = presets::scorpion();
    r.name = "mini scorpion".into();
    r.pack = small(1..=6, &Suit::ALL, 1);
    r.groups[2].count = 4;
    r.groups[2].empty = Some(EmptyRule::Rank(6));
    r.deal = vec![
        DealStep::to("tableau").piles(&[0, 1]).each(2).down(),
        DealStep::to("tableau").counts(&[3, 3, 5, 5]),
        DealStep::to("stock").rest().down(),
    ];
    r
}

fn mini_calculation() -> Rules {
    let mut r = presets::calculation();
    r.name = "mini calculation".into();
    r.pack = small(1..=5, &Suit::ALL, 1);
    r.groups[2].max_cards = Some(5);
    r.groups[3].count = 1;
    r
}

fn mini_sir_tommy() -> Rules {
    let mut r = presets::sir_tommy();
    r.name = "mini sir tommy".into();
    r.pack = small(1..=4, &Suit::ALL, 1);
    r.groups[3].count = 1;
    r
}

fn mini_bisley() -> Rules {
    let mut r = presets::bisley();
    r.name = "mini bisley".into();
    r.pack = small(1..=6, &Suit::ALL, 1);
    r.groups[0].piles[0].empty = Some(EmptyRule::Rank(6));
    r.groups[1].count = 5;
    r.deal = vec![
        DealStep::to("foundation").piles(&[0, 1, 2, 3]).ranks(&[1]),
        DealStep::to("tableau").counts(&[4, 4, 4, 4, 4]),
    ];
    r
}

fn mini_elevens() -> Rules {
    let mut r = presets::elevens();
    r.name = "mini elevens".into();
    r.pack = small(1..=13, &[Suit::Spades, Suit::Hearts], 1);
    r.groups[2].count = 5;
    r.win = WinRule::Count {
        group: "discard".into(),
        cards: 26,
    };
    r
}

fn mini_nestor() -> Rules {
    let mut r = presets::nestor();
    r.name = "mini nestor".into();
    r.pack = small(1..=7, &[Suit::Spades, Suit::Hearts], 1);
    r.groups[2].count = 5;
    r.deal = vec![
        DealStep::to("tableau").each(2),
        DealStep::to("reserve").each(1),
    ];
    r.win = WinRule::Count {
        group: "discard".into(),
        cards: 14,
    };
    r
}

/// Refills from the waste then the stock, and a stock that deals by itself.
fn mini_autofill() -> Rules {
    let mut r = mini_klondike(1, Some(1));
    r.name = "mini autofill".into();
    r.groups[3].empty = Some(EmptyRule::None);
    r.groups[3].refill_from = vec!["waste".into(), "stock".into()];
    r.stock.as_mut().unwrap().auto = true;
    r
}

fn mini_cruel() -> Rules {
    let mut r = presets::cruel();
    r.name = "mini cruel".into();
    r.pack = small(1..=5, &Suit::ALL, 1);
    r.groups[1].count = 4;
    r
}

/// Cruel whose foundations give cards back, so the tableau can hold more than four cards a pile
/// when it is gathered and dealt again.
fn mini_cruel_giving_back() -> Rules {
    let mut r = mini_cruel();
    r.name = "mini cruel, cards come back".into();
    r.groups[0] = r.groups[0].clone().moves(MoveUnit::Top);
    r
}

fn mini_lucie() -> Rules {
    let mut r = presets::la_belle_lucie();
    r.name = "mini la belle lucie".into();
    r.pack = small(1..=5, &Suit::ALL, 1);
    r.groups[1].count = 7;
    r.deal = vec![
        DealStep::to("tableau").piles(&[0, 1, 2, 3, 4, 5]).each(3),
        DealStep::to("tableau").piles(&[6]).each(2),
    ];
    r.redeal.as_mut().unwrap().times = 1;
    r
}

fn mini_montana() -> Rules {
    let mut r = presets::montana();
    r.name = "mini montana".into();
    r.pack = small(1..=6, &[Suit::Spades, Suit::Hearts], 1);
    r.groups[1].count = 12;
    r.groups[1].layout = Some(Layout::Grid { columns: 6 });
    r.win = WinRule::Rows {
        group: "tableau".into(),
        length: Some(5),
    };
    r.redeal.as_mut().unwrap().times = 1;
    r
}

fn mini_gaps() -> Rules {
    let mut r = mini_montana();
    r.name = "mini gaps".into();
    r.redeal = None;
    r
}

fn mini_monte_carlo() -> Rules {
    let mut r = presets::monte_carlo();
    r.name = "mini monte carlo".into();
    r.pack = small(1..=8, &[Suit::Spades, Suit::Hearts], 1);
    r.groups[2].count = 9;
    r.groups[2].layout = Some(Layout::Grid { columns: 3 });
    r.win = WinRule::Count {
        group: "discard".into(),
        cards: 16,
    };
    r
}

fn mini_accordion() -> Rules {
    let mut r = presets::accordion();
    r.name = "mini accordion".into();
    r.pack = small(1..=4, &Suit::ALL, 1);
    r.groups[1].count = 16;
    r
}

fn check(rules: Rules, seeds: std::ops::Range<u64>) {
    let g = Game::new(rules).unwrap();
    let seeds: Vec<u64> = seeds.collect();
    let outcomes = parallel_map(&seeds, |&seed| {
        let s = g.deal(seed);
        let truth = brute_force(&g, &s, 2_000_000).expect("mini game too big to brute-force");
        for strategy in [Strategy::Auto, Strategy::DepthFirst, Strategy::BestFirst] {
            let opts = SolveOptions::default()
                .strategy(strategy)
                .max_states(10_000_000);
            let r = solve(&g, &s, &opts);
            match (&r.outcome, truth) {
                (Outcome::Solved(m), true) => assert_wins(&g, &s, m),
                (Outcome::Unsolvable, false) => {}
                (o, t) => panic!(
                    "{} seed {seed} {strategy:?}: solver says {o:?}, brute force says {t}",
                    g.name()
                ),
            }
        }
        truth
    });
    let won = outcomes.iter().filter(|&&t| t).count();
    let lost = outcomes.len() - won;
    println!("{}: {won} won, {lost} lost", g.name());
    assert!(
        won > 0,
        "{}: no winnable deals in the sample, test is weak",
        g.name()
    );
}

#[test]
fn klondike_draw1_unlimited() {
    check(mini_klondike(1, None), 0..40);
}

#[test]
fn klondike_draw3_limited() {
    check(mini_klondike(3, Some(1)), 0..40);
}

#[test]
fn klondike_draw2_single_pass() {
    check(mini_klondike(2, Some(0)), 0..40);
}

#[test]
fn freecell_alternate() {
    check(mini_freecell(BuildBy::Alternate), 0..40);
}

#[test]
fn freecell_suit() {
    check(mini_freecell(BuildBy::Suit), 0..40);
}

#[test]
fn spider() {
    check(mini_spider(), 0..40);
}

#[test]
fn yukon() {
    check(mini_yukon(), 0..40);
}

#[test]
fn golf() {
    check(mini_golf(), 0..40);
}

#[test]
fn pyramid() {
    check(mini_pyramid(), 0..40);
}

#[test]
fn canfield() {
    check(mini_canfield(), 0..40);
}

#[test]
fn base_offset() {
    check(mini_base_offset(), 0..40);
}

#[test]
fn base_offset_in_json() {
    let r = mini_base_offset();
    let json = r.to_json();
    assert!(json.contains("\"dealt_base_offset\": -1"), "{json}");
    let back = Rules::from_json(&json).unwrap();
    assert_eq!(back.to_json(), json);
}

/// The rule itself, on a hand-built position: with an ace as the dealt base, an empty tableau
/// pile takes the five (the rank below an ace in a five-rank pack) and nothing else.
#[test]
fn base_offset_picks_the_card_below_the_base() {
    let game = Game::new(mini_base_offset()).unwrap();
    let all: Vec<String> = ["S", "H", "D", "C"]
        .iter()
        .flat_map(|s| (1..=5).map(move |r| format!("{r}{s}")))
        .collect();
    let on_table = ["5S", "4S", "3H"];
    let stock: Vec<String> = all
        .iter()
        .filter(|c| !on_table.contains(&c.as_str()))
        .map(|c| format!("~{c}"))
        .collect();
    let json = serde_json::json!({
        "piles": {
            "tableau": [[], ["5S"], ["4S"], ["3H"]],
            "stock": [stock],
        },
        "base_rank": 1,
    });
    let state = game.state_from_json(&json.to_string()).unwrap();
    let onto_empty: Vec<String> = game
        .legal_steps(&state)
        .iter()
        .filter_map(|st| match st {
            Step::Move { from, to, .. } if to.group == "tableau" && to.index == 0 => {
                Some(from.to_string())
            }
            _ => None,
        })
        .collect();
    assert_eq!(onto_empty, vec!["tableau 2".to_string()], "only the five");
}

#[test]
fn cruel_giving_back() {
    check(mini_cruel_giving_back(), 0..40);
}

/// A redeal only rearranges cards: after every step of random play, every card of the pack is
/// still on the table, including when more cards are gathered than `per_pile` for every pile.
#[test]
fn regroup_keeps_every_card() {
    let game = Game::new(mini_cruel_giving_back()).unwrap();
    let pack = 20;
    let mut surplus_redeals = 0;
    for seed in 0..30u64 {
        let mut state = game.deal(seed);
        let mut rng = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        for _ in 0..120 {
            let steps = game.legal_steps(&state);
            if steps.is_empty() {
                break;
            }
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let step = &steps[(rng >> 33) as usize % steps.len()];
            if *step == Step::Redeal {
                let json: serde_json::Value =
                    serde_json::from_str(&game.state_to_json(&state)).unwrap();
                let on_tableau: usize = json["piles"]["tableau"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p.as_array().unwrap().len())
                    .sum();
                if on_tableau > 4 * 4 {
                    surplus_redeals += 1;
                }
            }
            game.play_step(&mut state, step).unwrap();
            let json: serde_json::Value =
                serde_json::from_str(&game.state_to_json(&state)).unwrap();
            let total: usize = json["piles"]
                .as_object()
                .unwrap()
                .values()
                .map(|group| {
                    group
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| p.as_array().unwrap().len())
                        .sum::<usize>()
                })
                .sum();
            assert_eq!(total, pack, "seed {seed}: {step} lost or added a card");
        }
    }
    assert!(
        surplus_redeals > 0,
        "the walks never gathered a surplus; the test proves nothing"
    );
}

#[test]
fn aces_up() {
    check(mini_aces_up(), 0..40);
}

#[test]
fn scorpion() {
    check(mini_scorpion(), 0..40);
}

#[test]
fn calculation() {
    check(mini_calculation(), 0..40);
}

#[test]
fn sir_tommy() {
    check(mini_sir_tommy(), 0..40);
}

#[test]
fn bisley() {
    check(mini_bisley(), 0..40);
}

#[test]
fn elevens() {
    check(mini_elevens(), 0..40);
}

#[test]
fn nestor() {
    check(mini_nestor(), 0..40);
}

#[test]
fn autofill() {
    check(mini_autofill(), 0..40);
}

#[test]
fn cruel() {
    check(mini_cruel(), 0..40);
}

#[test]
fn la_belle_lucie() {
    check(mini_lucie(), 0..40);
}

#[test]
fn montana() {
    check(mini_montana(), 0..40);
}

#[test]
fn gaps() {
    check(mini_gaps(), 0..40);
}

#[test]
fn monte_carlo() {
    check(mini_monte_carlo(), 0..40);
}

#[test]
fn accordion() {
    check(mini_accordion(), 0..40);
}

/// Every ordering of a tiny pack, so losing Accordion deals are covered too.
#[test]
fn accordion_all_orders() {
    let mut r = presets::accordion();
    r.pack = small(1..=2, &[Suit::Spades, Suit::Hearts, Suit::Clubs], 1);
    r.groups[1].count = 6;
    let g = Game::new(r).unwrap();
    let cards = g.pack_cards();
    let mut lost = 0;
    let mut order: Vec<usize> = (0..cards.len()).collect();
    // Heap's algorithm over all 720 orders.
    let mut c = vec![0; order.len()];
    let mut i = 0;
    let mut check_one = |order: &[usize]| {
        let deck: Vec<Card> = order.iter().map(|&k| cards[k]).collect();
        let s = g.deal_ordered(&deck).unwrap();
        let truth = brute_force(&g, &s, 1_000_000).unwrap();
        let r = solve(&g, &s, &SolveOptions::default());
        match (&r.outcome, truth) {
            (Outcome::Solved(m), true) => assert_wins(&g, &s, m),
            (Outcome::Unsolvable, false) => lost += 1,
            (o, t) => panic!("{deck:?}: solver {o:?}, brute force {t}"),
        }
    };
    check_one(&order);
    while i < order.len() {
        if c[i] < i {
            if i % 2 == 0 {
                order.swap(0, i)
            } else {
                order.swap(c[i], i)
            }
            check_one(&order);
            c[i] += 1;
            i = 0;
        } else {
            c[i] = 0;
            i += 1;
        }
    }
    assert!(lost > 0, "expected some losing orders");
}

/// A hooked game: an empty tableau pile takes only a king or queen, and aces go to the
/// foundations by themselves. Pruning is off for hooked games, so verdicts must still match.
#[derive(Debug)]
struct OddRules;

impl Hooks for OddRules {
    fn accepts(&self, g: &Game, s: &State, from: u16, to: u16, n: usize) -> Option<bool> {
        let to_ref = g.pile_ref(to);
        if to_ref.group == "tableau" && s.piles[to as usize].is_empty() {
            let pile = &s.piles[from as usize];
            let card = pile[pile.len() - n];
            if card.rank() < 5 {
                return Some(false);
            }
        }
        None
    }

    fn auto_moves(&self, g: &Game, s: &State) -> Vec<(u16, u16, u16)> {
        let found = g.pile_index("foundation", 0).unwrap();
        let first_empty = (found..found + 4).find(|&p| s.piles[p as usize].is_empty());
        for p in 0..s.piles.len() as u16 {
            if g.pile_ref(p).group == "tableau"
                && s.piles[p as usize].last().is_some_and(|c| c.rank() == 1)
            {
                if let Some(f) = first_empty {
                    return vec![(p, f, 1)];
                }
            }
        }
        Vec::new()
    }
}

#[test]
fn hooks() {
    let g = Game::with_hooks(
        mini_freecell(BuildBy::Alternate),
        std::sync::Arc::new(OddRules),
    )
    .unwrap();
    let (mut won, mut lost) = (0, 0);
    for seed in 0..40 {
        let s = g.deal(seed);
        let truth = brute_force(&g, &s, 2_000_000).unwrap();
        for strategy in [Strategy::Auto, Strategy::DepthFirst, Strategy::BestFirst] {
            let r = solve(
                &g,
                &s,
                &SolveOptions::default()
                    .strategy(strategy)
                    .max_states(10_000_000),
            );
            match (&r.outcome, truth) {
                (Outcome::Solved(m), true) => assert_wins(&g, &s, m),
                (Outcome::Unsolvable, false) => {}
                (o, t) => panic!("hooks seed {seed} {strategy:?}: solver {o:?}, brute force {t}"),
            }
        }
        if truth {
            won += 1
        } else {
            lost += 1
        }
    }
    println!("hooked mini freecell: {won} won, {lost} lost");
    assert!(won > 0 && lost > 0);
}

/// Poker Squares in miniature: a five-by-two grid, six cards placed (six more are set aside
/// face down at the deal). With `any_pool`, the set-aside pile could take cards from anywhere,
/// so the solver can't assume the next cards are exactly the stock's.
fn mini_poker(table: PokerTable, any_pool: bool, target: u32) -> Rules {
    let mut r = presets::poker_squares();
    r.name = format!("mini poker squares (any pool: {any_pool})");
    r.pack = small(1..=6, &[Suit::Spades, Suit::Hearts], 1);
    r.groups[2].count = 10;
    let mut aside = GroupRules::new("aside", PileKind::Foundation, 1)
        .build(BuildBy::None, Dir::Down)
        .empty(EmptyRule::None);
    if !any_pool {
        aside = aside.accepts_from(&["stock"]);
    }
    r.groups.push(aside);
    r.deal = vec![
        DealStep::to("aside").each(6).down(),
        DealStep::to("stock").rest().down(),
    ];
    r.score.as_mut().unwrap().hands = Hands::Poker(table);
    r.win = WinRule::Score { at_least: target };
    r
}

/// Points that don't follow the hands' ranking, so the bound can't lean on that order.
fn odd_points() -> PokerTable {
    let mut p = PokerPoints::default();
    p.pair = 3;
    p.two_pair = 2;
    p.three = 4;
    p.straight = 8;
    p.flush = 1;
    p.full_house = 5;
    p.four = 6;
    p.straight_flush = 9;
    p.royal_flush = 10;
    PokerTable::Points(p)
}

fn check_max(rules: Rules, seeds: std::ops::Range<u64>) {
    let g = Game::new(rules).unwrap();
    let seeds: Vec<u64> = seeds.collect();
    let mut scores = parallel_map(&seeds, |&seed| {
        let s = g.deal(seed);
        let truth = brute_force_max(&g, &s, 2_000_000).expect("mini game too big to brute-force");
        for strategy in [Strategy::Auto, Strategy::DepthFirst, Strategy::BestFirst] {
            let opts = SolveOptions::default()
                .strategy(strategy)
                .max_states(10_000_000);
            let r = maximize(&g, &s, &opts);
            assert!(
                r.proven,
                "{} seed {seed} {strategy:?}: not proven",
                g.name()
            );
            assert_eq!(r.score, truth, "{} seed {seed} {strategy:?}", g.name());
            let mut end = s.clone();
            for &m in &r.moves {
                g.play(&mut end, m).unwrap();
            }
            assert!(
                g.legal_moves(&end).is_empty(),
                "{} seed {seed}: line doesn't finish",
                g.name()
            );
            assert_eq!(
                g.score(&end),
                Some(truth),
                "{} seed {seed}: line scores wrong",
                g.name()
            );
        }
        truth
    });
    println!("{}: best scores {scores:?}", g.name());
    scores.sort();
    scores.dedup();
    assert!(
        scores.len() > 2,
        "{}: too little variety, test is weak",
        g.name()
    );
}

#[test]
fn poker_squares_best_american() {
    check_max(
        mini_poker(PokerTable::Named(PokerScoring::American), false, 1),
        0..12,
    );
}

#[test]
fn poker_squares_best_american_any_pool() {
    check_max(
        mini_poker(PokerTable::Named(PokerScoring::American), true, 1),
        12..24,
    );
}

#[test]
fn poker_squares_best_odd_points() {
    check_max(mini_poker(odd_points(), false, 1), 24..36);
}

#[test]
fn poker_squares_target_american() {
    check(
        mini_poker(PokerTable::Named(PokerScoring::American), false, 15),
        0..16,
    );
}

#[test]
fn poker_squares_target_odd_points() {
    check(mini_poker(odd_points(), true, 9), 16..32);
}

// ----- Jokers ----------------------------------------------------------------------------

/// The same game with `n` jokers added, wild on the named groups as well as the default ones.
fn with_jokers(mut r: Rules, n: u8, also_wild: &[&str]) -> Rules {
    r.name = format!("{} + {n} joker(s)", r.name);
    r.pack.jokers = n;
    for g in &mut r.groups {
        if also_wild.contains(&g.name.as_str()) {
            g.wild = Some(true);
        }
    }
    r
}

#[test]
fn jokers_klondike() {
    check(with_jokers(mini_klondike(1, None), 1, &[]), 0..40);
    check(with_jokers(mini_klondike(3, Some(0)), 2, &[]), 40..80);
}

/// A joker on a foundation stands for the next card, and blocks the card it stood for.
#[test]
fn jokers_wild_foundations() {
    check(
        with_jokers(mini_klondike(1, Some(1)), 1, &["foundation"]),
        0..40,
    );
}

#[test]
fn jokers_freecell() {
    check(
        with_jokers(mini_freecell(BuildBy::Alternate), 1, &[]),
        0..40,
    );
    check(with_jokers(mini_freecell(BuildBy::Suit), 2, &[]), 40..80);
}

#[test]
fn jokers_spider() {
    check(with_jokers(mini_spider(), 1, &[]), 0..40);
}

#[test]
fn jokers_yukon() {
    let mut r = with_jokers(mini_yukon(), 1, &[]);
    r.deal[1] = DealStep::to("tableau").counts(&[4, 5, 5, 5]);
    let mut by_suit = r.clone();
    by_suit.name += ", by suit";
    by_suit.groups[1].build = Some(Build::new(BuildBy::Suit, Dir::Down));
    check(by_suit, 0..40);
    // Not wild: the joker is dead weight that only an empty pile taking anything would hold.
    r.name += ", not wild";
    r.groups[1].wild = Some(false);
    r.groups[1].empty = Some(EmptyRule::Any);
    check(r, 40..80);
}

#[test]
fn jokers_golf() {
    check(with_jokers(mini_golf(), 1, &["foundation"]), 0..40);
}

#[test]
fn jokers_removal() {
    check(with_jokers(mini_pyramid(), 1, &[]), 0..40);
    // Two jokers in place of the sevens, so the deal still fits.
    let mut nestor = with_jokers(mini_nestor(), 2, &[]);
    nestor.pack.ranks = (1..=6).collect();
    check(nestor, 0..40);
    check(with_jokers(mini_elevens(), 1, &[]), 0..40);
}

/// A joker can fill a gap as the card after its left neighbour, and stands in a built row.
#[test]
fn jokers_gaps() {
    let mut r = with_jokers(mini_gaps(), 1, &[]);
    r.groups[1].count = 14;
    r.groups[1].layout = Some(Layout::Grid { columns: 7 });
    check(r, 0..40);
}

/// Jokers in Poker Squares: wild in every hand, with points that do and don't follow the
/// hands' ranking (English pays less for a flush than a straight).
fn joker_poker_rules(table: PokerTable, any_pool: bool, n: u8, target: u32) -> Rules {
    let mut r = with_jokers(mini_poker(table, any_pool, target), n, &[]);
    r.deal[0] = DealStep::to("aside").each(6 + n as u16).down();
    r
}

#[test]
fn jokers_poker_squares_american() {
    check_max(
        joker_poker_rules(PokerTable::Named(PokerScoring::American), false, 1, 1),
        0..12,
    );
}

#[test]
fn jokers_poker_squares_english_two_jokers() {
    check_max(
        joker_poker_rules(PokerTable::Named(PokerScoring::English), false, 2, 1),
        12..24,
    );
}

#[test]
fn jokers_poker_squares_odd_points() {
    check_max(joker_poker_rules(odd_points(), true, 1, 1), 24..36);
}

#[test]
fn jokers_poker_squares_target() {
    check(
        joker_poker_rules(PokerTable::Named(PokerScoring::American), false, 1, 20),
        0..16,
    );
}

/// Cribbage Squares in miniature: a four-by-two grid (two four-card rows, four two-card
/// columns) from a twelve-card pack, the starter turned after the grid is full.
fn mini_cribbage(starter: &str, any_pool: bool, target: u32) -> Rules {
    let mut r = presets::cribbage_squares();
    r.name = format!("mini cribbage squares (starter: {starter}, any pool: {any_pool})");
    r.pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[3, 4, 5, 6, 10, 11]);
    r.groups[2].count = 8;
    r.groups[2].layout = Some(Layout::Grid { columns: 4 });
    let mut aside = GroupRules::new("aside", PileKind::Foundation, 1)
        .build(BuildBy::None, Dir::Down)
        .empty(EmptyRule::None);
    if !any_pool {
        aside = aside.accepts_from(&["stock"]);
    }
    r.groups.push(aside);
    r.deal = vec![DealStep::to("aside").each(2).down()];
    if starter == "fixed" {
        r.groups.push(
            GroupRules::new("starter", PileKind::Reserve, 1)
                .moves(MoveUnit::None)
                .empty(EmptyRule::None),
        );
        r.deal.push(DealStep::to("starter").each(1));
        if let Hands::Cribbage(c) = &mut r.score.as_mut().unwrap().hands {
            c.starter = Some("starter".into());
        }
    }
    r.deal.push(DealStep::to("stock").rest().down());
    r.win = WinRule::Score { at_least: target };
    r
}

#[test]
fn cribbage_squares_best_waste() {
    check_max(mini_cribbage("waste", false, 1), 0..12);
}

#[test]
fn cribbage_squares_best_waste_any_pool() {
    check_max(mini_cribbage("waste", true, 1), 12..24);
}

#[test]
fn cribbage_squares_best_fixed() {
    check_max(mini_cribbage("fixed", false, 1), 24..36);
}

#[test]
fn cribbage_squares_best_joker() {
    check_max(
        with_jokers(mini_cribbage("waste", false, 1), 1, &[]),
        36..48,
    );
}

#[test]
fn cribbage_squares_best_odd_points() {
    let mut odd = mini_cribbage("fixed", true, 1);
    if let Hands::Cribbage(c) = &mut odd.score.as_mut().unwrap().hands {
        let p = &mut c.points;
        p.fifteen = 1;
        p.pair = 3;
        p.run_card = 2;
        p.flush = 7;
        p.flush_starter = 2;
        p.nobs = 5;
        p.heels = 4;
    }
    check_max(odd, 48..60);
}

#[test]
fn cribbage_squares_target() {
    check(mini_cribbage("waste", false, 35), 0..16);
    check(mini_cribbage("fixed", true, 36), 16..32);
}

#[test]
fn top_or_whole() {
    let r = Rules::new(
        "mini top_or_whole",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::TopOrWhole),
        ],
        vec![DealStep::to("tableau").counts(&[3, 3, 2, 2])],
    );
    check(r, 0..40);
}

#[test]
fn empty_suit_foundations() {
    let r = Rules::new(
        "mini empty_suit",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Alternate, Dir::Up)
                .moves(MoveUnit::Top)
                .pile(PileOverride::on(&[0]).empty_suit(Suit::Spades))
                .pile(PileOverride::on(&[1]).empty_suit(Suit::Hearts)),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").counts(&[3, 3, 2, 2])],
    );
    check(r, 0..40);
}

#[test]
fn deal_of_base() {
    let mut step0 = DealStep::to("foundation").piles(&[0]).each(1);
    step0.set_base = true;
    let mut step1 = DealStep::to("foundation").piles(&[1]);
    step1.of_base = true;
    let r = Rules::new(
        "mini of_base",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build_rule(Build::new(BuildBy::Suit, Dir::Up).wrap())
                .empty(EmptyRule::DealtBase)
                .max_cards(5),
            GroupRules::new("cell", PileKind::Cell, 2),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build_rule(Build::new(BuildBy::Suit, Dir::Down).wrap())
                .empty(EmptyRule::DealtBaseOffset(-1))
                .moves(MoveUnit::Run),
        ],
        vec![step0, step1, DealStep::to("tableau").rest()],
    );
    check(r, 0..40);
}

#[test]
fn stock_to_multi_reserve() {
    let mut r = Rules::new(
        "mini stock to reserve",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("reserve", PileKind::Reserve, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .empty(EmptyRule::None),
        ],
        vec![
            DealStep::to("tableau").each(2),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("reserve"));
    check(r, 0..40);
}

#[test]
fn stock_skip_empty() {
    let mut r = Rules::new(
        "mini stock skip_empty",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .complete_runs()
                .build_rule(Build::new(BuildBy::Suit, Dir::Down).wrap())
                .empty(EmptyRule::Any),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build_rule(Build::new(BuildBy::Any, Dir::Down).wrap())
                .moves(MoveUnit::SuitRun),
        ],
        vec![
            DealStep::to("tableau").each(2),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.skip_empty = true;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn stock_face_up_removal() {
    let mut r = Rules::new(
        "mini face_up stock pyramid",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5, 6]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("discard", PileKind::Foundation, 1)
                .build(BuildBy::None, Dir::Up)
                .empty(EmptyRule::None),
            GroupRules::new("tableau", PileKind::Tableau, 6)
                .empty(EmptyRule::None)
                .moves(MoveUnit::None)
                .layout(Layout::Pyramid { rows: 3 }),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.redeals = 1;
    st.face_up = true;
    r.stock = Some(st);
    r.removal = vec![RemovalRules::new(
        RemovalKind::PairsSum {
            total: 6,
            alone: vec![6],
        },
        &["tableau", "waste", "stock"],
        "discard",
    )];
    r.win = WinRule::Empty(vec!["tableau".into()]);
    check(r, 0..40);
}

#[test]
fn requires_rank_on_prev() {
    let mut r = Rules::new(
        "mini requires_rank_on_prev",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Diamonds], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3)
                .build(BuildBy::Suit, Dir::Any)
                .empty(EmptyRule::Any)
                .requires_rank_on_prev(),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("tableau").each(2),
            DealStep::to("foundation").piles(&[0]).each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.unlimited_redeals = true;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn deal_ranks_with_count_and_suit() {
    let r = Rules::new(
        "mini deal ranks count",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]).with_copies(2),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .pile(PileOverride::on(&[0]).suit(Suit::Spades))
                .pile(PileOverride::on(&[1]).suit(Suit::Hearts)),
            GroupRules::new("foundation2", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("foundation").ranks(&[1]).each(1),
            DealStep::to("tableau").rest(),
        ],
    );
    check(r, 0..40);
}

#[test]
fn stock_tableau_redeal() {
    let mut r = Rules::new(
        "mini stock tableau redeal",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Diamonds], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.redeals = 1;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn match_longest_rank() {
    let mut r = Rules::new(
        "mini match_longest_rank",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Diamonds], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3)
                .build(BuildBy::Suit, Dir::Any)
                .empty(EmptyRule::Any)
                .match_longest_rank()
                .pile(PileOverride::on(&[0]).suit(Suit::Spades))
                .pile(PileOverride::on(&[1]).suit(Suit::Hearts))
                .pile(PileOverride::on(&[2]).suit(Suit::Diamonds)),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("tableau").each(2),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.unlimited_redeals = true;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn empty_max_accept() {
    let r = Rules::new(
        "mini empty_max_accept",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any)
                .empty_max_accept(1),
        ],
        vec![DealStep::to("tableau").each(3)],
    );
    check(r, 0..40);
}

#[test]
fn accept_when_stock_empty_and_empty_only() {
    let mut r = Rules::new(
        "mini stock_empty reserve",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("cell", PileKind::Cell, 1)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any)
                .max_cards(13)
                .max_accept(13)
                .accept_only_when_empty()
                .accept_when_stock_empty(),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Rank(4)),
        ],
        vec![
            DealStep::to("tableau").each(2),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("tableau"));
    check(r, 0..40);
}

#[test]
fn complete_runs_by_any_and_alternate() {
    let r = Rules::new(
        "mini complete runs alternate",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]).with_copies(2),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 4)
                .build(BuildBy::Alternate, Dir::Down)
                .complete_runs(),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::AnyFaceUp)
                .empty(EmptyRule::Any),
        ],
        vec![DealStep::to("tableau").each(4)],
    );
    check(r, 0..40);
}

#[test]
fn empty_ranks_list() {
    let r = Rules::new(
        "mini empty ranks",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Suit, Dir::Either)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Ranks(vec![1, 4])),
        ],
        vec![DealStep::to("tableau").each(3)],
    );
    check(r, 0..40);
}

#[test]
fn compact_face_up() {
    let mut r = Rules::new(
        "mini compact face up",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Rank(4))
                .compact_face_up(),
        ],
        vec![
            DealStep::to("tableau").counts(&[0, 1, 1, 1]).down(),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("waste"));
    check(r, 0..40);
}

#[test]
fn refill_largest_reserve() {
    let mut r = Rules::new(
        "mini refill largest",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("reserve", PileKind::Reserve, 2),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any)
                .refill_from(&["reserve", "waste"])
                .refill_largest()
                .refill_on_empty(),
        ],
        vec![
            DealStep::to("reserve").counts(&[2, 3]),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("waste"));
    check(r, 0..40);
}

#[test]
fn disjoint_override_foundations_and_sym_classes() {
    let mut r = Rules::new(
        "mini disjoint override foundations",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]).with_copies(2),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 8)
                .build(BuildBy::Suit, Dir::Down)
                .empty(EmptyRule::Rank(2))
                .max_cards(2)
                .pile({
                    let mut o = PileOverride::on(&[4, 5, 6, 7])
                        .build(Build::new(BuildBy::Suit, Dir::Up))
                        .empty(EmptyRule::Rank(3));
                    o.max_cards = Some(2);
                    o
                }),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .empty(EmptyRule::Any)
                .max_cards(1)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.redeals = 1;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn dump_pile_safe_autoplay() {
    let mut r = Rules::new(
        "mini dump pile autoplay",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Any, Dir::Any)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top)
                .max_accept(1)
                .accepts_from(&["waste"]),
        ],
        vec![DealStep::to("stock").rest().down()],
    );
    let mut st = StockRules::to("waste");
    st.only_when_empty = true;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn stock_count_per_round() {
    let mut r = Rules::new(
        "mini count per round",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.count_per_round = vec![3, 2, 1];
    st.redeals = 2;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn stock_final_to() {
    let mut r = Rules::new(
        "mini stock final_to",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("reserve", PileKind::Reserve, 2),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Suit, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("reserve").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("reserve");
    st.count = 2;
    st.final_to = Some("tableau".into());
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn stock_specific_piles() {
    let mut r = Rules::new(
        "mini stock piles",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any),
        ],
        vec![
            DealStep::to("tableau").each(2),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.piles = Some(vec![0]);
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn refill_via_multi_waste() {
    let mut r = Rules::new(
        "mini multi-waste refill",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any)
                .refill_from(&["waste", "stock"]),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.count = 2;
    st.redeals = 1;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn requires_prev_non_empty() {
    let mut r = Rules::new(
        "mini requires prev non empty",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 3)
                .build_rule(Build::new(BuildBy::Any, Dir::Down).wrap())
                .empty(EmptyRule::Rank(4))
                .max_cards(4)
                .requires_prev_non_empty()
                .pile(PileOverride::on(&[1]).empty(EmptyRule::Rank(3)))
                .pile(PileOverride::on(&[2]).empty(EmptyRule::Rank(2))),
        ],
        vec![DealStep::to("stock").rest().down()],
    );
    let mut st = StockRules::to("waste");
    st.redeals = 2;
    r.stock = Some(st);
    check(r, 0..40);
}

#[test]
fn max_same_rank_and_one_per_suit() {
    let mut step_f = DealStep::to("foundation");
    step_f.one_per_suit = true;
    let r = Rules::new(
        "mini max same rank + one per suit",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 3)
                .build_rule(Build::new(BuildBy::Suit, Dir::Up).wrap())
                .empty(EmptyRule::Any)
                .max_cards(4),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Any, Dir::Same)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::None)
                .max_same_rank(2),
        ],
        vec![step_f, DealStep::to("tableau").rest()],
    );
    check(r, 0..40);
}

#[test]
fn card_dependent_deal_steps() {
    let mut split_step = DealStep::to("tableau").rest();
    split_step.split_on_rank = Some(4);
    split_step.divert_ranks = Some(vec![1]);
    let r = Rules::new(
        "mini split + divert deal",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("cell", PileKind::Cell, 2),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::None),
        ],
        vec![split_step],
    );
    check(r, 0..40);

    let mut drain_step = DealStep::to("tableau").each(3);
    drain_step.drain_foundations = true;
    let r2 = Rules::new(
        "mini drain deal",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 3),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any),
        ],
        vec![drain_step],
    );
    check(r2, 0..40);
}

#[test]
fn limit_deal_waste_max_and_min_cards() {
    let mut step_f = DealStep::to("foundation").ranks(&[1]);
    step_f.limit = Some(2);
    let mut r_limit = Rules::new(
        "mini deal limit",
        Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 3)
                .pile(PileOverride::on(&[0]).suit(Suit::Spades))
                .pile(PileOverride::on(&[1]).suit(Suit::Hearts))
                .pile(PileOverride::on(&[2]).suit(Suit::Clubs)),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any),
        ],
        vec![step_f, DealStep::to("tableau").rest()],
    );
    r_limit.groups[0].max_cards = Some(4);
    check(r_limit, 0..40);

    let mut r_waste = Rules::new(
        "mini waste max_cards + min_cards",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1).max_cards(2),
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build_rule(Build::new(BuildBy::Any, Dir::Up).wrap())
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top)
                .min_cards(1)
                .max_cards(4),
            GroupRules::new("cell", PileKind::Cell, 1),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::None),
        ],
        vec![
            DealStep::to("foundation").each(1),
            DealStep::to("tableau").each(2),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.count = 2;
    st.redeals = 1;
    r_waste.stock = Some(st);
    check(r_waste, 0..40);
}

#[test]
fn rotate_bottom_final_piles_and_moves_when_stock_empty() {
    let mut r_rot = Rules::new(
        "mini rotate bottom",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build_rule(Build::new(BuildBy::Suit, Dir::Either).wrap())
                .moves(MoveUnit::Top)
                .empty(EmptyRule::None),
        ],
        vec![DealStep::to("tableau").rest()],
    );
    let mut rd = RedealRules::new("tableau", RedealHow::RotateBottom);
    rd.times = 2;
    r_rot.redeal = Some(rd);
    check(r_rot, 0..40);

    let mut r_fp = Rules::new(
        "mini final_piles + moves_when_stock_empty",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .moves(MoveUnit::Top)
                .moves_when_stock_empty(),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.final_piles = Some(vec![1, 2]);
    r_fp.stock = Some(st);
    check(r_fp, 0..40);
}

#[test]
fn alt_color_and_same_color_runs() {
    let pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]);
    let r_alt = Rules::new(
        "mini-alt-run",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .complete_runs()
                .build(BuildBy::Alternate, Dir::Down),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::AltColorRun),
        ],
        vec![DealStep::to("tableau").counts(&[3, 3, 2])],
    );
    check(r_alt, 0..40);

    let r_sc = Rules::new(
        "mini-same-color-run",
        Pack::new(&[Suit::Spades, Suit::Clubs], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .complete_runs()
                .build(BuildBy::SameColor, Dir::Down),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::SameColorRun),
        ],
        vec![DealStep::to("tableau").counts(&[3, 3, 2])],
    );
    check(r_sc, 0..40);
}

#[test]
fn multi_card_refill_and_stock_modes() {
    let pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5]);
    // Multi-card refill + deal_per_pile + redeal_reverse.
    let mut r_per = Rules::new(
        "mini-per-pile",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .empty(EmptyRule::None)
                .refill_from(&["stock"])
                .refill_count(2),
            GroupRules::new("cell", PileKind::Cell, 2),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("cell").each(1),
            DealStep::to("tableau").each(2),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.count = 2;
    st.redeals = 1;
    st.deal_per_pile = true;
    st.redeal_reverse = true;
    r_per.stock = Some(st);
    check(r_per, 0..40);

    // deal_empty_first + CountAndSpread.
    let mut r_ef = Rules::new(
        "mini-empty-first",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st2 = StockRules::to("tableau");
    st2.deal_empty_first = true;
    r_ef.stock = Some(st2);
    r_ef.win = WinRule::CountAndSpread {
        group: "foundation".into(),
        cards: 7,
        spread: "tableau".into(),
    };
    check(r_ef, 0..40);
}

#[test]
fn choose_base_and_empty_accepts_from() {
    let pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]);
    let mut r_cb = Rules::new(
        "mini-choose-base",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build_rule(Build::new(BuildBy::Alternate, Dir::Up).wrap())
                .empty(EmptyRule::DealtBase)
                .max_cards(4)
                .pile(PileOverride::on(&[0]).empty_suit(Suit::Spades))
                .pile(PileOverride::on(&[1]).empty_suit(Suit::Hearts)),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build_rule(Build::new(BuildBy::Alternate, Dir::Down).wrap())
                .accepts_from(&["tableau", "waste"])
                .empty_accepts_from(&["waste"]),
            GroupRules::new("reserve", PileKind::Reserve, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("reserve").each(2),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r_cb.stock = Some(StockRules::to("waste"));
    let mut cb = ChooseBase::from("tableau");
    cb.refill_from_stock = true;
    cb.deal_waste = true;
    r_cb.choose_base = Some(cb);
    check(r_cb, 0..40);

    // Dealt set_base with empty_suit matching.
    let mut r_sb = Rules::new(
        "mini-set-base-suit",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build_rule(Build::new(BuildBy::Alternate, Dir::Up).wrap())
                .empty(EmptyRule::DealtBase)
                .max_cards(4)
                .pile(PileOverride::on(&[0]).empty_suit(Suit::Spades))
                .pile(PileOverride::on(&[1]).empty_suit(Suit::Hearts)),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build_rule(Build::new(BuildBy::Alternate, Dir::Down).wrap())
                .accepts_from(&["tableau", "waste"]),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("foundation").each(1).set_base(),
            DealStep::to("waste").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r_sb.stock = Some(StockRules::to("waste"));
    check(r_sb, 0..40);
}

#[test]
fn waste_talon_with_moves_when_stock_empty_and_min_cards_override() {
    // Waste talon + moves_when_stock_empty must not enable talon compression, otherwise the
    // solver cannot deal out the stock to unlock the gated group.
    let mut r = Rules::new(
        "mini-waste-moves-when-stock-empty",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("reserve", PileKind::Reserve, 1).moves_when_stock_empty(),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any),
        ],
        vec![
            DealStep::to("reserve").each(2),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("waste"));
    check(r, 0..40);

    // Per-pile min_cards override must not collapse distinct piles into one symmetry class.
    let mut ov = PileOverride::on(&[0]);
    ov.min_cards = Some(1);
    let mut r_min = Rules::new(
        "mini-min-cards-override-sym",
        Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any)
                .pile(ov),
        ],
        vec![DealStep::to("tableau").counts(&[2, 3, 3])],
    );
    r_min.win = WinRule::Count {
        group: "foundation".into(),
        cards: 7,
    };
    check(r_min, 0..40);
}

#[test]
fn flip_only_last_and_stock_divert_ranks() {
    let pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]);

    // Reserve with flip_only_last refills tableau face-up while staying face-down until 1 card left.
    let mut r_fol = Rules::new(
        "mini-flip-only-last",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("reserve", PileKind::Reserve, 1).flip_only_last(),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Alternate, Dir::Down)
                .moves(MoveUnit::Top)
                .refill_from(&["reserve"]),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("reserve").each(3).down(),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r_fol.stock = Some(StockRules::to("waste"));
    check(r_fol, 0..40);

    // Stock dealing to tableau diverts Aces straight to foundations and keeps dealing to same pile.
    let mut r_div = Rules::new(
        "mini-stock-divert-ranks",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Top),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.divert_ranks = vec![1];
    r_div.stock = Some(st);
    check(r_div, 0..40);
}

#[test]
fn grid_shuffle_random_gaps_and_after_pile_final() {
    let pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]);

    // Grid shuffle with random gap placement.
    let mut r_gap = Rules::new(
        "mini-grid-random-gaps",
        pack.clone(),
        vec![
            GroupRules::new("tableau", PileKind::Tableau, 8)
                .layout(Layout::Grid { columns: 4 })
                .build(BuildBy::Suit, Dir::Up)
                .empty(EmptyRule::AfterLeft { first: 2 })
                .moves(MoveUnit::Top)
                .max_cards(1),
            GroupRules::new("discard", PileKind::Reserve, 1)
                .moves(MoveUnit::None)
                .empty(EmptyRule::None),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("discard").collect(&[1]),
        ],
    );
    r_gap.redeal = Some(RedealRules::new(
        "tableau",
        RedealHow::GridShuffleRandomGaps,
    ));
    r_gap.win = WinRule::Rows {
        group: "tableau".into(),
        length: Some(3),
    };
    check(r_gap, 0..40);

    // Per-pile dynamic base rank (after_pile) + final card must come from same-index reserve.
    let mut r_ap = Rules::new(
        "mini-after-pile-final",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build_rule(Build::new(BuildBy::Any, Dir::Up).wrap())
                .empty(EmptyRule::AfterPile {
                    group: "reserve".into(),
                    offset: 1,
                })
                .max_cards(4)
                .accepts_from(&["tableau"])
                .final_from_same_index("reserve"),
            GroupRules::new("reserve", PileKind::Reserve, 2),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::None),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("reserve").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r_ap.stock = Some(StockRules::to("tableau"));
    check(r_ap, 0..40);
}

#[test]
fn lock_build_dir_no_flip_refill_same_index_and_piles_per_round() {
    let pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]);

    // lock_build_dir: first 2-card foundation locks direction (up or down, wrapping) for all.
    let mut r_lbd = Rules::new(
        "mini-lock-build-dir",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build_rule(Build::new(BuildBy::Suit, Dir::Either).wrap())
                .empty(EmptyRule::DealtBase)
                .max_cards(4)
                .lock_build_dir(),
            GroupRules::new("reserve", PileKind::Reserve, 1).no_flip(),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any)
                .max_cards(1)
                .refill_from(&["reserve"]),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("foundation").piles(&[0]).each(1).set_base(),
            DealStep::to("reserve").each(2).down(),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.redeals = 1;
    r_lbd.stock = Some(st);
    check(r_lbd, 0..40);

    // refill_same_index + piles_per_round (shrinking target piles across rounds).
    let mut r_well = Rules::new(
        "mini-piles-per-round",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("reserve", PileKind::Reserve, 2),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Suit, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any)
                .refill_from(&["reserve"])
                .refill_same_index(),
            GroupRules::new("wastes", PileKind::Reserve, 2),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("reserve").each(1),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st2 = StockRules::to("wastes");
    st2.redeals = 1;
    st2.piles_per_round = vec![vec![0, 1], vec![0]];
    r_well.stock = Some(st2);
    check(r_well, 0..40);
}

#[test]
fn require_valid_base_and_multi_source_refill_count() {
    let pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5]);
    let mut r = Rules::new(
        "mini-gallery",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::Any, Dir::Same)
                .empty(EmptyRule::Rank(1))
                .max_cards(2),
            GroupRules::new("gallery", PileKind::Tableau, 4)
                .build_rule(Build::new(BuildBy::Suit, Dir::Up).step(2))
                .moves(MoveUnit::Top)
                .max_cards(2)
                .max_accept(1)
                .require_valid_base()
                .pile(PileOverride::on(&[0, 1]).empty(EmptyRule::Rank(2)))
                .pile(PileOverride::on(&[2, 3]).empty(EmptyRule::Rank(3))),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Suit, Dir::Down)
                .moves(MoveUnit::Top)
                .max_accept(1)
                .refill_from(&["waste", "stock"])
                .refill_count(2),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("stock", PileKind::Stock, 1),
        ],
        vec![
            DealStep::to("gallery").each(1),
            DealStep::to("tableau").each(1),
            DealStep::to("waste").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.auto = true;
    r.stock = Some(st);
    r.win = WinRule::Empty(vec!["stock".into(), "waste".into(), "tableau".into()]);
    check(r, 0..40);
}

#[test]
fn ribbon_replace_and_removals() {
    let pack = Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4, 5]);

    // Relaxed ribbon with replace_on_drop + reach_both_ways + compact.
    let mut row = GroupRules::new("tableau", PileKind::Tableau, 10)
        .build(BuildBy::SuitOrRank, Dir::Any)
        .moves(MoveUnit::Top)
        .empty(EmptyRule::None)
        .max_cards(1);
    row.reach = Some(vec![1, 3]);
    row.reach_both_ways = true;
    row.replace_on_drop = Some("foundation".into());
    row.compact = true;
    let mut r_acc = Rules::new(
        "mini-relaxed-ribbon",
        pack.clone(),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::None, Dir::Up)
                .empty(EmptyRule::None),
            row,
        ],
        vec![
            DealStep::to("tableau").piles(&[0, 1, 2]).each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.first_empty = true;
    r_acc.stock = Some(st.clone());
    r_acc.win = WinRule::Count {
        group: "foundation".into(),
        cards: 9,
    };
    check(r_acc, 0..40);

    // ReachSuitOrRank, BetweenMatch, and ContiguousSumMultiple removals on compacting ribbons.
    for (name, kind, target_cards) in [
        (
            "mini-reach-remove",
            RemovalKind::ReachSuitOrRank {
                distances: vec![1, 3],
            },
            10u16,
        ),
        (
            "mini-between-match",
            RemovalKind::BetweenMatch { max_len: 2 },
            8u16,
        ),
        (
            "mini-contiguous-sum",
            RemovalKind::ContiguousSumMultiple {
                divisor: 5,
                max_len: 3,
            },
            10u16,
        ),
    ] {
        let mut t = GroupRules::new("tableau", PileKind::Tableau, 10)
            .empty(EmptyRule::None)
            .moves(MoveUnit::None)
            .max_cards(1);
        t.compact = true;
        let mut r = Rules::new(
            name,
            pack.clone(),
            vec![
                GroupRules::new("stock", PileKind::Stock, 1),
                GroupRules::new("foundation", PileKind::Foundation, 1)
                    .build(BuildBy::None, Dir::Up)
                    .empty(EmptyRule::None),
                t,
            ],
            vec![
                DealStep::to("tableau").piles(&[0, 1, 2]).each(1),
                DealStep::to("stock").rest().down(),
            ],
        );
        r.stock = Some(st.clone());
        r.removal = vec![RemovalRules::new(kind, &["tableau"], "foundation")];
        r.win = WinRule::Count {
            group: "foundation".into(),
            cards: target_cards,
        };
        check(r, 0..40);
    }
}

#[test]
fn neighbor_refill_nearest_and_final_split() {
    // 1) AfterNeighbor grid (2 suits x 4 ranks, 10 slots = 2 rows of 5).
    let grid_pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);
    let mut r_nb = Rules::new(
        "mini-after-neighbor",
        grid_pack,
        vec![GroupRules::new("tableau", PileKind::Tableau, 10)
            .build(BuildBy::Suit, Dir::Up)
            .empty(EmptyRule::AfterNeighbor { first: 1 })
            .moves(MoveUnit::Top)
            .max_cards(1)
            .layout(Layout::Grid { columns: 5 })],
        vec![DealStep::to("tableau")
            .piles(&[1, 2, 3, 4, 6, 7, 8, 9])
            .each(1)],
    );
    r_nb.redeal = Some(RedealRules::new("tableau", RedealHow::GridShuffle));
    r_nb.win = WinRule::Rows {
        group: "tableau".into(),
        length: Some(4),
    };
    check(r_nb, 0..40);

    // 2) refill_nearest from face-down no-flip reserves.
    let small_pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4, 5]);
    let r_near = Rules::new(
        "mini-refill-nearest",
        small_pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("reserve", PileKind::Reserve, 3)
                .moves(MoveUnit::None)
                .no_flip(),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Suit, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top)
                .max_accept(1)
                .refill_from(&["reserve"])
                .refill_nearest(),
        ],
        vec![
            DealStep::to("reserve").counts(&[2, 2, 3]).down(),
            DealStep::to("tableau").each(1),
        ],
    );
    check(r_near, 0..40);

    // 3) final_split_to: deal 2 to reserves, then when 2 remain in stock deal 1 to waste and flip last stock card.
    let mut r_split = Rules::new(
        "mini-final-split",
        small_pack,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1)
                .moves(MoveUnit::Top)
                .no_flip(),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("reserve", PileKind::Reserve, 2),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Run),
        ],
        vec![
            DealStep::to("tableau").each(2),
            DealStep::to("reserve").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("reserve");
    st.final_split_to = Some("waste".into());
    r_split.stock = Some(st);
    check(r_split, 0..40);
}

#[test]
fn moves_when_empty_gates() {
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4, 5]);
    // 1) moves_when_any_empty
    let r_any = Rules::new(
        "mini-moves-when-any-empty",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("cell", PileKind::Cell, 1).moves_when_any_empty("reserve"),
            GroupRules::new("reserve", PileKind::Reserve, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Suit, Dir::Either)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("reserve").each(1),
            DealStep::to("tableau").counts(&[3, 3, 2]),
        ],
    );
    check(r_any, 0..40);

    // 2) moves_when_same_index_empty
    let r_same = Rules::new(
        "mini-moves-when-same-index-empty",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("cell", PileKind::Cell, 2).moves_when_same_index_empty("reserve"),
            GroupRules::new("reserve", PileKind::Reserve, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Suit, Dir::Either)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("reserve").each(1),
            DealStep::to("tableau").counts(&[3, 3, 2]),
        ],
    );
    check(r_same, 0..40);
}

#[test]
fn swap_bases_and_all_full_win() {
    // 1) SwapBases::BothSingle + require_no_empty_group (2 suits x 5 ranks: A=1, 2, 3, 4, 5).
    let pack1 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4, 5]);
    let gallery1 = GroupRules::new("gallery", PileKind::Tableau, 4)
        .build_rule(Build::new(BuildBy::Suit, Dir::Up).step(2))
        .moves(MoveUnit::Top)
        .max_accept(1)
        .require_valid_base()
        .swap_bases(SwapBases::BothSingle)
        .pile(
            PileOverride::on(&[0, 1])
                .empty(EmptyRule::Rank(2))
                .build(Build::new(BuildBy::Suit, Dir::Up).step(2)),
        )
        .pile(
            PileOverride::on(&[2, 3])
                .empty(EmptyRule::Rank(3))
                .build(Build::new(BuildBy::Suit, Dir::Up).step(2)),
        );
    let mut r1 = Rules::new(
        "mini-swap-single",
        pack1,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::Any, Dir::Same)
                .empty(EmptyRule::Rank(1))
                .moves(MoveUnit::None)
                .max_cards(2),
            gallery1,
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("gallery").each(1),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.require_no_empty_group = Some("gallery".into());
    r1.stock = Some(st);
    r1.win = WinRule::Empty(vec!["stock".into(), "tableau".into()]);
    check(r1, 0..40);

    // 2) SwapBases::EitherPile + WinRule::AllFull (2 suits x 4 ranks: 2, 3, 4, 5).
    let pack2 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[2, 3, 4, 5]);
    let gallery2 = GroupRules::new("gallery", PileKind::Tableau, 4)
        .build_rule(Build::new(BuildBy::Suit, Dir::Up).step(2))
        .moves(MoveUnit::Run)
        .max_cards(2)
        .max_accept(2)
        .refill_from(&["stock", "waste"])
        .swap_bases(SwapBases::EitherPile)
        .pile(PileOverride::on(&[0, 1]).empty(EmptyRule::Rank(2)))
        .pile(PileOverride::on(&[2, 3]).empty(EmptyRule::Rank(3)));
    let mut r2 = Rules::new(
        "mini-swap-either",
        pack2,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            gallery2,
        ],
        vec![
            DealStep::to("gallery").each(1),
            DealStep::to("waste").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st2 = StockRules::to("waste");
    st2.unlimited_redeals = true;
    r2.stock = Some(st2);
    r2.win = WinRule::AllFull {
        group: "gallery".into(),
    };
    check(r2, 0..40);
}

#[test]
fn stock_redeal_shuffle_and_final_to() {
    // 1) StockRules::redeal_shuffle (2 suits x 4 ranks = 8 cards, 2 tableau piles, 1 redeal).
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);
    let mut r1 = Rules::new(
        "mini-redeal-shuffle",
        pack.clone(),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st1 = StockRules::to("tableau");
    st1.redeals = 1;
    st1.redeal_shuffle = true;
    r1.stock = Some(st1);
    check(r1, 0..40);

    // 2) StockRules::final_to bypassing require_no_empty (2 suits x 5 ranks = 10 cards).
    let pack2 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4, 5]);
    let mut r2 = Rules::new(
        "mini-final-to-no-empty",
        pack2,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("reserve", PileKind::Reserve, 2)
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Run),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st2 = StockRules::to("tableau");
    st2.require_no_empty = true;
    st2.final_to = Some("reserve".into());
    r2.stock = Some(st2);
    check(r2, 0..40);
}

#[test]
fn wave3_mechanics() {
    // 1) Dir::UpAny + WinRule::Rows with custom build (2 suits x 4 ranks in 2x5 grid).
    let pack1 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);
    let mut r1 = Rules::new(
        "mini-up-any-rows",
        pack1,
        vec![GroupRules::new("tableau", PileKind::Tableau, 10)
            .layout(Layout::Grid { columns: 5 })
            .build(BuildBy::Suit, Dir::UpAny)
            .empty(EmptyRule::AfterLeft { first: 1 })
            .moves(MoveUnit::Top)
            .max_cards(1)],
        vec![DealStep::to("tableau")
            .piles(&[1, 2, 3, 4, 6, 7, 8, 9])
            .each(1)],
    );
    r1.win = WinRule::RowsBuilt {
        group: "tableau".into(),
        length: Some(4),
        build: Build::new(BuildBy::Suit, Dir::Up),
    };
    check(r1, 0..40);

    // 2) GroupRules::unrestricted_from (2 suits x 4 ranks, waste drops anywhere on tableau).
    let pack2 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);
    let mut r2 = Rules::new(
        "mini-unrestricted-from",
        pack2,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .empty(EmptyRule::Rank(4))
                .moves(MoveUnit::Top)
                .max_accept(1)
                .accepts_from(&["tableau"])
                .unrestricted_from(&["waste"]),
        ],
        vec![
            DealStep::to("waste").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r2.stock = Some(StockRules::to("waste"));
    check(r2, 0..40);

    // 3) GroupRules::lock_pile_dir + GroupRules::rank_sequence (2 copies x 1 suit x 3 ranks = 6 cards).
    let pack3 = Pack::new(&[Suit::Clubs], &[1, 2, 3]).with_copies(2);
    let mut r3 = Rules::new(
        "mini-lock-pile-and-rank-seq",
        pack3,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::Suit, Dir::Any)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::None)
                .max_cards(6)
                .rank_sequence(&[1, 2, 3, 3, 2, 1]),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Suit, Dir::Either)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top)
                .max_accept(1)
                .lock_pile_dir(),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("waste").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r3.stock = Some(StockRules::to("waste"));
    check(r3, 0..40);

    // 4) GroupRules::supermove_complete_run (2 suits x 3 ranks = 6 cards, 0 cells, 3 columns).
    let pack4 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let r4 = Rules::new(
        "mini-supermove-complete",
        pack4,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Alternate, Dir::Down)
                .complete_runs(),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Supermove)
                .supermove_complete_run(),
        ],
        vec![DealStep::to("tableau").each(2)],
    );
    check(r4, 0..40);
}

#[test]
fn non_empty_max_accept_unique_suits_and_stock_restrictions() {
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);

    // 1) GroupRules::non_empty_max_accept (runs on empty, 1 card on non-empty).
    let mut tab1 = GroupRules::new("tableau", PileKind::Tableau, 3)
        .build(BuildBy::Alternate, Dir::Down)
        .empty(EmptyRule::Any)
        .moves(MoveUnit::Run);
    tab1.non_empty_max_accept = Some(1);
    let r1 = Rules::new(
        "mini-non-empty-max-accept",
        pack.clone(),
        vec![GroupRules::new("foundation", PileKind::Foundation, 2), tab1],
        vec![DealStep::to("tableau").counts(&[3, 3, 2])],
    );
    check(r1, 0..40);

    // 2) GroupRules::unique_suits (2 foundations with different base ranks, distinct suits).
    let pack2 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let mut f2 = GroupRules::new("foundation", PileKind::Foundation, 2)
        .build_rule(Build::new(BuildBy::Suit, Dir::Up).wrap())
        .max_cards(3)
        .pile(PileOverride::on(&[0]).empty(EmptyRule::Rank(1)))
        .pile(PileOverride::on(&[1]).empty(EmptyRule::Rank(2)));
    f2.unique_suits = true;
    let r2 = Rules::new(
        "mini-unique-suits",
        pack2,
        vec![
            f2,
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build_rule(Build::new(BuildBy::Alternate, Dir::Down).wrap())
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Run),
        ],
        vec![DealStep::to("tableau").each(2)],
    );
    check(r2, 0..40);

    // 3) no_build_single_while_stock + refill_first_face_down.
    let mut tab3 = GroupRules::new("tableau", PileKind::Tableau, 2)
        .build(BuildBy::OtherSuit, Dir::Down)
        .empty(EmptyRule::Any)
        .moves(MoveUnit::Top)
        .refill_from(&["stock"]);
    tab3.refill_count = Some(2);
    tab3.no_build_single_while_stock = true;
    tab3.refill_first_face_down = true;
    let mut r3 = Rules::new(
        "mini-no-build-single",
        pack.clone(),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2),
            tab3,
        ],
        vec![
            DealStep::to("tableau").each(1).down(),
            DealStep::to("tableau").each(1),
            DealStep::to("waste").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r3.stock = Some(StockRules::to("waste"));
    check(r3, 0..40);

    // 4) Open-top stock accepting cards once empty (or onto face-up cards).
    let mut st_grp = GroupRules::new("stock", PileKind::Stock, 1)
        .build(BuildBy::Any, Dir::Any)
        .empty(EmptyRule::Any)
        .moves(MoveUnit::Top)
        .max_accept(1);
    st_grp.no_flip = true;
    let mut r4 = Rules::new(
        "mini-accept-on-face-down",
        pack,
        vec![
            st_grp,
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Rank(4))
                .moves(MoveUnit::Run),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r4.stock = Some(StockRules::to("tableau"));
    check(r4, 0..40);
}

#[test]
fn redeal_deal_rows_redistribute_shuffle_and_alternating() {
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);

    // 1) RedealHow::DealRows.
    let mut r1 = Rules::new(
        "mini-deal-rows",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").each(2)],
    );
    r1.redeal = Some(RedealRules::new("tableau", RedealHow::DealRows));
    check(r1, 0..40);

    // 1b) RedealHow::DealStaircase: the same gathering, dealt with each row starting a pile later.
    let mut r1b = Rules::new(
        "mini-deal-staircase",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").each(2)],
    );
    r1b.redeal = Some(RedealRules::new("tableau", RedealHow::DealStaircase));
    check(r1b, 0..40);

    // 1c) RedealHow::ShuffleTopUp.
    let mut r1c = Rules::new(
        "mini-shuffle-top-up",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").each(2)],
    );
    let mut rd1c = RedealRules::new("tableau", RedealHow::ShuffleTopUp { per_pile: 2 });
    rd1c.times = 2;
    r1c.redeal = Some(rd1c);
    check(r1c, 0..40);

    // 2) RedealHow::RedistributePile.
    let mut r2 = Rules::new(
        "mini-redistribute-pile",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").each(2)],
    );
    let mut rd2 = RedealRules::new("tableau", RedealHow::RedistributePile);
    rd2.times = 3;
    r2.redeal = Some(rd2);
    check(r2, 0..40);

    // 3) RedealHow::ShuffleRedeal.
    let mut r3 = Rules::new(
        "mini-shuffle-redeal",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Suit, Dir::Down)
                .empty(EmptyRule::Rank(3))
                .moves(MoveUnit::AnyFaceUp),
        ],
        vec![
            DealStep::to("tableau").piles(&[1, 2]).each(1).down(),
            DealStep::to("tableau").each(1),
            DealStep::to("tableau").piles(&[0]).each(1),
        ],
    );
    r3.redeal = Some(RedealRules::new("tableau", RedealHow::ShuffleRedeal));
    check(r3, 0..40);

    // 4) StockRules::redeal_deal_all_alternating + divert_ranks.
    let pack4 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);
    let mut r4 = Rules::new(
        "mini-redeal-alternating",
        pack4,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Run),
        ],
        vec![
            DealStep::to("tableau").each(2),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st4 = StockRules::to("tableau");
    st4.redeals = 1;
    st4.redeal_shuffle = true;
    st4.redeal_deal_all_alternating = true;
    st4.divert_ranks = vec![1];
    r4.stock = Some(st4);
    check(r4, 0..40);
}

#[test]
fn piles_built_down_any_and_pairs_sum_same_rank() {
    // 1) Dir::DownAny + WinRule::PilesBuilt targeting a specific pile index (symmetry must be off).
    let pack1 = Pack::new(&[Suit::Hearts], &[1, 2, 3, 4]);
    let mut r1 = Rules::new(
        "mini-tower-target",
        pack1,
        vec![GroupRules::new("tableau", PileKind::Tableau, 3)
            .build(BuildBy::Any, Dir::DownAny)
            .empty(EmptyRule::Any)
            .moves(MoveUnit::Top)],
        vec![
            DealStep::to("tableau").piles(&[0]).each(2),
            DealStep::to("tableau").piles(&[1]).each(2),
        ],
    );
    r1.win = WinRule::PilesBuilt {
        group: "tableau".into(),
        length: Some(4),
        build: Some(Build::new(BuildBy::Any, Dir::Down)),
        empty: Vec::new(),
        non_empty: true,
        pile: Some(2),
        bottom: None,
    };
    check(r1, 0..40);

    // 2) WinRule::PilesBuilt across all non-empty piles with required empty stock.
    let pack2 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let mut r2 = Rules::new(
        "mini-piles-built",
        pack2,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::SuitRun),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r2.stock = Some(StockRules::to("tableau"));
    r2.win = WinRule::PilesBuilt {
        group: "tableau".into(),
        length: Some(3),
        build: Some(Build::new(BuildBy::Suit, Dir::Down)),
        empty: vec!["stock".into()],
        non_empty: true,
        pile: None,
        bottom: None,
    };
    check(r2, 0..40);

    // 3) RemovalKind::PairsSum with same_rank pairs + WinRule::Count.
    let pack3 = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4, 5, 6]);
    let mut r3 = Rules::new(
        "mini-pairs-sum-same-rank",
        pack3,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::None, Dir::Up)
                .empty(EmptyRule::None)
                .moves(MoveUnit::None),
            {
                let mut t = GroupRules::new("tableau", PileKind::Tableau, 4)
                    .empty(EmptyRule::None)
                    .moves(MoveUnit::None);
                t.max_cards = Some(1);
                t.refill_from = vec!["stock".into()];
                t
            },
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r3.removal = vec![
        RemovalRules::new(
            RemovalKind::PairsSum {
                total: 5,
                alone: Vec::new(),
            },
            &["tableau"],
            "foundation",
        ),
        RemovalRules::new(
            RemovalKind::SameRankIn { ranks: vec![5, 6] },
            &["tableau"],
            "foundation",
        ),
    ];
    r3.win = WinRule::Count {
        group: "foundation".into(),
        cards: 12,
    };
    check(r3, 0..40);
}

#[test]
fn same_base_down_or_same_locked_flip_and_suit_or_same_rank() {
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);

    // 1) EmptyRule::SameBase on wrapping suit foundations.
    let mut f1 = GroupRules::new("foundation", PileKind::Foundation, 2)
        .build(BuildBy::Suit, Dir::Up)
        .empty(EmptyRule::SameBase)
        .max_cards(3);
    f1.build.as_mut().unwrap().wrap = true;
    let r1 = Rules::new(
        "mini-same-base",
        pack.clone(),
        vec![
            f1,
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Suit, Dir::Either)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").each(2)],
    );
    check(r1, 0..40);

    // 2) Dir::DownOrSame + MoveUnit::TopOrWhole.
    let r2 = Rules::new(
        "mini-down-or-same",
        pack.clone(),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Alternate, Dir::DownOrSame)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::TopOrWhole),
        ],
        vec![DealStep::to("tableau").each(2)],
    );
    check(r2, 0..40);

    // 3) GroupRules::flip_when_same_index_full (reserve 0 flips when foundation 0 hits 3 cards).
    let mut res3 = GroupRules::new("reserve", PileKind::Reserve, 1);
    res3.no_flip = true;
    res3.flip_when_same_index_full = Some("foundation".into());
    let r3 = Rules::new(
        "mini-locked-flip",
        pack.clone(),
        vec![
            res3,
            GroupRules::new("foundation", PileKind::Foundation, 2).max_cards(3),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Run),
        ],
        vec![
            DealStep::to("reserve").each(1).down(),
            DealStep::to("tableau").counts(&[3, 2]),
        ],
    );
    check(r3, 0..40);

    // 4) RedealHow::GridShuffleEndGaps.
    let mut r4 = Rules::new(
        "mini-grid-end-gaps",
        pack.clone(),
        vec![GroupRules::new("tableau", PileKind::Tableau, 8)
            .build(BuildBy::Suit, Dir::Up)
            .empty(EmptyRule::AfterLeft { first: 1 })
            .moves(MoveUnit::Top)
            .max_cards(1)
            .layout(Layout::Grid { columns: 4 })],
        vec![DealStep::to("tableau").piles(&[0, 1, 2, 4, 5, 6]).each(1)],
    );
    r4.redeal = Some(RedealRules::new("tableau", RedealHow::GridShuffleEndGaps));
    r4.win = WinRule::Rows {
        group: "tableau".into(),
        length: Some(3),
    };
    check(r4, 0..40);

    // 5) BuildBy::SuitOrSameRank + MoveUnit::SuitRunOrSameRank + WinRule::PilesBuilt.
    let mut r5 = Rules::new(
        "mini-suit-or-same-rank",
        pack,
        vec![GroupRules::new("tableau", PileKind::Tableau, 3)
            .build(BuildBy::SuitOrSameRank, Dir::Down)
            .empty(EmptyRule::Any)
            .moves(MoveUnit::SuitRunOrSameRank)],
        vec![DealStep::to("tableau").each(2)],
    );
    r5.win = WinRule::PilesBuilt {
        group: "tableau".into(),
        length: Some(3),
        build: Some(Build::new(BuildBy::Suit, Dir::Down)),
        empty: Vec::new(),
        non_empty: true,
        pile: None,
        bottom: None,
    };
    check(r5, 0..40);
}

#[test]
fn freeze_when_built_closes_complete_piles() {
    // 2 suits x 3 ranks = 6 cards in 3 piles of at most 2: a pile of two same-rank cards is closed.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let mut tab = GroupRules::new("tableau", PileKind::Tableau, 4)
        .build(BuildBy::Any, Dir::Same)
        .empty(EmptyRule::Any)
        .moves(MoveUnit::Run)
        .max_cards(2);
    tab.freeze_when_built = true;
    let mut r = Rules::new(
        "mini-freeze-when-built",
        pack,
        vec![tab],
        vec![DealStep::to("tableau").counts(&[2, 2, 2, 0])],
    );
    r.win = WinRule::PilesBuilt {
        group: "tableau".into(),
        length: Some(2),
        build: Some(Build::new(BuildBy::Any, Dir::Same)),
        empty: Vec::new(),
        non_empty: false,
        pile: None,
        bottom: None,
    };
    check(r, 0..40);
}

#[test]
fn gap_ranks_break_adjacency_in_stripped_packs() {
    // Ace and 5..8 of two suits: the ace is not next to the 5.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 5, 6, 7, 8]);
    let r = Rules::new(
        "mini-gap-ranks",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .build_rule(Build::new(BuildBy::Suit, Dir::Either).gap_ranks(&[5]))
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").counts(&[3, 3, 2, 2])],
    );
    check(r, 0..40);
}

#[test]
fn suit_sequence_moves_only_consecutive_suit_runs() {
    // Builds on any lower card of its suit, but moves only consecutive same-suit runs.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4, 5]);
    let mut r = Rules::new(
        "mini-suit-sequence",
        pack,
        vec![GroupRules::new("tableau", PileKind::Tableau, 4)
            .build(BuildBy::Suit, Dir::DownAny)
            .empty(EmptyRule::Any)
            .moves(MoveUnit::SuitSequence)],
        vec![DealStep::to("tableau").counts(&[3, 3, 2, 2])],
    );
    r.win = WinRule::PilesBuilt {
        group: "tableau".into(),
        length: Some(5),
        build: Some(Build::new(BuildBy::Suit, Dir::Down)),
        empty: Vec::new(),
        non_empty: false,
        pile: None,
        bottom: None,
    };
    check(r, 0..40);
}

#[test]
fn same_suit_removal_with_count_and_empty_win() {
    // 2 suits x 4 ranks: remove same-suit pairs; win when all discarded and the stock is empty.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);
    let mut r = Rules::new(
        "mini-same-suit",
        pack,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .moves(MoveUnit::None)
                .max_accept(0)
                .max_cards(8),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .empty(EmptyRule::None)
                .moves(MoveUnit::None),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("tableau"));
    r.removal = vec![RemovalRules::new(
        RemovalKind::SameSuit,
        &["tableau"],
        "foundation",
    )];
    r.win = WinRule::CountAndEmpty {
        group: "foundation".into(),
        cards: 8,
        empty: vec!["stock".into()],
    };
    check(r, 0..40);
}

#[test]
fn exclude_suit_keeps_a_suit_off_a_group() {
    // Hearts never go onto the tableau, nor does anything go onto a heart.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);
    let mut tab = GroupRules::new("tableau", PileKind::Tableau, 3)
        .build(BuildBy::Any, Dir::Down)
        .empty(EmptyRule::Any)
        .moves(MoveUnit::Run);
    tab.exclude_suit = Some(Suit::Hearts);
    let r = Rules::new(
        "mini-exclude-suit",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Suit, Dir::Up)
                .empty(EmptyRule::Any)
                .max_cards(4),
            tab,
        ],
        vec![DealStep::to("tableau").counts(&[3, 3, 2])],
    );
    check(r, 0..100);
}

#[test]
fn requires_same_index_non_empty_gates_a_second_foundation_row() {
    // Rank 3 of a suit may start its own foundation only once that suit's 1-2 foundation has begun.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let low = GroupRules::new("low", PileKind::Foundation, 2)
        .build(BuildBy::Suit, Dir::Up)
        .empty(EmptyRule::Rank(1))
        .max_cards(2)
        .pile(PileOverride::on(&[0]).suit(Suit::Clubs))
        .pile(PileOverride::on(&[1]).suit(Suit::Hearts));
    let high = GroupRules::new("high", PileKind::Foundation, 2)
        .build(BuildBy::Suit, Dir::Up)
        .empty(EmptyRule::Rank(3))
        .max_cards(1)
        .pile(PileOverride::on(&[0]).suit(Suit::Clubs))
        .pile(PileOverride::on(&[1]).suit(Suit::Hearts))
        .requires_same_index_non_empty("low");
    let r = Rules::new(
        "mini-same-index-non-empty",
        pack,
        vec![
            low,
            high,
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").counts(&[2, 2, 2])],
    );
    check(r, 0..60);
}

#[test]
fn flip_gates_open_reserve_piles_in_order() {
    // Reserve piles are dealt face down; each turns up only after the one before is empty and
    // while some tableau pile is empty.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let mut reserve = GroupRules::new("reserve", PileKind::Reserve, 2)
        .moves(MoveUnit::Top)
        .empty(EmptyRule::None);
    reserve.flip_after_prev_empty = true;
    reserve.flip_when_any_empty = Some("tableau".into());
    let r = Rules::new(
        "mini-flip-gates",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Suit, Dir::Up)
                .max_cards(3),
            reserve,
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Alternate, Dir::Down)
                .empty(EmptyRule::Any)
                .moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("reserve").each(2).down(),
            DealStep::to("tableau").each(1),
        ],
    );
    check(r, 0..60);
}

#[test]
fn piles_built_win_checks_the_bottom_rank() {
    // Two suits of 4 ranks built down by suit; a finished pile must start on the 4.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]);
    let mut r = Rules::new(
        "mini-piles-bottom",
        pack,
        vec![GroupRules::new("tableau", PileKind::Tableau, 4)
            .build(BuildBy::Suit, Dir::Down)
            .empty(EmptyRule::Any)
            .moves(MoveUnit::Run)],
        vec![DealStep::to("tableau").counts(&[3, 3, 1, 1])],
    );
    r.win = WinRule::PilesBuilt {
        group: "tableau".into(),
        length: Some(4),
        build: Some(Build::new(BuildBy::Suit, Dir::Down)),
        empty: Vec::new(),
        non_empty: false,
        pile: None,
        bottom: Some(4),
    };
    check(r, 0..60);
}

#[test]
fn accept_when_same_index_empty_gates_an_upper_row() {
    // Upper piles take cards from lower piles, but a pile whose lower neighbour still holds cards
    // takes only cards from that neighbour.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2]);
    let mut upper = GroupRules::new("upper", PileKind::Tableau, 2)
        .build(BuildBy::Any, Dir::Any)
        .empty(EmptyRule::Any)
        .moves(MoveUnit::Top)
        .accepts_from(&["lower"])
        .accept_when_same_index_empty("lower");
    upper.empty_accepts_from = Some(vec!["lower".into()]);
    let r = {
        let mut r = Rules::new(
            "mini-accept-when-empty",
            pack,
            vec![
                upper,
                GroupRules::new("lower", PileKind::Tableau, 2)
                    .build(BuildBy::None, Dir::Any)
                    .empty(EmptyRule::None)
                    .moves(MoveUnit::Top),
            ],
            vec![DealStep::to("lower").counts(&[2, 2])],
        );
        r.win = WinRule::Count {
            group: "upper".into(),
            cards: 4,
        };
        r
    };
    check(r, 0..60);
}

#[test]
fn move_from_frontier_lets_only_the_end_pile_move() {
    // Cards leave only the last non-empty pile of the row.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let mut row = GroupRules::new("row", PileKind::Tableau, 3)
        .build(BuildBy::Suit, Dir::Down)
        .empty(EmptyRule::None)
        .moves(MoveUnit::Top);
    row.move_from_frontier = Some(Frontier::Last);
    let r = Rules::new(
        "mini-frontier",
        pack,
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Suit, Dir::Up)
                .max_cards(3),
            row,
        ],
        vec![DealStep::to("row").counts(&[2, 2, 2])],
    );
    check(r, 0..60);
}

#[test]
fn double_up_foundation_with_skipped_ranks() {
    // Of five ranks, positions double up as 0 -> 1 -> 3 -> 2 -> 0; the fifth is a dead end that
    // never reaches the foundation, so it is dealt past the first cards.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4, 5]);
    let mut r = Rules::new(
        "mini-double-up",
        pack,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::Any, Dir::DoubleUp)
                .empty(EmptyRule::Any)
                .max_cards(10),
            GroupRules::new("cell", PileKind::Reserve, 2).moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("foundation").each(1).skip_ranks(&[5]),
            DealStep::to("cell").each(1).skip_ranks(&[5]),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("waste"));
    r.win = WinRule::Count {
        group: "foundation".into(),
        cards: 8,
    };
    check(r, 0..60);
}

#[test]
fn empty_open_after_bases_relaxes_the_empty_rule() {
    // Empty piles take only a 3 until two piles start with a 3, then anything.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let mut tab = GroupRules::new("tableau", PileKind::Tableau, 4)
        .build(BuildBy::Suit, Dir::Down)
        .empty(EmptyRule::Rank(3))
        .moves(MoveUnit::Run);
    tab.empty_open_after_bases = Some(2);
    let mut r = Rules::new(
        "mini-empty-open",
        pack,
        vec![tab],
        vec![DealStep::to("tableau").counts(&[2, 2, 1, 1])],
    );
    r.win = WinRule::PilesBuilt {
        group: "tableau".into(),
        length: Some(3),
        build: Some(Build::new(BuildBy::Suit, Dir::Down)),
        empty: Vec::new(),
        non_empty: false,
        pile: None,
        bottom: None,
    };
    check(r, 0..100);
}

#[test]
fn routed_deal_steps() {
    let pack = || Pack::new(&[Suit::Spades, Suit::Hearts, Suit::Clubs], &[1, 2, 3, 4]);
    let tableau = |n| {
        GroupRules::new("tableau", PileKind::Tableau, n)
            .build(BuildBy::Any, Dir::Down)
            .moves(MoveUnit::Top)
            .empty(EmptyRule::Any)
    };
    let mut by_rank = DealStep::to("reserve").rest();
    by_rank.route = Some(DealRoute::Rank { first_rank: 2 });
    check(
        Rules::new(
            "mini route by rank",
            pack(),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 3),
                GroupRules::new("reserve", PileKind::Reserve, 3),
                tableau(2),
            ],
            vec![
                DealStep::to("foundation").ranks(&[1]),
                DealStep::to("tableau").each(1),
                by_rank,
            ],
        ),
        0..40,
    );

    let mut by_suit = DealStep::to("tableau").piles(&[0, 1, 2]).rest();
    by_suit.route = Some(DealRoute::Suit);
    check(
        Rules::new(
            "mini route by suit",
            pack(),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 3),
                tableau(4),
            ],
            vec![by_suit],
        ),
        0..40,
    );

    let mut chain = DealStep::to("tableau").rest();
    chain.route = Some(DealRoute::Chain {
        rank: 4,
        to: "foundation".into(),
    });
    check(
        Rules::new(
            "mini route chain",
            pack(),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 3)
                    .build_rule(Build::new(BuildBy::Any, Dir::Up).wrap())
                    .empty(EmptyRule::Rank(4))
                    .max_cards(4),
                tableau(3),
            ],
            vec![chain],
        ),
        0..40,
    );
}

#[test]
fn elevens_style_reserves() {
    // Pairs adding up to 4 leave the rows; fours and fives (one of each) go to reserves that clear
    // themselves when full. Rows refill from the stock, only while the reserves are empty in the
    // second variant.
    let pack = || Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 3, 4, 5]);
    let build = |name: &str, gated: bool, rank_rule: u8| {
        let mut cell = GroupRules::new("cell", PileKind::Cell, 2)
            .moves(MoveUnit::None)
            .max_cards(1)
            .empty(EmptyRule::Ranks(vec![4, 5]))
            .accepts_from(&["tableau"]);
        match rank_rule {
            0 => cell.unique_ranks = true,
            1 => cell.same_suit_group = true,
            _ => cell.same_rank_group = true,
        }
        cell.clear_when_full = Some("foundation".into());
        let mut tableau = GroupRules::new("tableau", PileKind::Tableau, 4)
            .empty(EmptyRule::None)
            .moves(MoveUnit::Top)
            .refill_from(&["stock"]);
        if gated {
            tableau.refill_when_empty = Some("cell".into());
        }
        let mut r = Rules::new(
            name,
            pack(),
            vec![
                GroupRules::new("stock", PileKind::Stock, 1),
                cell,
                GroupRules::new("foundation", PileKind::Foundation, 1)
                    .build(BuildBy::None, Dir::Up)
                    .empty(EmptyRule::None),
                tableau,
            ],
            vec![
                DealStep::to("tableau").each(1),
                DealStep::to("stock").rest().down(),
            ],
        );
        let kind = if rank_rule == 1 {
            RemovalKind::PairsSumSameSuit { total: 4 }
        } else {
            RemovalKind::PairsSum {
                total: 4,
                alone: vec![],
            }
        };
        r.removal = vec![RemovalRules::new(kind, &["tableau"], "foundation")];
        r
    };
    check(build("mini elevens unique", false, 0), 0..40);
    check(build("mini elevens too", true, 0), 0..40);
    check(build("mini suit elevens", false, 1), 0..40);
    check(build("mini tens", true, 2), 0..40);
}

#[test]
fn stock_requires_group_empty() {
    let mut cell = GroupRules::new("cell", PileKind::Cell, 2)
        .moves(MoveUnit::None)
        .max_cards(1)
        .empty(EmptyRule::Ranks(vec![4, 5]))
        .accepts_from(&["tableau", "waste"]);
    cell.unique_ranks = true;
    cell.clear_when_full = Some("foundation".into());
    let mut r = Rules::new(
        "mini lucky number",
        Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 3, 4, 5]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            cell,
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::None, Dir::Up)
                .empty(EmptyRule::None),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .empty(EmptyRule::None)
                .moves(MoveUnit::Top)
                .refill_from(&["waste", "stock"]),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("waste");
    st.require_group_empty = Some("cell".into());
    r.stock = Some(st);
    r.removal = vec![RemovalRules::new(
        RemovalKind::PairsSum {
            total: 4,
            alone: vec![],
        },
        &["tableau", "waste"],
        "foundation",
    )];
    check(r, 0..40);
}

#[test]
fn mirror_build_pairs_ranks_from_both_ends() {
    // Of five ranks the first goes on the last, the second on the fourth, the third on itself.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4, 5]);
    let mut r = Rules::new(
        "mini-mirror",
        pack,
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Mirror)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any)
                .accepts_from(&["waste"]),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("waste"));
    r.win = WinRule::Empty(vec!["stock".into(), "waste".into()]);
    check(r, 0..60);
}

#[test]
fn reserves_clear_when_they_share_a_rank() {
    // Two reserves clear to the discard once both hold the same rank; the rows build any suit down.
    let pack = Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]);
    let mut cell = GroupRules::new("cell", PileKind::Cell, 2)
        .moves(MoveUnit::None)
        .max_cards(1)
        .empty(EmptyRule::Any)
        .accepts_from(&["tableau"]);
    cell.clear_when_full = Some("foundation".into());
    cell.clear_same_rank = true;
    let r = Rules::new(
        "mini foursome",
        pack,
        vec![
            cell,
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::None, Dir::Up)
                .empty(EmptyRule::None),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any),
        ],
        vec![DealStep::to("tableau").rest()],
    );
    check(r, 0..60);
}

#[test]
fn pairs_sum_other_suit_removal() {
    let mut r = Rules::new(
        "mini other suit pairs",
        Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 3]),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::None, Dir::Up)
                .empty(EmptyRule::None),
            GroupRules::new("tableau", PileKind::Tableau, 4)
                .empty(EmptyRule::None)
                .moves(MoveUnit::None),
        ],
        vec![DealStep::to("tableau").each(1)],
    );
    r.removal = vec![RemovalRules::new(
        RemovalKind::PairsSumOtherSuit { total: 4 },
        &["tableau"],
        "foundation",
    )];
    check(r, 0..40);
}

#[test]
fn stock_skips_built_king_piles() {
    let mut r = Rules::new(
        "mini skip built kings",
        Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Suit, Dir::Up)
                .moves(MoveUnit::None),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Rank(4)),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.skip_built_from = Some(4);
    r.stock = Some(st);
    check(r, 0..60);
}

#[test]
fn waste_feeds_foundations_and_emptied_rows() {
    // Q.C.-style: every waste card a foundation takes goes there; a move that empties a row
    // deals once from the stock, and the row takes the card if it stays in the waste.
    let mut r = mini_klondike(1, Some(1));
    r.name = "mini q.c.".into();
    r.groups[1].auto_foundation_rank = Some(0);
    r.groups[3] = r.groups[3]
        .clone()
        .empty(EmptyRule::Any)
        .build(BuildBy::Suit, Dir::Down)
        .refill_from(&["waste"])
        .refill_on_empty();
    let st = r.stock.as_mut().unwrap();
    st.auto = true;
    st.auto_for_empty = Some("tableau".into());
    check(r, 0..40);
}

#[test]
fn blind_rows_take_anything_on_a_face_down_card() {
    let mut r = mini_klondike(1, Some(1));
    r.name = "mini blind".into();
    r.groups[3].no_flip = true;
    r.groups[3].accept_on_face_down = true;
    r.groups[3].move_face_down_top = true;
    r.groups[3].empty = Some(EmptyRule::Any);
    check(r, 0..40);
}

#[test]
fn lower_rows_take_nothing_from_the_first_rows() {
    let mut r = mini_klondike(1, Some(1));
    r.name = "mini club".into();
    r.groups[3].empty = Some(EmptyRule::Any);
    r.groups[3].one_way_split = Some(1);
    r.groups[3].moves = Some(MoveUnit::Top);
    check(r, 0..40);
}

#[test]
fn open_stock_drops_aces_by_itself() {
    let mut stock = GroupRules::new("stock", PileKind::Stock, 1).moves(MoveUnit::Top);
    stock.auto_foundation_rank = Some(1);
    let r = Rules::new(
        "mini auto aces",
        Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            stock,
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Suit, Dir::Up)
                .moves(MoveUnit::None)
                .pile(PileOverride::on(&[0]).suit(Suit::Clubs))
                .pile(PileOverride::on(&[1]).suit(Suit::Hearts)),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any)
                .accepts_from(&["stock", "tableau"]),
        ],
        vec![DealStep::to("stock").rest()],
    );
    check(r, 0..40);
}

#[test]
fn reserve_flips_only_once_dealt_out() {
    let mut reserve = GroupRules::new("reserve", PileKind::Reserve, 1).moves(MoveUnit::Top);
    reserve.flip_when_dealt_out = true;
    let mut r = Rules::new(
        "mini last chance",
        Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            reserve,
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Suit, Dir::Up)
                .moves(MoveUnit::None)
                .pile(PileOverride::on(&[0]).suit(Suit::Clubs))
                .pile(PileOverride::on(&[1]).suit(Suit::Hearts)),
            GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Top)
                .empty(EmptyRule::Any),
        ],
        vec![
            DealStep::to("reserve").each(1).down(),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules::to("waste"));
    check(r, 0..60);
}

#[test]
fn deck_first_and_last_fix_the_ends_of_the_pack() {
    let mut r = Rules::new(
        "mini fixed ends",
        Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 1),
            GroupRules::new("tableau", PileKind::Tableau, 2).moves(MoveUnit::Top),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.deck_first = vec![CardRef {
        suit: Suit::Hearts,
        rank: 3,
    }];
    r.deck_last = vec![CardRef {
        suit: Suit::Clubs,
        rank: 4,
    }];
    let g = Game::new(r).unwrap();
    for seed in 0..20 {
        let s = g.deal(seed);
        let first = s.piles[2][0];
        let last = *s.piles[0].last().unwrap();
        assert_eq!((first.suit(), first.rank()), (Suit::Hearts, 3));
        assert_eq!((last.suit(), last.rank()), (Suit::Clubs, 4));
    }
}

#[test]
fn color_split_win() {
    // Red cards in the first pile, black in the second; cards build down on any suit.
    let r = {
        let mut r = Rules::new(
            "mini color split",
            Pack::new(&[Suit::Hearts, Suit::Spades], &[1, 2, 3]),
            vec![GroupRules::new("tableau", PileKind::Tableau, 2)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any)],
            vec![DealStep::to("tableau").rest()],
        );
        r.win = WinRule::ColorSplit {
            group: "tableau".into(),
            red: 1,
        };
        r
    };
    check(r, 0..60);
}

#[test]
fn each_row_score_win() {
    // Swap cards around a 2x2 grid until both rows are pairs.
    let mut r = presets::poker_squares();
    r.name = "mini each row".into();
    r.pack = small(1..=2, &[Suit::Spades, Suit::Hearts], 1);
    r.groups.retain(|g| g.name == "tableau");
    r.groups[0].count = 4;
    r.groups[0].layout = Some(Layout::Grid { columns: 2 });
    r.groups[0].moves = Some(MoveUnit::Top);
    r.groups[0].accepts_from = None;
    r.groups[0].swap_bases = Some(SwapBases::BothSingle);
    r.groups[0].empty = Some(EmptyRule::Any);
    r.stock = None;
    r.deal = vec![DealStep::to("tableau").each(1)];
    r.win = WinRule::EachRowScore { at_least: 2 };
    check(r, 0..40);
}

#[test]
fn rank_distance_removal_and_limited_deals() {
    let r = {
        let mut r = Rules::new(
            "mini cheops-ish",
            Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3]),
            vec![
                GroupRules::new("stock", PileKind::Stock, 1),
                GroupRules::new("cell", PileKind::Reserve, 1).moves(MoveUnit::None),
                GroupRules::new("discard", PileKind::Foundation, 1)
                    .build(BuildBy::None, Dir::Up)
                    .empty(EmptyRule::None),
                GroupRules::new("tableau", PileKind::Tableau, 4)
                    .empty(EmptyRule::None)
                    .moves(MoveUnit::None),
            ],
            vec![
                DealStep::to("tableau").each(1),
                DealStep::to("stock").rest().down(),
            ],
        );
        let mut st = StockRules::to("cell");
        st.max_deals = Some(2);
        st.require_group_empty = Some("cell".into());
        r.stock = Some(st);
        r.removal = vec![RemovalRules::new(
            RemovalKind::RankDistance {
                distances: vec![0, 1],
            },
            &["tableau", "cell"],
            "discard",
        )];
        r
    };
    check(r, 0..60);
}

#[test]
fn stock_skips_piles_with_a_king_run_on_top() {
    let mut r = Rules::new(
        "mini skip king runs",
        Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 2)
                .build(BuildBy::Suit, Dir::Up)
                .moves(MoveUnit::None),
            GroupRules::new("tableau", PileKind::Tableau, 3)
                .build(BuildBy::Any, Dir::Down)
                .moves(MoveUnit::Run)
                .empty(EmptyRule::Any),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut st = StockRules::to("tableau");
    st.skip_run_with_rank = Some(4);
    r.stock = Some(st);
    check(r, 0..60);
}

#[test]
fn separate_decks_deal_one_pack_then_the_next() {
    let mut r = Rules::new(
        "mini separate decks",
        small(1..=3, &[Suit::Clubs], 2),
        vec![
            GroupRules::new("foundation", PileKind::Foundation, 1),
            GroupRules::new("tableau", PileKind::Tableau, 1).moves(MoveUnit::Top),
        ],
        vec![DealStep::to("tableau").rest()],
    );
    r.separate_decks = true;
    let g = Game::new(r).unwrap();
    for seed in 0..20 {
        let s = g.deal(seed);
        let ranks: Vec<u8> = s.piles[1].iter().map(|c| c.rank()).collect();
        assert_eq!(ranks[..3].iter().map(|&r| r as u32).sum::<u32>(), 6);
        assert_eq!(ranks[3..].iter().map(|&r| r as u32).sum::<u32>(), 6);
        let mut a = ranks[..3].to_vec();
        a.sort();
        assert_eq!(a, vec![1, 2, 3]);
    }
}

#[test]
fn same_line_removal_and_compaction_after_the_stock_is_used() {
    // A 2x3 grid; same-rank cards in the same row or column pair off. Gaps refill from the stock and
    // close up only once it is empty.
    let mut tableau = GroupRules::new("tableau", PileKind::Tableau, 6)
        .empty(EmptyRule::None)
        .moves(MoveUnit::None)
        .refill_from(&["stock"]);
    tableau.compact = true;
    tableau.compact_when_stock_empty = true;
    tableau.layout = Some(Layout::Grid { columns: 3 });
    let mut r = Rules::new(
        "mini quatorze",
        Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("foundation", PileKind::Foundation, 1)
                .build(BuildBy::None, Dir::Up)
                .empty(EmptyRule::None),
            tableau,
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    let mut rem = RemovalRules::new(RemovalKind::SameRank, &["tableau"], "foundation");
    rem.adjacent = Some(Adjacency::SameLine);
    r.removal = vec![rem];
    check(r, 0..60);
}

#[test]
fn split_route_and_alternating_colour_deal() {
    let mut split = DealStep::to("cell").rest();
    split.limit = Some(3);
    split.route = Some(DealRoute::Split {
        rank: 1,
        to: "foundation".into(),
    });
    check(
        Rules::new(
            "mini frog deal",
            Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 2)
                    .build(BuildBy::Any, Dir::Up)
                    .empty(EmptyRule::Rank(1))
                    .max_cards(4),
                GroupRules::new("cell", PileKind::Reserve, 1).moves(MoveUnit::Top),
                GroupRules::new("tableau", PileKind::Tableau, 2)
                    .build(BuildBy::Any, Dir::Down)
                    .moves(MoveUnit::Top)
                    .empty(EmptyRule::Any),
            ],
            vec![split, DealStep::to("tableau").rest()],
        ),
        0..40,
    );

    let mut alt = DealStep::to("foundation").ranks(&[1, 2]);
    alt.one_each = true;
    alt.alternate_colors = true;
    check(
        Rules::new(
            "mini colours deal",
            Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3]),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 2)
                    .build_rule(Build::new(BuildBy::Any, Dir::Up).wrap())
                    .empty(EmptyRule::None)
                    .max_cards(3),
                GroupRules::new("tableau", PileKind::Tableau, 2)
                    .build(BuildBy::Any, Dir::Down)
                    .moves(MoveUnit::Top)
                    .empty(EmptyRule::Any),
            ],
            vec![alt, DealStep::to("tableau").rest()],
        ),
        0..40,
    );
}

#[test]
fn same_rank_runs_of_a_set_length_go_to_one_foundation() {
    let mut f = GroupRules::new("foundation", PileKind::Foundation, 1)
        .build(BuildBy::Any, Dir::Same)
        .empty(EmptyRule::Any)
        .max_cards(8)
        .max_accept(2);
    f.complete_runs = true;
    f.run_len = Some(2);
    check(
        Rules::new(
            "mini simplex",
            Pack::new(&[Suit::Clubs, Suit::Hearts], &[1, 2, 3, 4]),
            vec![
                f,
                GroupRules::new("tableau", PileKind::Tableau, 4)
                    .build(BuildBy::Any, Dir::Same)
                    .moves(MoveUnit::Run)
                    .empty(EmptyRule::Any),
            ],
            vec![DealStep::to("tableau").rest()],
        ),
        0..40,
    );
}

#[test]
fn suit_cycle_builds() {
    let cycle = [Suit::Spades, Suit::Hearts, Suit::Clubs];
    check(
        Rules::new(
            "mini following",
            Pack::new(&cycle, &[1, 2, 3]),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 3)
                    .build_rule(Build::new(BuildBy::SuitCycle, Dir::Up).suit_cycle(&cycle))
                    .empty(EmptyRule::Rank(1))
                    .max_cards(3),
                GroupRules::new("tableau", PileKind::Tableau, 3)
                    .build_rule(Build::new(BuildBy::SuitCycle, Dir::Down).suit_cycle(&cycle))
                    .moves(MoveUnit::Run)
                    .empty(EmptyRule::Any),
            ],
            vec![DealStep::to("tableau").rest()],
        ),
        0..60,
    );
}

#[test]
fn retry_after_divert_gives_the_pile_the_next_card() {
    let mut step = DealStep::to("tableau").each(2);
    step.auto_foundation = true;
    step.retry_after_divert = true;
    check(
        Rules::new(
            "mini dieppe deal",
            Pack::new(&[Suit::Spades, Suit::Hearts], &[1, 2, 3, 4]),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 2)
                    .build(BuildBy::Suit, Dir::Up)
                    .pile(PileOverride::on(&[0]).suit(Suit::Spades))
                    .pile(PileOverride::on(&[1]).suit(Suit::Hearts)),
                GroupRules::new("tableau", PileKind::Tableau, 2)
                    .build(BuildBy::Any, Dir::Down)
                    .moves(MoveUnit::Top)
                    .empty(EmptyRule::Any),
            ],
            vec![step, DealStep::to("tableau").rest()],
        ),
        0..40,
    );
}
