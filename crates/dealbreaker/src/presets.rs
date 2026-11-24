//! Ready-made rules for 50 well-known games, stored as compact JSON in `presets/*.json`.
//!
//! These are traditional games, described from their commonly published rules. Where sources
//! disagree, the choice made is noted on the preset. Each preset is also a worked example for
//! writing your own rules: `dealbreaker rules <id>` prints any of them as JSON.

use crate::rules::Rules;

/// A preset: an id (kebab-case) and its rules.
pub struct Preset {
    pub id: &'static str,
    pub rules: fn() -> Rules,
}

fn load(json: &str) -> Rules {
    serde_json::from_str(json).expect("built-in preset JSON is valid")
}

macro_rules! json_preset {
    ($(#[$meta:meta])* $fn_name:ident, $file:literal) => {
        $(#[$meta])*
        pub fn $fn_name() -> Rules {
            load(include_str!(concat!("../presets/", $file)))
        }
    };
}

/// Every built-in preset (50 games).
pub const PRESETS: &[Preset] = &[
    // Klondike family
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
        id: "whitehead",
        rules: whitehead,
    },
    Preset {
        id: "easthaven",
        rules: easthaven,
    },
    Preset {
        id: "king-albert",
        rules: king_albert,
    },
    Preset {
        id: "raglan",
        rules: raglan,
    },
    // FreeCell family
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
        id: "penguin",
        rules: penguin,
    },
    // Spider & Scorpion family
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
        id: "simple-simon",
        rules: simple_simon,
    },
    Preset {
        id: "scorpion",
        rules: scorpion,
    },
    Preset {
        id: "wasp",
        rules: wasp,
    },
    // Yukon family
    Preset {
        id: "yukon",
        rules: yukon,
    },
    Preset {
        id: "russian",
        rules: russian,
    },
    // Forty Thieves family
    Preset {
        id: "forty-thieves",
        rules: forty_thieves,
    },
    Preset {
        id: "gypsy",
        rules: gypsy,
    },
    // Open builders & fans
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
        id: "flower-garden",
        rules: flower_garden,
    },
    Preset {
        id: "grandfathers-clock",
        rules: grandfathers_clock,
    },
    Preset {
        id: "little-spider",
        rules: little_spider,
    },
    // Canfield
    Preset {
        id: "canfield",
        rules: canfield,
    },
    // Removal & pairing games
    Preset {
        id: "golf",
        rules: golf,
    },
    Preset {
        id: "black-hole",
        rules: black_hole,
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
        id: "elevens",
        rules: elevens,
    },
    Preset {
        id: "nestor",
        rules: nestor,
    },
    // Calculation, gaps, redeal, and scored games
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

// ----- Klondike family ---------------------------------------------------------------------------

json_preset!(
    /// Klondike, turning one card at a time, unlimited passes through the stock.
    klondike1,
    "klondike.json"
);

json_preset!(
    /// Klondike, turning three at a time, unlimited passes.
    klondike3,
    "klondike-draw3.json"
);

json_preset!(
    /// Klondike with the two jokers left in: wild on the tableau, where a joker stands for any
    /// card that would fit. Jokers never go to the foundations; the game is won with every other
    /// card home.
    klondike_jokers,
    "klondike-jokers.json"
);

json_preset!(
    /// Vegas: three at a time, three passes in all (two redeals).
    vegas,
    "klondike-vegas.json"
);

json_preset!(
    /// Whitehead: all-face-up Klondike building down in the same colour, moving in-suit runs,
    /// with any card allowed on an empty column and one pass through the stock.
    whitehead,
    "whitehead.json"
);

json_preset!(
    /// Easthaven: one-pack Gypsy; seven piles (two down, one up), build down in alternate colours,
    /// sequences move together, stock deals one card to each pile.
    easthaven,
    "easthaven.json"
);

json_preset!(
    /// King Albert: open nine-pile Klondike (1..9 cards face up) with a seven-card reserve;
    /// one card moves at a time, any card to an empty column.
    king_albert,
    "king-albert.json"
);

json_preset!(
    /// Raglan: open nine-pile builder with aces pre-dealt to foundations and a six-card reserve;
    /// build down in alternate colours, one card at a time.
    raglan,
    "raglan.json"
);

json_preset!(
    /// Yukon: no stock; any face-up group of cards moves, building down in alternate colours.
    yukon,
    "yukon.json"
);

json_preset!(
    /// Russian Solitaire: Yukon building in suit.
    russian,
    "russian.json"
);

json_preset!(
    /// Canfield: 13-card reserve, four tableau piles refilled from it, foundations start at the
    /// rank of the first card dealt to them and wrap round. Stock three at a time, unlimited
    /// passes. Tableau sequences move as units.
    canfield,
    "canfield.json"
);

// ----- FreeCell family ---------------------------------------------------------------------------

json_preset!(
    /// FreeCell: four cells, eight piles, alternate colours, sequences move as far as free space
    /// allows.
    freecell,
    "freecell.json"
);

json_preset!(
    /// Baker's Game: FreeCell building in suit.
    bakers_game,
    "bakers-game.json"
);

json_preset!(
    /// Eight Off: eight cells, four filled by the deal; build in suit; only kings to empty piles.
    eight_off,
    "eight-off.json"
);

json_preset!(
    /// Seahaven Towers: ten piles of five, four cells with two cards; build in suit; kings to
    /// empties.
    seahaven,
    "seahaven-towers.json"
);

json_preset!(
    /// Penguin: seven cells ("flipper"), seven piles of seven; the first card dealt sets the
    /// foundation base rank (its three twins start on foundations); build down in suit wrapping,
    /// with only the rank below the base allowed on an empty column.
    penguin,
    "penguin.json"
);

// ----- Spider & Scorpion family ------------------------------------------------------------------

/// Spider with 1, 2 or 4 suits (104 cards either way). Runs in one suit move together; whole
/// suits from king to ace go to the foundations; the stock deals a card to every pile, and only
/// when none is empty.
pub fn spider(suits: u8) -> Rules {
    match suits {
        1 => load(include_str!("../presets/spider-1suit.json")),
        2 => load(include_str!("../presets/spider-2suit.json")),
        _ => load(include_str!("../presets/spider.json")),
    }
}

json_preset!(
    /// Spiderette: one-pack Spider on a Klondike layout.
    spiderette,
    "spiderette.json"
);

json_preset!(
    /// Simple Simon: open one-pack Spider dealt across ten piles (8, 8, 8, 7, 6, 5, 4, 3, 2, 1),
    /// no stock.
    simple_simon,
    "simple-simon.json"
);

json_preset!(
    /// Scorpion: seven piles of seven, three face down in each of the first four; any face-up
    /// group moves; build down in suit; kings to empty piles; the last three cards are dealt
    /// to the first three piles on request.
    scorpion,
    "scorpion.json"
);

json_preset!(
    /// Wasp: Scorpion where any card (or face-up group) may fill an empty column.
    wasp,
    "wasp.json"
);

// ----- Forty Thieves family ----------------------------------------------------------------------

json_preset!(
    /// Forty Thieves: two packs, ten piles of four face up, build down in suit, one card moves at
    /// a time, one pass through the stock.
    forty_thieves,
    "forty-thieves.json"
);

json_preset!(
    /// Gypsy: two packs, eight piles (two down, one up), build down in alternate colours,
    /// sequences move together, the stock deals one card to each pile.
    gypsy,
    "gypsy.json"
);

// ----- Open builders -----------------------------------------------------------------------------

json_preset!(
    /// Baker's Dozen: thirteen piles of four with kings moved to the bottom; build down regardless
    /// of suit, one card at a time; empty piles stay empty.
    bakers_dozen,
    "bakers-dozen.json"
);

json_preset!(
    /// Beleaguered Castle: aces start on the foundations; eight piles of six; build down
    /// regardless of suit, one card at a time; any card to an empty pile.
    beleaguered_castle,
    "beleaguered-castle.json"
);

json_preset!(
    /// Streets and Alleys: Beleaguered Castle with the aces dealt into the tableau.
    streets_and_alleys,
    "streets-and-alleys.json"
);

json_preset!(
    /// Fortress: ten piles (two of six, eight of five), build up or down in suit, one card at a
    /// time.
    fortress,
    "fortress.json"
);

json_preset!(
    /// Flower Garden: six flower beds of six face up plus a sixteen-card open bouquet reserve;
    /// build down regardless of suit, one card at a time.
    flower_garden,
    "flower-garden.json"
);

json_preset!(
    /// Grandfather's Clock: twelve clock-face foundations starting from specific cards and
    /// building up in suit (wrapping) to each hour's rank, fed from eight open piles of five.
    grandfathers_clock,
    "grandfathers-clock.json"
);

json_preset!(
    /// Little Spider: two red foundations building up from aces and two black foundations building
    /// down from kings; eight tableau piles building up or down regardless of suit (wrapping),
    /// with the stock dealing one card to each pile.
    little_spider,
    "little-spider.json"
);

// ----- Removal & pairing games -------------------------------------------------------------------

json_preset!(
    /// Golf: seven piles of five face up; play the top of any pile onto the foundation one rank up
    /// or down, regardless of suit, no wrapping, nothing on a king; the stock deals to the
    /// foundation one card at a time, once. Won when the tableau is clear.
    golf,
    "golf.json"
);

json_preset!(
    /// Black Hole: open Golf; the ace of spades starts on the foundation and the other 51 cards
    /// form 17 fans of three; build up or down regardless of suit, wrapping king and ace.
    black_hole,
    "black-hole.json"
);

json_preset!(
    /// TriPeaks: three overlapping peaks of 28 cards; play uncovered cards to the foundation one
    /// rank up or down, regardless of suit, king and ace wrapping; stock deals to the foundation.
    tripeaks,
    "tripeaks.json"
);

json_preset!(
    /// Pyramid: a 28-card pyramid; remove uncovered cards in pairs adding to 13, kings alone; the
    /// stock turns one card at a time to a waste whose top card also pairs; three passes in all.
    /// Won when the pyramid is clear.
    pyramid,
    "pyramid.json"
);

json_preset!(
    /// Aces Up: four tableau piles; discard any top card that is outranked by another top card of
    /// the same suit (aces high); any top card may move into an empty pile; the stock deals one
    /// card to each pile. Won when only the four aces remain on the tableau.
    aces_up,
    "aces-up.json"
);

json_preset!(
    /// Elevens: nine tableau piles; remove pairs summing to 11, or a jack, queen and king
    /// together; empty piles refill automatically from the stock. Won when every card has been
    /// removed.
    elevens,
    "elevens.json"
);

json_preset!(
    /// Nestor: eight columns of six face up plus four single-card reserves; remove exposed pairs
    /// of the same rank. Won when every card has been removed.
    nestor,
    "nestor.json"
);

// ----- Calculation, gaps, redeal, and scored games -----------------------------------------------

json_preset!(
    /// Calculation: four foundations starting A, 2, 3, 4 and building up regardless of suit by
    /// steps of 1, 2, 3 and 4 respectively (wrapping past king, stopping at king); four waste
    /// piles where the player may stack cards from the stock one at a time.
    calculation,
    "calculation.json"
);

json_preset!(
    /// Sir Tommy: four foundations building up from ace regardless of suit; four waste piles
    /// receiving cards one at a time from the stock.
    sir_tommy,
    "sir-tommy.json"
);

json_preset!(
    /// Bisley: four ace foundations building up in suit and four king foundations building down in
    /// suit; thirteen open piles building up or down in suit, one card at a time, empty piles
    /// stay empty.
    bisley,
    "bisley.json"
);

json_preset!(
    /// Gaps (single deal): all 52 cards in a 4×13 grid with the aces removed to leave four gaps;
    /// move the card one rank higher in suit than the card to a gap's left (or any two into the
    /// first column). No redeals.
    gaps_single_deal,
    "gaps-single-deal.json"
);

json_preset!(
    /// Montana: Gaps with two redeals; cards already in place from the left edge of each row stay
    /// put while the rest are gathered, shuffled and dealt back.
    montana,
    "montana.json"
);

json_preset!(
    /// Cruel: aces start on the foundations; twelve piles of four; build down in suit, one card at
    /// a time; empty piles stay empty; unlimited redeals gathering the piles in order without
    /// shuffling and dealing back in fours.
    cruel,
    "cruel.json"
);

json_preset!(
    /// La Belle Lucie: seventeen fans of three and one single card; build down in suit, one card
    /// at a time; empty piles stay empty; two shuffling redeals.
    la_belle_lucie,
    "la-belle-lucie.json"
);

json_preset!(
    /// Monte Carlo: 5×5 grid of cards; remove pairs of the same rank that are adjacent
    /// horizontally, vertically or diagonally; compact the grid and refill from the stock as a
    /// redeal action.
    monte_carlo,
    "monte-carlo.json"
);

json_preset!(
    /// Accordion: all 52 cards in a row; move a pile onto the pile one or three places to its left
    /// if their top cards match in suit or rank, closing the gap. Won when one pile remains.
    accordion,
    "accordion.json"
);

json_preset!(
    /// Poker Squares: place 25 cards from the stock into a 5×5 grid to maximise the sum of poker
    /// hands across the five rows and five columns (American scoring).
    poker_squares,
    "poker-squares.json"
);

json_preset!(
    /// Cribbage Squares: place 16 cards into a 4×4 grid with a 17th starter card to maximise the
    /// sum of cribbage hands across the four rows and four columns.
    cribbage_squares,
    "cribbage-squares.json"
);
