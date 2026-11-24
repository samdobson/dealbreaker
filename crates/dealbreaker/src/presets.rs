//! Ready-made rules for well-known games, written in the same data model anyone can use.
//!
//! These are traditional games, described from their commonly published rules. Where sources
//! disagree, the choice made is noted on the preset. Each preset is also a worked example for
//! writing your own rules: `dealbreaker rules <id>` prints any of them as JSON.

use crate::card::Suit;
use crate::rules::*;

/// A preset: an id (kebab-case) and its rules.
pub struct Preset {
    pub id: &'static str,
    pub rules: fn() -> Rules,
}

/// Every preset.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "klondike",
        rules: klondike1,
    },
    Preset {
        id: "klondike-draw3",
        rules: klondike3,
    },
    Preset {
        id: "klondike-vegas",
        rules: vegas,
    },
    Preset {
        id: "klondike-jokers",
        rules: klondike_jokers,
    },
    Preset {
        id: "freecell",
        rules: freecell,
    },
    Preset {
        id: "bakers-game",
        rules: bakers_game,
    },
    Preset {
        id: "eight-off",
        rules: eight_off,
    },
    Preset {
        id: "seahaven-towers",
        rules: seahaven,
    },
    Preset {
        id: "spider",
        rules: || spider(4),
    },
    Preset {
        id: "spider-2suit",
        rules: || spider(2),
    },
    Preset {
        id: "spider-1suit",
        rules: || spider(1),
    },
    Preset {
        id: "spiderette",
        rules: spiderette,
    },
    Preset {
        id: "scorpion",
        rules: scorpion,
    },
    Preset {
        id: "yukon",
        rules: yukon,
    },
    Preset {
        id: "russian",
        rules: russian,
    },
    Preset {
        id: "forty-thieves",
        rules: forty_thieves,
    },
    Preset {
        id: "gypsy",
        rules: gypsy,
    },
    Preset {
        id: "bakers-dozen",
        rules: bakers_dozen,
    },
    Preset {
        id: "beleaguered-castle",
        rules: beleaguered_castle,
    },
    Preset {
        id: "streets-and-alleys",
        rules: streets_and_alleys,
    },
    Preset {
        id: "fortress",
        rules: fortress,
    },
    Preset {
        id: "canfield",
        rules: canfield,
    },
    Preset {
        id: "golf",
        rules: golf,
    },
    Preset {
        id: "tripeaks",
        rules: tripeaks,
    },
    Preset {
        id: "pyramid",
        rules: pyramid,
    },
    Preset {
        id: "aces-up",
        rules: aces_up,
    },
    Preset {
        id: "calculation",
        rules: calculation,
    },
    Preset {
        id: "sir-tommy",
        rules: sir_tommy,
    },
    Preset {
        id: "bisley",
        rules: bisley,
    },
    Preset {
        id: "elevens",
        rules: elevens,
    },
    Preset {
        id: "nestor",
        rules: nestor,
    },
    Preset {
        id: "gaps-single-deal",
        rules: gaps_single_deal,
    },
    Preset {
        id: "montana",
        rules: montana,
    },
    Preset {
        id: "cruel",
        rules: cruel,
    },
    Preset {
        id: "monte-carlo",
        rules: monte_carlo,
    },
    Preset {
        id: "accordion",
        rules: accordion,
    },
    Preset {
        id: "la-belle-lucie",
        rules: la_belle_lucie,
    },
    Preset {
        id: "poker-squares",
        rules: poker_squares,
    },
    Preset {
        id: "cribbage-squares",
        rules: cribbage_squares,
    },
];

/// Look up a preset by id.
pub fn preset(id: &str) -> Option<Rules> {
    PRESETS.iter().find(|p| p.id == id).map(|p| (p.rules)())
}

fn foundations(n: u16) -> GroupRules {
    GroupRules::new("foundation", PileKind::Foundation, n)
}

fn tableau(n: u16) -> GroupRules {
    GroupRules::new("tableau", PileKind::Tableau, n)
}

fn base(name: &str, pack: Pack, groups: Vec<GroupRules>, deal: Vec<DealStep>) -> Rules {
    Rules::new(name, pack, groups, deal)
}

fn stock_waste() -> Vec<GroupRules> {
    vec![
        GroupRules::new("stock", PileKind::Stock, 1),
        GroupRules::new("waste", PileKind::Waste, 1),
    ]
}

fn to_waste(count: u16, redeals: Option<u32>) -> StockRules {
    StockRules {
        to: "waste".into(),
        count,
        redeals: redeals.unwrap_or(0),
        unlimited_redeals: redeals.is_none(),
        require_no_empty: false,
        auto: false,
        first_empty: false,
        only_when_empty: false,
        skip_empty: false,
        face_up: false,
        count_per_round: Vec::new(),
        final_to: None,
        piles: None,
        final_piles: None,
        deal_per_pile: false,
        redeal_reverse: false,
        deal_empty_first: false,
        divert_ranks: Vec::new(),
        piles_per_round: Vec::new(),
    }
}

// ----- Klondike family ---------------------------------------------------------------------------

fn klondike(name: &str, draw: u16, redeals: Option<u32>) -> Rules {
    let mut groups = stock_waste();
    groups.push(foundations(4));
    groups.push(
        tableau(7)
            .build(BuildBy::Alternate, Dir::Down)
            .empty(EmptyRule::Rank(13))
            .moves(MoveUnit::Run),
    );
    let mut r = base(
        name,
        Pack::default(),
        groups,
        vec![
            DealStep::to("tableau")
                .counts(&[0, 1, 2, 3, 4, 5, 6])
                .down(),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(to_waste(draw, redeals));
    r.search.blocker_weight = 4;
    r
}

/// Klondike, turning one card at a time, unlimited passes through the stock.
pub fn klondike1() -> Rules {
    klondike("Klondike", 1, None)
}

/// Klondike, turning three at a time, unlimited passes.
pub fn klondike3() -> Rules {
    klondike("Klondike (draw 3)", 3, None)
}

/// Klondike with the two jokers left in: wild on the tableau, where a joker stands for any card
/// that would fit. Jokers never go to the foundations; the game is won with every other card
/// home.
pub fn klondike_jokers() -> Rules {
    let mut r = klondike("Klondike (with jokers)", 1, None);
    r.pack.jokers = 2;
    r
}

/// Vegas: three at a time, three passes in all (two redeals).
pub fn vegas() -> Rules {
    klondike("Klondike (Vegas)", 3, Some(2))
}

/// Yukon: no stock; any face-up group of cards moves, building down in alternate colours.
pub fn yukon() -> Rules {
    yukon_like("Yukon", BuildBy::Alternate)
}

/// Russian Solitaire: Yukon building in suit.
pub fn russian() -> Rules {
    yukon_like("Russian Solitaire", BuildBy::Suit)
}

fn yukon_like(name: &str, by: BuildBy) -> Rules {
    base(
        name,
        Pack::default(),
        vec![
            foundations(4),
            tableau(7)
                .build(by, Dir::Down)
                .empty(EmptyRule::Rank(13))
                .moves(MoveUnit::AnyFaceUp),
        ],
        vec![
            DealStep::to("tableau")
                .counts(&[0, 1, 2, 3, 4, 5, 6])
                .down(),
            DealStep::to("tableau").counts(&[1, 5, 5, 5, 5, 5, 5]),
        ],
    )
}

/// Canfield: 13-card reserve, four tableau piles refilled from it, foundations start at the
/// rank of the first card dealt to them and wrap round. Stock three at a time, unlimited passes.
/// Tableau sequences move as units.
pub fn canfield() -> Rules {
    let mut groups = stock_waste();
    groups.push(
        foundations(4)
            .build_rule(Build {
                by: BuildBy::Suit,
                dir: Dir::Up,
                wrap: true,
                ..Build::default()
            })
            .empty(EmptyRule::DealtBase),
    );
    groups.push(GroupRules::new("reserve", PileKind::Reserve, 1));
    groups.push(
        tableau(4)
            .build_rule(Build {
                by: BuildBy::Alternate,
                dir: Dir::Down,
                wrap: true,
                ..Build::default()
            })
            .moves(MoveUnit::Run)
            .refill_from(&["reserve"]),
    );
    let mut r = base(
        "Canfield",
        Pack::default(),
        groups,
        vec![
            DealStep::to("reserve").each(12).down(),
            DealStep::to("reserve").each(1),
            DealStep::to("foundation").piles(&[0]).each(1).set_base(),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(to_waste(3, None));
    r
}

// ----- FreeCell family ---------------------------------------------------------------------------

fn freecell_like(name: &str, by: BuildBy, cells: u16) -> Rules {
    base(
        name,
        Pack::default(),
        vec![
            GroupRules::new("cell", PileKind::Cell, cells),
            foundations(4),
            tableau(8).build(by, Dir::Down).moves(MoveUnit::Supermove),
        ],
        vec![DealStep::to("tableau").rest()],
    )
}

/// FreeCell: four cells, eight piles, alternate colours, sequences move as far as free space allows.
pub fn freecell() -> Rules {
    freecell_like("FreeCell", BuildBy::Alternate, 4)
}

/// Baker's Game: FreeCell building in suit.
pub fn bakers_game() -> Rules {
    freecell_like("Baker's Game", BuildBy::Suit, 4)
}

/// Eight Off: eight cells, four filled by the deal; build in suit; only kings to empty piles.
pub fn eight_off() -> Rules {
    base(
        "Eight Off",
        Pack::default(),
        vec![
            GroupRules::new("cell", PileKind::Cell, 8),
            foundations(4),
            tableau(8)
                .build(BuildBy::Suit, Dir::Down)
                .empty(EmptyRule::Rank(13))
                .moves(MoveUnit::Supermove),
        ],
        vec![
            DealStep::to("tableau").each(6),
            DealStep::to("cell").piles(&[0, 1, 2, 3]).each(1),
        ],
    )
}

/// Seahaven Towers: ten piles of five, four cells with two cards; build in suit; kings to empties.
pub fn seahaven() -> Rules {
    base(
        "Seahaven Towers",
        Pack::default(),
        vec![
            GroupRules::new("cell", PileKind::Cell, 4),
            foundations(4),
            tableau(10)
                .build(BuildBy::Suit, Dir::Down)
                .empty(EmptyRule::Rank(13))
                .moves(MoveUnit::Supermove),
        ],
        vec![
            DealStep::to("tableau").each(5),
            DealStep::to("cell").piles(&[1, 2]).each(1),
        ],
    )
}

// ----- Spider family -----------------------------------------------------------------------------

/// Spider with 1, 2 or 4 suits (104 cards either way). Runs in one suit move together; whole
/// suits from king to ace go to the foundations; the stock deals a card to every pile, and only
/// when none is empty.
pub fn spider(suits: u8) -> Rules {
    let (name, suit_list) = match suits {
        1 => ("Spider (1 suit)", vec![Suit::Spades]),
        2 => ("Spider (2 suits)", vec![Suit::Spades, Suit::Hearts]),
        _ => ("Spider", Suit::ALL.to_vec()),
    };
    let copies = 8 / suit_list.len() as u8;
    let mut groups = vec![GroupRules::new("stock", PileKind::Stock, 1)];
    groups.push(foundations(8).complete_runs());
    groups.push(
        tableau(10)
            .build(BuildBy::Any, Dir::Down)
            .moves(MoveUnit::SuitRun),
    );
    let mut r = base(
        name,
        Pack {
            copies,
            suits: suit_list,
            ranks: (1..=13).collect(),
            jokers: 0,
        },
        groups,
        vec![
            DealStep::to("tableau")
                .counts(&[5, 5, 5, 5, 4, 4, 4, 4, 4, 4])
                .down(),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules {
        to: "tableau".into(),
        count: 1,
        redeals: 0,
        unlimited_redeals: false,
        require_no_empty: true,
        auto: false,
        first_empty: false,
        only_when_empty: false,
        skip_empty: false,
        face_up: false,
        count_per_round: Vec::new(),
        final_to: None,
        piles: None,
        final_piles: None,
        deal_per_pile: false,
        redeal_reverse: false,
        deal_empty_first: false,
        divert_ranks: Vec::new(),
        piles_per_round: Vec::new(),
    });
    if suits >= 4 {
        // Tuned on deals 101-120, checked on 121-140: with four suits, cards out of sequence
        // and face-down cards matter more, and the fast phase deserves most of the budget.
        // (Two suits does as well or better untuned.)
        r.search.disorder_weight = 3;
        r.search.down_weight = 5;
        r.search.fast_share = 90;
    }
    r
}

/// Spiderette: one-pack Spider on a Klondike layout.
pub fn spiderette() -> Rules {
    let mut r = spider(4);
    r.name = "Spiderette".into();
    r.pack = Pack::default();
    r.groups[1].count = 4;
    r.groups[2].count = 7;
    r.deal = vec![
        DealStep::to("tableau")
            .counts(&[0, 1, 2, 3, 4, 5, 6])
            .down(),
        DealStep::to("tableau").each(1),
        DealStep::to("stock").rest().down(),
    ];
    r
}

/// Scorpion: seven piles of seven, three face down in each of the first four; any face-up
/// group moves; build down in suit; kings to empty piles; the last three cards are dealt
/// to the first three piles on request.
pub fn scorpion() -> Rules {
    let mut groups = vec![GroupRules::new("stock", PileKind::Stock, 1)];
    groups.push(foundations(4).complete_runs());
    groups.push(
        tableau(7)
            .build(BuildBy::Suit, Dir::Down)
            .empty(EmptyRule::Rank(13))
            .moves(MoveUnit::AnyFaceUp),
    );
    let mut r = base(
        "Scorpion",
        Pack::default(),
        groups,
        vec![
            DealStep::to("tableau").piles(&[0, 1, 2, 3]).each(3).down(),
            DealStep::to("tableau").counts(&[4, 4, 4, 4, 7, 7, 7]),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules {
        to: "tableau".into(),
        count: 1,
        redeals: 0,
        unlimited_redeals: false,
        require_no_empty: false,
        auto: false,
        first_empty: false,
        only_when_empty: false,
        skip_empty: false,
        face_up: false,
        count_per_round: Vec::new(),
        final_to: None,
        piles: None,
        final_piles: None,
        deal_per_pile: false,
        redeal_reverse: false,
        deal_empty_first: false,
        divert_ranks: Vec::new(),
        piles_per_round: Vec::new(),
    });
    r
}

// ----- Forty Thieves family ----------------------------------------------------------------------

/// Forty Thieves: two packs, ten piles of four face up, build down in suit, one card moves at a
/// time, one pass through the stock.
pub fn forty_thieves() -> Rules {
    let mut groups = stock_waste();
    groups.push(foundations(8));
    groups.push(tableau(10).build(BuildBy::Suit, Dir::Down));
    let mut r = base(
        "Forty Thieves",
        Pack::decks(2),
        groups,
        vec![
            DealStep::to("tableau").each(4),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(to_waste(1, Some(0)));
    r
}

/// Gypsy: two packs, eight piles (two down, one up), build down in alternate colours, sequences
/// move together, the stock deals one card to each pile.
pub fn gypsy() -> Rules {
    let mut groups = vec![GroupRules::new("stock", PileKind::Stock, 1)];
    groups.push(foundations(8));
    groups.push(
        tableau(8)
            .build(BuildBy::Alternate, Dir::Down)
            .moves(MoveUnit::Run),
    );
    let mut r = base(
        "Gypsy",
        Pack::decks(2),
        groups,
        vec![
            DealStep::to("tableau").each(2).down(),
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules {
        to: "tableau".into(),
        count: 1,
        redeals: 0,
        unlimited_redeals: false,
        require_no_empty: false,
        auto: false,
        first_empty: false,
        only_when_empty: false,
        skip_empty: false,
        face_up: false,
        count_per_round: Vec::new(),
        final_to: None,
        piles: None,
        final_piles: None,
        deal_per_pile: false,
        redeal_reverse: false,
        deal_empty_first: false,
        divert_ranks: Vec::new(),
        piles_per_round: Vec::new(),
    });
    r
}

// ----- Open builders -----------------------------------------------------------------------------

/// Baker's Dozen: thirteen piles of four with kings moved to the bottom; build down regardless
/// of suit, one card at a time; empty piles stay empty.
pub fn bakers_dozen() -> Rules {
    let mut r = base(
        "Baker's Dozen",
        Pack::default(),
        vec![
            foundations(4),
            tableau(13)
                .build(BuildBy::Any, Dir::Down)
                .empty(EmptyRule::None),
        ],
        vec![DealStep::to("tableau").each(4).kings_to_bottom()],
    );
    r.search.blocker_weight = 4;
    r
}

/// Beleaguered Castle: aces start on the foundations; eight piles of six; build down regardless
/// of suit, one card at a time; any card to an empty pile.
pub fn beleaguered_castle() -> Rules {
    base(
        "Beleaguered Castle",
        Pack::default(),
        vec![foundations(4), tableau(8).build(BuildBy::Any, Dir::Down)],
        vec![
            DealStep::to("foundation").ranks(&[1]),
            DealStep::to("tableau").each(6),
        ],
    )
}

/// Streets and Alleys: Beleaguered Castle with the aces dealt into the tableau.
pub fn streets_and_alleys() -> Rules {
    base(
        "Streets and Alleys",
        Pack::default(),
        vec![foundations(4), tableau(8).build(BuildBy::Any, Dir::Down)],
        vec![DealStep::to("tableau").rest()],
    )
}

/// Fortress: ten piles (two of six, eight of five), build up or down in suit, one card at a time.
pub fn fortress() -> Rules {
    base(
        "Fortress",
        Pack::default(),
        vec![
            foundations(4),
            tableau(10).build(BuildBy::Suit, Dir::Either),
        ],
        vec![DealStep::to("tableau").rest()],
    )
}

// ----- Removal games -----------------------------------------------------------------------------

/// Golf: seven piles of five face up; play the top of any pile onto the foundation one rank up
/// or down, regardless of suit, no wrapping, nothing on a king; the stock deals to the
/// foundation one card at a time, once. Won when the tableau is clear.
pub fn golf() -> Rules {
    let mut r = base(
        "Golf",
        Pack::default(),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            foundations(1)
                .build_rule(Build {
                    by: BuildBy::Any,
                    dir: Dir::Either,
                    blocked_ranks: vec![13],
                    ..Build::default()
                })
                .empty(EmptyRule::Any)
                .accepts_from(&["tableau"]),
            tableau(7).empty(EmptyRule::None),
        ],
        vec![
            DealStep::to("tableau").each(5),
            DealStep::to("foundation").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules {
        to: "foundation".into(),
        count: 1,
        redeals: 0,
        unlimited_redeals: false,
        require_no_empty: false,
        auto: false,
        first_empty: false,
        only_when_empty: false,
        skip_empty: false,
        face_up: false,
        count_per_round: Vec::new(),
        final_to: None,
        piles: None,
        final_piles: None,
        deal_per_pile: false,
        redeal_reverse: false,
        deal_empty_first: false,
        divert_ranks: Vec::new(),
        piles_per_round: Vec::new(),
    });
    r.win = WinRule::Empty(vec!["tableau".into()]);
    r
}

/// TriPeaks: three overlapping peaks of 28 cards; play uncovered cards to the foundation one
/// rank up or down, regardless of suit, king and ace wrapping; stock deals to the foundation.
pub fn tripeaks() -> Rules {
    let mut r = golf();
    r.name = "TriPeaks".into();
    r.groups[1].build = Some(Build {
        by: BuildBy::Any,
        dir: Dir::Either,
        wrap: true,
        ..Build::default()
    });
    r.groups[2] = tableau(28).empty(EmptyRule::None).layout(Layout::TriPeaks);
    r.deal = vec![
        DealStep::to("tableau")
            .piles(&(0..18).collect::<Vec<_>>())
            .each(1)
            .down(),
        DealStep::to("tableau")
            .piles(&(18..28).collect::<Vec<_>>())
            .each(1),
        DealStep::to("foundation").each(1),
        DealStep::to("stock").rest().down(),
    ];
    r
}

/// Pyramid: a 28-card pyramid; remove uncovered cards in pairs adding to 13, kings alone; the
/// stock turns one card at a time to a waste whose top card also pairs; three passes in all.
/// Won when the pyramid is clear.
pub fn pyramid() -> Rules {
    let mut groups = stock_waste();
    groups.push(
        GroupRules::new("discard", PileKind::Foundation, 1)
            .build(BuildBy::None, Dir::Down)
            .empty(EmptyRule::None),
    );
    groups.push(
        tableau(28)
            .empty(EmptyRule::None)
            .moves(MoveUnit::None)
            .layout(Layout::Pyramid { rows: 7 }),
    );
    groups[1].moves = Some(MoveUnit::None);
    let mut r = base(
        "Pyramid",
        Pack::default(),
        groups,
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(to_waste(1, Some(2)));
    r.removal = vec![RemovalRules {
        kind: RemovalKind::PairsSum {
            total: 13,
            alone: vec![13],
        },
        from: vec!["tableau".into(), "waste".into()],
        to: "discard".into(),
        adjacent: None,
    }];
    r.win = WinRule::Empty(vec!["tableau".into()]);
    r
}

/// Aces Up: four piles; discard a top card when a higher card of its suit is on top elsewhere
/// (aces high); any top card may move to an empty pile; the stock deals a card to each pile.
/// Won when only the four aces remain.
pub fn aces_up() -> Rules {
    let mut r = base(
        "Aces Up",
        Pack {
            ranks: (2..=13).chain([1]).collect(),
            ..Pack::default()
        },
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("discard", PileKind::Foundation, 1)
                .build(BuildBy::None, Dir::Down)
                .empty(EmptyRule::None),
            tableau(4),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules {
        to: "tableau".into(),
        count: 1,
        redeals: 0,
        unlimited_redeals: false,
        require_no_empty: false,
        auto: false,
        first_empty: false,
        only_when_empty: false,
        skip_empty: false,
        face_up: false,
        count_per_round: Vec::new(),
        final_to: None,
        piles: None,
        final_piles: None,
        deal_per_pile: false,
        redeal_reverse: false,
        deal_empty_first: false,
        divert_ranks: Vec::new(),
        piles_per_round: Vec::new(),
    });
    r.removal = vec![RemovalRules {
        kind: RemovalKind::LowerOfSuit,
        from: vec!["tableau".into()],
        to: "discard".into(),
        adjacent: None,
    }];
    r.win = WinRule::Count {
        group: "discard".into(),
        cards: 48,
    };
    // Tuned on deals 101-120, checked on 121-140.
    r.search.fast_share = 90;
    r
}

// ----- Builders with a hand card -----------------------------------------------------------------

fn hand_stock() -> StockRules {
    StockRules {
        to: "waste".into(),
        count: 1,
        redeals: 0,
        unlimited_redeals: false,
        require_no_empty: false,
        auto: false,
        first_empty: false,
        only_when_empty: true,
        skip_empty: false,
        face_up: false,
        count_per_round: Vec::new(),
        final_to: None,
        piles: None,
        final_piles: None,
        deal_per_pile: false,
        redeal_reverse: false,
        deal_empty_first: false,
        divert_ranks: Vec::new(),
        piles_per_round: Vec::new(),
    }
}

/// Four waste piles that take any card from the hand, and play only to the foundations.
fn waste_piles() -> GroupRules {
    tableau(4)
        .build(BuildBy::Any, Dir::Any)
        .accepts_from(&["waste"])
}

/// Calculation (Broken Intervals): an A, 2, 3 and 4 start the foundations, which build
/// regardless of suit by ones, twos, threes and fours (wrapping past the king) up to a king.
/// Each card turned from the stock goes to a foundation or onto one of four waste piles, whose
/// top cards play only to the foundations.
pub fn calculation() -> Rules {
    let mut groups = stock_waste();
    let mut f = foundations(4).empty(EmptyRule::None).max_cards(13);
    for k in 0..4u16 {
        f = f.pile(PileOverride::on(&[k]).build(Build {
            by: BuildBy::Any,
            dir: Dir::Up,
            wrap: true,
            step: k as u8 + 1,
            ..Build::default()
        }));
    }
    groups.push(f);
    groups.push(waste_piles());
    let mut r = base(
        "Calculation",
        Pack::default(),
        groups,
        vec![
            DealStep::to("foundation").ranks(&[1, 2, 3, 4]).one_each(),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(hand_stock());
    r.search.blocker_weight = 4;
    r
}

/// Sir Tommy (Old Patience): Calculation's four waste piles, with ordinary foundations built
/// up from the ace regardless of suit.
pub fn sir_tommy() -> Rules {
    let mut groups = stock_waste();
    groups.push(foundations(4).build(BuildBy::Any, Dir::Up));
    groups.push(waste_piles());
    let mut r = base(
        "Sir Tommy",
        Pack::default(),
        groups,
        vec![DealStep::to("stock").rest().down()],
    );
    r.stock = Some(hand_stock());
    r
}

/// Bisley: the aces start four foundations built up in suit; kings, as they come free, start
/// four more built down in suit. Thirteen piles (three cards beside each ace, four in the
/// rest) build up or down in suit, one card at a time; spaces aren't filled.
pub fn bisley() -> Rules {
    base(
        "Bisley",
        Pack::default(),
        vec![
            foundations(8).pile(
                PileOverride::on(&[4, 5, 6, 7])
                    .build(Build {
                        by: BuildBy::Suit,
                        dir: Dir::Down,
                        ..Build::default()
                    })
                    .empty(EmptyRule::Rank(13)),
            ),
            tableau(13)
                .build(BuildBy::Suit, Dir::Either)
                .empty(EmptyRule::None),
        ],
        vec![
            DealStep::to("foundation").piles(&[0, 1, 2, 3]).ranks(&[1]),
            DealStep::to("tableau").counts(&[3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4]),
        ],
    )
}

// ----- Pairing games -----------------------------------------------------------------------------

fn discard() -> GroupRules {
    GroupRules::new("discard", PileKind::Foundation, 1)
        .build(BuildBy::None, Dir::Down)
        .empty(EmptyRule::None)
}

/// Elevens: nine cards face up; remove pairs adding to eleven (ace counts one), or a jack,
/// queen and king together; refill the gaps from the stock. Won when every card is removed.
pub fn elevens() -> Rules {
    let mut r = base(
        "Elevens",
        Pack::default(),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            discard(),
            tableau(9)
                .empty(EmptyRule::None)
                .moves(MoveUnit::None)
                .refill_from(&["stock"]),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.removal = vec![
        RemovalRules {
            kind: RemovalKind::PairsSum {
                total: 11,
                alone: vec![],
            },
            from: vec!["tableau".into()],
            to: "discard".into(),
            adjacent: None,
        },
        RemovalRules {
            kind: RemovalKind::RankSet {
                ranks: vec![11, 12, 13],
            },
            from: vec!["tableau".into()],
            to: "discard".into(),
            adjacent: None,
        },
    ];
    r.win = WinRule::Count {
        group: "discard".into(),
        cards: 52,
    };
    r
}

/// Nestor: eight piles of six face up and a reserve of four; remove any two available cards of
/// the same rank. Won when every card is removed.
pub fn nestor() -> Rules {
    let mut r = base(
        "Nestor",
        Pack::default(),
        vec![
            discard(),
            GroupRules::new("reserve", PileKind::Reserve, 4).moves(MoveUnit::None),
            tableau(8).empty(EmptyRule::None).moves(MoveUnit::None),
        ],
        vec![
            DealStep::to("tableau").each(6),
            DealStep::to("reserve").each(1),
        ],
    );
    r.removal = vec![RemovalRules {
        kind: RemovalKind::SameRank,
        from: vec!["tableau".into(), "reserve".into()],
        to: "discard".into(),
        adjacent: None,
    }];
    r.win = WinRule::Count {
        group: "discard".into(),
        cards: 52,
    };
    r
}

// ----- Gap games ---------------------------------------------------------------------------------

/// Gaps (Montana family), played without redeals: all 52 cards dealt in four rows of 13, the
/// aces removed to leave four gaps. A gap takes the card one rank higher, in the same suit, than
/// the card to its left; a gap in the first column takes any two. Won when each row runs two to
/// king in one suit from the left.
pub fn gaps_single_deal() -> Rules {
    let mut r = base(
        "Gaps (single deal)",
        Pack::default(),
        vec![
            discard(),
            tableau(52)
                .layout(Layout::Grid { columns: 13 })
                .build(BuildBy::Suit, Dir::Up)
                .empty(EmptyRule::AfterLeft { first: 2 })
                .max_cards(1),
        ],
        vec![
            DealStep::to("tableau").rest(),
            DealStep::to("discard").collect(&[1]),
        ],
    );
    r.win = WinRule::Rows {
        group: "tableau".into(),
        length: Some(12),
    };
    r
}

/// Montana: Gaps with two redeals. A redeal leaves each row's sequence from the left in place,
/// shuffles the other cards, and deals them back leaving one gap after each sequence.
pub fn montana() -> Rules {
    let mut r = gaps_single_deal();
    r.name = "Montana".into();
    r.redeal = Some(RedealRules {
        group: "tableau".into(),
        times: 2,
        unlimited: false,
        how: RedealHow::GridShuffle,
    });
    r
}

// ----- Redealing builders ------------------------------------------------------------------------

/// Cruel: the aces start the foundations; twelve piles of four, building down in suit one card
/// at a time; spaces aren't filled. Any number of times, the piles may be gathered in order
/// and dealt out again in fours.
pub fn cruel() -> Rules {
    let mut r = base(
        "Cruel",
        Pack::default(),
        vec![
            foundations(4),
            tableau(12)
                .build(BuildBy::Suit, Dir::Down)
                .empty(EmptyRule::None),
        ],
        vec![
            DealStep::to("foundation").ranks(&[1]),
            DealStep::to("tableau").each(4),
        ],
    );
    r.redeal = Some(RedealRules {
        group: "tableau".into(),
        times: 0,
        unlimited: true,
        how: RedealHow::Regroup { per_pile: 4 },
    });
    r
}

/// La Belle Lucie (Fan): seventeen fans of three and one of one; build down in suit one card at
/// a time; spaces aren't filled. Twice, the fans may be gathered, shuffled and dealt again in
/// threes.
pub fn la_belle_lucie() -> Rules {
    let mut r = base(
        "La Belle Lucie",
        Pack::default(),
        vec![
            foundations(4),
            tableau(18)
                .build(BuildBy::Suit, Dir::Down)
                .empty(EmptyRule::None),
        ],
        vec![
            DealStep::to("tableau")
                .piles(&(0..17).collect::<Vec<_>>())
                .each(3),
            DealStep::to("tableau").piles(&[17]).each(1),
        ],
    );
    r.redeal = Some(RedealRules {
        group: "tableau".into(),
        times: 2,
        unlimited: false,
        how: RedealHow::Shuffle { per_pile: 3 },
    });
    r
}

/// Monte Carlo (Weddings): twenty-five cards in a five-by-five grid; remove two cards of the same
/// rank that touch, side by side, above and below, or diagonally. At any time, close the gaps by
/// sliding the cards forward in reading order and fill the places left at the end from the
/// stock. Won when every card is removed.
pub fn monte_carlo() -> Rules {
    let mut r = base(
        "Monte Carlo",
        Pack::default(),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            discard(),
            tableau(25)
                .layout(Layout::Grid { columns: 5 })
                .empty(EmptyRule::None)
                .moves(MoveUnit::None)
                .max_cards(1),
        ],
        vec![
            DealStep::to("tableau").each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.removal = vec![RemovalRules {
        kind: RemovalKind::SameRank,
        from: vec!["tableau".into()],
        to: "discard".into(),
        adjacent: Some(Adjacency::Eight),
    }];
    r.redeal = Some(RedealRules {
        group: "tableau".into(),
        times: 0,
        unlimited: true,
        how: RedealHow::Compact {
            refill_from: Some("stock".into()),
        },
    });
    r.win = WinRule::Count {
        group: "discard".into(),
        cards: 52,
    };
    r
}

/// Accordion: deal the cards one at a time into a row. A pile may be moved, whole, onto the
/// pile just to its left or three to its left when their top cards match in suit or rank; the
/// row closes up. Won when every card is dealt into a single pile.
pub fn accordion() -> Rules {
    let mut row = tableau(52)
        .build(BuildBy::SuitOrRank, Dir::Any)
        .moves(MoveUnit::Whole)
        .empty(EmptyRule::None);
    row.reach = Some(vec![1, 3]);
    row.compact = true;
    let mut r = base(
        "Accordion",
        Pack::default(),
        vec![GroupRules::new("stock", PileKind::Stock, 1), row],
        vec![
            DealStep::to("tableau").piles(&[0]).each(1),
            DealStep::to("stock").rest().down(),
        ],
    );
    r.stock = Some(StockRules {
        to: "tableau".into(),
        count: 1,
        redeals: 0,
        unlimited_redeals: false,
        require_no_empty: false,
        auto: false,
        first_empty: true,
        only_when_empty: false,
        skip_empty: false,
        face_up: false,
        count_per_round: Vec::new(),
        final_to: None,
        piles: None,
        final_piles: None,
        deal_per_pile: false,
        redeal_reverse: false,
        deal_empty_first: false,
        divert_ranks: Vec::new(),
        piles_per_round: Vec::new(),
    });
    r.win = WinRule::AtMostPiles {
        group: "tableau".into(),
        piles: 1,
    };
    r
}

// ----- Score games -------------------------------------------------------------------------------

/// Poker Squares: turn cards one at a time and place each in any empty space of a five-by-five
/// grid. When the grid is full, each row and each column scores as a poker hand (American
/// points). 200 points is a common target.
pub fn poker_squares() -> Rules {
    let mut r = base(
        "Poker Squares",
        Pack::default(),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            tableau(25)
                .layout(Layout::Grid { columns: 5 })
                .moves(MoveUnit::None)
                .max_cards(1)
                .accepts_from(&["waste"]),
        ],
        vec![DealStep::to("stock").rest().down()],
    );
    r.stock = Some(StockRules {
        auto: true,
        ..hand_stock()
    });
    r.score = Some(ScoreRules {
        group: "tableau".into(),
        hands: Hands::Poker(PokerTable::Named(PokerScoring::American)),
    });
    r.win = WinRule::Score { at_least: 200 };
    r
}

/// Cribbage Squares: turn cards one at a time and place each in any empty space of a
/// four-by-four grid. When the grid is full, the next card is turned as the starter, and each
/// row and column scores as a cribbage hand with it (fifteens, pairs, runs, flushes, nobs, and
/// two for his heels if the starter is a jack). Sources differ on the score that counts as a
/// win; this preset plays for 61.
pub fn cribbage_squares() -> Rules {
    let mut r = base(
        "Cribbage Squares",
        Pack::default(),
        vec![
            GroupRules::new("stock", PileKind::Stock, 1),
            GroupRules::new("waste", PileKind::Waste, 1),
            tableau(16)
                .layout(Layout::Grid { columns: 4 })
                .moves(MoveUnit::None)
                .max_cards(1)
                .accepts_from(&["waste"]),
        ],
        vec![DealStep::to("stock").rest().down()],
    );
    r.stock = Some(StockRules {
        auto: true,
        ..hand_stock()
    });
    r.score = Some(ScoreRules {
        group: "tableau".into(),
        hands: Hands::Cribbage(CribbageRules {
            starter: Some("waste".into()),
            points: CribbagePoints::default(),
        }),
    });
    r.win = WinRule::Score { at_least: 61 };
    r
}
