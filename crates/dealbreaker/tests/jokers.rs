//! What jokers may do, checked move by move on hand-built positions.

use std::collections::BTreeMap;

use dealbreaker::*;

/// Four tableau piles building down in alternate colours (runs move together), four
/// foundations, one free cell, and a reserve nothing leaves, holding the cards a test doesn't
/// use. The pack is 6 to 9 of every suit and one joker.
fn game(tableau_wild: Option<bool>, foundation_wild: Option<bool>) -> Game {
    let mut tableau = GroupRules::new("tableau", PileKind::Tableau, 4)
        .build(BuildBy::Alternate, Dir::Down)
        .moves(MoveUnit::Run)
        .empty(EmptyRule::Rank(9));
    tableau.wild = tableau_wild;
    let mut foundation =
        GroupRules::new("foundation", PileKind::Foundation, 4).empty(EmptyRule::Rank(6));
    foundation.wild = foundation_wild;
    let rules = Rules::new(
        "joker test",
        Pack::new(&Suit::ALL, &[6, 7, 8, 9]).with_jokers(1),
        vec![
            tableau,
            foundation,
            GroupRules::new("cell", PileKind::Cell, 1),
            GroupRules::new("reserve", PileKind::Reserve, 1).moves(MoveUnit::None),
        ],
        vec![DealStep::to("tableau").rest()],
    );
    Game::new(rules).unwrap()
}

/// A position from the named piles; every other card goes to the reserve.
fn position(g: &Game, piles: &[(&str, usize, &[&str])]) -> State {
    let mut data = StateData::default();
    let mut used = Vec::new();
    for &(group, i, cards) in piles {
        let entry = data.piles.entry(group.to_string()).or_insert_with(Vec::new);
        while entry.len() <= i {
            entry.push(Vec::new());
        }
        entry[i] = cards.iter().map(|c| c.to_string()).collect();
        used.extend(cards.iter().map(|c| c.parse::<Card>().unwrap().up()));
    }
    let mut rest: Vec<String> = Vec::new();
    for c in g.pack_cards() {
        if let Some(k) = used.iter().position(|&u| u == c) {
            used.remove(k);
        } else {
            rest.push(c.to_string());
        }
    }
    data.piles.insert("reserve".into(), vec![rest]);
    g.state_from_data(&data).unwrap()
}

fn pile(g: &Game, group: &str, i: u16) -> u16 {
    g.pile_index(group, i).unwrap()
}

fn can(g: &Game, s: &State, from: (&str, u16), to: (&str, u16), count: u16) -> bool {
    let mv = Move::new(Action::Transfer {
        from: pile(g, from.0, from.1),
        to: pile(g, to.0, to.1),
        count,
    });
    g.is_legal(s, mv)
}

#[test]
fn a_joker_stands_for_a_card_that_fits_beneath_it() {
    let g = game(None, None);
    // The joker on the 8♠ is a red 7, so a black 6 may go on it but not a red one.
    let s = position(
        &g,
        &[
            ("tableau", 0, &["8S", "JK"]),
            ("tableau", 1, &["6C"]),
            ("tableau", 2, &["6H"]),
        ],
    );
    assert!(can(&g, &s, ("tableau", 1), ("tableau", 0), 1));
    assert!(!can(&g, &s, ("tableau", 2), ("tableau", 0), 1));
}

#[test]
fn a_run_through_a_joker_must_stay_built_where_it_goes() {
    let g = game(None, None);
    let s = position(
        &g,
        &[
            ("tableau", 0, &["8S", "JK", "6C"]),
            ("tableau", 1, &["8C"]),
            ("tableau", 2, &["9D"]),
        ],
    );
    // On 8♣ the joker is a red 7 again, and the 6♣ still fits on it.
    assert!(can(&g, &s, ("tableau", 0), ("tableau", 1), 2));
    // On 9♦ the joker would be a black 8, which can't take a black 6.
    assert!(!can(&g, &s, ("tableau", 0), ("tableau", 2), 2));
    // The whole run, 8♠ first, goes on the 9♦.
    assert!(can(&g, &s, ("tableau", 0), ("tableau", 2), 3));
}

#[test]
fn a_joker_can_be_built_on_anything_that_takes_a_card() {
    let g = game(None, None);
    let s = position(
        &g,
        &[
            ("tableau", 0, &["7D"]),
            ("tableau", 1, &["JK"]),
            ("tableau", 2, &["6S"]),
        ],
    );
    // On the 7♦ the joker is a black 6.
    assert!(can(&g, &s, ("tableau", 1), ("tableau", 0), 1));
    // Nothing goes on a 6 here, so not a joker either.
    assert!(!can(&g, &s, ("tableau", 1), ("tableau", 2), 1));
    // An empty tableau pile takes a 9, and so a joker standing for one.
    assert!(can(&g, &s, ("tableau", 1), ("tableau", 3), 1));
    // A free cell takes any card.
    assert!(can(&g, &s, ("tableau", 1), ("cell", 0), 1));
}

#[test]
fn jokers_stay_off_foundations_unless_wild_there() {
    let g = game(None, None);
    let s = position(&g, &[("tableau", 0, &["JK"]), ("foundation", 0, &["6H"])]);
    assert!(!can(&g, &s, ("tableau", 0), ("foundation", 0), 1));
    assert!(!can(&g, &s, ("tableau", 0), ("foundation", 1), 1));

    let g = game(None, Some(true));
    let s = position(
        &g,
        &[
            ("tableau", 0, &["JK"]),
            ("tableau", 1, &["8H"]),
            ("tableau", 2, &["8S"]),
            ("foundation", 0, &["6H"]),
        ],
    );
    assert!(can(&g, &s, ("tableau", 0), ("foundation", 0), 1));
    assert!(can(&g, &s, ("tableau", 0), ("foundation", 1), 1));
    // On the 6♥, the joker is the 7♥: the 8♥ follows it and the 8♠ doesn't.
    let mut t = s;
    g.play(
        &mut t,
        Move::new(Action::Transfer {
            from: pile(&g, "tableau", 0),
            to: pile(&g, "foundation", 0),
            count: 1,
        }),
    )
    .unwrap();
    assert!(can(&g, &t, ("tableau", 1), ("foundation", 0), 1));
    assert!(!can(&g, &t, ("tableau", 2), ("foundation", 0), 1));
}

#[test]
fn where_jokers_arent_wild_nothing_builds_on_them() {
    let g = game(Some(false), None);
    let s = position(
        &g,
        &[
            ("tableau", 0, &["8S"]),
            ("tableau", 1, &["JK"]),
            ("tableau", 2, &["6C"]),
        ],
    );
    assert!(!can(&g, &s, ("tableau", 1), ("tableau", 0), 1));
    assert!(!can(&g, &s, ("tableau", 2), ("tableau", 1), 1));
    // Empty tableau piles take only nines, which a joker that isn't wild can't be.
    assert!(!can(&g, &s, ("tableau", 1), ("tableau", 3), 1));
    assert!(can(&g, &s, ("tableau", 1), ("cell", 0), 1));
}

#[test]
fn every_card_but_the_jokers_home_wins() {
    let g = game(None, None);
    let mut piles: BTreeMap<String, Vec<Vec<String>>> = BTreeMap::new();
    let suits = ["C", "D", "H", "S"];
    piles.insert(
        "foundation".into(),
        suits
            .iter()
            .map(|s| (6..=9).map(|r| format!("{r}{s}")).collect())
            .collect(),
    );
    piles.insert("tableau".into(), vec![vec!["JK".into()]]);
    let mut data = StateData::default();
    data.piles = piles;
    let s = g.state_from_data(&data).unwrap();
    assert!(g.is_won(&s));
}

#[test]
fn jokers_pair_with_anything_in_removals() {
    let mut r = presets::pyramid();
    r.pack.jokers = 1;
    let g = Game::new(r).unwrap();
    // Deal until the joker is uncovered at the start, then check it pairs with every other
    // uncovered card.
    for seed in 0..200 {
        let s = g.deal(seed);
        let tops: Vec<u16> = (0..g.num_piles() as u16)
            .filter(|&p| s.piles[p as usize].last().is_some_and(|c| c.is_joker()))
            .collect();
        let Some(&jp) = tops.first() else { continue };
        let pairs = g
            .legal_moves(&s)
            .into_iter()
            .filter(|m| matches!(m.action, Action::Remove { n: 2, piles, .. } if piles[..2].contains(&jp)))
            .count();
        if pairs > 0 {
            return;
        }
    }
    panic!("no deal paired the joker");
}
