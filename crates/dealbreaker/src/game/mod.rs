//! The engine: rules compiled into lookup tables, positions, legal moves, and reversible play.

mod compile;
mod deal;
mod moves;
mod play;

use std::fmt;

use crate::card::{Card, FACES};
use crate::rules::*;

/// Why a [`Rules`] value can't be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesError(pub String);

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid rules: {}", self.0)
    }
}

impl std::error::Error for RulesError {}

fn err<T>(msg: impl Into<String>) -> Result<T, RulesError> {
    Err(RulesError(msg.into()))
}

/// A position: the cards on every pile, bottom first, plus the bits of game state that aren't
/// cards. Piles are numbered across groups in the order the rules list them.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct State {
    pub piles: Vec<Vec<Card>>,
    /// Times the waste has been turned back into the stock.
    pub redeals_used: u32,
    /// The rank set by the deal for [`EmptyRule::DealtBase`] foundations.
    pub base_rank: Option<u8>,
    /// Seed for shuffles during play (shuffling redeals). Fixed for the game.
    pub seed: u64,
}

/// What a move does after any talon steps.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Action {
    /// Move the top `count` cards of `from` onto `to`, keeping their order.
    Transfer { from: u16, to: u16, count: u16 },
    /// Deal from the stock, or turn the waste back over if the stock is empty.
    Deal,
    /// Remove the top cards of some piles together, to the rule's discard pile.
    /// `rule` is the index of the removal rule; `piles[..n]` are sorted pile numbers.
    Remove { rule: u8, n: u8, piles: [u16; 4] },
}

/// A move as the solver plays it: `talon` stock deals (or redeals) first, then the action.
///
/// For games whose stock deals to a waste pile, the solver folds runs of deals into the move
/// that plays the card they turn up, which shortens solutions and searches a great deal.
/// [`Game::steps`] expands moves into single steps for display.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Move {
    pub talon: u16,
    pub action: Action,
}

impl Move {
    pub fn new(action: Action) -> Move {
        Move { talon: 0, action }
    }
}

/// A primitive, invertible change to a state. The journal of these makes undo exact.
#[derive(Copy, Clone, Debug)]
pub(crate) enum Op {
    /// Move `count` cards as a block.
    Shift { from: u16, to: u16, count: u16 },
    /// Move `count` cards one at a time (reversing them), turning each face up or down.
    Turn {
        from: u16,
        to: u16,
        count: u16,
        up: bool,
    },
    /// Turn over the top card of a pile.
    Flip(u16),
    /// `redeals_used += 1`.
    Redeal,
    /// Set `s.base_rank = Some(r)` (undo restores `None`).
    SetBaseRank(u8),
    /// Swap two cards of a pile (shuffles are recorded as swaps, so they undo exactly).
    Swap { pile: u16, i: u16, j: u16 },
    /// Swap the entire contents of two piles.
    SwapPiles(u16, u16),
}

/// Compiled opening base-rank choice rules.
#[derive(Copy, Clone, Debug)]
pub(crate) struct CompiledChooseBase {
    pub from_group: u16,
    pub also_from_group: Option<u16>,
    pub refill_from_stock: bool,
    pub deal_waste: bool,
    pub free_moves: bool,
}

/// A compiled group.
#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub name: String,
    pub kind: PileKind,
    pub start: u16,
    pub count: u16,
    /// Bitmask of groups whose cards this group accepts.
    pub accepts_from: u64,
    /// Bitmask of groups whose cards an empty pile of this group accepts.
    pub empty_accepts_from: u64,
    /// Groups to refill an emptied pile from, in order.
    pub refill_from: Vec<u16>,
    /// Cards moved per automatic refill.
    pub refill_count: u16,
    /// Its piles are interchangeable: permuting them gives an equivalent position.
    pub symmetric: bool,
    /// Becomes symmetric once the stock is empty and has no redeals left.
    pub symmetric_when_no_stock: bool,
    /// Becomes symmetric once all tableau redeals have been used.
    pub symmetric_when_no_redeal: bool,
    /// Equivalence classes of interchangeable piles within a non-uniform group (e.g. a
    /// foundation group with per-pile overrides). Empty unless at least one class has > 1 pile.
    pub sym_classes: Vec<Vec<u16>>,
    /// Grid layout: piles per row.
    pub columns: Option<u16>,
    /// Internal scratch pile (for redeals); not part of positions shown to callers.
    pub hidden: bool,
    /// Accept only from piles of this group this many places later.
    pub reach: Option<Vec<u16>>,
    /// With `reach`: also accept from piles earlier in the group.
    pub reach_both_ways: bool,
    pub reach_grid: bool,
    /// (source group, for each source pile the target index in this group or -1).
    pub from_pile_map: Option<(u16, Vec<i16>)>,
    pub clear_tops: Option<(u16, u16)>,
    /// (target pile index in this group, source group, source pile index in that group) banned in the first round.
    pub first_round_bans: Vec<(u16, u16, u16)>,
    pub row_start: Option<u8>,
    /// When placing onto a non-empty pile of this group, move existing cards to this pile first.
    pub replace_on_drop: Option<u16>,
    /// Slide piles forward to close gaps after every move.
    pub compact: bool,
    /// Each pile after the first only accepts ranks present on the previous pile.
    pub requires_rank_on_prev: bool,
    /// Shorter piles must match the rank at the same depth of the longest pile in this group.
    pub match_longest_rank: bool,
    /// Accept cards only when the destination pile is empty.
    pub accept_only_when_empty: bool,
    /// Accept cards only when the stock pile is empty.
    pub accept_when_stock_empty: bool,
    pub accept_when_waste_empty: bool,
    /// Shift the movable face-up run from the next pile into an emptied pile.
    pub compact_face_up: bool,
    /// Pick the largest pile when refilling from a multi-pile group.
    pub refill_largest: bool,
    /// Refill a pile only when a move just emptied it.
    pub refill_on_empty: bool,
    /// Each pile after the first may only start once the previous pile is non-empty.
    pub requires_prev_non_empty: bool,
    /// Most consecutive face-up cards of the same rank allowed at the top of a pile.
    pub max_same_rank: Option<u8>,
    /// Cards may leave piles of this group only once the stock pile is empty.
    pub moves_when_stock_empty: bool,
    /// Cards may leave piles of this group only once at least one pile in this group is empty.
    pub moves_when_any_empty: Option<u16>,
    /// Cards may leave pile `i` of this group only once pile `i` of this group is empty.
    pub moves_when_same_index_empty: Option<u16>,
    /// Cards may be added to pile `i` only once pile `i` of this group is non-empty.
    pub requires_same_index_non_empty: Option<u16>,
    /// Pile `i` accepts cards (except from the same-index pile of this group) only while that pile is empty.
    pub accept_when_same_index_empty: Option<u16>,
    /// Cards may leave only the first or last non-empty pile.
    pub move_from_frontier: Option<crate::rules::Frontier>,
    /// Empty piles accept any card once this many piles start with the `empty` rank.
    pub empty_open_after_bases: Option<u16>,
    pub empty_open_when_full: Option<u16>,
    pub empty_needs_first_full: bool,
    /// Only auto-flip the top card of a pile when it is the last remaining card.
    pub flip_only_last: bool,
    /// When a pile is one card shy of `max_cards`, its final card must come from the same-index
    /// pile of this group.
    pub final_from_same_index: Option<u16>,
    /// When `build.dir` is `either`, the first two-card pile locks build direction for the group.
    pub lock_build_dir: bool,
    /// Never auto-flip face-down cards in this group.
    pub no_flip: bool,
    /// Refill each emptied pile from the same-index pile of `refill_from`.
    pub refill_same_index: bool,
    /// Refill each emptied pile from the nearest non-empty pile of `refill_from`.
    pub refill_nearest: bool,
    /// Only allow building on a non-empty pile if its bottom card satisfies the pile's `empty` rule.
    pub require_valid_base: bool,
    /// Swap-on-drop mode for piles within this group.
    pub swap_bases: Option<SwapBases>,
    /// Allow moving a complete suit-length run regardless of supermove capacity.
    pub supermove_complete_run: bool,
    /// Source groups whose cards bypass `build` and `empty` rules when dropped onto this group.
    pub unrestricted_from: u64,
    /// When `build.dir` is `either`, each pile locks its own build direction once it has 2 cards.
    pub lock_pile_dir: bool,
    /// No card of this suit may be added, or built on.
    pub exclude_suit: Option<crate::card::Suit>,
    /// A full, fully built pile may not be moved from.
    pub freeze_when_built: bool,
    pub accept_on_face_down: bool,
    pub one_way_split: Option<u16>,
    pub move_face_down_top: bool,
    /// Fixed rank sequence required by pile depth.
    pub rank_sequence: Vec<u8>,
    /// No two non-empty piles in this group may share the same suit on their bottom card.
    pub unique_suits: bool,
    pub unique_ranks: bool,
    pub cyclic_ranks: bool,
    pub same_suit_group: bool,
    pub same_rank_group: bool,
    /// Move every pile's cards to the first pile of this group once all piles hold a card.
    pub clear_when_full: Option<u16>,
    pub clear_same_rank: bool,
    pub run_len: Option<u8>,
    pub compact_when_stock_empty: bool,
    pub flip_when_dealt_out: bool,
    pub auto_foundation_rank: Option<u8>,
    /// Refill only while every pile of this group is empty.
    pub refill_when_empty: Option<u16>,
    /// While stock is non-empty, forbid building onto or moving from single-card piles onto non-empty piles.
    pub no_build_single_while_stock: bool,
    /// When refilling from stock with `refill_count > 1`, deal the first card face down unless it is the last card.
    pub refill_first_face_down: bool,
    /// Auto-flip pile `i` when pile `i` of this group reaches `max_cards`.
    pub flip_when_same_index_full: Option<u16>,
    /// A face-down top turns up only once the previous pile is empty.
    pub flip_after_prev_empty: bool,
    pub flip_when_all_at_most: Option<u16>,
    /// A face-down top turns up only while some pile of this group is empty.
    pub flip_when_any_empty: Option<u16>,
}

#[derive(Clone, Debug)]
pub(crate) struct Redeal {
    pub group: u16,
    pub times: u32,
    pub how: RedealHow,
    /// The scratch pile cards are gathered on.
    pub gather: u16,
    /// Where a compaction refills from.
    pub refill: Option<u16>,
}

/// The compiled rules of one pile.
#[derive(Clone, Debug)]
pub(crate) struct PileRules {
    pub build: Build,
    pub empty: EmptyRule,
    pub moves: MoveUnit,
    pub max_cards: u16,
    pub min_cards: u16,
    pub max_accept: u16,
    pub empty_max_accept: u16,
    pub non_empty_max_accept: u16,
    pub complete_runs: bool,
    /// Foundations the solver may play to automatically when safe.
    pub autoplay: bool,
    /// True if another foundation with a different policy can accept some of the same faces.
    pub competing_foundation: bool,
    /// Cards never leave, so only the top card and the count matter to the future.
    pub sink: bool,
    /// Faces this pile accepts at all (suit and colour limits), as a bit set.
    pub admits: u64,
    /// If set, an empty pile accepts only cards of this suit.
    pub empty_suit: Option<crate::card::Suit>,
    /// `fits[under] >> over & 1`: may `over` be built on `under` here? Real cards only: jokers
    /// are handled by [`Game::accept_set`] and friends.
    pub fits: [u64; FACES],
    /// Jokers are wild here.
    pub wild: bool,
    /// Resolved target group and rank offset for [`EmptyRule::AfterPile`].
    pub after_pile: Option<(u16, i8)>,
}

impl PileRules {
    /// True if two piles have identical rule policies (ignoring derived tables like `fits`,
    /// `autoplay`, and `sink`). Exhaustive destructuring ensures new fields are considered here.
    pub(crate) fn same_policy(&self, other: &PileRules) -> bool {
        let PileRules {
            build,
            empty,
            moves,
            max_cards,
            min_cards,
            max_accept,
            empty_max_accept,
            non_empty_max_accept,
            complete_runs,
            admits,
            empty_suit,
            wild,
            after_pile,
            autoplay: _,
            competing_foundation: _,
            sink: _,
            fits: _,
        } = self;
        *build == other.build
            && *empty == other.empty
            && *moves == other.moves
            && *max_cards == other.max_cards
            && *min_cards == other.min_cards
            && *max_accept == other.max_accept
            && *empty_max_accept == other.empty_max_accept
            && *non_empty_max_accept == other.non_empty_max_accept
            && *complete_runs == other.complete_runs
            && *admits == other.admits
            && *empty_suit == other.empty_suit
            && *wild == other.wild
            && *after_pile == other.after_pile
    }

    /// True if two foundation piles accept the exact same card sequences (for autoplay conflict
    /// detection).
    pub(crate) fn same_foundation_policy(&self, other: &PileRules) -> bool {
        self.build == other.build
            && self.empty == other.empty
            && self.empty_suit == other.empty_suit
            && self.max_cards == other.max_cards
            && self.admits == other.admits
    }
}

impl Group {
    pub fn piles(&self) -> std::ops::Range<u16> {
        self.start..self.start + self.count
    }

    /// True if pile position/order within this group affects legality or transitions.
    pub(crate) fn has_positional_rules(&self) -> bool {
        self.reach.is_some()
            || self.compact
            || self.compact_face_up
            || self.requires_rank_on_prev
            || self.requires_prev_non_empty
            || self.final_from_same_index.is_some()
            || self.moves_when_same_index_empty.is_some()
            || self.requires_same_index_non_empty.is_some()
            || self.accept_when_same_index_empty.is_some()
            || self.move_from_frontier.is_some()
            || self.empty_open_after_bases.is_some()
            || self.empty_open_when_full.is_some()
            || self.empty_needs_first_full
            || self.flip_when_same_index_full.is_some()
            || self.flip_after_prev_empty
            || self.flip_when_any_empty.is_some()
            || self.refill_same_index
            || self.refill_nearest
            || self.one_way_split.is_some()
    }

    /// True if moves to or from this group are gated on the stock being empty.
    pub(crate) fn depends_on_stock_empty(&self) -> bool {
        self.accept_when_stock_empty
            || self.accept_when_waste_empty
            || self.moves_when_stock_empty
            || self.no_build_single_while_stock
    }

    /// True if this group participates in automatic post-move compaction or refilling.
    pub(crate) fn needs_settle(&self) -> bool {
        self.compact
            || self.compact_face_up
            || !self.refill_from.is_empty()
            || self.clear_when_full.is_some()
            || self.clear_tops.is_some()
            || self.auto_foundation_rank.is_some()
            || self.flip_when_same_index_full.is_some()
            || self.flip_after_prev_empty
            || self.flip_when_all_at_most.is_some()
            || self.flip_when_any_empty.is_some()
            || self.flip_when_dealt_out
    }

    /// Internal single-pile scratch group (used for redeal gathering).
    pub(crate) fn hidden_scratch(name: &str, start: u16) -> Group {
        Group {
            name: name.into(),
            kind: PileKind::Reserve,
            start,
            count: 1,
            accepts_from: 0,
            empty_accepts_from: 0,
            refill_from: Vec::new(),
            refill_count: 1,
            symmetric: true,
            symmetric_when_no_stock: false,
            symmetric_when_no_redeal: false,
            sym_classes: Vec::new(),
            columns: None,
            hidden: true,
            reach: None,
            reach_both_ways: false,
            reach_grid: false,
            from_pile_map: None,
            clear_tops: None,
            first_round_bans: Vec::new(),
            row_start: None,
            replace_on_drop: None,
            compact: false,
            requires_rank_on_prev: false,
            match_longest_rank: false,
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
            unrestricted_from: 0,
            lock_pile_dir: false,
            exclude_suit: None,
            freeze_when_built: false,
            accept_on_face_down: false,
            one_way_split: None,
            move_face_down_top: false,
            rank_sequence: Vec::new(),
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
}

#[derive(Clone, Debug)]
pub(crate) struct Stock {
    pub pile: u16,
    pub target: u16,
    pub target_kind: PileKind,
    pub count: u16,
    pub count_per_round: Vec<u16>,
    pub final_to: Option<u16>,
    /// Resolved target pile indices (either `StockRules::piles` or all piles of `target`).
    pub targets: Vec<u16>,
    pub piles_per_round: Vec<Vec<u16>>,
    pub final_piles: Option<Vec<u16>>,
    pub redeals: u32,
    pub require_no_empty: bool,
    pub auto: bool,
    pub only_when_empty: bool,
    pub first_empty: bool,
    pub skip_empty: bool,
    pub face_up: bool,
    pub deal_per_pile: bool,
    pub redeal_reverse: bool,
    pub redeal_flip_stock: bool,
    pub redeal_shuffle: bool,
    pub redeal_deal_all_alternating: bool,
    pub deal_empty_first: bool,
    pub divert_ranks: Vec<u8>,
    pub final_split_to: Option<u16>,
    pub require_no_empty_group: Option<u16>,
    pub require_group_empty: Option<u16>,
    pub auto_for_empty: Option<u16>,
    pub skip_built_from: Option<u8>,
    pub max_deals: Option<u32>,
    pub skip_run_with_rank: Option<u8>,
}

impl Stock {
    #[inline]
    pub fn deal_count(&self, redeals_used: u32) -> u16 {
        if self.count_per_round.is_empty() {
            self.count
        } else {
            let i = (redeals_used as usize).min(self.count_per_round.len() - 1);
            self.count_per_round[i]
        }
    }

    #[inline]
    pub fn round_targets(&self, redeals_used: u32) -> &[u16] {
        if self.piles_per_round.is_empty() {
            &self.targets
        } else {
            let i = (redeals_used as usize).min(self.piles_per_round.len() - 1);
            &self.piles_per_round[i]
        }
    }

    /// True if emptying a target pile can block or alter how a stock deal behaves.
    pub fn sensitive_to_empty_piles(&self) -> bool {
        self.require_no_empty
            || self.require_no_empty_group.is_some()
            || self.require_group_empty.is_some()
            || self.auto_for_empty.is_some()
            || self.skip_built_from.is_some()
            || self.max_deals.is_some()
            || self.skip_run_with_rank.is_some()
            || self.skip_empty
            || self.first_empty
            || self.deal_empty_first
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Removal {
    pub kind: RemovalKind,
    pub from: u64,
    pub to: u16,
    /// Neighbour rule, with the grid group it applies to.
    pub adjacent: Option<(Adjacency, u16)>,
}

#[derive(Clone, Debug)]
pub(crate) enum Win {
    AllOnFoundations,
    Empty(Vec<u16>),
    Count(u16, u16),
    /// Group, card count, groups that must be empty.
    CountAndEmpty(u16, u16, Vec<u16>),
    /// Group, card count, spread group (every pile holds at most one card).
    CountAndSpread(u16, u16, u16),
    /// Grid group, row length to check, first-column rank.
    Rows(u16, u16, u8),
    /// Group, most non-empty piles.
    AtMostPiles(u16, u16),
    /// At least this many points.
    Score(u32),
    /// Every pile in `group` holds `max_cards` cards with a valid base card.
    AllFull(u16),
    /// Group, number of leading piles that must hold only red cards (the rest only black).
    ColorSplit(u16, u16),
    /// Every row of the score grid scores at least this many points.
    EachRowScore(u32),
    /// Non-empty piles of `group` (or single `pile` index) satisfy optional `length` and `build`,
    /// with every group in `empty` empty.
    PilesBuilt {
        group: u16,
        length: Option<u16>,
        has_build: bool,
        empty: Vec<u16>,
        non_empty: bool,
        pile: Option<u16>,
        bottom: Option<u8>,
    },
}

/// A game ready to play and solve: [`Rules`] compiled into tables.
#[derive(Clone, Debug)]
pub struct Game {
    rules: Rules,
    pub(crate) groups: Vec<Group>,
    pub(crate) pile_group: Vec<u16>,
    pub(crate) pile_rules: Vec<PileRules>,
    pub(crate) covered_by: Vec<Vec<u16>>,
    pub(crate) has_covers: bool,
    pub(crate) stock: Option<Stock>,
    pub(crate) stock_pile: Option<u16>,
    pub(crate) redeal: Option<Redeal>,
    pub(crate) removal: Vec<Removal>,
    pub(crate) win: Win,
    /// Optional compiled fits table overriding the group's build rule in `row_prefix`.
    pub(crate) row_win_fits: Option<[u64; FACES]>,
    /// Position of each rank (1..=13) in the pack's sequence; 255 if not in the pack.
    pub(crate) rank_idx: [u8; 14],
    pub(crate) nranks: u8,
    pub(crate) copies: u8,
    pub(crate) total_cards: u16,
    /// Jokers in the pack.
    pub(crate) jokers: u8,
    pub(crate) in_pack: [bool; FACES],
    /// Real faces of each rank (index 1..=13), and of each suit.
    pub(crate) rank_faces: [u64; 14],
    pub(crate) suit_faces: [u64; 4],
    /// For each face, the faces that could be built on it somewhere other than a foundation.
    pub(crate) dependents: Vec<Vec<u8>>,
    pub(crate) any_autoplay: bool,
    /// The stock deals to a waste pile that cards can be played from.
    pub(crate) compress_talon: bool,
    /// No compact, refill, auto-stock, or hooks: `settle` only needs `auto_flip`.
    pub(crate) simple_settle: bool,
    /// Piles that can ever be a move source (`moves != MoveUnit::None`).
    pub(crate) movable_piles: Vec<u16>,
    /// For each source group, the destination piles that could ever accept from it.
    pub(crate) targets_for_group: Vec<Vec<u16>>,
    /// True if any pile uses `MoveUnit::Supermove`.
    pub(crate) has_supermove: bool,
    /// Global equivalence-class index for each pile in a non-uniform group with symmetric
    /// sub-classes (`u16::MAX` if not in a multi-pile sub-class).
    pub(crate) pile_sym_class: Vec<u16>,
    /// Number of multi-pile equivalence classes across all groups.
    pub(crate) num_sym_classes: usize,
    /// True if `RemovalKind::LowerOfSuit` is soundly forced when available.
    pub(crate) force_lower_of_suit: bool,
    pub(crate) waste: Option<u16>,
    pub(crate) choose_base: Option<CompiledChooseBase>,
    pub(crate) hooks: Option<std::sync::Arc<dyn crate::hooks::Hooks>>,
    pub(crate) score: Option<crate::score::Score>,
}

impl Game {
    pub fn rules(&self) -> &Rules {
        &self.rules
    }

    /// Points for a position, if the game is scored.
    pub fn score(&self, s: &State) -> Option<u32> {
        self.score.as_ref().map(|sc| sc.score(s))
    }

    /// Is the game played for points?
    pub fn is_scored(&self) -> bool {
        self.score.is_some()
    }

    /// The score that wins, for games won by reaching a score.
    pub fn target_score(&self) -> Option<u32> {
        match self.win {
            Win::Score(t) => Some(t),
            _ => None,
        }
    }

    /// The legal moves, leaving out any that a symmetry of the position makes equivalent to an
    /// earlier one (in a scored grid, a card going into a second empty row is no different from
    /// the first).
    pub fn distinct_moves(&self, s: &State) -> Vec<Move> {
        let mut moves = self.legal_moves(s);
        if let Some(sc) = self.score.as_ref().filter(|sc| sc.symmetric) {
            moves.retain(|m| match m.action {
                Action::Transfer { to, .. } => !sc.redundant(self, s, to),
                _ => true,
            });
        }
        moves
    }

    pub fn name(&self) -> &str {
        &self.rules.name
    }

    pub fn num_piles(&self) -> usize {
        self.pile_group.len()
    }

    /// The group name and index within the group of a pile.
    pub fn pile_ref(&self, pile: u16) -> PileRef {
        let g = &self.groups[self.pile_group[pile as usize] as usize];
        PileRef {
            group: g.name.clone(),
            index: pile - g.start,
        }
    }

    /// The pile number for a group name and index.
    pub fn pile_index(&self, group: &str, index: u16) -> Option<u16> {
        let g = self.groups.iter().find(|g| g.name == group)?;
        (index < g.count).then_some(g.start + index)
    }

    pub fn pile_kind(&self, pile: u16) -> PileKind {
        self.group_of(pile).kind
    }

    #[inline]
    pub(crate) fn group_of(&self, pile: u16) -> &Group {
        &self.groups[self.pile_group[pile as usize] as usize]
    }

    /// Group names with their pile counts, in pile order.
    pub fn groups(&self) -> impl Iterator<Item = (&str, PileKind, u16)> {
        self.groups
            .iter()
            .map(|g| (g.name.as_str(), g.kind, g.count))
    }

    // ----- Display --------------------------------------------------------------------------

    /// Expand solver moves into single steps, for display or for driving a UI.
    pub fn steps(&self, start: &State, moves: &[Move]) -> Vec<Step> {
        let mut s = start.clone();
        let mut j = Vec::new();
        let mut out = Vec::new();
        for &mv in moves {
            for _ in 0..mv.talon {
                out.push(self.deal_step_kind(&s));
                self.talon_step(&mut s, &mut j);
            }
            match mv.action {
                Action::Deal => out.push(self.deal_step_kind(&s)),
                Action::Transfer { from, to, count } => out.push(Step::Move {
                    from: self.pile_ref(from),
                    to: self.pile_ref(to),
                    count,
                }),
                Action::Remove { n, piles, .. } => out.push(Step::Remove {
                    piles: piles[..n as usize]
                        .iter()
                        .map(|&p| self.pile_ref(p))
                        .collect(),
                }),
            }
            self.apply(&mut s, Move::new(mv.action), &mut j);
            j.clear();
        }
        out
    }

    /// The move a single step makes in `s`, if it's legal there.
    pub fn step_to_move(&self, s: &State, step: &Step) -> Option<Move> {
        let idx = |r: &PileRef| self.pile_index(&r.group, r.index);
        let mv = match step {
            Step::Deal | Step::Redeal if self.redeal.is_some() => {
                if !matches!(step, Step::Redeal) {
                    return None;
                }
                Move::new(Action::Deal)
            }
            Step::Deal | Step::Redeal => {
                let empty = self
                    .stock
                    .as_ref()
                    .is_some_and(|st| s.piles[st.pile as usize].is_empty());
                if empty != matches!(step, Step::Redeal) {
                    return None;
                }
                Move::new(Action::Deal)
            }
            Step::Move { from, to, count } => Move::new(Action::Transfer {
                from: idx(from)?,
                to: idx(to)?,
                count: *count,
            }),
            Step::Remove { piles } => {
                // Any order names the same set; find the rule that allows it.
                let mut want: Vec<u16> = piles.iter().map(idx).collect::<Option<_>>()?;
                want.sort_unstable();
                let mut moves = Vec::new();
                self.gen_removals(s, None, &mut moves);
                return moves.into_iter().find(|m| {
                    matches!(m.action, Action::Remove { n, piles, .. } if piles[..n as usize] == want[..])
                });
            }
        };
        self.is_legal(s, mv).then_some(mv)
    }

    /// Every legal single step in this position.
    pub fn legal_steps(&self, s: &State) -> Vec<Step> {
        let mut out = Vec::new();
        self.gen_actions_unpruned(s, &mut out);
        let mut steps: Vec<Step> = out.iter().map(|&m| self.steps(s, &[m]).remove(0)).collect();
        if self.can_deal(s) {
            steps.push(self.deal_step_kind(s));
        }
        steps
    }

    /// Play a single step. Returns an error if it isn't legal.
    pub fn play_step(&self, s: &mut State, step: &Step) -> Result<(), IllegalStep> {
        let mv = self
            .step_to_move(s, step)
            .ok_or_else(|| IllegalStep(step.clone()))?;
        let mut j = Vec::new();
        self.apply(s, mv, &mut j);
        Ok(())
    }

    fn deal_step_kind(&self, s: &State) -> Step {
        if self.redeal.is_some() {
            return Step::Redeal;
        }
        let st = self.stock.as_ref().unwrap();
        if s.piles[st.pile as usize].is_empty() {
            Step::Redeal
        } else {
            Step::Deal
        }
    }

    /// Render a position as text, one pile per line.
    pub fn render(&self, s: &State) -> String {
        let mut out = String::new();
        for g in self.groups.iter().filter(|g| !g.hidden) {
            for p in g.piles() {
                let cards: Vec<String> =
                    s.piles[p as usize].iter().map(|c| c.to_string()).collect();
                let label = if g.count == 1 {
                    g.name.clone()
                } else {
                    format!("{} {}", g.name, p - g.start + 1)
                };
                out.push_str(&format!("{label:>14}: {}\n", cards.join(" ")));
            }
        }
        if s.redeals_used > 0 {
            out.push_str(&format!("{:>14}: {}\n", "redeals used", s.redeals_used));
        }
        out
    }
}

/// A move that isn't legal in the position it was played in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalMove(pub Move);

impl fmt::Display for IllegalMove {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "illegal move: {:?}", self.0)
    }
}

impl std::error::Error for IllegalMove {}

/// A step that isn't legal in the position it was played in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IllegalStep(pub Step);

impl fmt::Display for IllegalStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "illegal step: {}", self.0)
    }
}

impl std::error::Error for IllegalStep {}

/// A pile, named by its group and its index within the group (from 0).
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PileRef {
    pub group: String,
    pub index: u16,
}

impl fmt::Display for PileRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.group, self.index + 1)
    }
}

/// One step of a solution, as a player would make it. Automatic actions (turning up exposed
/// cards, refilling from a reserve) follow each step and aren't listed.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Step {
    /// Deal from the stock.
    Deal,
    /// Turn the waste back over to form a new stock.
    Redeal,
    /// Move `count` cards from the top of `from` onto `to`.
    Move {
        from: PileRef,
        to: PileRef,
        count: u16,
    },
    /// Remove the top cards of these piles.
    Remove { piles: Vec<PileRef> },
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Step::Deal => write!(f, "deal"),
            Step::Redeal => write!(f, "redeal"),
            Step::Move { from, to, count: 1 } => write!(f, "{from} -> {to}"),
            Step::Move { from, to, count } => write!(f, "{from} -> {to} ({count} cards)"),
            Step::Remove { piles } => {
                let names: Vec<String> = piles.iter().map(|p| p.to_string()).collect();
                write!(f, "remove {}", names.join(" + "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    #[test]
    fn game_metadata_render_and_steps() {
        let mut r = presets::klondike1();
        r.stock.as_mut().unwrap().unlimited_redeals = false;
        r.stock.as_mut().unwrap().redeals = 2;
        let g = Game::new(r).unwrap();
        assert_eq!(g.name(), "Klondike");
        assert_eq!(g.groups().count(), 4);
        assert_eq!(g.pile_kind(0), PileKind::Stock);

        let mut s = g.deal(1);
        assert!(g.legal_steps(&s).contains(&Step::Deal));
        g.play_step(&mut s, &Step::Deal).unwrap();
        assert!(g.play_step(&mut s, &Step::Redeal).is_err());

        // Drain stock to test Redeal and render with redeals_used > 0.
        while g.step_to_move(&s, &Step::Deal).is_some() {
            g.play_step(&mut s, &Step::Deal).unwrap();
        }
        assert!(g.legal_steps(&s).contains(&Step::Redeal));
        g.play_step(&mut s, &Step::Redeal).unwrap();
        let rendered = g.render(&s);
        assert!(rendered.contains("redeals used"), "{rendered}");

        // Display impls for Step, IllegalMove, IllegalStep.
        assert_eq!(Step::Deal.to_string(), "deal");
        assert_eq!(Step::Redeal.to_string(), "redeal");
        let p0 = PileRef {
            group: "tableau".into(),
            index: 0,
        };
        let p1 = PileRef {
            group: "tableau".into(),
            index: 1,
        };
        assert_eq!(
            Step::Move {
                from: p0.clone(),
                to: p1.clone(),
                count: 1
            }
            .to_string(),
            "tableau 1 -> tableau 2"
        );
        assert_eq!(
            Step::Move {
                from: p0.clone(),
                to: p1.clone(),
                count: 3
            }
            .to_string(),
            "tableau 1 -> tableau 2 (3 cards)"
        );
        assert_eq!(
            Step::Remove {
                piles: vec![p0, p1]
            }
            .to_string(),
            "remove tableau 1 + tableau 2"
        );
        let im = IllegalMove(Move::new(Action::Deal));
        assert!(im.to_string().contains("illegal move"));
        let is = IllegalStep(Step::Redeal);
        assert!(is.to_string().contains("illegal step"));
    }

    #[test]
    fn step_to_move_handles_redeal_and_remove() {
        let g_lbl = Game::new(presets::la_belle_lucie()).unwrap();
        let s_lbl = g_lbl.deal(1);
        assert!(g_lbl.step_to_move(&s_lbl, &Step::Deal).is_none());
        assert!(g_lbl.step_to_move(&s_lbl, &Step::Redeal).is_some());

        let g_pyr = Game::new(presets::pyramid()).unwrap();
        let s_pyr = g_pyr.deal(1);
        let remove = g_pyr
            .legal_steps(&s_pyr)
            .into_iter()
            .find(|st| matches!(st, Step::Remove { .. }))
            .expect("pyramid seed 1 has a removal");
        assert!(g_pyr.step_to_move(&s_pyr, &remove).is_some());
    }
}
