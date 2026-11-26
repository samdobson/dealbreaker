//! The rules model: a game is described as data.
//!
//! A [`Rules`] value names the pack, the groups of piles on the table (tableau, foundations,
//! cells, reserves, stock, waste), what each pile accepts and lets go, how the stock deals, how
//! the cards are dealt at the start, any card-removal mechanic (Pyramid's pairs, Aces Up's
//! discards), and what counts as a win.
//!
//! Every field that has a sensible default for its pile kind may be left out, so most games are
//! a dozen lines of JSON. The presets in [`crate::presets`] are written with the same types.

use serde::{Deserialize, Serialize};

use crate::card::{Color, Suit};

/// Accepts an optional `"$schema"` metadata string in JSON files and ignores it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct IgnoredSchema;

impl Serialize for IgnoredSchema {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_none()
    }
}

impl<'de> Deserialize<'de> for IgnoredSchema {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let _: Option<String> = Option::deserialize(d)?;
        Ok(IgnoredSchema)
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for IgnoredSchema {
    fn schema_name() -> String {
        "SchemaUri".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        <Option<String>>::json_schema(gen)
    }
}

/// A complete description of one solitaire game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Rules {
    /// Optional JSON Schema URI for editor validation (ignored by the solver).
    #[serde(rename = "$schema", default, skip_serializing)]
    _schema: IgnoredSchema,
    /// Human-readable name, e.g. "FreeCell".
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default_pack")]
    pub pack: Pack,
    /// The piles on the table, in groups of identical piles.
    pub groups: Vec<GroupRules>,
    /// How the stock deals. Needs a group of kind `stock`. A stock without these rules only
    /// feeds refills (`refill_from`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<StockRules>,
    /// Gathering the tableau and dealing it out again, as a player action (Cruel, La Belle
    /// Lucie, Montana). Can't be combined with `stock`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redeal: Option<RedealRules>,
    /// The opening deal, step by step. Cards come off a shuffled pack.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deal: Vec<DealStep>,
    /// Card-removal mechanics, for games that discard cards instead of (or as well as)
    /// building. One object or a list.
    #[serde(
        default,
        deserialize_with = "one_or_many",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub removal: Vec<RemovalRules>,
    /// Points for the position, for games played for a score (Poker Squares).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<ScoreRules>,
    #[serde(default, skip_serializing_if = "is_default_win")]
    pub win: WinRule,
    /// Turn up a face-down card when it becomes the top of an uncovered pile. Default true.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub auto_flip: bool,
    /// Let the solver play cards to foundations automatically when it can prove nothing could
    /// ever need to be built on them. Sound for in-suit foundations; default true.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub safe_autoplay: bool,
    /// Tuning for the solver's heuristic. Changes how fast a win is found, never whether a
    /// position is judged winnable.
    #[serde(default, skip_serializing_if = "SearchHints::is_default")]
    pub search: SearchHints,
    /// Opening choice of the foundation base rank: before a base rank is set, only a single-card
    /// move from this group to a foundation is legal, and its rank becomes the base rank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choose_base: Option<ChooseBase>,
    /// Cards put on top of the shuffled pack before dealing, in the order they are dealt
    /// (Royal Marriage: the queen of hearts first). Only for seeded deals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deck_first: Vec<CardRef>,
    /// Shuffle each copy of the pack on its own, so one whole pack is dealt before the next
    /// (Microbe). Only for seeded deals.
    #[serde(default, skip_serializing_if = "is_false")]
    pub separate_decks: bool,
    /// Cards put at the bottom of the shuffled pack before dealing, in the order they are dealt
    /// (Royal Marriage: the king of hearts last). Only for seeded deals.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deck_last: Vec<CardRef>,
}

/// A card named by suit and rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CardRef {
    pub suit: Suit,
    /// 1 = ace, 13 = king.
    pub rank: u8,
}

/// Rules for choosing the foundation base rank on the opening move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ChooseBase {
    /// Group whose cards may be played to a foundation to choose the base rank.
    pub from: String,
    /// Immediately deal one card from the stock to the emptied source pile after the choice.
    #[serde(default)]
    pub refill_from_stock: bool,
    /// Immediately deal one card from the stock to the waste after the choice.
    #[serde(default)]
    pub deal_waste: bool,
    /// Until the base is chosen, only foundation plays are restricted (to one card from `from`);
    /// every other move and the stock stay open (Nine Across).
    #[serde(default)]
    pub free_moves: bool,
    /// A second group whose cards may also be played to a foundation to choose the base.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub also_from: Option<String>,
    /// Groups whose cards cannot move (and, for the stock, cannot be dealt) until the base is
    /// chosen, even with `free_moves`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lock_until_base: Vec<String>,
}

impl ChooseBase {
    pub fn from(group: &str) -> ChooseBase {
        ChooseBase {
            from: group.to_string(),
            refill_from_stock: false,
            deal_waste: false,
            free_moves: false,
            also_from: None,
            lock_until_base: Vec::new(),
        }
    }
}

/// Heuristic weights and search tuning. They change how fast the solver finds a win (or how
/// soon it gives up), never whether a position is judged winnable. Every field has a neutral
/// default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, default)]
#[non_exhaustive]
pub struct SearchHints {
    /// Cost of each card still to go home, or each step still missing from the win (default 4).
    pub progress_weight: u32,
    /// Cost of each face-down card (default 2).
    pub down_weight: u32,
    /// Cost of each card out of sequence in a building pile, and of each such pile in use
    /// (default 1).
    pub disorder_weight: u32,
    /// Cost of a card resting on a card its foundation needs sooner (default 1).
    pub blocker_weight: u32,
    /// Cost of each card in a free cell (default 1).
    pub cell_weight: u32,
    /// Cost of each card still in a stock that deals to the tableau (default 1).
    pub stock_weight: u32,
    /// Percent of the budget the default strategy spends on its fast, pruned best-first phase
    /// before the sound depth-first search (default 50).
    pub fast_share: u32,
}

impl Default for SearchHints {
    fn default() -> Self {
        SearchHints {
            progress_weight: 4,
            down_weight: 2,
            disorder_weight: 1,
            blocker_weight: 1,
            cell_weight: 1,
            stock_weight: 1,
            fast_share: 50,
        }
    }
}

impl Rules {
    /// Rules with a name, pack, groups and deal; everything else at its default (no stock
    /// rules, redeal, removal or score; won with every card on a foundation). Set the other
    /// fields directly.
    pub fn new(name: &str, pack: Pack, groups: Vec<GroupRules>, deal: Vec<DealStep>) -> Rules {
        Rules {
            _schema: IgnoredSchema,
            name: name.to_string(),
            pack,
            groups,
            stock: None,
            redeal: None,
            deal,
            removal: Vec::new(),
            score: None,
            win: WinRule::AllOnFoundations,
            auto_flip: true,
            safe_autoplay: true,
            search: SearchHints::default(),
            choose_base: None,
            deck_first: Vec::new(),
            separate_decks: false,
            deck_last: Vec::new(),
        }
    }
}

impl SearchHints {
    pub fn is_default(&self) -> bool {
        *self == SearchHints::default()
    }
}

fn yes() -> bool {
    true
}

fn one() -> u16 {
    1
}

/// The cards in play.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Pack {
    /// Copies of each (suit, rank) card. Two-pack games use 2; one-suit Spider uses 8.
    #[serde(default = "one_u8", skip_serializing_if = "is_one_u8")]
    pub copies: u8,
    /// Suits in use. Default: all four.
    #[serde(default = "all_suits", skip_serializing_if = "is_all_suits")]
    pub suits: Vec<Suit>,
    /// Ranks in use, in sequence order: building goes from each rank to the next one in this
    /// list. Default `[1..=13]` (ace low). Aces high is `[2, 3, …, 13, 1]`; a stripped pack
    /// leaves ranks out.
    #[serde(default = "all_ranks", skip_serializing_if = "is_all_ranks")]
    pub ranks: Vec<u8>,
    /// Jokers in play, in all (not per copy). Jokers are wild: see [`GroupRules::wild`],
    /// [`RemovalKind`] and [`Hands`] for what they stand for where.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub jokers: u8,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

fn is_one_u8(n: &u8) -> bool {
    *n == 1
}

fn is_one_u16(n: &u16) -> bool {
    *n == 1
}

fn is_default_pack(p: &Pack) -> bool {
    *p == Pack::default()
}

fn is_default_win(w: &WinRule) -> bool {
    *w == WinRule::AllOnFoundations
}

fn is_all_suits(s: &[Suit]) -> bool {
    s == Suit::ALL
}

fn is_all_ranks(r: &[u8]) -> bool {
    r.len() == 13 && r.iter().enumerate().all(|(i, &x)| x == (i + 1) as u8)
}

fn one_u8() -> u8 {
    1
}

fn all_suits() -> Vec<Suit> {
    Suit::ALL.to_vec()
}

fn all_ranks() -> Vec<u8> {
    (1..=13).collect()
}

impl Default for Pack {
    fn default() -> Self {
        Pack {
            copies: 1,
            suits: all_suits(),
            ranks: all_ranks(),
            jokers: 0,
        }
    }
}

impl Pack {
    /// One copy of each card of these suits and ranks (ranks in sequence order), no jokers.
    pub fn new(suits: &[Suit], ranks: &[u8]) -> Pack {
        Pack {
            copies: 1,
            suits: suits.to_vec(),
            ranks: ranks.to_vec(),
            jokers: 0,
        }
    }

    /// `n` copies of every card.
    pub fn with_copies(mut self, n: u8) -> Pack {
        self.copies = n;
        self
    }

    pub fn decks(n: u8) -> Pack {
        Pack {
            copies: n,
            ..Pack::default()
        }
    }

    /// Cards in the pack, jokers included.
    pub fn len(&self) -> usize {
        self.copies as usize * self.suits.len() * self.ranks.len() + self.jokers as usize
    }

    /// Add `n` jokers.
    pub fn with_jokers(mut self, n: u8) -> Pack {
        self.jokers = n;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What a group of piles is for. The kind sets defaults and tells the solver how to treat it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PileKind {
    /// Main playing piles.
    Tableau,
    /// Where cards are built to win (or discarded to, in removal games).
    Foundation,
    /// Free cells: hold one card each by default.
    Cell,
    /// Cards that may be played from but not onto.
    Reserve,
    /// The face-down stock. At most one pile.
    Stock,
    /// Where the stock deals to. At most one pile.
    Waste,
}

/// One group of identical piles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct GroupRules {
    /// Name used in deal steps, solutions and states, e.g. "tableau".
    pub name: String,
    pub kind: PileKind,
    #[serde(default = "one", skip_serializing_if = "is_one_u16")]
    pub count: u16,
    /// What may be placed on a pile's top card. Default: foundations build up in suit; every
    /// other kind accepts nothing on a non-empty pile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<Build>,
    /// What an empty pile accepts. Default: foundations take the pack's first rank; tableaux and
    /// cells take any card; reserves, stock and waste take none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<EmptyRule>,
    /// Which cards may leave a pile. Default: one card for tableaux, cells, reserves and the
    /// waste; none for foundations and the stock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves: Option<MoveUnit>,
    /// Most cards the pile may hold. Default: 1 for cells, unlimited otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cards: Option<u16>,
    /// Fewest cards that must remain on the pile (cards at or below this depth cannot leave).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cards: Option<u16>,
    /// Most cards the pile accepts in one move. Default: 1 for foundations (a whole suit for
    /// `complete_runs`), unlimited otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_accept: Option<u16>,
    /// Only cards coming from these groups are accepted (default: any group).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepts_from: Option<Vec<String>>,
    /// Only cards coming from these groups are accepted onto an empty pile (default: same as
    /// `accepts_from`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_accepts_from: Option<Vec<String>>,
    /// When a pile in this group empties, refill it automatically with the top card of the
    /// first non-empty pile of these groups, tried in order (Canfield's tableau from its
    /// reserve; many games from the waste, then the stock). Stock cards are turned face up.
    /// Groups to try in order; a group name or a list.
    #[serde(
        default,
        deserialize_with = "one_or_many",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub refill_from: Vec<String>,
    /// Cards moved per automatic refill from `refill_from` (default 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refill_count: Option<u16>,
    /// For piles that overlap (Pyramid, TriPeaks): which piles cover which.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// Foundation only accepts a whole suit sequence at once, moved together (Spider).
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete_runs: bool,
    /// A pile accepts cards only from piles of its own group exactly this many places after
    /// it (Accordion: 1 and 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<Vec<u16>>,
    /// With `reach`: also accept from piles this many places before the destination pile.
    #[serde(default, skip_serializing_if = "is_false")]
    pub reach_both_ways: bool,
    /// With `reach` on a grid layout: a distance of one place only counts within a row (so a pile
    /// does not reach round the end of a row). With distances of one and the row length the
    /// piles reached are the four neighbours (Rectangular Rovers).
    #[serde(default, skip_serializing_if = "is_false")]
    pub reach_grid: bool,
    /// Empty piles take cards from the waste only while every pile of this named group is empty
    /// (Dutchess: an empty row takes a waste card once the reserves are gone).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_needs_group_empty: Option<String>,
    /// Cards from piles of this same group are taken only as the whole pile (Dutchess).
    #[serde(default, skip_serializing_if = "is_false")]
    pub accept_whole_from_group: bool,
    /// A pile takes this many cards of one rank in a row, then goes on to the next rank of the
    /// pack, wrapping round (Four by Four). Use with `build: any/any` so nothing is ruled out
    /// before the pile itself is looked at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat_rank: Option<u16>,
    /// With a `covers` layout: a pile covered by one pile only may take that whole pile at once
    /// even while it is covered, if the cards build on it (Interlock's drop-down move).
    #[serde(default, skip_serializing_if = "is_false")]
    pub dropdown: bool,
    /// See [`FromPileMap`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_pile_map: Option<FromPileMap>,
    /// See [`ClearTops`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_tops: Option<ClearTops>,
    /// While no redeal has happened yet, these bans apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub first_round_bans: Vec<FirstRoundBan>,
    /// For a win by rows on a group whose `empty` rule is not `after_left`: the rank each row
    /// starts with (Rectangular Rovers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_start: Option<u8>,
    /// When a card is placed on a non-empty pile of this group, first move the destination pile's
    /// existing cards to this group's first pile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace_on_drop: Option<String>,
    /// Keep the group packed: whenever a pile empties, the piles after it slide forward.
    #[serde(default, skip_serializing_if = "is_false")]
    pub compact: bool,
    /// Piles accept only cards of this suit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suit: Option<Suit>,
    /// Piles accept no card of this suit, and nothing may be built on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude_suit: Option<Suit>,
    /// Empty piles accept only cards of this suit (non-empty piles follow `build` as usual).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_suit: Option<Suit>,
    /// Piles accept only cards of this colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// Different rules for particular piles of the group (Calculation's foundations, each
    /// building by a different step from a different base).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub piles: Vec<PileOverride>,
    /// Jokers are wild on these piles: a joker stands for any card that would be legal where
    /// it lies. It may be placed wherever some card could be (by the build rule, or the empty
    /// rule on an empty pile), and a card may be built on it if it could be built on some card
    /// the joker could stand for, given the cards beneath it. The joker's stand-in is never
    /// fixed: it is whatever keeps the pile legal. Where jokers aren't wild, a joker can only
    /// go to an empty pile that takes any card, and nothing is built on it. Default: wild
    /// everywhere but foundations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wild: Option<bool>,
    /// Each pile after the first only accepts a card whose rank is already present on the
    /// previous pile in the group (when empty, it must match the previous pile's bottom card).
    #[serde(default, skip_serializing_if = "is_false")]
    pub requires_rank_on_prev: bool,
    /// A pile with fewer cards than the longest pile in this group only accepts a card whose rank
    /// matches the card at the same index in the longest pile.
    #[serde(default, skip_serializing_if = "is_false")]
    pub match_longest_rank: bool,
    /// Most cards that may be placed onto an empty pile in one move. Default: same as `max_accept`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_max_accept: Option<u16>,
    /// Accept cards only when the destination pile is empty (even if `build` is set for moving runs).
    #[serde(default, skip_serializing_if = "is_false")]
    pub accept_only_when_empty: bool,
    /// Accept cards only when the stock pile is empty.
    #[serde(default, skip_serializing_if = "is_false")]
    pub accept_when_stock_empty: bool,
    /// Accept cards only when the waste pile is empty as well (with `accept_when_stock_empty`:
    /// Zodiac's foundations wait until the whole deal is played).
    #[serde(default, skip_serializing_if = "is_false")]
    pub accept_when_waste_empty: bool,
    /// When a pile in this group becomes empty, shift the movable face-up run from the next pile
    /// into it, leaving any face-down cards behind.
    #[serde(default, skip_serializing_if = "is_false")]
    pub compact_face_up: bool,
    /// When refilling from a multi-pile group in `refill_from`, take from the pile with the most
    /// cards (ties broken by lowest index) rather than the first non-empty pile.
    #[serde(default, skip_serializing_if = "is_false")]
    pub refill_largest: bool,
    /// Refill a pile in this group only when a move just emptied it, rather than also filling
    /// pre-existing empty piles when a refill source later receives a card.
    #[serde(default, skip_serializing_if = "is_false")]
    pub refill_on_empty: bool,
    /// Each pile after the first may only be started (while empty) once the previous pile in the
    /// group is non-empty.
    #[serde(default, skip_serializing_if = "is_false")]
    pub requires_prev_non_empty: bool,
    /// Most consecutive face-up cards of the same rank allowed at the top of a pile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_same_rank: Option<u8>,
    /// Cards may leave piles of this group only once the stock pile is empty.
    #[serde(default, skip_serializing_if = "is_false")]
    pub moves_when_stock_empty: bool,
    /// Cards may leave piles of this group only once at least one pile in the named group is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves_when_any_empty: Option<String>,
    /// Cards may leave pile `i` of this group only once pile `i` of the named group is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves_when_same_index_empty: Option<String>,
    /// Cards may be added to pile `i` of this group only once pile `i` of the named group is
    /// non-empty (the groups must have the same pile count).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_same_index_non_empty: Option<String>,
    /// Pile `i` of this group accepts cards only while pile `i` of the named group is empty,
    /// except cards coming from that very pile (the groups must have the same pile count).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept_when_same_index_empty: Option<String>,
    /// Cards may leave only the first or last non-empty pile of this group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_from_frontier: Option<Frontier>,
    /// With an `empty` rule of one rank: once this many piles of the group start with a card of
    /// that rank, empty piles accept any card (Scorpion Towers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_open_after_bases: Option<u16>,
    /// Empty piles take any card once the first pile of the named group is full (The Plot: once
    /// the first foundation holds a whole suit the empty rows are open).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_open_when_full: Option<String>,
    /// Empty piles of this group take nothing until the group's first pile is full (The Plot's
    /// foundations after the first).
    #[serde(default, skip_serializing_if = "is_false")]
    pub empty_needs_first_full: bool,
    /// Only automatically turn a pile's face-down top card face up when it is the sole card left
    /// in the pile.
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_only_last: bool,
    /// When a pile is one card shy of `max_cards`, its final card must come from the pile at the
    /// same index within this group (overriding `accepts_from`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_from_same_index: Option<String>,
    /// When `build.dir` is `either`, the first two-card pile in this group locks the build
    /// direction (up or down) for every pile in the group.
    #[serde(default, skip_serializing_if = "is_false")]
    pub lock_build_dir: bool,
    /// Never automatically turn face-down cards in this group face up (cards may still be drawn
    /// face up by `refill_from`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_flip: bool,
    /// When refilling an emptied pile from `refill_from`, take from the pile at the same index
    /// within the source group rather than the first non-empty pile.
    #[serde(default, skip_serializing_if = "is_false")]
    pub refill_same_index: bool,
    /// When refilling an emptied pile from `refill_from`, take from the pile at index `i % m`
    /// within the source group (where `m` is the source group's size), falling back to the nearest
    /// non-empty pile (`+1`, `-1`, `+2`, `-2`, ...).
    #[serde(default, skip_serializing_if = "is_false")]
    pub refill_nearest: bool,
    /// Only allow building onto a non-empty pile if its bottom card satisfies the pile's `empty`
    /// rule.
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_valid_base: bool,
    /// When set, moving from one pile of this group onto another may swap the two piles instead
    /// of building.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap_bases: Option<SwapBases>,
    /// With `moves: supermove`, also allow moving a complete suit-length built run regardless of
    /// empty cells and columns.
    #[serde(default, skip_serializing_if = "is_false")]
    pub supermove_complete_run: bool,
    /// Cards coming from these groups are accepted onto empty or non-empty piles without checking
    /// `build` or `empty` rules (capacity limits still apply).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unrestricted_from: Option<Vec<String>>,
    /// When `build.dir` is `either`, each pile locks its own build direction (up or down) once it
    /// holds two cards.
    #[serde(default, skip_serializing_if = "is_false")]
    pub lock_pile_dir: bool,
    /// A pile holding `max_cards` cards that all follow its build rule is closed: no card may be moved from it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub freeze_when_built: bool,
    /// A pile whose top card is face down takes any cards at all (Blind Patience).
    #[serde(default, skip_serializing_if = "is_false")]
    pub accept_on_face_down: bool,
    /// Piles of this group from this index on take nothing from the piles of this group before it
    /// (Club: the three top rows never give to the lower eight).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub one_way_split: Option<u16>,
    /// A face-down top card may be moved: it turns face up as it goes (Blind Patience, where the
    /// player turns a card over by hand and then plays it).
    #[serde(default, skip_serializing_if = "is_false")]
    pub move_face_down_top: bool,
    /// Fixed sequence of ranks required at each pile depth (index 0 for the bottom card, index 1
    /// for the second, etc.). Replaces the rank check of `empty` and `build.dir`; `build.by`,
    /// `suit`, `empty_suit`, `color`, and `max_cards` still apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rank_sequence: Vec<u8>,
    /// Maximum cards accepted in a single move onto a non-empty pile of this group.
    /// Defaults to `max_accept`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_empty_max_accept: Option<u16>,
    /// No two non-empty piles in this group may share the same suit on their bottom card.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unique_suits: bool,
    /// No two non-empty piles in this group may share the same rank on their bottom card
    /// (Elevens' reserves).
    #[serde(default, skip_serializing_if = "is_false")]
    pub unique_ranks: bool,
    /// The bottom cards of this group's piles must always have different ranks forming one run
    /// of consecutive ranks that may wrap round the pack (Triple Alliance's reserves).
    #[serde(default, skip_serializing_if = "is_false")]
    pub cyclic_ranks: bool,
    /// Every non-empty pile in this group must have the same suit on its bottom card as the
    /// others (Suit Elevens' reserves).
    #[serde(default, skip_serializing_if = "is_false")]
    pub same_suit_group: bool,
    /// Every non-empty pile in this group must have the same rank on its bottom card as the
    /// others (Tens' reserves).
    #[serde(default, skip_serializing_if = "is_false")]
    pub same_rank_group: bool,
    /// As soon as every pile of this group holds a card, move all their cards to the first pile
    /// of the named group (Elevens' reserves going to the discard).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_when_full: Option<String>,
    /// With `clear_when_full`: clear only when every pile's bottom card has the same rank
    /// (Foursome's four reserves).
    #[serde(default, skip_serializing_if = "is_false")]
    pub clear_same_rank: bool,
    /// Whenever the top card of a pile of this group is face up and of this rank (1 = ace), play
    /// it to the first foundation that takes it, whatever that foundation's other conditions
    /// (Strategy +: aces leave the talon by themselves). Rank 0 means any rank (Q.C.: every waste
    /// card that fits a foundation goes there).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_foundation_rank: Option<u8>,
    /// A face-down top card is turned up only once the stock and the waste are both empty
    /// (Last Chance's reserve).
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_when_dealt_out: bool,
    /// With `compact`: close gaps only once the stock is empty; until then they are refilled
    /// from it (Quatorze).
    #[serde(default, skip_serializing_if = "is_false")]
    pub compact_when_stock_empty: bool,
    /// With `complete_runs`: how many cards make a complete run (default: the number of ranks).
    /// Simplex: four of a kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_len: Option<u8>,
    /// Refill emptied piles only while every pile of the named group is empty (Elevens Too:
    /// rows are refilled only when the reserves are empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refill_when_empty: Option<String>,
    /// While the stock is non-empty, forbid building onto a single-card pile in this group and
    /// forbid moving a card from a single-card pile in this group onto another non-empty pile of
    /// this group.
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_build_single_while_stock: bool,
    /// When refilling an emptied pile from the stock with `refill_count > 1`, deal the first card
    /// face down unless it is the last card in the stock, and the remaining cards face up.
    #[serde(default, skip_serializing_if = "is_false")]
    pub refill_first_face_down: bool,
    /// Auto-flip the top card of pile `i` in this group once pile `i` of the named group reaches
    /// its `max_cards` capacity (overrides `no_flip`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_when_same_index_full: Option<String>,
    /// A face-down top card turns up only once the previous pile of this group is empty (the
    /// first pile always qualifies).
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_after_prev_empty: bool,
    /// Only turn a face-down top card up once every pile of this group holds at most this many
    /// cards (Striptease: the turned card waits until the reserves are down to two).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_when_all_at_most: Option<u16>,
    /// A face-down top card turns up only while some pile of the named group is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flip_when_any_empty: Option<String>,
}

/// When two piles in the same group may swap their contents on a move from one to the other.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SwapBases {
    /// Swap two single-card piles when each card satisfies the other pile's `empty` rule.
    BothSingle,
    /// Swap two non-empty piles with different `empty` rules when their bottom cards differ in
    /// rank and at least one bottom card satisfies the other pile's `empty` rule.
    EitherPile,
}

/// Which pile of a group is the only one whose cards may move.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Frontier {
    /// The last non-empty pile of the group.
    Last,
    /// The first non-empty pile of the group.
    First,
}

/// When every pile of the group holds a card and the top cards' ranks (ace 1 ... king 13) add
/// up to `sum`, each top card goes to the `to` group (Ninety-One).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ClearTops {
    pub to: String,
    pub sum: u16,
}

impl ClearTops {
    pub fn new(to: &str, sum: u16) -> ClearTops {
        ClearTops {
            to: to.to_string(),
            sum,
        }
    }
}

/// Which pile of this group a pile of `group` may send cards to (Rittenhouse's foundations).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct FromPileMap {
    /// Group the cards come from.
    pub group: String,
    /// For each pile of that group, the only pile of this group it may send cards to, or -1 for
    /// any.
    pub to: Vec<i16>,
}

impl FromPileMap {
    pub fn new(group: &str, to: &[i16]) -> FromPileMap {
        FromPileMap {
            group: group.to_string(),
            to: to.to_vec(),
        }
    }
}

/// Cards from some piles of a group may not go to some piles of this group until the stock or
/// redeal has been used once (St. Helena's foundations).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct FirstRoundBan {
    /// Piles (indices within the group that holds this ban) that refuse the cards.
    pub to: Vec<u16>,
    /// Group the refused cards come from.
    pub from_group: String,
    /// Piles (indices within `from_group`) whose cards are refused.
    pub from: Vec<u16>,
}

/// Rules for some piles of a group that differ from the group's. Fields left out keep the
/// group's value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PileOverride {
    /// Indices within the group.
    pub piles: Vec<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<Build>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<EmptyRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moves: Option<MoveUnit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cards: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cards: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_accept: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_max_accept: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_empty_max_accept: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suit: Option<Suit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty_suit: Option<Suit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

impl PileOverride {
    pub fn on(piles: &[u16]) -> PileOverride {
        PileOverride {
            piles: piles.to_vec(),
            ..PileOverride::default()
        }
    }

    pub fn build(mut self, b: Build) -> Self {
        self.build = Some(b);
        self
    }

    pub fn empty(mut self, e: EmptyRule) -> Self {
        self.empty = Some(e);
        self
    }

    pub fn suit(mut self, s: Suit) -> Self {
        self.suit = Some(s);
        self
    }

    pub fn empty_suit(mut self, s: Suit) -> Self {
        self.empty_suit = Some(s);
        self
    }
}

/// Accept `"x"` or `["x", "y"]` (or one object or a list of them).
fn one_or_many<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany<T> {
        One(T),
        Many(Vec<T>),
    }
    Ok(match OneOrMany::<T>::deserialize(d)? {
        OneOrMany::One(t) => vec![t],
        OneOrMany::Many(v) => v,
    })
}

impl GroupRules {
    /// A group with every optional field at its kind's default.
    pub fn new(name: &str, kind: PileKind, count: u16) -> GroupRules {
        GroupRules {
            name: name.to_string(),
            kind,
            count,
            build: None,
            empty: None,
            moves: None,
            max_cards: None,
            min_cards: None,
            max_accept: None,
            accepts_from: None,
            empty_accepts_from: None,
            refill_from: Vec::new(),
            refill_count: None,
            reach: None,
            reach_both_ways: false,
            reach_grid: false,
            empty_needs_group_empty: None,
            accept_whole_from_group: false,
            repeat_rank: None,
            dropdown: false,
            from_pile_map: None,
            clear_tops: None,
            first_round_bans: Vec::new(),
            row_start: None,
            replace_on_drop: None,
            compact: false,
            suit: None,
            empty_suit: None,
            exclude_suit: None,
            color: None,
            piles: Vec::new(),
            layout: None,
            complete_runs: false,
            wild: None,
            requires_rank_on_prev: false,
            match_longest_rank: false,
            empty_max_accept: None,
            accept_only_when_empty: false,
            accept_when_stock_empty: false,
            accept_when_waste_empty: false,
            compact_face_up: false,
            refill_largest: false,
            refill_on_empty: false,
            requires_prev_non_empty: false,
            max_same_rank: None,
            moves_when_stock_empty: false,
            moves_when_any_empty: None,
            moves_when_same_index_empty: None,
            requires_same_index_non_empty: None,
            accept_when_same_index_empty: None,
            move_from_frontier: None,
            empty_open_after_bases: None,
            empty_open_when_full: None,
            empty_needs_first_full: false,
            flip_only_last: false,
            final_from_same_index: None,
            lock_build_dir: false,
            no_flip: false,
            refill_same_index: false,
            refill_nearest: false,
            require_valid_base: false,
            swap_bases: None,
            supermove_complete_run: false,
            unrestricted_from: None,
            lock_pile_dir: false,
            freeze_when_built: false,
            accept_on_face_down: false,
            one_way_split: None,
            move_face_down_top: false,
            rank_sequence: Vec::new(),
            non_empty_max_accept: None,
            unique_suits: false,
            unique_ranks: false,
            cyclic_ranks: false,
            same_suit_group: false,
            same_rank_group: false,
            clear_when_full: None,
            clear_same_rank: false,
            run_len: None,
            compact_when_stock_empty: false,
            flip_when_dealt_out: false,
            auto_foundation_rank: None,
            refill_when_empty: None,
            no_build_single_while_stock: false,
            refill_first_face_down: false,
            flip_when_same_index_full: None,
            flip_after_prev_empty: false,
            flip_when_all_at_most: None,
            flip_when_any_empty: None,
        }
    }

    pub fn build(mut self, by: BuildBy, dir: Dir) -> Self {
        self.build = Some(Build {
            by,
            dir,
            ..Build::default()
        });
        self
    }

    pub fn build_rule(mut self, b: Build) -> Self {
        self.build = Some(b);
        self
    }

    pub fn empty(mut self, e: EmptyRule) -> Self {
        self.empty = Some(e);
        self
    }

    pub fn moves(mut self, m: MoveUnit) -> Self {
        self.moves = Some(m);
        self
    }

    pub fn max_cards(mut self, n: u16) -> Self {
        self.max_cards = Some(n);
        self
    }

    pub fn min_cards(mut self, n: u16) -> Self {
        self.min_cards = Some(n);
        self
    }

    pub fn max_accept(mut self, n: u16) -> Self {
        self.max_accept = Some(n);
        self
    }

    pub fn accepts_from(mut self, groups: &[&str]) -> Self {
        self.accepts_from = Some(groups.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn empty_accepts_from(mut self, groups: &[&str]) -> Self {
        self.empty_accepts_from = Some(groups.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn refill_from(mut self, groups: &[&str]) -> Self {
        self.refill_from = groups.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn refill_count(mut self, n: u16) -> Self {
        self.refill_count = Some(n);
        self
    }

    pub fn suit(mut self, s: Suit) -> Self {
        self.suit = Some(s);
        self
    }

    pub fn empty_suit(mut self, s: Suit) -> Self {
        self.empty_suit = Some(s);
        self
    }

    /// Override rules for some piles of the group.
    pub fn pile(mut self, o: PileOverride) -> Self {
        self.piles.push(o);
        self
    }

    pub fn layout(mut self, l: Layout) -> Self {
        self.layout = Some(l);
        self
    }

    pub fn complete_runs(mut self) -> Self {
        self.complete_runs = true;
        self
    }

    pub fn wild(mut self, wild: bool) -> Self {
        self.wild = Some(wild);
        self
    }

    pub fn requires_rank_on_prev(mut self) -> Self {
        self.requires_rank_on_prev = true;
        self
    }

    pub fn match_longest_rank(mut self) -> Self {
        self.match_longest_rank = true;
        self
    }

    pub fn empty_max_accept(mut self, n: u16) -> Self {
        self.empty_max_accept = Some(n);
        self
    }

    pub fn accept_only_when_empty(mut self) -> Self {
        self.accept_only_when_empty = true;
        self
    }

    pub fn accept_when_stock_empty(mut self) -> Self {
        self.accept_when_stock_empty = true;
        self
    }

    pub fn compact_face_up(mut self) -> Self {
        self.compact_face_up = true;
        self
    }

    pub fn refill_largest(mut self) -> Self {
        self.refill_largest = true;
        self
    }

    pub fn refill_on_empty(mut self) -> Self {
        self.refill_on_empty = true;
        self
    }

    pub fn requires_prev_non_empty(mut self) -> Self {
        self.requires_prev_non_empty = true;
        self
    }

    pub fn max_same_rank(mut self, n: u8) -> Self {
        self.max_same_rank = Some(n);
        self
    }

    pub fn moves_when_stock_empty(mut self) -> Self {
        self.moves_when_stock_empty = true;
        self
    }

    pub fn moves_when_any_empty(mut self, group: &str) -> Self {
        self.moves_when_any_empty = Some(group.to_string());
        self
    }

    pub fn accept_when_same_index_empty(mut self, group: &str) -> Self {
        self.accept_when_same_index_empty = Some(group.to_string());
        self
    }

    pub fn requires_same_index_non_empty(mut self, group: &str) -> Self {
        self.requires_same_index_non_empty = Some(group.to_string());
        self
    }

    pub fn moves_when_same_index_empty(mut self, group: &str) -> Self {
        self.moves_when_same_index_empty = Some(group.to_string());
        self
    }

    pub fn flip_only_last(mut self) -> Self {
        self.flip_only_last = true;
        self
    }

    pub fn final_from_same_index(mut self, group: &str) -> Self {
        self.final_from_same_index = Some(group.to_string());
        self
    }

    pub fn lock_build_dir(mut self) -> Self {
        self.lock_build_dir = true;
        self
    }

    pub fn no_flip(mut self) -> Self {
        self.no_flip = true;
        self
    }

    pub fn refill_same_index(mut self) -> Self {
        self.refill_same_index = true;
        self
    }

    pub fn refill_nearest(mut self) -> Self {
        self.refill_nearest = true;
        self
    }

    pub fn require_valid_base(mut self) -> Self {
        self.require_valid_base = true;
        self
    }

    pub fn swap_bases(mut self, mode: SwapBases) -> Self {
        self.swap_bases = Some(mode);
        self
    }

    pub fn supermove_complete_run(mut self) -> Self {
        self.supermove_complete_run = true;
        self
    }

    pub fn unrestricted_from(mut self, groups: &[&str]) -> Self {
        self.unrestricted_from = Some(groups.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn lock_pile_dir(mut self) -> Self {
        self.lock_pile_dir = true;
        self
    }

    pub fn freeze_when_built(mut self) -> Self {
        self.freeze_when_built = true;
        self
    }

    pub fn rank_sequence(mut self, ranks: &[u8]) -> Self {
        self.rank_sequence = ranks.to_vec();
        self
    }
}

/// How cards are built on one another.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Build {
    pub by: BuildBy,
    #[serde(default)]
    pub dir: Dir,
    /// The sequence wraps: king to ace and ace to king.
    #[serde(default, skip_serializing_if = "is_false")]
    pub wrap: bool,
    /// Ranks between consecutive cards (default 1).
    #[serde(default = "one_u8", skip_serializing_if = "is_one_u8")]
    pub step: u8,
    /// Nothing may be built on cards of these ranks (Golf: kings).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_ranks: Vec<u8>,
    /// With a stripped pack: ranks that are not adjacent to the pack rank just below them
    /// (an ace-and-7-to-king pack: `[7]`). Applies to `step` 1 builds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gap_ranks: Vec<u8>,
    /// Pairs of ranks that also build on each other, in either order, whatever the direction
    /// (Striptease: jack and king).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_pairs: Vec<(u8, u8)>,
    /// `(under, over)` rank pairs that also build, in that order only, whatever the direction
    /// (Napoleon's Tomb: a six goes on an ace).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_over: Vec<(u8, u8)>,
    /// With `by: suit_cycle`: the suits in the order they follow each other.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suit_cycle: Vec<Suit>,
}

impl Build {
    /// Build `by` a suit or colour relation in direction `dir`, one rank at a time.
    pub fn new(by: BuildBy, dir: Dir) -> Build {
        Build {
            by,
            dir,
            ..Build::default()
        }
    }

    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    pub fn step(mut self, step: u8) -> Self {
        self.step = step;
        self
    }

    pub fn blocked_ranks(mut self, ranks: &[u8]) -> Self {
        self.blocked_ranks = ranks.to_vec();
        self
    }

    pub fn suit_cycle(mut self, suits: &[Suit]) -> Self {
        self.suit_cycle = suits.to_vec();
        self
    }

    pub fn gap_ranks(mut self, ranks: &[u8]) -> Self {
        self.gap_ranks = ranks.to_vec();
        self
    }
}

impl Default for Build {
    fn default() -> Self {
        Build {
            by: BuildBy::None,
            dir: Dir::Down,
            wrap: false,
            step: 1,
            blocked_ranks: Vec::new(),
            gap_ranks: Vec::new(),
            extra_pairs: Vec::new(),
            extra_over: Vec::new(),
            suit_cycle: Vec::new(),
        }
    }
}

/// The suit or colour relation between a card and the card placed on it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BuildBy {
    /// Nothing may be built.
    None,
    /// Any suit.
    Any,
    /// Same suit.
    Suit,
    /// Alternate colours.
    Alternate,
    /// Same colour.
    SameColor,
    /// Any suit but the same one.
    OtherSuit,
    /// The same suit or the same rank (Accordion; pair with `dir: any`).
    SuitOrRank,
    /// Same rank of any suit, or same suit following `dir`.
    SuitOrSameRank,
    /// The suit that follows the one below in `Build::suit_cycle` (Following: spades, hearts,
    /// clubs, diamonds, and round again).
    SuitCycle,
    /// A card goes on another of the same suit that is one rank below it, on one of the same
    /// colour and another suit that is one rank above it, or on one of the same rank and the
    /// other colour; ranks wrap round the pack (Sticko). `dir` is ignored.
    RankSuitColor,
}

/// Rank direction from a card to the card placed on it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Dir {
    #[default]
    Down,
    Up,
    /// Up or down.
    Either,
    /// Same rank.
    Same,
    /// Any rank at all (a pile that takes anything, subject to `by`).
    Any,
    /// Any strictly higher rank in the pack's rank sequence (no wrap).
    UpAny,
    /// Any strictly lower rank in the pack's rank sequence (no wrap).
    DownAny,
    /// Same rank or one step down.
    DownOrSame,
    /// The card's position in the pack's ranks is double the one below it, plus one, modulo the
    /// number of ranks (Doublets: A 2 4 8 3 6 Q J 9 5 T 7 A).
    DoubleUp,
    /// The card's position in the pack's ranks mirrors the one below it: the first on the last,
    /// the second on the second last, and so on (Eiffel Tower: A on K, 2 on Q, 7 on 7).
    Mirror,
}

/// What an empty pile accepts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EmptyRule {
    /// Nothing.
    None,
    /// Any card (or any movable unit).
    Any,
    /// Only a card of this rank (13 for "kings only").
    Rank(u8),
    /// Only a card whose rank is in this list (e.g. `[1, 13]` for Ace or King).
    Ranks(Vec<u8>),
    /// Only a card of the rank turned up by the deal (Canfield). See [`DealStep::set_base`].
    DealtBase,
    /// Only the card this many places after the rank turned up by the deal, going round the
    /// pack's ranks: `-1` is the rank below the base (Penguin's columns, which take the card
    /// that would sit under the foundations' base). See [`DealStep::set_base`].
    DealtBaseOffset(i8),
    /// In a grid: a card that could be built (by this pile's build rule) on the top card of
    /// the pile to the left; in the first column, a card of rank `first` (Montana's gaps).
    AfterLeft { first: u8 },
    /// In a grid: like [`EmptyRule::AfterLeft`], or a card that the top card of the pile to the
    /// right (if any) could be built on.
    AfterNeighbor { first: u8 },
    /// A card whose rank is `offset` places after the top card of the pile at the same index in
    /// `group`, wrapping around the pack's rank sequence.
    AfterPile { group: String, offset: i8 },
    /// Any card if every pile in this group is empty; otherwise only a card matching the bottom
    /// card's rank of the first non-empty pile in this group.
    SameBase,
}

/// Which cards may leave a pile together.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MoveUnit {
    /// No card may leave.
    None,
    /// The top card only.
    Top,
    /// Any built sequence (by this group's build rule) from the top.
    Run,
    /// Any built sequence from the top whose cards are all one suit (Spider).
    SuitRun,
    /// Any number of face-up cards from the top, built or not (Yukon).
    AnyFaceUp,
    /// A built sequence, as long as free cells and empty piles would allow moving it one card at
    /// a time (FreeCell's "supermove"): (empty cells + 1) × 2^(empty piles).
    Supermove,
    /// Only the whole pile, all face up.
    Whole,
    /// Either the top card alone, or the whole pile when all face up and built in sequence.
    TopOrWhole,
    /// Any built sequence from the top whose cards alternate in colour.
    AltColorRun,
    /// Any built sequence from the top whose cards are all one colour.
    SameColorRun,
    /// Any built same-suit sequence or any equal-rank run from the top.
    SuitRunOrSameRank,
    /// Any run of one suit whose ranks are consecutive, from the top (for piles that build more
    /// loosely than they move).
    SuitSequence,
}

/// Overlapping piles: a pile is playable only once every pile covering it is empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Layout {
    /// A triangle with `rows` rows (row r has r + 1 piles); each pile is covered by the two
    /// below it. Group count must be rows × (rows + 1) / 2.
    Pyramid { rows: u16 },
    /// Three peaks of 3 rows each over a base row of 10 (28 piles).
    TriPeaks,
    /// Explicit: `covered_by[i]` lists the piles (indices within the group) covering pile i.
    Covers { covered_by: Vec<Vec<u16>> },
    /// Rows of `columns` piles, read left to right (gap games: Montana). Nothing covers
    /// anything; the layout gives each pile a left neighbour.
    Grid { columns: u16 },
}

/// How the stock deals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct StockRules {
    /// Group the stock deals to. If it is a waste or foundation, `count` cards go onto its first
    /// pile. Otherwise (tableau, reserve, cell), `count` rounds of one card go to each pile in
    /// turn while cards remain.
    pub to: String,
    /// Cards per deal (or rounds per deal for multi-pile targets).
    #[serde(default = "one", skip_serializing_if = "is_one_u16")]
    pub count: u16,
    /// Times the waste may be turned back into the stock.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub redeals: u32,
    /// The waste may be turned back any number of times (overrides `redeals`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub unlimited_redeals: bool,
    /// The stock may not deal while any pile of the target group is empty (Spider).
    #[serde(default, skip_serializing_if = "is_false")]
    pub require_no_empty: bool,
    /// Deal by itself whenever the waste is empty (and the stock isn't).
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto: bool,
    /// For a multi-pile target: deal `count` cards into the first empty pile instead of one to
    /// every pile (Accordion).
    #[serde(default, skip_serializing_if = "is_false")]
    pub first_empty: bool,
    /// Deal only when the target pile is empty: each card must be played before the next is
    /// turned (Calculation, Sir Tommy).
    #[serde(default, skip_serializing_if = "is_false")]
    pub only_when_empty: bool,
    /// For a multi-pile target: deal only to non-empty piles (or to the first pile if all are
    /// empty).
    #[serde(default, skip_serializing_if = "is_false")]
    pub skip_empty: bool,
    /// Keep the top card of the stock turned face up so it may be played or removed directly.
    #[serde(default, skip_serializing_if = "is_false")]
    pub face_up: bool,
    /// Cards per deal on each round (0-indexed by redeals used). When non-empty, overrides `count`;
    /// rounds past the end reuse the last entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub count_per_round: Vec<u16>,
    /// For a multi-pile target: when the remaining stock has at most as many cards as this group
    /// has piles, deal one card to each pile of this group instead of `to`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_to: Option<String>,
    /// Specific piles of `to` to deal to (indices within the group). Default: all piles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piles: Option<Vec<u16>>,
    /// For a multi-pile target: when the remaining stock has at most `final_piles.len()` cards,
    /// deal one card to each of these piles of `to` (indices within the group) instead of `piles`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_piles: Option<Vec<u16>>,
    /// For a multi-pile target with `count > 1`: deal `count` cards to each pile in turn rather
    /// than `count` rounds of one card per pile.
    #[serde(default, skip_serializing_if = "is_false")]
    pub deal_per_pile: bool,
    /// When gathering a multi-pile target back into the stock on redeal, gather piles in reverse
    /// order.
    #[serde(default, skip_serializing_if = "is_false")]
    pub redeal_reverse: bool,
    /// When gathering a multi-pile target back into the stock on redeal, turn the gathered
    /// stock upside down, so the first card gathered is the first one dealt again.
    #[serde(default, skip_serializing_if = "is_false")]
    pub redeal_flip_stock: bool,
    /// On redeal only gather the target group back into the stock (nothing is dealt); the stock
    /// is played from its top (Puss in the Corner).
    #[serde(default, skip_serializing_if = "is_false")]
    pub redeal_gather_only: bool,
    /// When gathering a multi-pile target back into the stock on redeal, shuffle the gathered
    /// cards before dealing them out again.
    #[serde(default, skip_serializing_if = "is_false")]
    pub redeal_shuffle: bool,
    /// On a multi-pile stock redeal, deal all gathered cards back out in rows alternating face-up
    /// and face-down (ending face-up), honoring `divert_ranks`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub redeal_deal_all_alternating: bool,
    /// For a multi-pile target: if any target piles are empty, deal only to the empty piles;
    /// otherwise deal to every target pile.
    #[serde(default, skip_serializing_if = "is_false")]
    pub deal_empty_first: bool,
    /// When dealing to a multi-pile target, divert cards of these ranks to the first accepting
    /// foundation and keep dealing until a non-diverted card lands on the current target pile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub divert_ranks: Vec<u8>,
    /// Target pile indices within `to` for each round (0-indexed by redeals used). When non-empty,
    /// overrides `piles`; rounds past the end reuse the last entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub piles_per_round: Vec<Vec<u16>>,
    /// When set, dealing requires at least 2 cards in the stock. If dealing leaves (or starts
    /// with) exactly 2 cards in the stock, one card is dealt face up to this group's first pile
    /// and the last card remaining in the stock is turned face up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_split_to: Option<String>,
    /// When exactly as many cards are left as the target and this group have piles together,
    /// the deal also puts one card on each pile of this group (Baroness's last seven cards).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_also_to: Option<String>,
    /// The stock may not deal while any pile of this named group is empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_no_empty_group: Option<String>,
    /// The stock may deal only while every pile of this named group is empty (The Lucky Number's
    /// reserves).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_group_empty: Option<String>,
    /// With `auto`: deal to the empty waste by itself only right after a move empties a pile of
    /// this named group, once, and let that pile (with a `refill_on_empty` refill from the waste)
    /// take the card if it stays (Q.C.: a card that fits a foundation goes there, and the row
    /// stays empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_for_empty: Option<String>,
    /// A deal passes over every target pile that is a whole built run from this rank (1 = ace)
    /// down to its last card, face up (Churchill: finished king piles). Nothing deals when
    /// every pile is passed over.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_built_from: Option<u8>,
    /// The most times the stock may deal (a limit on draws, not on turning a waste back).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_deals: Option<u32>,
    /// A deal passes over every target pile whose top built run (face up, as far as the pile's
    /// build rule goes) includes a card of this rank (Fechter's Game: a king run on top).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_run_with_rank: Option<u8>,
}

impl StockRules {
    /// Deal one card at a time to `group`, with no redeals and no other conditions.
    pub fn to(group: &str) -> StockRules {
        StockRules {
            to: group.to_string(),
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
            redeal_flip_stock: false,
            redeal_gather_only: false,
            redeal_shuffle: false,
            redeal_deal_all_alternating: false,
            deal_empty_first: false,
            divert_ranks: Vec::new(),
            piles_per_round: Vec::new(),
            final_split_to: None,
            final_also_to: None,
            require_no_empty_group: None,
            require_group_empty: None,
            auto_for_empty: None,
            skip_built_from: None,
            max_deals: None,
            skip_run_with_rank: None,
        }
    }
}

/// Gathering a group's cards and dealing them out again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct RedealRules {
    /// The group gathered and redealt.
    pub group: String,
    /// How many redeals are allowed.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub times: u32,
    /// Any number of redeals (overrides `times`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub unlimited: bool,
    pub how: RedealHow,
}

impl RedealRules {
    /// One redeal of `group`, dealt `how`. Set `times` or `unlimited` for more.
    pub fn new(group: &str, how: RedealHow) -> RedealRules {
        RedealRules {
            group: group.to_string(),
            times: 1,
            unlimited: false,
            how,
        }
    }
}

/// How a redeal deals the gathered cards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RedealHow {
    /// Keep the cards in order and deal them back `per_pile` at a time to each pile in turn,
    /// starting from the first pile's top cards (Cruel). Fewer piles may end up in use. If there
    /// are more cards than `per_pile` for every pile, the surplus is shared out: each pile gets
    /// an equal part and the first few one more.
    Regroup { per_pile: u16 },
    /// Shuffle the gathered cards, then deal `per_pile` to each pile in turn (La Belle Lucie),
    /// sharing out any surplus as for [`RedealHow::Regroup`].
    Shuffle { per_pile: u16 },
    /// As `shuffle`, but every card is dealt face down except the top card of a pile that gets at
    /// least two (Fascination Fan: the dealing goes round in three passes, the last face up).
    ShuffleTopUp { per_pile: u16 },
    /// Grid games: leave each row's built sequence in place, shuffle every other card, and
    /// deal them back leaving one gap straight after each sequence (Montana).
    GridShuffle,
    /// Close the gaps: slide the cards forward in reading order, then fill the empty places
    /// at the end from `refill_from`'s top card, turned face up (Monte Carlo).
    Compact {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refill_from: Option<String>,
    },
    /// Grid games: leave each row's built sequence in place, shuffle every other card, and deal
    /// them back leaving the gaps at random positions among the non-sequence slots.
    GridShuffleRandomGaps,
    /// Grid games: leave each row's built sequence in place, shuffle every other card, and deal
    /// them back leaving one gap at the last column of each row.
    GridShuffleEndGaps,
    /// Move each pile's bottom card to its top (Crescent).
    RotateBottom,
    /// Gather piles left-to-right (reversing each pile) and deal all cards back row-by-row.
    DealRows,
    /// Gather as `deal_rows`, then deal row by row where each row starts one pile further right
    /// than the last: the first row to every pile, the next to all but the first, and so on
    /// (Usk).
    DealStaircase,
    /// On redeal `k` (0-indexed), pick up pile `k` and deal its cards one-by-one across the group
    /// starting at pile `k` and wrapping around.
    RedistributePile,
    /// Gather all cards of the group, shuffle them, and replay the group's opening deal steps.
    ShuffleRedeal,
}

/// One step of the opening deal.
///
/// Cards come off the shuffled pack in order. With `count` or `counts`, they are dealt in rows:
/// one card to each selected pile, then the next row, as a person would deal them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct DealStep {
    /// Target group.
    pub to: String,
    /// Piles of the group to deal to (indices). Default: all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub piles: Option<Vec<u16>>,
    /// Cards to each selected pile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u16>,
    /// Cards to each selected pile, per pile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<Vec<u16>>,
    /// Deal every remaining card, round-robin over the selected piles.
    #[serde(default, skip_serializing_if = "is_false")]
    pub rest: bool,
    /// Take every card of these ranks out of the pack and put one on each selected pile in
    /// turn (Beleaguered Castle's aces).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ranks: Option<Vec<u8>>,
    /// With `ranks` (and no `count`): take at most this many matching cards in total instead of
    /// all of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
    /// Face up (default) or face down.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub face_up: bool,
    /// Cards of these ranks met while dealing this step are passed over and put at the bottom of
    /// the pack instead (Doublets' kings).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skip_ranks: Vec<u8>,
    /// After dealing, move each pile's kings to its bottom, keeping their order (Baker's Dozen).
    #[serde(default, skip_serializing_if = "is_false")]
    pub kings_to_bottom: bool,
    /// With `one_each`: each card taken must be of the colour opposite to the one before
    /// (Colours: a 2 and a 4 of one colour, a 3 and a 5 of the other).
    #[serde(default, skip_serializing_if = "is_false")]
    pub alternate_colors: bool,
    /// With `divert_ranks` or `auto_foundation`: a card sent to a foundation does not use up its
    /// turn; the same pile gets the next card instead (Dieppe: cards go to the foundations until
    /// one does not fit, and that one goes to the row).
    #[serde(default, skip_serializing_if = "is_false")]
    pub retry_after_divert: bool,
    /// With `ranks`: take only the first card of each listed rank, in the listed order
    /// (Calculation's A, 2, 3, 4).
    #[serde(default, skip_serializing_if = "is_false")]
    pub one_each: bool,
    /// After this step, take every card of these ranks off the table and put them on this
    /// step's group (Montana: deal all 52, then remove the aces to leave gaps).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collect: Option<Vec<u8>>,
    /// The first card this step deals sets the base rank for [`EmptyRule::DealtBase`].
    #[serde(default, skip_serializing_if = "is_false")]
    pub set_base: bool,
    /// Take every remaining card of the dealt base rank out of the pack and put one on each
    /// selected pile in turn (requires an earlier step with `set_base`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub of_base: bool,
    /// Pull one card of each suit (in pack suit order) from the deck per round (repeated `count`
    /// times, default 1), placing them on the selected piles in turn.
    #[serde(default, skip_serializing_if = "is_false")]
    pub one_per_suit: bool,
    /// With `rest`: deal sequentially into each selected pile, advancing to the next pile whenever
    /// a card of this rank is dealt to a non-empty pile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_on_rank: Option<u8>,
    /// During this step, place any dealt card directly on the first foundation that accepts it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto_foundation: bool,
    /// During this step, divert dealt cards of these ranks to the first foundation that accepts
    /// them; with `drain_foundations`, restrict draining to these ranks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divert_ranks: Option<Vec<u8>>,
    /// After each dealt row, repeatedly move exposed top cards of the selected piles onto any
    /// accepting foundation (restricted to `divert_ranks` if set).
    #[serde(default, skip_serializing_if = "is_false")]
    pub drain_foundations: bool,
    /// The deal may run out of cards part-way through a round (the last round is short).
    #[serde(default, skip_serializing_if = "is_false")]
    pub stop_when_out: bool,
    /// With `rest`: send each card to the selected pile its rank or suit names instead of dealing
    /// round-robin (Lady of the Manor's reserves by rank, Four Colours' rows by suit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<DealRoute>,
}

/// How a `rest` deal step picks each card's pile.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DealRoute {
    /// Pile `i` of the selection takes the cards of rank `first_rank + i` (ranks are 1 = ace).
    Rank { first_rank: u8 },
    /// Pile `i` of the selection takes the cards of the pack's `i`th suit.
    Suit,
    /// Each card of `rank` starts the next pile of group `to`; every other card goes on the
    /// selected pile with the index of the latest such start (Xerxes: the nines start the
    /// foundations and the cards after each one go on the matching row).
    Chain { rank: u8, to: String },
    /// Each card of `rank` starts the next pile of group `to`; every other card goes on the first
    /// selected pile, until `limit` of them are down (Frog: aces to the foundations, thirteen
    /// other cards to the reserve).
    Split { rank: u8, to: String },
}

impl DealStep {
    pub fn to(group: &str) -> DealStep {
        DealStep {
            to: group.to_string(),
            face_up: true,
            ..DealStep::default()
        }
    }

    pub fn each(mut self, n: u16) -> Self {
        self.count = Some(n);
        self
    }

    pub fn counts(mut self, c: &[u16]) -> Self {
        self.counts = Some(c.to_vec());
        self
    }

    pub fn rest(mut self) -> Self {
        self.rest = true;
        self
    }

    pub fn piles(mut self, p: &[u16]) -> Self {
        self.piles = Some(p.to_vec());
        self
    }

    pub fn ranks(mut self, r: &[u8]) -> Self {
        self.ranks = Some(r.to_vec());
        self
    }

    pub fn down(mut self) -> Self {
        self.face_up = false;
        self
    }

    pub fn skip_ranks(mut self, ranks: &[u8]) -> Self {
        self.skip_ranks = ranks.to_vec();
        self
    }

    pub fn kings_to_bottom(mut self) -> Self {
        self.kings_to_bottom = true;
        self
    }

    pub fn collect(mut self, ranks: &[u8]) -> Self {
        self.collect = Some(ranks.to_vec());
        self
    }

    pub fn one_each(mut self) -> Self {
        self.one_each = true;
        self
    }

    pub fn set_base(mut self) -> Self {
        self.set_base = true;
        self
    }
}

/// Card-removal mechanics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct RemovalRules {
    pub kind: RemovalKind,
    /// Groups whose playable top cards may be removed.
    pub from: Vec<String>,
    /// Group (its first pile) the removed cards go to.
    pub to: String,
    /// Cards must be neighbours in their grid (all from one grid group).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adjacent: Option<Adjacency>,
    /// Cards of these ranks never take part in a removal (Aces Square's aces).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_ranks: Vec<u8>,
}

impl RemovalRules {
    /// Remove cards of `kind` from the playable tops of `from` to `to`'s first pile.
    pub fn new(kind: RemovalKind, from: &[&str], to: &str) -> RemovalRules {
        RemovalRules {
            kind,
            from: from.iter().map(|s| s.to_string()).collect(),
            to: to.to_string(),
            adjacent: None,
            exclude_ranks: Vec::new(),
        }
    }
}

/// Which grid positions count as neighbours.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Adjacency {
    /// Horizontally, vertically or diagonally next to each other (Monte Carlo).
    Eight,
    /// Horizontally or vertically next to each other.
    Four,
    /// Anywhere in the same row or the same column (Quatorze).
    SameLine,
}

/// Jokers are wild in removals: a joker pairs with any card (another joker too) and stands for
/// any one rank of a set. `lower_of_suit` ignores them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RemovalKind {
    /// Remove two playable cards whose ranks add up to `total` (Pyramid: 13), or one card
    /// whose rank is listed in `alone` (Pyramid: kings).
    PairsSum {
        total: u8,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        alone: Vec<u8>,
    },
    /// Remove two playable cards of the same suit whose ranks add up to `total` (Suit Elevens).
    PairsSumSameSuit { total: u8 },
    /// Remove two playable cards whose ranks differ by one of `distances` (Cheops: 0 or 1,
    /// Neptune: 1), counting ace as 1 and king as 13 with no wrap.
    RankDistance { distances: Vec<u8> },
    /// Remove two playable cards of different suits whose ranks add up to `total` (Exit's
    /// queen and king).
    PairsSumOtherSuit { total: u8 },
    /// Remove a playable card when another playable card is of the same suit and higher in the
    /// pack's rank order (Aces Up).
    LowerOfSuit,
    /// Remove two playable cards of the same rank (Nestor, Simple Pairs).
    SameRank,
    /// Remove two playable cards of the same suit.
    SameSuit,
    /// Remove playable cards whose ranks are exactly this set, together (Elevens' J-Q-K).
    RankSet { ranks: Vec<u8> },
    /// Remove two playable cards in the same group whose pile indices differ by one of
    /// `distances`, when they match in suit or rank.
    ReachSuitOrRank { distances: Vec<u16> },
    /// Remove `1..=max_len` consecutive non-empty piles in a group when the piles immediately
    /// before and after the run match in suit or rank.
    BetweenMatch { max_len: u8 },
    /// Remove `1..=max_len` consecutive non-empty piles in a group when the sum of their ranks is
    /// a multiple of `divisor`.
    ContiguousSumMultiple { divisor: u8, max_len: u8 },
    /// Remove two playable cards of the same rank when that rank is listed in `ranks`.
    SameRankIn { ranks: Vec<u8> },
}

/// When the game is won.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WinRule {
    /// Every card is on a foundation.
    #[default]
    AllOnFoundations,
    /// These groups are empty (Pyramid, Golf, TriPeaks: the tableau).
    Empty(Vec<String>),
    /// A group holds at least this many cards (Aces Up: 48 discarded).
    Count { group: String, cards: u16 },
    /// A group holds at least this many cards and every group in `empty` is empty.
    CountAndEmpty {
        group: String,
        cards: u16,
        empty: Vec<String>,
    },
    /// A group holds at least `cards` cards and every pile of `spread` holds at most one card.
    CountAndSpread {
        group: String,
        cards: u16,
        spread: String,
    },
    /// At most this many piles of a group hold cards (Accordion: 1).
    AtMostPiles { group: String, piles: u16 },
    /// Every row of a grid group holds a built sequence from its first column, `length` cards
    /// long (default: one less than the row), starting with the rank its first column takes
    /// (Montana).
    Rows {
        group: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<u16>,
    },
    /// The position scores at least this many points (see [`Rules::score`]).
    Score { at_least: u32 },
    /// Every pile in `group` holds `max_cards` cards with a bottom card satisfying its `empty` rule.
    AllFull { group: String },
    /// The grid of the score rules is full and every row scores at least this many points, each
    /// on its own (Maverick: every row a straight or better).
    EachRowScore { at_least: u32 },
    /// The first `red` piles of `group` hold only red cards and the rest only black ones (Fire and
    /// Ice).
    ColorSplit { group: String, red: u16 },
    /// Like [`WinRule::Rows`], but checks each row's ordered prefix using `build` instead of the
    /// group's build rule.
    RowsBuilt {
        group: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<u16>,
        build: Build,
    },
    /// Every non-empty pile of `group` (or the single pile at `pile`) satisfies optional `length`
    /// and `build` sequence checks, and every group in `empty` is empty.
    PilesBuilt {
        group: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<u16>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        build: Option<Build>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        empty: Vec<String>,
        /// Require at least one non-empty pile in `group`.
        #[serde(default, skip_serializing_if = "is_false")]
        non_empty: bool,
        /// Check only this 0-based pile index within `group` (it must be non-empty if `length` or
        /// `non_empty` is set).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pile: Option<u16>,
        /// Every non-empty pile must start (at its bottom) with a card of this rank.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bottom: Option<u8>,
    },
}

/// How a position is scored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ScoreRules {
    /// A grid group. Each of its rows and each of its columns is scored as a hand.
    pub group: String,
    pub hands: Hands,
}

impl ScoreRules {
    /// Score each row and column of the grid group `group` as a hand.
    pub fn new(group: &str, hands: Hands) -> ScoreRules {
        ScoreRules {
            group: group.to_string(),
            hands,
        }
    }
}

/// How a hand (the cards in one row or column) scores.
///
/// A joker in a hand counts as whichever card scores that hand best. Each hand is valued on its
/// own, so a joker shared by a row and a column (or a joker starter) may stand for different
/// cards in each.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Hands {
    /// As a poker hand: its best five-card poker combination. Straights and flushes need five
    /// cards; pairs and sets count in a line of any length.
    Poker(PokerTable),
    /// As a cribbage hand, together with a shared starter card (Cribbage Squares).
    Cribbage(CribbageRules),
}

/// Cribbage scoring. Each line is a hand; once every pile of the grid holds a card, the
/// starter joins every hand as a fifth card. Fifteens, pairs and runs count in a line of any
/// length; a flush needs a full line of at least four cards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct CribbageRules {
    /// Group whose first pile's top card is the starter, read when the grid is full. Usually
    /// the waste: the card turned after the grid is filled. No starter if left out or empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starter: Option<String>,
    #[serde(default, skip_serializing_if = "CribbagePoints::is_standard")]
    pub points: CribbagePoints,
}

impl CribbageRules {
    /// Standard points, with the starter on top of `group`'s first pile.
    pub fn with_starter(group: &str) -> CribbageRules {
        CribbageRules {
            starter: Some(group.to_string()),
            points: CribbagePoints::default(),
        }
    }
}

impl Default for CribbageRules {
    /// Standard points, no starter.
    fn default() -> Self {
        CribbageRules {
            starter: None,
            points: CribbagePoints::default(),
        }
    }
}

/// Points for each cribbage combination. Defaults are the standard values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, default)]
#[non_exhaustive]
pub struct CribbagePoints {
    /// Each different set of cards adding up to 15 (court cards count 10, aces 1).
    pub fifteen: u32,
    /// Each pair of cards of the same rank.
    pub pair: u32,
    /// Each card of each run of three or more ranks in sequence (ace low), counted once per
    /// different set of cards making the run.
    pub run_card: u32,
    /// A full line all of one suit.
    pub flush: u32,
    /// More when the starter is that suit too.
    pub flush_starter: u32,
    /// For each hand holding the jack of the starter's suit ("one for his nob").
    pub nobs: u32,
    /// Once, if the starter is a jack ("two for his heels").
    pub heels: u32,
}

impl Default for CribbagePoints {
    fn default() -> Self {
        CribbagePoints {
            fifteen: 2,
            pair: 2,
            run_card: 1,
            flush: 4,
            flush_starter: 1,
            nobs: 1,
            heels: 2,
        }
    }
}

impl CribbagePoints {
    pub fn is_standard(&self) -> bool {
        *self == CribbagePoints::default()
    }
}

/// Points for each poker hand: a named table or explicit points.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
#[non_exhaustive]
pub enum PokerTable {
    Named(PokerScoring),
    Points(PokerPoints),
}

impl PokerTable {
    pub fn points(&self) -> PokerPoints {
        match self {
            PokerTable::Named(PokerScoring::American) => PokerPoints {
                pair: 2,
                two_pair: 5,
                three: 10,
                straight: 15,
                flush: 20,
                full_house: 25,
                four: 50,
                straight_flush: 75,
                royal_flush: 100,
            },
            PokerTable::Named(PokerScoring::English) => PokerPoints {
                pair: 1,
                two_pair: 3,
                three: 6,
                straight: 12,
                flush: 5,
                full_house: 10,
                four: 16,
                straight_flush: 30,
                royal_flush: 30,
            },
            PokerTable::Points(p) => p.clone(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PokerScoring {
    /// Pair 2, two pair 5, three of a kind 10, straight 15, flush 20, full house 25, four of a
    /// kind 50, straight flush 75, royal flush 100.
    American,
    /// Pair 1, two pair 3, three of a kind 6, straight 12, flush 5, full house 10, four of a
    /// kind 16, straight or royal flush 30.
    English,
}

/// Points per poker hand. A hand counts as its highest-ranking combination (in the usual poker
/// order), whatever the points.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields, default)]
#[non_exhaustive]
pub struct PokerPoints {
    pub pair: u32,
    pub two_pair: u32,
    pub three: u32,
    pub straight: u32,
    pub flush: u32,
    pub full_house: u32,
    pub four: u32,
    pub straight_flush: u32,
    pub royal_flush: u32,
}

#[cfg(feature = "json")]
impl Rules {
    pub fn from_json(s: &str) -> Result<Rules, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("rules serialize")
    }
}
