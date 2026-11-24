//! The solver's pruning (symmetry, safe autoplay, talon compression, sink hashing, the fast
//! phase) must never change a verdict. Small games are searched exhaustively both ways.

mod common;

use common::{assert_wins, brute_force, brute_force_max};
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
    let (mut won, mut lost) = (0, 0);
    for seed in seeds {
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
        if truth {
            won += 1
        } else {
            lost += 1
        }
    }
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
    let mut scores = Vec::new();
    for seed in seeds {
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
        scores.push(truth);
    }
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
fn poker_squares_best() {
    let american = || PokerTable::Named(PokerScoring::American);
    check_max(mini_poker(american(), false, 1), 0..12);
    check_max(mini_poker(american(), true, 1), 12..24);
    check_max(mini_poker(odd_points(), false, 1), 24..36);
}

#[test]
fn poker_squares_target() {
    check(
        mini_poker(PokerTable::Named(PokerScoring::American), false, 15),
        0..16,
    );
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
#[test]
fn jokers_poker_squares() {
    let jokers = |table: PokerTable, any_pool: bool, n: u8, target: u32| {
        let mut r = with_jokers(mini_poker(table, any_pool, target), n, &[]);
        r.deal[0] = DealStep::to("aside").each(6 + n as u16).down();
        r
    };
    let american = || PokerTable::Named(PokerScoring::American);
    let english = || PokerTable::Named(PokerScoring::English);
    check_max(jokers(american(), false, 1, 1), 0..12);
    check_max(jokers(english(), false, 2, 1), 12..24);
    check_max(jokers(odd_points(), true, 1, 1), 24..36);
    check(jokers(american(), false, 1, 20), 0..16);
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
fn cribbage_squares_best() {
    check_max(mini_cribbage("waste", false, 1), 0..12);
    check_max(mini_cribbage("waste", true, 1), 12..24);
    check_max(mini_cribbage("fixed", false, 1), 24..36);
    check_max(
        with_jokers(mini_cribbage("waste", false, 1), 1, &[]),
        36..48,
    );
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
