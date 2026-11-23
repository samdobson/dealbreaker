//! The engine: rules compiled into lookup tables, positions, legal moves, and reversible play.

use std::fmt;

use crate::card::{Card, FACES, JOKER, REAL_FACES};
use crate::rng::Rng;
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
    /// Swap two cards of a pile (shuffles are recorded as swaps, so they undo exactly).
    Swap { pile: u16, i: u16, j: u16 },
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
    /// Groups to refill an emptied pile from, in order.
    pub refill_from: Vec<u16>,
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
    /// Shift the movable face-up run from the next pile into an emptied pile.
    pub compact_face_up: bool,
    /// Pick the largest pile when refilling from a multi-pile group.
    pub refill_largest: bool,
    /// Refill a pile only when a move just emptied it.
    pub refill_on_empty: bool,
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
    pub max_accept: u16,
    pub empty_max_accept: u16,
    pub complete_runs: bool,
    /// Foundations the solver may play to automatically when safe.
    pub autoplay: bool,
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
}

impl Group {
    pub fn piles(&self) -> std::ops::Range<u16> {
        self.start..self.start + self.count
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Stock {
    pub pile: u16,
    pub target: u16,
    pub target_kind: PileKind,
    pub count: u16,
    pub redeals: u32,
    pub require_no_empty: bool,
    pub auto: bool,
    pub only_when_empty: bool,
    pub first_empty: bool,
    pub skip_empty: bool,
    pub face_up: bool,
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
    /// Grid group, row length to check, first-column rank.
    Rows(u16, u16, u8),
    /// Group, most non-empty piles.
    AtMostPiles(u16, u16),
    /// At least this many points.
    Score(u32),
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
    pub(crate) hooks: Option<std::sync::Arc<dyn crate::hooks::Hooks>>,
    pub(crate) score: Option<crate::score::Score>,
}

impl Game {
    /// Check and compile rules.
    pub fn new(rules: Rules) -> Result<Game, RulesError> {
        let pack = &rules.pack;
        if pack.copies == 0 || pack.suits.is_empty() || pack.ranks.is_empty() {
            return err("the pack is empty");
        }
        let mut rank_idx = [255u8; 14];
        for (i, &r) in pack.ranks.iter().enumerate() {
            if !(1..=13).contains(&r) {
                return err(format!("rank {r} is not 1..=13"));
            }
            if rank_idx[r as usize] != 255 {
                return err(format!("rank {r} listed twice"));
            }
            rank_idx[r as usize] = i as u8;
        }
        let mut in_pack = [false; FACES];
        for &s in &pack.suits {
            for &r in &pack.ranks {
                in_pack[Card::new(s, r).face() as usize] = true;
            }
        }
        in_pack[JOKER as usize] = pack.jokers > 0;
        let mut rank_faces = [0u64; 14];
        let mut suit_faces = [0u64; 4];
        for f in 0..JOKER {
            let c = Card::from_face(f);
            rank_faces[c.rank() as usize] |= 1 << f;
            suit_faces[c.suit() as usize] |= 1 << f;
        }
        let nranks = pack.ranks.len() as u8;
        let total = pack.len();
        if total > u16::MAX as usize || pack.copies as usize * pack.suits.len() > 255 {
            return err("pack too large");
        }
        if rules.groups.len() > 64 {
            return err("at most 64 groups");
        }

        let find = |name: &str| -> Result<u16, RulesError> {
            rules
                .groups
                .iter()
                .position(|g| g.name == name)
                .map(|i| i as u16)
                .ok_or_else(|| RulesError(format!("no group named {name:?}")))
        };

        let stock_redeals_non_waste = rules.stock.as_ref().is_some_and(|sr| {
            (sr.redeals > 0 || sr.unlimited_redeals)
                && rules
                    .groups
                    .iter()
                    .any(|gr| gr.name == sr.to && gr.kind != PileKind::Waste)
        });
        let mut groups = Vec::new();
        let mut pile_group = Vec::new();
        let mut pile_rules = Vec::new();
        let mut start = 0u16;
        let first = pack.ranks[0];
        let last = *pack.ranks.last().unwrap();
        for (gi, g) in rules.groups.iter().enumerate() {
            if rules.groups[..gi].iter().any(|h| h.name == g.name) {
                return err(format!("two groups named {:?}", g.name));
            }
            if g.count == 0 {
                return err(format!("group {:?} has no piles", g.name));
            }
            if matches!(g.kind, PileKind::Stock | PileKind::Waste) && g.count != 1 {
                return err(format!(
                    "{:?}: stock and waste groups have exactly one pile",
                    g.name
                ));
            }
            if g.complete_runs && g.kind != PileKind::Foundation {
                return err(format!("{:?}: complete_runs is for foundations", g.name));
            }
            for o in &g.piles {
                if let Some(&i) = o.piles.iter().find(|&&i| i >= g.count) {
                    return err(format!(
                        "{:?}: override names pile {i}, which doesn't exist",
                        g.name
                    ));
                }
            }
            for i in 0..g.count {
                // Later overrides win over earlier ones; any override wins over the group.
                let mut build = g.build.clone();
                let mut empty = g.empty.clone();
                let mut moves = g.moves;
                let mut max_cards = g.max_cards;
                let mut max_accept = g.max_accept;
                let mut empty_max_accept = g.empty_max_accept;
                let mut suit = g.suit;
                let mut empty_suit = g.empty_suit;
                let mut color = g.color;
                for o in g.piles.iter().filter(|o| o.piles.contains(&i)) {
                    build = o.build.clone().or(build);
                    empty = o.empty.clone().or(empty);
                    moves = o.moves.or(moves);
                    max_cards = o.max_cards.or(max_cards);
                    max_accept = o.max_accept.or(max_accept);
                    empty_max_accept = o.empty_max_accept.or(empty_max_accept);
                    suit = o.suit.or(suit);
                    empty_suit = o.empty_suit.or(empty_suit);
                    color = o.color.or(color);
                }
                let build = build.unwrap_or_else(|| match g.kind {
                    PileKind::Foundation => Build {
                        by: BuildBy::Suit,
                        dir: Dir::Up,
                        ..Build::default()
                    },
                    _ => Build::default(),
                });
                if build.step == 0 || build.step >= nranks.max(2) {
                    return err(format!("{:?}: build step out of range", g.name));
                }
                let empty = empty.unwrap_or(match g.kind {
                    PileKind::Foundation => {
                        EmptyRule::Rank(if build.dir == Dir::Down { last } else { first })
                    }
                    PileKind::Tableau | PileKind::Cell => EmptyRule::Any,
                    _ => EmptyRule::None,
                });
                let moves = moves.unwrap_or(match g.kind {
                    PileKind::Foundation | PileKind::Stock => MoveUnit::None,
                    _ => MoveUnit::Top,
                });
                let max_cards = max_cards.unwrap_or(match g.kind {
                    PileKind::Cell => 1,
                    _ if g.complete_runs => nranks as u16,
                    _ => u16::MAX,
                });
                let max_accept = max_accept.unwrap_or(match g.kind {
                    PileKind::Foundation if g.complete_runs => nranks as u16,
                    PileKind::Foundation => 1,
                    _ => u16::MAX,
                });
                let mut admits = 0u64;
                for f in 0..JOKER {
                    let c = Card::from_face(f);
                    if suit.is_none_or(|s| c.suit() == s) && color.is_none_or(|k| c.color() == k) {
                        admits |= 1 << f;
                    }
                }
                // A joker can stand for a card of any suit.
                admits |= 1 << JOKER;
                // With a tableau redeal, which cards are still out decides how the rest are
                // regathered and dealt, so no foundation play is free of consequences. A joker
                // can stand on any card, so with jokers about no card is ever done with.
                // Competing foundations with different rules are filtered out after `fits` is
                // compiled below.
                let autoplay = rules.safe_autoplay
                    && rules.redeal.is_none()
                    && !stock_redeals_non_waste
                    && pack.jokers == 0
                    && g.kind == PileKind::Foundation
                    && !g.complete_runs
                    && !g.requires_rank_on_prev
                    && !g.match_longest_rank
                    && build.by == BuildBy::Suit
                    && matches!(build.dir, Dir::Up | Dir::Down)
                    && (!build.wrap || max_cards <= nranks as u16)
                    && build.blocked_ranks.is_empty()
                    && matches!(
                        empty,
                        EmptyRule::Rank(_) | EmptyRule::DealtBase | EmptyRule::DealtBaseOffset(_)
                    );
                pile_rules.push(PileRules {
                    build,
                    empty,
                    moves,
                    max_cards,
                    max_accept,
                    empty_max_accept: empty_max_accept.unwrap_or(max_accept),
                    complete_runs: g.complete_runs,
                    autoplay,
                    sink: moves == MoveUnit::None
                        && g.kind != PileKind::Stock
                        && !g.requires_rank_on_prev
                        && !g.match_longest_rank,
                    admits,
                    empty_suit,
                    fits: [0; FACES],
                    wild: g.wild.unwrap_or(g.kind != PileKind::Foundation),
                });
                pile_group.push(gi as u16);
            }
            let accepts_from = match &g.accepts_from {
                None => u64::MAX,
                Some(names) => {
                    let mut m = 0u64;
                    for n in names {
                        m |= 1 << find(n)?;
                    }
                    m
                }
            };
            let refill_from = g
                .refill_from
                .iter()
                .map(|n| find(n))
                .collect::<Result<Vec<_>, _>>()?;
            groups.push(Group {
                name: g.name.clone(),
                kind: g.kind,
                start,
                count: g.count,
                accepts_from,
                refill_from,
                symmetric: g.layout.is_none()
                    && g.piles.is_empty()
                    && g.reach.is_none()
                    && !g.compact
                    && !g.compact_face_up
                    && !g.requires_rank_on_prev,
                symmetric_when_no_stock: false,
                symmetric_when_no_redeal: false,
                sym_classes: Vec::new(),
                columns: match g.layout {
                    Some(Layout::Grid { columns }) => Some(columns),
                    _ => None,
                },
                hidden: false,
                reach: g.reach.clone(),
                compact: g.compact,
                requires_rank_on_prev: g.requires_rank_on_prev,
                match_longest_rank: g.match_longest_rank,
                accept_only_when_empty: g.accept_only_when_empty,
                accept_when_stock_empty: g.accept_when_stock_empty,
                compact_face_up: g.compact_face_up,
                refill_largest: g.refill_largest,
                refill_on_empty: g.refill_on_empty,
            });
            start = start
                .checked_add(g.count)
                .ok_or_else(|| RulesError("too many piles".into()))?;
        }
        // A redeal gathers cards on a hidden scratch pile.
        let redeal = match &rules.redeal {
            None => None,
            Some(rd) => {
                if rules.stock.is_some() {
                    return err("a game can't have both stock rules and a tableau redeal");
                }
                let gi = find(&rd.group)?;
                if groups[gi as usize].symmetric && groups[gi as usize].count > 1 {
                    groups[gi as usize].symmetric_when_no_redeal = true;
                }
                groups[gi as usize].symmetric = false;
                if rd.how == RedealHow::GridShuffle && groups[gi as usize].columns.is_none() {
                    return err("grid_shuffle redeals need a grid layout");
                }
                if let RedealHow::Regroup { per_pile: 0 } | RedealHow::Shuffle { per_pile: 0 } =
                    rd.how
                {
                    return err("redeal per_pile must be at least 1");
                }
                groups.push(Group {
                    name: "__gather".into(),
                    kind: PileKind::Reserve,
                    start,
                    count: 1,
                    accepts_from: 0,
                    refill_from: Vec::new(),
                    symmetric: true,
                    symmetric_when_no_stock: false,
                    symmetric_when_no_redeal: false,
                    sym_classes: Vec::new(),
                    columns: None,
                    hidden: true,
                    reach: None,
                    compact: false,
                    requires_rank_on_prev: false,
                    match_longest_rank: false,
                    accept_only_when_empty: false,
                    accept_when_stock_empty: false,
                    compact_face_up: false,
                    refill_largest: false,
                    refill_on_empty: false,
                });
                pile_rules.push(PileRules {
                    build: Build::default(),
                    empty: EmptyRule::None,
                    moves: MoveUnit::None,
                    max_cards: u16::MAX,
                    max_accept: 0,
                    empty_max_accept: 0,
                    complete_runs: false,
                    autoplay: false,
                    sink: false,
                    admits: 0,
                    empty_suit: None,
                    fits: [0; FACES],
                    wild: false,
                });
                pile_group.push(groups.len() as u16 - 1);
                start += 1;
                Some(Redeal {
                    group: gi,
                    times: if rd.unlimited { u32::MAX } else { rd.times },
                    how: rd.how.clone(),
                    gather: start - 1,
                    refill: match &rd.how {
                        RedealHow::Compact {
                            refill_from: Some(n),
                        } => Some(find(n)?),
                        _ => None,
                    },
                })
            }
        };
        let npiles = start as usize;

        // Covering layouts.
        let mut covered_by = vec![Vec::new(); npiles];
        let mut has_covers = false;
        for (gi, g) in rules.groups.iter().enumerate() {
            let Some(layout) = &g.layout else { continue };
            if let Layout::Grid { columns } = layout {
                if *columns == 0 || g.count % columns != 0 {
                    return err(format!("{:?}: a grid's piles must fill whole rows", g.name));
                }
                continue;
            }
            has_covers = true;
            let base = groups[gi].start;
            let local = layout_covers(layout, g.count)
                .map_err(|e| RulesError(format!("{:?}: {e}", g.name)))?;
            for (i, cov) in local.into_iter().enumerate() {
                covered_by[base as usize + i] = cov.into_iter().map(|c| base + c).collect();
            }
        }

        // Stock.
        let stock_groups: Vec<usize> = groups
            .iter()
            .enumerate()
            .filter(|(_, g)| g.kind == PileKind::Stock)
            .map(|(i, _)| i)
            .collect();
        if stock_groups.len() > 1 {
            return err("at most one stock group");
        }
        let stock = match (&rules.stock, stock_groups.first()) {
            (Some(sr), Some(&sg)) => {
                let target = find(&sr.to)?;
                let tk = groups[target as usize].kind;
                if matches!(tk, PileKind::Stock) {
                    return err("the stock can't deal to itself");
                }
                if sr.count == 0 {
                    return err("stock count must be at least 1");
                }
                if sr.skip_empty && (sr.require_no_empty || sr.first_empty) {
                    return err(
                        "stock skip_empty can't be combined with require_no_empty or first_empty",
                    );
                }
                // A group that receives one card per pile from the stock isn't symmetric while
                // the stock can still deal: which pile gets which card depends on its position.
                // Once the stock is empty (and cannot redeal), the group becomes symmetric.
                if tk != PileKind::Waste && tk != PileKind::Foundation {
                    if groups[target as usize].symmetric && groups[target as usize].count > 1 {
                        groups[target as usize].symmetric_when_no_stock = true;
                    }
                    groups[target as usize].symmetric = false;
                }
                Some(Stock {
                    pile: groups[sg].start,
                    target,
                    target_kind: tk,
                    count: sr.count,
                    redeals: if sr.unlimited_redeals {
                        u32::MAX
                    } else {
                        sr.redeals
                    },
                    require_no_empty: sr.require_no_empty,
                    auto: sr.auto,
                    only_when_empty: sr.only_when_empty,
                    first_empty: sr.first_empty,
                    skip_empty: sr.skip_empty,
                    face_up: sr.face_up,
                })
            }
            (None, _) => None,
            (Some(_), None) => return err("stock rules given but no stock group"),
        };
        let stock_pile = stock_groups.first().map(|&sg| groups[sg].start);
        if let Some(s) = &stock {
            if s.redeals > 0 && s.target_kind == PileKind::Foundation {
                return err("redeals can't gather from a foundation group");
            }
        }
        // Groups that refill others are read from their first non-empty pile, so their order
        // matters.
        let mut is_refill_source = vec![false; groups.len()];
        for i in 0..groups.len() {
            for r in groups[i].refill_from.clone() {
                if groups[r as usize].count > 1 {
                    groups[r as usize].symmetric = false;
                    groups[r as usize].symmetric_when_no_stock = false;
                    groups[r as usize].symmetric_when_no_redeal = false;
                    is_refill_source[r as usize] = true;
                }
            }
        }

        let mut removal = Vec::new();
        for r in &rules.removal {
            let mut from = 0u64;
            for n in &r.from {
                from |= 1 << find(n)?;
            }
            let adjacent = match r.adjacent {
                None => None,
                Some(a) => {
                    let [name] = r.from.as_slice() else {
                        return err("adjacent removal takes cards from exactly one grid group");
                    };
                    let gi = find(name)?;
                    if groups[gi as usize].columns.is_none() {
                        return err(format!(
                            "adjacent removal needs {name:?} to have a grid layout"
                        ));
                    }
                    Some((a, gi))
                }
            };
            removal.push(Removal {
                kind: r.kind.clone(),
                from,
                to: groups[find(&r.to)? as usize].start,
                adjacent,
            });
        }

        let win = match &rules.win {
            WinRule::AllOnFoundations => {
                if !groups.iter().any(|g| g.kind == PileKind::Foundation) {
                    return err("win is all_on_foundations but there are no foundations");
                }
                Win::AllOnFoundations
            }
            WinRule::Empty(names) => {
                Win::Empty(names.iter().map(|n| find(n)).collect::<Result<_, _>>()?)
            }
            WinRule::Count { group, cards } => Win::Count(find(group)?, *cards),
            WinRule::AtMostPiles { group, piles } => Win::AtMostPiles(find(group)?, *piles),
            WinRule::Rows { group, length } => {
                let gi = find(group)?;
                let g = &groups[gi as usize];
                let cols = g.columns.ok_or_else(|| {
                    RulesError(format!("win by rows needs {group:?} to have a grid layout"))
                })?;
                let EmptyRule::AfterLeft { first } = pile_rules[g.start as usize].empty else {
                    return err(format!(
                        "win by rows needs {group:?} to use the after_left empty rule"
                    ));
                };
                let len = length.unwrap_or(cols - 1);
                if len > cols {
                    return err("rows win: length longer than a row");
                }
                Win::Rows(gi, len, first)
            }
            WinRule::Score { at_least } => {
                if rules.score.is_none() {
                    return err("win by score needs a score rule");
                }
                Win::Score(*at_least)
            }
        };

        let waste = groups
            .iter()
            .find(|g| g.kind == PileKind::Waste)
            .map(|g| g.start);
        let compress_talon = match (&stock, waste) {
            (Some(s), Some(w))
                if s.target_kind == PileKind::Waste
                    && !s.face_up
                    && !groups.iter().any(|g| g.accept_when_stock_empty) =>
            {
                let wg = &groups[s.target as usize];
                let sg = pile_group[s.pile as usize];
                debug_assert_eq!(w, wg.start);
                let stock_playable = pile_rules[s.pile as usize].moves != MoveUnit::None
                    || removal.iter().any(|r| r.from & (1 << sg) != 0);
                !stock_playable
                    && (pile_rules[w as usize].moves != MoveUnit::None
                        || removal.iter().any(|r| r.from & (1 << s.target) != 0))
            }
            _ => false,
        };

        let simple_settle = !groups
            .iter()
            .any(|g| g.compact || g.compact_face_up || !g.refill_from.is_empty())
            && !stock.as_ref().is_some_and(|st| st.auto);
        let copies = pack.copies;
        let jokers = pack.jokers;
        let mut game = Game {
            rules,
            groups,
            pile_group,
            pile_rules,
            covered_by,
            has_covers,
            stock,
            stock_pile,
            redeal,
            removal,
            win,
            rank_idx,
            nranks,
            copies,
            total_cards: total as u16,
            jokers,
            in_pack,
            rank_faces,
            suit_faces,
            dependents: vec![Vec::new(); FACES],
            any_autoplay: false,
            compress_talon,
            simple_settle,
            movable_piles: Vec::new(),
            targets_for_group: Vec::new(),
            has_supermove: false,
            pile_sym_class: vec![u16::MAX; npiles],
            num_sym_classes: 0,
            force_lower_of_suit: false,
            waste,
            hooks: None,
            score: None,
        };
        game.score = game.compile_score()?;
        if let Some(sc) = &game.score {
            // Every card of a scored pile counts, not just its top; and a foundation play can
            // take a card some line wanted.
            for p in game.groups[sc.group as usize].piles() {
                game.pile_rules[p as usize].sink = false;
            }
            for r in &mut game.pile_rules {
                r.autoplay = false;
            }
        }

        for p in 0..game.pile_rules.len() {
            let mut t = [0u64; FACES];
            let pr = &game.pile_rules[p];
            for (u, row) in t.iter_mut().enumerate() {
                for o in 0..FACES {
                    if pr.admits >> o & 1 != 0
                        && game.fits(
                            &pr.build,
                            Card::from_face(u as u8),
                            Card::from_face(o as u8),
                        )
                    {
                        *row |= 1 << o;
                    }
                }
            }
            game.pile_rules[p].fits = t;
        }

        // Disable autoplay on any foundation pile that can compete for the same faces with
        // another foundation pile having a different build/empty/admits rule (e.g. an Up and a
        // Down foundation of the same suit whose reachable rank sets overlap).
        let foundation_reachable = |p: usize| -> u64 {
            let pr = &game.pile_rules[p];
            if pr.max_accept == 0 || pr.max_cards == 0 {
                return 0;
            }
            let real = pr.admits & REAL_FACES;
            let start_mask = match &pr.empty {
                EmptyRule::None => 0,
                EmptyRule::Any | EmptyRule::DealtBase | EmptyRule::DealtBaseOffset(_) => {
                    return real;
                }
                EmptyRule::Rank(r) => real & game.rank_faces[*r as usize],
                EmptyRule::Ranks(rs) => rs
                    .iter()
                    .fold(0u64, |m, &r| m | (real & game.rank_faces[r as usize])),
                EmptyRule::AfterLeft { .. } => return real,
            };
            let mut cur = start_mask;
            if let Some(es) = pr.empty_suit {
                cur &= game.suit_faces[es as usize];
            }
            let mut seen = cur;
            let steps = (pr.max_cards as usize)
                .saturating_sub(1)
                .min(game.nranks as usize);
            for _ in 0..steps {
                let mut next = 0u64;
                let mut m = cur;
                while m != 0 {
                    let f = m.trailing_zeros() as usize;
                    next |= pr.fits[f] & REAL_FACES;
                    m &= m - 1;
                }
                cur = next & !seen;
                seen |= next;
                if cur == 0 {
                    break;
                }
            }
            seen
        };
        let found_piles: Vec<usize> = (0..game.num_piles())
            .filter(|&p| game.pile_kind(p as u16) == PileKind::Foundation)
            .collect();
        let found_reach: Vec<u64> = found_piles
            .iter()
            .map(|&p| foundation_reachable(p))
            .collect();
        let same_found_rule = |a: &PileRules, b: &PileRules| -> bool {
            a.build == b.build
                && a.empty == b.empty
                && a.empty_suit == b.empty_suit
                && a.max_cards == b.max_cards
                && a.admits == b.admits
        };
        for (i, &p) in found_piles.iter().enumerate() {
            if !game.pile_rules[p].autoplay {
                continue;
            }
            let rp = found_reach[i];
            let conflicts = found_piles.iter().enumerate().any(|(j, &q)| {
                i != j
                    && (rp & found_reach[j]) != 0
                    && !same_found_rule(&game.pile_rules[p], &game.pile_rules[q])
            });
            if conflicts {
                game.pile_rules[p].autoplay = false;
            }
        }

        // Which faces can be built on which, outside foundations: used to decide when playing
        // a card to a foundation can never hurt. Unrestricted dump piles (any card on any card,
        // empty accepts any, no card cap) never create a reason to hold a card off a foundation.
        let mut all_real_in_pack = 0u64;
        for (f, &present) in game.in_pack.iter().enumerate().take(JOKER as usize) {
            if present {
                all_real_in_pack |= 1 << f;
            }
        }
        let is_dump_pile: Vec<bool> = (0..game.num_piles())
            .map(|p| {
                let pr = &game.pile_rules[p];
                let gr = &game.groups[game.pile_group[p] as usize];
                pr.build.by == BuildBy::Any
                    && pr.build.dir == Dir::Any
                    && pr.build.blocked_ranks.is_empty()
                    && pr.empty == EmptyRule::Any
                    && pr.empty_suit.is_none()
                    && pr.max_cards == u16::MAX
                    && pr.empty_max_accept >= pr.max_accept
                    && !gr.accept_only_when_empty
                    && !gr.requires_rank_on_prev
                    && !gr.match_longest_rank
                    && (pr.admits & all_real_in_pack) == all_real_in_pack
            })
            .collect();
        game.any_autoplay = game.pile_rules.iter().any(|r| r.autoplay);
        for f in 0..FACES {
            for o in 0..FACES {
                if !game.in_pack[f] || !game.in_pack[o] {
                    continue;
                }
                let dep = (0..game.num_piles()).any(|p| {
                    game.pile_kind(p as u16) != PileKind::Foundation
                        && !is_dump_pile[p]
                        && game.pile_rules[p].fits[f] >> o & 1 != 0
                });
                if dep {
                    game.dependents[f].push(o as u8);
                }
            }
        }

        // Equivalence-class symmetry for groups that are non-uniform only because of per-pile
        // overrides (e.g. foundations with both up and down sub-sets, or paired suit piles).
        let same_pile_rules = |a: &PileRules, b: &PileRules| -> bool {
            a.build == b.build
                && a.empty == b.empty
                && a.moves == b.moves
                && a.max_cards == b.max_cards
                && a.max_accept == b.max_accept
                && a.empty_max_accept == b.empty_max_accept
                && a.complete_runs == b.complete_runs
                && a.admits == b.admits
                && a.empty_suit == b.empty_suit
                && a.wild == b.wild
        };
        let mut next_sym_class = 0u16;
        for gi in 0..game.groups.len() {
            let gr = &game.groups[gi];
            if gr.symmetric
                || gr.symmetric_when_no_stock
                || gr.symmetric_when_no_redeal
                || gr.count <= 1
                || gr.hidden
                || gr.columns.is_some()
                || gr.reach.is_some()
                || gr.compact
                || gr.compact_face_up
                || gr.requires_rank_on_prev
                || is_refill_source[gi]
                || game
                    .redeal
                    .as_ref()
                    .is_some_and(|rd| rd.group as usize == gi)
                || game.stock.as_ref().is_some_and(|st| {
                    st.target as usize == gi
                        && st.target_kind != PileKind::Waste
                        && st.target_kind != PileKind::Foundation
                })
                || gr.piles().any(|p| !game.covered_by[p as usize].is_empty())
            {
                continue;
            }
            let mut classes: Vec<Vec<u16>> = Vec::new();
            for p in gr.piles() {
                if let Some(cls) = classes.iter_mut().find(|c| {
                    same_pile_rules(
                        &game.pile_rules[c[0] as usize],
                        &game.pile_rules[p as usize],
                    )
                }) {
                    cls.push(p);
                } else {
                    classes.push(vec![p]);
                }
            }
            if classes.iter().any(|c| c.len() > 1) {
                for cls in &classes {
                    if cls.len() > 1 && (next_sym_class as usize) < 64 {
                        for &p in cls {
                            game.pile_sym_class[p as usize] = next_sym_class;
                        }
                        next_sym_class += 1;
                    }
                }
                game.groups[gi].sym_classes = classes;
            }
        }
        game.num_sym_classes = next_sym_class as usize;

        // `RemovalKind::LowerOfSuit` is soundly forced when nothing can build on non-foundation
        // piles, there is only one removal rule, no redeal/jokers/score, and emptying a pile
        // cannot block or alter a stock deal or trigger a refill/compaction.
        game.force_lower_of_suit = game.rules.safe_autoplay
            && game.jokers == 0
            && game.redeal.is_none()
            && game.score.is_none()
            && game.removal.len() == 1
            && matches!(game.removal[0].kind, RemovalKind::LowerOfSuit)
            && game
                .stock
                .as_ref()
                .is_none_or(|st| !st.require_no_empty && !st.skip_empty && !st.first_empty)
            && game.groups.iter().all(|gr| {
                gr.kind == PileKind::Foundation
                    || (!gr.compact
                        && !gr.compact_face_up
                        && gr.refill_from.is_empty()
                        && gr
                            .piles()
                            .all(|p| game.pile_rules[p as usize].build.by == BuildBy::None))
            });

        // Precompute movable sources and valid target piles per source group for fast move-gen.
        game.has_supermove = game
            .pile_rules
            .iter()
            .any(|r| r.moves == MoveUnit::Supermove);
        game.movable_piles = (0..game.num_piles() as u16)
            .filter(|&p| game.pile_rules[p as usize].moves != MoveUnit::None)
            .collect();
        let ngroups = game.groups.len();
        let npiles = game.num_piles() as u16;
        game.targets_for_group = (0..ngroups)
            .map(|fg| {
                let from_kind = game.groups[fg].kind;
                (0..npiles)
                    .filter(|&to| {
                        let tgi = game.pile_group[to as usize] as usize;
                        let tg = &game.groups[tgi];
                        let tr = &game.pile_rules[to as usize];
                        if tg.kind == PileKind::Stock || tg.accepts_from & (1 << fg) == 0 {
                            return false;
                        }
                        if from_kind == PileKind::Foundation && tg.kind == PileKind::Foundation {
                            return false;
                        }
                        if tg.reach.is_some() && tgi != fg {
                            return false;
                        }
                        tr.complete_runs
                            || tr.empty != EmptyRule::None
                            || tr.build.by != BuildBy::None
                    })
                    .collect()
            })
            .collect();

        // Validate the deal by dealing once.
        game.try_deal(0)?;
        Ok(game)
    }

    /// Compile rules with [`crate::hooks::Hooks`] for the parts data can't say.
    pub fn with_hooks(
        rules: Rules,
        hooks: std::sync::Arc<dyn crate::hooks::Hooks>,
    ) -> Result<Game, RulesError> {
        let mut g = Game::new(rules)?;
        if !hooks.keeps_pruning_sound() {
            for gr in &mut g.groups {
                gr.symmetric = false;
                gr.symmetric_when_no_stock = false;
                gr.symmetric_when_no_redeal = false;
                gr.sym_classes.clear();
            }
            g.pile_sym_class.fill(u16::MAX);
            g.num_sym_classes = 0;
            for r in &mut g.pile_rules {
                r.autoplay = false;
            }
            g.any_autoplay = false;
            g.force_lower_of_suit = false;
            g.compress_talon = false;
            if let Some(sc) = &mut g.score {
                sc.symmetric = false;
                sc.pool = crate::score::Pool::Unbounded;
            }
        }
        g.hooks = Some(hooks);
        g.simple_settle = false;
        // Re-validate: hooks may change what the opening position does.
        g.try_deal(0)?;
        Ok(g)
    }

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

    fn compile_score(&self) -> Result<Option<crate::score::Score>, RulesError> {
        use crate::score::{points_order, points_table, HandScoring, Pool, Score, StarterBound};
        let Some(sr) = &self.rules.score else {
            return Ok(None);
        };
        let rules = &self.rules;
        let find = |n: &str| {
            self.groups
                .iter()
                .position(|g| !g.hidden && g.name == n)
                .map(|i| i as u16)
                .ok_or_else(|| RulesError(format!("no group named {n:?}")))
        };
        let gi = find(&sr.group)?;
        let g = &self.groups[gi as usize];
        let Some(columns) = g.columns else {
            return err(format!("score: {:?} needs a grid layout", sr.group));
        };
        let rows = g.count / columns;
        let mut lines = Vec::new();
        for r in 0..rows {
            lines.push((0..columns).map(|c| g.start + r * columns + c).collect());
        }
        for c in 0..columns {
            lines.push((0..rows).map(|r| g.start + r * columns + c).collect());
        }
        let bit = 1u64 << gi;
        let leaves = g
            .piles()
            .any(|p| self.pile_rules[p as usize].moves != MoveUnit::None)
            || self.removal.iter().any(|r| r.from & bit != 0)
            || self.redeal.as_ref().is_some_and(|rd| rd.group == gi)
            || g.compact
            || self.stock.as_ref().is_some_and(|st| st.target == gi);
        let pool = if leaves {
            Pool::Unbounded
        } else {
            match (&self.stock, self.waste) {
                (Some(st), Some(w))
                    if st.target_kind == PileKind::Waste
                        && st.count == 1
                        && st.only_when_empty
                        && st.redeals == 0
                        && g.accepts_from == 1 << self.pile_group[w as usize]
                        && self
                            .removal
                            .iter()
                            .all(|r| r.from >> self.pile_group[w as usize] & 1 == 0)
                        && self.groups.iter().enumerate().all(|(j, o)| {
                            let wg = self.pile_group[w as usize] as usize;
                            let sg = self.pile_group[st.pile as usize] as usize;
                            j == gi as usize
                                || j == wg
                                || j == sg
                                || (o.accepts_from >> wg & 1 == 0
                                    && !o
                                        .refill_from
                                        .iter()
                                        .any(|&r| r as usize == wg || r as usize == sg))
                        }) =>
                {
                    Pool::NextFromStock {
                        stock: st.pile,
                        waste: w,
                    }
                }
                _ => Pool::Outside,
            }
        };
        let rg = &rules.groups[gi as usize];
        let symmetric = rg.piles.is_empty()
            && !matches!(
                self.pile_rules[g.start as usize].empty,
                EmptyRule::AfterLeft { .. }
            )
            && g.reach.is_none()
            && !g.compact
            && !g.compact_face_up
            && g.refill_from.is_empty()
            && !self.groups.iter().any(|o| o.refill_from.contains(&gi))
            && self.removal.iter().all(|r| r.from & bit == 0)
            && self.redeal.as_ref().is_none_or(|rd| rd.group != gi)
            && self.stock.as_ref().is_none_or(|st| st.target != gi)
            && !matches!(self.win, Win::Rows(wg, ..) if wg == gi);
        let hands = match &sr.hands {
            crate::rules::Hands::Poker(table) => {
                let points = points_table(&table.points());
                HandScoring::Poker {
                    order: points_order(&points),
                    monotone: points.windows(2).all(|w| w[0] <= w[1]),
                    points,
                }
            }
            crate::rules::Hands::Cribbage(cr) => {
                let starter = match &cr.starter {
                    None => None,
                    Some(n) => {
                        let si = find(n)?;
                        if si == gi {
                            return err("score: the starter can't come from the scored grid");
                        }
                        Some(self.groups[si as usize].start)
                    }
                };
                let starter_bound = match starter {
                    None => StarterBound::None,
                    Some(p) if matches!(pool, Pool::NextFromStock { waste, .. } if waste == p) => {
                        StarterBound::NextInSequence
                    }
                    Some(p) if self.frozen(p) => StarterBound::Fixed(p),
                    Some(_) => StarterBound::Unknown,
                };
                HandScoring::Cribbage {
                    // A joker can be a fifth card of a rank.
                    best: std::sync::Arc::new(crate::score::best_rank_points(
                        &cr.points,
                        if self.jokers > 0 {
                            u8::MAX
                        } else {
                            self.copies.saturating_mul(rules.pack.suits.len() as u8)
                        },
                    )),
                    points: cr.points.clone(),
                    starter,
                    starter_bound,
                }
            }
        };
        Ok(Some(Score {
            group: gi,
            lines,
            hands,
            pool,
            symmetric,
            rows,
            columns,
        }))
    }

    /// Nothing ever enters or leaves this pile: no move, deal, refill, redeal or removal.
    fn frozen(&self, p: u16) -> bool {
        let gi = self.pile_group[p as usize];
        let g = &self.groups[gi as usize];
        let r = self.rule(p);
        let takes = r.max_accept > 0
            && (r.build.by != BuildBy::None || r.empty != EmptyRule::None)
            && g.accepts_from != 0;
        r.moves == MoveUnit::None
            && !takes
            && !g.compact
            && g.kind != PileKind::Stock
            && self.stock.as_ref().is_none_or(|st| st.target != gi)
            && self.redeal.as_ref().is_none_or(|rd| rd.group != gi)
            && !self.groups.iter().any(|o| o.refill_from.contains(&gi))
            && g.refill_from.is_empty()
            && self
                .removal
                .iter()
                .all(|rm| rm.from >> gi & 1 == 0 && rm.to != p)
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

    // ----- Card relations -------------------------------------------------------------------

    /// Can `over` be placed on `under` under build rule `b`?
    pub(crate) fn fits(&self, b: &Build, under: Card, over: Card) -> bool {
        let ok_suit = match b.by {
            BuildBy::None => return false,
            BuildBy::Any => true,
            BuildBy::Suit => under.suit() == over.suit(),
            BuildBy::Alternate => under.color() != over.color(),
            BuildBy::SameColor => under.color() == over.color(),
            BuildBy::OtherSuit => under.suit() != over.suit(),
            BuildBy::SuitOrRank => under.suit() == over.suit() || under.rank() == over.rank(),
        };
        if !ok_suit || b.blocked_ranks.contains(&under.rank()) {
            return false;
        }
        let (u, o) = (
            self.rank_idx[under.rank() as usize],
            self.rank_idx[over.rank() as usize],
        );
        if u == 255 || o == 255 {
            return false;
        }
        let n = self.nranks as i16;
        let step = b.step as i16;
        let up = |from: u8, to: u8| -> bool {
            let d = to as i16 - from as i16;
            d == step || (b.wrap && d == step - n)
        };
        match b.dir {
            Dir::Up => up(u, o),
            Dir::Down => up(o, u),
            Dir::Either => up(u, o) || up(o, u),
            Dir::Same => u == o,
            Dir::Any => true,
        }
    }

    #[inline]
    pub(crate) fn rule(&self, pile: u16) -> &PileRules {
        &self.pile_rules[pile as usize]
    }

    pub(crate) fn empty_accepts(&self, pile: u16, card: Card, s: &State) -> bool {
        let r = self.rule(pile);
        if r.admits >> card.face() & 1 == 0 {
            return false;
        }
        if card.is_joker() {
            return (r.empty == EmptyRule::Any && r.empty_suit.is_none())
                || (r.wild && self.empty_set(pile, s) != 0);
        }
        if let Some(es) = r.empty_suit {
            if card.suit() != es {
                return false;
            }
        }
        match &r.empty {
            EmptyRule::None => false,
            EmptyRule::Any => true,
            EmptyRule::Rank(r) => card.rank() == *r,
            EmptyRule::Ranks(ranks) => ranks.contains(&card.rank()),
            EmptyRule::DealtBase => s.base_rank.is_none_or(|b| card.rank() == b),
            EmptyRule::DealtBaseOffset(o) => s
                .base_rank
                .is_none_or(|b| card.rank() == self.rank_from(b, *o)),
            EmptyRule::AfterLeft { .. } => self.empty_set(pile, s) >> card.face() & 1 != 0,
        }
    }

    /// The rank `offset` places after `base` in the pack's sequence of ranks, going round.
    pub(crate) fn rank_from(&self, base: u8, offset: i8) -> u8 {
        let n = self.nranks as i16;
        let at = self.rank_idx[base as usize];
        if at == 255 {
            return base;
        }
        let to = (at as i16 + offset as i16).rem_euclid(n);
        self.rules.pack.ranks[to as usize]
    }

    /// The real faces an empty `pile` would accept.
    pub(crate) fn empty_set(&self, pile: u16, s: &State) -> u64 {
        let r = self.rule(pile);
        let mut real = r.admits & REAL_FACES;
        if let Some(es) = r.empty_suit {
            real &= self.suit_faces[es as usize];
        }
        match &r.empty {
            EmptyRule::None => 0,
            EmptyRule::Any => real,
            EmptyRule::Rank(k) => real & self.rank_faces[*k as usize],
            EmptyRule::Ranks(ranks) => ranks
                .iter()
                .fold(0u64, |m, &k| m | (real & self.rank_faces[k as usize])),
            EmptyRule::DealtBase => s
                .base_rank
                .map_or(real, |b| real & self.rank_faces[b as usize]),
            EmptyRule::DealtBaseOffset(o) => s.base_rank.map_or(real, |b| {
                real & self.rank_faces[self.rank_from(b, *o) as usize]
            }),
            EmptyRule::AfterLeft { first } => {
                let first = *first;
                let g = self.group_of(pile);
                let cols = g.columns.unwrap_or(g.count);
                if (pile - g.start) % cols == 0 {
                    return real & self.rank_faces[first as usize];
                }
                let left = &s.piles[pile as usize - 1];
                if left.is_empty() {
                    return 0;
                }
                // Only a left pile of nothing but jokers needs to know what it was built from.
                let base = if left.iter().all(|c| c.is_joker()) {
                    self.empty_set(pile - 1, s)
                } else {
                    0
                };
                real & self.accept_set(r, left, base, false)
            }
        }
    }

    /// Real faces that may be built, under rule `r`, on a card that could be any face in `set`.
    /// With `suit_only`, only cards of the same suit (Spider's runs).
    #[inline]
    pub(crate) fn step_set(&self, r: &PileRules, set: u64, suit_only: bool) -> u64 {
        let mut out = 0u64;
        let mut m = set & REAL_FACES;
        while m != 0 {
            let f = m.trailing_zeros() as usize;
            let mut fit = r.fits[f];
            if suit_only {
                fit &= self.suit_faces[f / 13];
            }
            out |= fit;
            m &= m - 1;
        }
        out & REAL_FACES
    }

    /// Real faces that may be built, under rule `r`, on top of `cards`, where an empty pile
    /// would take `base`. A joker on top stands for any card the cards beneath allow; one on a
    /// face-down card, for any card at all.
    pub(crate) fn accept_set(
        &self,
        r: &PileRules,
        cards: &[Card],
        base: u64,
        suit_only: bool,
    ) -> u64 {
        let Some(&top) = cards.last() else {
            return base;
        };
        if !top.is_face_up() {
            return 0;
        }
        if !top.is_joker() {
            let f = top.face() as usize;
            let mut fit = r.fits[f];
            if suit_only {
                fit &= self.suit_faces[f / 13];
            }
            return fit & REAL_FACES;
        }
        if !r.wild {
            return 0;
        }
        let k = cards
            .iter()
            .rev()
            .take_while(|c| c.is_joker() && c.is_face_up())
            .count();
        let below = cards.len() - k;
        // What the lowest joker of the top run of jokers can stand for.
        let mut id = if below == 0 {
            base
        } else {
            let c = cards[below - 1];
            if c.is_face_up() {
                self.step_set(r, 1 << c.face(), suit_only)
            } else {
                r.admits & REAL_FACES
            }
        };
        for _ in 1..k {
            id = self.step_set(r, id, suit_only);
        }
        self.step_set(r, id, suit_only)
    }

    /// Could a unit (bottom card first) be placed where `allowed` real faces may go, under
    /// rule `r`? `empty_any` says the target is an empty pile taking any card. A built unit
    /// (`run`) must stay built through any jokers at its bottom, whatever they now stand for.
    fn unit_fits(
        &self,
        r: &PileRules,
        allowed: u64,
        empty_any: bool,
        unit: &[Card],
        run: bool,
    ) -> bool {
        let bottom = unit[0];
        if !bottom.is_joker() {
            return allowed >> bottom.face() & 1 != 0;
        }
        if r.admits >> JOKER & 1 == 0 {
            return false;
        }
        if !r.wild {
            // Nothing can be built on a joker that isn't wild.
            return empty_any && (unit.len() == 1 || !run);
        }
        let mut id = allowed;
        if id == 0 && !empty_any {
            return false;
        }
        if empty_any && id == 0 {
            id = r.admits & REAL_FACES;
        }
        if !run {
            return true;
        }
        for &c in &unit[1..] {
            let next = self.step_set(r, id, false);
            if !c.is_joker() {
                return next >> c.face() & 1 != 0;
            }
            if next == 0 {
                return false;
            }
            id = next;
        }
        true
    }

    /// Every pile covering this one is empty.
    #[inline]
    pub(crate) fn available(&self, s: &State, pile: u16) -> bool {
        !self.has_covers
            || self.covered_by[pile as usize]
                .iter()
                .all(|&c| s.piles[c as usize].is_empty())
    }

    /// How many cards from the top of `pile` form a unit that may move together (0 if none).
    pub(crate) fn movable(&self, s: &State, pile: u16) -> usize {
        let cards = &s.piles[pile as usize];
        let Some(&top) = cards.last() else { return 0 };
        if !top.is_face_up() {
            return 0;
        }
        if let Some(n) = self.hooks.as_ref().and_then(|h| h.movable(self, s, pile)) {
            return n.min(cards.len());
        }
        let g = self.rule(pile);
        match g.moves {
            MoveUnit::None => 0,
            MoveUnit::Top => 1,
            MoveUnit::Whole => {
                if cards.iter().all(|c| c.is_face_up()) {
                    cards.len()
                } else {
                    0
                }
            }
            MoveUnit::AnyFaceUp => cards.iter().rev().take_while(|c| c.is_face_up()).count(),
            MoveUnit::Run | MoveUnit::Supermove | MoveUnit::SuitRun | MoveUnit::TopOrWhole
                if self.jokers > 0 =>
            {
                let n = self.movable_run_wild(g, cards, g.moves == MoveUnit::SuitRun);
                if g.moves == MoveUnit::TopOrWhole && n != cards.len() {
                    1
                } else {
                    n
                }
            }
            MoveUnit::Run | MoveUnit::Supermove | MoveUnit::SuitRun | MoveUnit::TopOrWhole => {
                let suit_only = g.moves == MoveUnit::SuitRun;
                let mut n = 1;
                while n < cards.len() {
                    let (under, over) = (cards[cards.len() - n - 1], cards[cards.len() - n]);
                    if !under.is_face_up()
                        || g.fits[under.face() as usize] >> over.face() & 1 == 0
                        || (suit_only && under.suit() != over.suit())
                    {
                        break;
                    }
                    n += 1;
                }
                if g.moves == MoveUnit::TopOrWhole && n != cards.len() {
                    1
                } else {
                    n
                }
            }
        }
    }

    /// The longest built run on top of `cards` when jokers may be among them. Between two real
    /// cards, the jokers must be able to stand for a built sequence joining them; jokers at
    /// the bottom of the run for one leading up to the cards above.
    fn movable_run_wild(&self, r: &PileRules, cards: &[Card], suit_only: bool) -> usize {
        let len = cards.len();
        let top = cards[len - 1];
        let steps = |mut set: u64, k: usize| {
            for _ in 0..k {
                set = self.step_set(r, set, suit_only);
            }
            set
        };
        let any = r.admits & REAL_FACES;
        // The lowest real card of the run so far, and the jokers beneath it in the run.
        let (mut b, mut j) = if top.is_joker() {
            if !r.wild {
                return 1;
            }
            (None, 1usize)
        } else {
            (Some(top.face()), 0usize)
        };
        let mut n = 1;
        while n < len {
            let under = cards[len - n - 1];
            if !under.is_face_up() {
                break;
            }
            let ok = if under.is_joker() {
                r.wild
                    && match b {
                        Some(bf) => steps(any, j + 1) >> bf & 1 != 0,
                        None => steps(any, j) != 0,
                    }
            } else {
                let a = 1u64 << under.face();
                match b {
                    Some(bf) => steps(a, j + 1) >> bf & 1 != 0,
                    None => steps(a, j) != 0,
                }
            };
            if !ok {
                break;
            }
            if under.is_joker() {
                j += 1;
            } else {
                b = Some(under.face());
                j = 0;
            }
            n += 1;
        }
        n
    }

    /// Largest unit a supermove from `from` to `to` may carry.
    fn supermove_limit(&self, s: &State, from: u16, to: u16) -> usize {
        let fg = self.pile_group[from as usize];
        let mut cells = 0usize;
        let mut empties = 0u32;
        for (p, pile) in s.piles.iter().enumerate() {
            if !pile.is_empty() || p as u16 == to || p as u16 == from {
                continue;
            }
            if self.group_of(p as u16).kind == PileKind::Cell {
                cells += 1;
            } else if self.pile_group[p] == fg && self.pile_rules[p].empty == EmptyRule::Any {
                empties += 1;
            }
        }
        (cells + 1) << empties.min(16)
    }

    /// Is the top `n` cards of `cards` a complete sequence matching `by`, ready for a complete-run foundation?
    fn is_complete_run(&self, cards: &[Card], n: usize, by: BuildBy, wrap: bool) -> bool {
        if n != self.nranks as usize || cards.len() < n {
            return false;
        }
        let run = &cards[cards.len() - n..];
        let b = Build {
            by,
            dir: Dir::Down,
            wrap,
            ..Build::default()
        };
        let b_up = Build {
            by,
            dir: Dir::Up,
            wrap,
            ..Build::default()
        };
        let all = |b: &Build| {
            run.windows(2)
                .all(|w| w[0].is_face_up() && self.fits(b, w[0], w[1]))
        };
        run[0].is_face_up() && run[n - 1].is_face_up() && (all(&b) || all(&b_up))
    }

    // ----- Moves ----------------------------------------------------------------------------

    /// Could `n` cards from the top of `from` go to `to`? Doesn't check the unit is movable.
    pub(crate) fn can_place(&self, s: &State, from: u16, to: u16, n: usize) -> bool {
        self.can_place_with_limit(s, from, to, n, None)
    }

    fn can_place_with_limit(
        &self,
        s: &State,
        from: u16,
        to: u16,
        n: usize,
        supermove_cap: Option<usize>,
    ) -> bool {
        if from == to {
            return false;
        }
        if let Some(h) = &self.hooks {
            let ok_shape = n > 0
                && n <= s.piles[from as usize].len()
                && self.available(s, to)
                && self.group_of(to).kind != PileKind::Stock;
            if ok_shape {
                if let Some(b) = h.accepts(self, s, from, to, n) {
                    return b;
                }
            }
        }
        let tg = self.group_of(to);
        let tr = self.rule(to);
        let fgi = self.pile_group[from as usize];
        if tg.accepts_from & (1 << fgi) == 0 || matches!(tg.kind, PileKind::Stock) {
            return false;
        }
        if let Some(reach) = &tg.reach {
            let same = self.pile_group[to as usize] as usize == fgi as usize;
            if !same || from < to || !reach.contains(&(from - to)) {
                return false;
            }
        }
        let src = &s.piles[from as usize];
        let dst = &s.piles[to as usize];
        if tg.accept_only_when_empty && !dst.is_empty() {
            return false;
        }
        if tg.accept_when_stock_empty
            && self
                .stock_pile
                .is_some_and(|sp| !s.piles[sp as usize].is_empty())
        {
            return false;
        }
        if n == 0
            || n > src.len()
            || n > tr.max_accept as usize
            || (dst.is_empty() && n > tr.empty_max_accept as usize)
            || dst.len() + n > tr.max_cards as usize
        {
            return false;
        }
        if !self.available(s, to) {
            return false;
        }
        let card = src[src.len() - n];
        if tr.complete_runs {
            return dst.is_empty() && self.is_complete_run(src, n, tr.build.by, tr.build.wrap);
        }
        let ok = if self.jokers == 0 {
            match dst.last() {
                None => self.empty_accepts(to, card, s),
                Some(&top) => {
                    top.is_face_up() && tr.fits[top.face() as usize] >> card.face() & 1 != 0
                }
            }
        } else {
            let base = if dst.iter().all(|c| c.is_joker()) {
                self.empty_set(to, s)
            } else {
                0
            };
            let allowed = self.accept_set(tr, dst, base, false);
            let run = matches!(
                self.rule(from).moves,
                MoveUnit::Run | MoveUnit::SuitRun | MoveUnit::Supermove | MoveUnit::TopOrWhole
            );
            let empty_any = dst.is_empty() && tr.empty == EmptyRule::Any;
            self.unit_fits(tr, allowed, empty_any, &src[src.len() - n..], run)
        };
        if !ok {
            return false;
        }
        if tg.requires_rank_on_prev && to > tg.start {
            let prev = &s.piles[to as usize - 1];
            if prev.is_empty() {
                return false;
            }
            if !card.is_joker() {
                let rank_ok = if dst.is_empty() {
                    prev[0].is_joker() || prev[0].rank() == card.rank()
                } else {
                    prev.iter().any(|c| c.is_joker() || c.rank() == card.rank())
                };
                if !rank_ok {
                    return false;
                }
            }
        }
        if tg.match_longest_rank && !card.is_joker() {
            let mut max_p = None;
            let mut max_len = 0usize;
            for p in tg.piles() {
                let len = s.piles[p as usize].len();
                if len > max_len {
                    max_len = len;
                    max_p = Some(p);
                }
            }
            if dst.len() < max_len {
                let ref_card = s.piles[max_p.unwrap() as usize][dst.len()];
                if !ref_card.is_joker() && ref_card.rank() != card.rank() {
                    return false;
                }
            }
        }
        if self.rule(from).moves == MoveUnit::Supermove && n > 1 {
            let cap = supermove_cap.unwrap_or_else(|| self.supermove_limit(s, from, to));
            return n <= cap;
        }
        true
    }

    /// True if all piles of group `gi` are interchangeable in state `s` (either always, or
    /// because the stock / tableau redeals that broke symmetry can no longer fire).
    #[inline]
    pub(crate) fn is_group_symmetric(&self, gi: usize, s: &State) -> bool {
        let g = &self.groups[gi];
        if g.symmetric {
            return true;
        }
        if g.symmetric_when_no_stock {
            if let Some(st) = &self.stock {
                if s.piles[st.pile as usize].is_empty()
                    && st.redeals != u32::MAX
                    && s.redeals_used >= st.redeals
                {
                    return true;
                }
            }
        }
        if g.symmetric_when_no_redeal {
            if let Some(rd) = &self.redeal {
                if rd.times != u32::MAX && s.redeals_used >= rd.times {
                    return true;
                }
            }
        }
        false
    }

    /// First empty pile of each symmetric group (first 64) and each symmetric sub-class (next 64):
    /// moving to another empty pile of the same group/class is equivalent.
    pub(crate) fn compute_first_empty(&self, s: &State) -> ([u16; 64], [u16; 64]) {
        let mut first_empty = [u16::MAX; 64];
        let mut class_empty = [u16::MAX; 64];
        for (gi, g) in self.groups.iter().enumerate() {
            if self.is_group_symmetric(gi, s) {
                if let Some(p) = g.piles().find(|&p| s.piles[p as usize].is_empty()) {
                    first_empty[gi] = p;
                }
            } else if !g.sym_classes.is_empty() {
                for cls in &g.sym_classes {
                    if cls.len() > 1 {
                        let cid = self.pile_sym_class[cls[0] as usize];
                        if cid != u16::MAX {
                            if let Some(&p) = cls.iter().find(|&&p| s.piles[p as usize].is_empty())
                            {
                                class_empty[cid as usize] = p;
                            }
                        }
                    }
                }
            }
        }
        (first_empty, class_empty)
    }

    /// Generate transfers and removals (no stock actions). With `only`, just the moves that use
    /// that pile's top card.
    pub(crate) fn gen_actions(&self, s: &State, only: Option<u16>, out: &mut Vec<Move>) {
        if let Some(p) = only {
            if s.piles[p as usize].is_empty() {
                return;
            }
        }
        let first_empty = self.compute_first_empty(s);
        self.gen_actions_with_empty(s, only, &first_empty, out);
    }

    pub(crate) fn gen_actions_with_empty(
        &self,
        s: &State,
        only: Option<u16>,
        first_empty: &([u16; 64], [u16; 64]),
        out: &mut Vec<Move>,
    ) {
        let npiles = s.piles.len() as u16;
        // Precompute empty cell and empty-any-tableau counts once when supermoves are in play.
        let (empty_cells, empty_any_group) = if self.has_supermove {
            let mut cells = 0usize;
            let mut by_group = [0u16; 64];
            for (p, pile) in s.piles.iter().enumerate() {
                if !pile.is_empty() {
                    continue;
                }
                let gi = self.pile_group[p] as usize;
                if self.groups[gi].kind == PileKind::Cell {
                    cells += 1;
                } else if self.pile_rules[p].empty == EmptyRule::Any {
                    by_group[gi] += 1;
                }
            }
            (cells, by_group)
        } else {
            (0, [0u16; 64])
        };

        let no_hooks = self.hooks.is_none();
        let mut single = [0u16; 1];
        let sources: &[u16] = match only {
            Some(p) => {
                single[0] = p;
                &single
            }
            None if no_hooks => &self.movable_piles,
            None => &[],
        };
        let all_piles_iter = if only.is_none() && !no_hooks {
            0..npiles
        } else {
            0..0
        };

        let mut emit_from = |from: u16| {
            if !self.available(s, from) {
                return;
            }
            let unit = self.movable(s, from);
            if unit == 0 {
                return;
            }
            let fg = self.pile_group[from as usize];
            let src = &s.piles[from as usize];
            let src_len = src.len();
            let from_rule = self.rule(from);
            let from_kind = self.groups[fg as usize].kind;
            let from_moves = from_rule.moves;
            let targets: &[u16] = if no_hooks {
                &self.targets_for_group[fg as usize]
            } else {
                &[]
            };
            let target_count = if no_hooks {
                targets.len()
            } else {
                npiles as usize
            };
            for ti in 0..target_count {
                let to = targets.get(ti).copied().unwrap_or(ti as u16);
                if to == from {
                    continue;
                }
                let tgi = self.pile_group[to as usize] as usize;
                let tg = &self.groups[tgi];
                let dst = &s.piles[to as usize];
                let dst_empty = dst.is_empty();
                let tg_sym = self.is_group_symmetric(tgi, s);
                if dst_empty {
                    if tg_sym {
                        if first_empty.0[tgi] != to {
                            continue;
                        }
                    } else {
                        let cid = self.pile_sym_class[to as usize];
                        if cid != u16::MAX && first_empty.1[cid as usize] != to {
                            continue;
                        }
                    }
                }
                if !no_hooks && from_kind == PileKind::Foundation && tg.kind == PileKind::Foundation
                {
                    continue;
                }
                let tr = self.rule(to);
                if no_hooks {
                    if dst.len() >= tr.max_cards as usize {
                        continue;
                    }
                    if !tr.complete_runs {
                        if dst_empty {
                            if tr.empty == EmptyRule::None {
                                continue;
                            }
                        } else if tr.build.by == BuildBy::None {
                            continue;
                        }
                    }
                }
                let (lo, hi) = if tr.complete_runs {
                    let n = self.nranks as usize;
                    if n > unit {
                        continue;
                    }
                    (n, n)
                } else if from_moves == MoveUnit::Whole {
                    (unit, unit)
                } else {
                    let max_n = if no_hooks {
                        let room = (tr.max_cards as usize).saturating_sub(dst.len());
                        let cap = if dst_empty {
                            tr.empty_max_accept.min(tr.max_accept) as usize
                        } else {
                            tr.max_accept as usize
                        };
                        unit.min(cap).min(room)
                    } else {
                        unit.min(tr.max_accept as usize)
                    };
                    if max_n == 0 {
                        continue;
                    }
                    // On a non-empty pile, at most one depth can fit for most build rules,
                    // but either-way building can allow two; check them all.
                    (1, max_n)
                };
                // Fast bitmask pre-filter for non-empty, non-joker destinations.
                let fit_mask = if no_hooks && self.jokers == 0 && !tr.complete_runs && !dst_empty {
                    let top = *dst.last().unwrap();
                    if !top.is_face_up() {
                        continue;
                    }
                    Some(tr.fits[top.face() as usize])
                } else {
                    None
                };
                let supermove_cap = if from_moves == MoveUnit::Supermove && hi > 1 {
                    let mut cells = empty_cells;
                    let mut empties = empty_any_group[fg as usize] as u32;
                    if dst_empty {
                        if tg.kind == PileKind::Cell {
                            cells = cells.saturating_sub(1);
                        } else if tgi == fg as usize && tr.empty == EmptyRule::Any {
                            empties = empties.saturating_sub(1);
                        }
                    }
                    Some((cells + 1) << empties.min(16))
                } else {
                    None
                };
                for n in lo..=hi {
                    if from_moves == MoveUnit::TopOrWhole && n != 1 && n != src_len {
                        continue;
                    }
                    // Moving a whole pile into an empty pile of the same symmetric group/class is a no-op.
                    if dst_empty && n == src_len && tgi == fg as usize {
                        let same_sym = tg_sym
                            || (self.pile_sym_class[from as usize] != u16::MAX
                                && self.pile_sym_class[from as usize]
                                    == self.pile_sym_class[to as usize]);
                        if same_sym {
                            continue;
                        }
                    }
                    if let Some(mask) = fit_mask {
                        if mask >> src[src_len - n].face() & 1 == 0 {
                            continue;
                        }
                    }
                    if self.can_place_with_limit(s, from, to, n, supermove_cap) {
                        out.push(Move::new(Action::Transfer {
                            from,
                            to,
                            count: n as u16,
                        }));
                    }
                }
            }
        };

        if only.is_some() || no_hooks {
            for &from in sources {
                emit_from(from);
            }
        } else {
            for from in all_piles_iter {
                emit_from(from);
            }
        }
        if !self.removal.is_empty() {
            self.gen_removals(s, only, out);
        }
    }

    fn gen_removals(&self, s: &State, only: Option<u16>, out: &mut Vec<Move>) {
        for (ri, r) in self.removal.iter().enumerate() {
            let mut tops: Vec<(u16, Card)> = Vec::new();
            for (p, pile) in s.piles.iter().enumerate() {
                let p = p as u16;
                if r.from & (1 << self.pile_group[p as usize]) == 0 || !self.available(s, p) {
                    continue;
                }
                if let Some(&c) = pile.last() {
                    if c.is_face_up() {
                        tops.push((p, c));
                    }
                }
            }
            let neighbours = |piles: &[u16]| -> bool {
                let Some((adj, gi)) = r.adjacent else {
                    return true;
                };
                let g = &self.groups[gi as usize];
                let cols = g.columns.unwrap() as i32;
                let pos = |p: u16| {
                    let i = (p - g.start) as i32;
                    (i / cols, i % cols)
                };
                piles.windows(2).all(|w| {
                    let ((r1, c1), (r2, c2)) = (pos(w[0]), pos(w[1]));
                    let (dr, dc) = ((r1 - r2).abs(), (c1 - c2).abs());
                    match adj {
                        Adjacency::Eight => dr.max(dc) == 1,
                        Adjacency::Four => dr + dc == 1,
                    }
                })
            };
            let mut emit = |piles: &[u16]| {
                if only.is_none_or(|o| piles.contains(&o)) && neighbours(piles) {
                    out.push(Move::new(removal_action(ri, piles)));
                }
            };
            match &r.kind {
                RemovalKind::PairsSum { total, alone } => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        if alone.contains(&c.rank()) {
                            emit(&[p]);
                            continue;
                        }
                        for &(q, d) in &tops[i + 1..] {
                            if c.is_joker() || d.is_joker() || c.rank() + d.rank() == *total {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::SameRank => {
                    for (i, &(p, c)) in tops.iter().enumerate() {
                        for &(q, d) in &tops[i + 1..] {
                            if c.rank() == d.rank() || c.is_joker() || d.is_joker() {
                                emit(&[p, q]);
                            }
                        }
                    }
                }
                RemovalKind::LowerOfSuit => {
                    for &(p, c) in tops.iter().filter(|(_, c)| !c.is_joker()) {
                        let ci = self.rank_idx[c.rank() as usize];
                        if tops.iter().any(|&(q, d)| {
                            q != p
                                && !d.is_joker()
                                && d.suit() == c.suit()
                                && self.rank_idx[d.rank() as usize] > ci
                        }) {
                            emit(&[p]);
                        }
                    }
                }
                RemovalKind::RankSet { ranks } => {
                    // Every way of picking one playable card per listed rank, from distinct piles.
                    let mut sets: Vec<Vec<u16>> = Vec::new();
                    let mut pick: Vec<u16> = Vec::new();
                    fn rec(
                        ranks: &[u8],
                        tops: &[(u16, Card)],
                        pick: &mut Vec<u16>,
                        sets: &mut Vec<Vec<u16>>,
                    ) {
                        let Some((&r, rest)) = ranks.split_first() else {
                            let mut v = pick.clone();
                            v.sort_unstable();
                            if !sets.contains(&v) {
                                sets.push(v);
                            }
                            return;
                        };
                        for &(p, c) in tops {
                            if (c.rank() == r || c.is_joker()) && !pick.contains(&p) {
                                pick.push(p);
                                rec(rest, tops, pick, sets);
                                pick.pop();
                            }
                        }
                    }
                    rec(ranks, &tops, &mut pick, &mut sets);
                    for set in sets {
                        emit(&set);
                    }
                }
            }
        }
    }

    /// Can the stock deal (or the waste be turned back) now?
    pub fn can_deal(&self, s: &State) -> bool {
        if let Some(rd) = &self.redeal {
            if s.redeals_used >= rd.times {
                return false;
            }
            if matches!(rd.how, RedealHow::Compact { .. }) {
                let g = &self.groups[rd.group as usize];
                let mut saw_empty = false;
                let mut can_shift = false;
                for p in g.piles() {
                    if s.piles[p as usize].is_empty() {
                        saw_empty = true;
                    } else if saw_empty {
                        can_shift = true;
                        break;
                    }
                }
                let can_refill = saw_empty
                    && rd.refill.is_some_and(|rf| {
                        self.groups[rf as usize]
                            .piles()
                            .any(|p| !s.piles[p as usize].is_empty())
                    });
                return can_shift || can_refill;
            }
            return true;
        }
        let Some(st) = &self.stock else { return false };
        if !s.piles[st.pile as usize].is_empty() {
            if st.only_when_empty
                && self.groups[st.target as usize]
                    .piles()
                    .any(|p| !s.piles[p as usize].is_empty())
            {
                return false;
            }
            if st.first_empty
                && !self.groups[st.target as usize]
                    .piles()
                    .any(|p| s.piles[p as usize].is_empty())
            {
                return false;
            }
            if st.require_no_empty {
                let tg = &self.groups[st.target as usize];
                if tg.piles().any(|p| s.piles[p as usize].is_empty()) {
                    return false;
                }
            }
            return true;
        }
        s.redeals_used < st.redeals
            && self.groups[st.target as usize]
                .piles()
                .any(|p| !s.piles[p as usize].is_empty())
    }

    /// Every legal move in this position, as single steps (stock deals are separate moves).
    pub fn legal_moves(&self, s: &State) -> Vec<Move> {
        let mut out = Vec::new();
        self.gen_actions(s, None, &mut out);
        if self.can_deal(s) {
            out.push(Move::new(Action::Deal));
        }
        out
    }

    /// Is `mv` legal in `s`? (Including any talon steps it starts with.)
    pub fn is_legal(&self, s: &State, mv: Move) -> bool {
        let mut st = s.clone();
        let mut j = Vec::new();
        for _ in 0..mv.talon {
            if !self.talon_step(&mut st, &mut j) {
                return false;
            }
        }
        let mut moves = Vec::new();
        match mv.action {
            Action::Deal => return self.can_deal(&st),
            _ => self.gen_actions_unpruned(&st, &mut moves),
        }
        moves.contains(&Move::new(mv.action))
    }

    /// Like `gen_actions` but without the symmetry pruning, for checking moves made by people.
    fn gen_actions_unpruned(&self, s: &State, out: &mut Vec<Move>) {
        let n = s.piles.len() as u16;
        for from in 0..n {
            if !self.available(s, from) {
                continue;
            }
            let unit = self.movable(s, from);
            for to in 0..n {
                for k in 1..=unit {
                    if self.rule(from).moves == MoveUnit::Whole && k != unit {
                        continue;
                    }
                    if self.rule(from).moves == MoveUnit::TopOrWhole
                        && k != 1
                        && k != s.piles[from as usize].len()
                    {
                        continue;
                    }
                    if self.can_place(s, from, to, k) {
                        out.push(Move::new(Action::Transfer {
                            from,
                            to,
                            count: k as u16,
                        }));
                    }
                }
            }
        }
        self.gen_removals(s, None, out);
    }

    // ----- Playing --------------------------------------------------------------------------

    pub(crate) fn do_op(&self, s: &mut State, op: Op) {
        match op {
            Op::Shift { from, to, count } => {
                let (from, to) = (from as usize, to as usize);
                if count == 1 {
                    let c = s.piles[from].pop().expect("shift from empty pile");
                    s.piles[to].push(c);
                } else {
                    let at = s.piles[from].len() - count as usize;
                    if from < to {
                        let (left, right) = s.piles.split_at_mut(to);
                        let src = &mut left[from];
                        right[0].extend_from_slice(&src[at..]);
                        src.truncate(at);
                    } else {
                        let (left, right) = s.piles.split_at_mut(from);
                        let src = &mut right[0];
                        left[to].extend_from_slice(&src[at..]);
                        src.truncate(at);
                    }
                }
            }
            Op::Turn {
                from,
                to,
                count,
                up,
            } => {
                for _ in 0..count {
                    let c = s.piles[from as usize].pop().expect("turn from empty pile");
                    s.piles[to as usize].push(if up { c.up() } else { c.down() });
                }
            }
            Op::Flip(p) => {
                let c = s.piles[p as usize].last_mut().expect("flip empty pile");
                *c = c.flipped();
            }
            Op::Redeal => s.redeals_used += 1,
            Op::Swap { pile, i, j } => s.piles[pile as usize].swap(i as usize, j as usize),
        }
    }

    fn undo_op(&self, s: &mut State, op: Op) {
        match op {
            Op::Shift { from, to, count } => self.do_op(
                s,
                Op::Shift {
                    from: to,
                    to: from,
                    count,
                },
            ),
            Op::Turn {
                from,
                to,
                count,
                up,
            } => self.do_op(
                s,
                Op::Turn {
                    from: to,
                    to: from,
                    count,
                    up: !up,
                },
            ),
            Op::Flip(p) => self.do_op(s, Op::Flip(p)),
            Op::Redeal => s.redeals_used -= 1,
            Op::Swap { .. } => self.do_op(s, op),
        }
    }

    fn push(&self, s: &mut State, j: &mut Vec<Op>, op: Op) {
        self.do_op(s, op);
        j.push(op);
    }

    /// Undo journal entries back to `mark`.
    pub(crate) fn undo_to(&self, s: &mut State, j: &mut Vec<Op>, mark: usize) {
        while j.len() > mark {
            let op = j.pop().unwrap();
            self.undo_op(s, op);
        }
    }

    /// Deal `count` cards face up from `from` (a stock pile) onto `to`, preserving exact undo
    /// even when the top of the stock is already face up.
    fn deal_from_stock(&self, s: &mut State, j: &mut Vec<Op>, from: u16, to: u16, mut count: u16) {
        while count > 0
            && s.piles[from as usize]
                .last()
                .is_some_and(|c| c.is_face_up())
        {
            self.push(s, j, Op::Shift { from, to, count: 1 });
            count -= 1;
        }
        if count > 0 {
            self.push(
                s,
                j,
                Op::Turn {
                    from,
                    to,
                    count,
                    up: true,
                },
            );
        }
    }

    /// One stock action: deal, or turn the waste back. Returns false if neither is possible.
    pub(crate) fn talon_step(&self, s: &mut State, j: &mut Vec<Op>) -> bool {
        if !self.can_deal(s) {
            return false;
        }
        if self.redeal.is_some() {
            self.tableau_redeal(s, j);
            self.settle(s, j);
            return true;
        }
        let st = self.stock.as_ref().unwrap();
        let stock_len = s.piles[st.pile as usize].len() as u16;
        let tg = &self.groups[st.target as usize];
        if stock_len == 0 {
            if st.target_kind == PileKind::Waste {
                let n = s.piles[tg.start as usize].len() as u16;
                self.push(
                    s,
                    j,
                    Op::Turn {
                        from: tg.start,
                        to: st.pile,
                        count: n,
                        up: false,
                    },
                );
                // With unlimited redeals the count can't matter, and tracking it would make every
                // pass through the stock a new position.
                if st.redeals != u32::MAX {
                    self.push(s, j, Op::Redeal);
                }
                if !self.simple_settle || st.face_up || self.has_covers {
                    self.settle(s, j);
                }
                return true;
            }
            for p in tg.piles() {
                while let Some(&top) = s.piles[p as usize].last() {
                    if top.is_face_up() {
                        self.push(
                            s,
                            j,
                            Op::Turn {
                                from: p,
                                to: st.pile,
                                count: 1,
                                up: false,
                            },
                        );
                    } else {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: p,
                                to: st.pile,
                                count: 1,
                            },
                        );
                    }
                }
            }
            if st.redeals != u32::MAX {
                self.push(s, j, Op::Redeal);
            }
        }
        let stock_len = s.piles[st.pile as usize].len() as u16;
        if stock_len > 0 {
            match st.target_kind {
                PileKind::Waste | PileKind::Foundation => {
                    let n = st.count.min(stock_len);
                    self.deal_from_stock(s, j, st.pile, tg.start, n);
                }
                _ if st.first_empty => {
                    if let Some(p) = tg.piles().find(|&p| s.piles[p as usize].is_empty()) {
                        let n = st.count.min(stock_len);
                        self.deal_from_stock(s, j, st.pile, p, n);
                    }
                }
                _ if st.skip_empty => {
                    let mut targets: Vec<u16> = tg
                        .piles()
                        .filter(|&p| !s.piles[p as usize].is_empty())
                        .collect();
                    if targets.is_empty() {
                        targets.push(tg.start);
                    }
                    for _ in 0..st.count {
                        for &p in &targets {
                            if s.piles[st.pile as usize].is_empty() {
                                break;
                            }
                            self.deal_from_stock(s, j, st.pile, p, 1);
                        }
                    }
                }
                _ => {
                    for _ in 0..st.count {
                        for p in tg.piles() {
                            if s.piles[st.pile as usize].is_empty() {
                                break;
                            }
                            self.deal_from_stock(s, j, st.pile, p, 1);
                        }
                    }
                }
            }
        }
        if !self.simple_settle || st.target_kind != PileKind::Waste || st.face_up || self.has_covers
        {
            self.settle(s, j);
        }
        true
    }

    /// Gather and redeal the tableau (see [`RedealHow`]).
    fn tableau_redeal(&self, s: &mut State, j: &mut Vec<Op>) {
        let rd = self.redeal.as_ref().unwrap();
        let g = &self.groups[rd.group as usize];
        let gp = rd.gather;
        let len = |s: &State, p: u16| s.piles[p as usize].len() as u16;
        match rd.how {
            RedealHow::Regroup { per_pile } | RedealHow::Shuffle { per_pile } => {
                // Last pile first, so the first pile's cards end on top of the gathered stack.
                for p in g.piles().rev() {
                    let n = len(s, p);
                    if n > 0 {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: p,
                                to: gp,
                                count: n,
                            },
                        );
                    }
                }
                if matches!(rd.how, RedealHow::Shuffle { .. }) {
                    self.shuffle_pile(s, j, gp);
                }
                // More cards than `per_pile` for every pile (a card came back from a foundation):
                // share the surplus out, an equal part to each pile and one more to the first few.
                let per = per_pile as usize;
                let count = g.count as usize;
                let surplus = (len(s, gp) as usize).saturating_sub(per * count);
                let share = |i: usize| per + surplus / count + usize::from(i < surplus % count);
                if surplus > 0 {
                    self.arrange_surplus(s, j, gp, per, (0..count).map(share).collect());
                }
                for (i, p) in g.piles().enumerate() {
                    let n = share(i).min(len(s, gp) as usize) as u16;
                    if n == 0 {
                        break;
                    }
                    self.push(
                        s,
                        j,
                        Op::Shift {
                            from: gp,
                            to: p,
                            count: n,
                        },
                    );
                }
            }
            RedealHow::Compact { .. } => {
                let mut write = g.start;
                for p in g.piles() {
                    let n = len(s, p);
                    if n == 0 {
                        continue;
                    }
                    if write != p {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: p,
                                to: write,
                                count: n,
                            },
                        );
                    }
                    write += 1;
                }
                if let Some(src) = rd.refill {
                    let src_kind = self.groups[src as usize].kind;
                    for p in write..g.start + g.count {
                        let from = self.groups[src as usize].piles().find(|&q| len(s, q) > 0);
                        let Some(from) = from else { break };
                        if src_kind == PileKind::Stock {
                            self.deal_from_stock(s, j, from, p, 1);
                        } else {
                            self.push(
                                s,
                                j,
                                Op::Shift {
                                    from,
                                    to: p,
                                    count: 1,
                                },
                            );
                        }
                    }
                }
            }
            RedealHow::GridShuffle => {
                let cols = g.columns.unwrap();
                let prefix: Vec<u16> = (0..g.count / cols)
                    .map(|row| self.row_prefix(s, rd.group, row) as u16)
                    .collect();
                for (row, &k) in prefix.iter().enumerate() {
                    for col in k..cols {
                        let p = g.start + row as u16 * cols + col;
                        let n = len(s, p);
                        if n > 0 {
                            self.push(
                                s,
                                j,
                                Op::Shift {
                                    from: p,
                                    to: gp,
                                    count: n,
                                },
                            );
                        }
                    }
                }
                self.shuffle_pile(s, j, gp);
                for (row, &k) in prefix.iter().enumerate() {
                    for col in k + 1..cols {
                        if len(s, gp) == 0 {
                            break;
                        }
                        let p = g.start + row as u16 * cols + col;
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: gp,
                                to: p,
                                count: 1,
                            },
                        );
                    }
                }
            }
        }
        if rd.times != u32::MAX {
            self.push(s, j, Op::Redeal);
        }
    }

    /// Shuffle a pile with the game's play seed, as a series of swaps.
    /// Reorder the gathered cards of a redeal that has more cards than `per_pile` for every pile,
    /// so that dealing `sizes[i]` cards from the top to each pile in turn (as blocks) gives every
    /// pile a run of the cards dealt in order: blocks of `per_pile` taken from the top, each kept
    /// in its own order, then cut into the piles' shares. Dealing `per_pile` to each pile in turn
    /// is already this order, so only a surplus needs it.
    fn arrange_surplus(
        &self,
        s: &mut State,
        j: &mut Vec<Op>,
        pile: u16,
        per_pile: usize,
        sizes: Vec<usize>,
    ) {
        let cards = s.piles[pile as usize].clone();
        let n = cards.len();
        // The order the cards are dealt in.
        let mut dealt: Vec<Card> = Vec::with_capacity(n);
        let mut end = n;
        while end > 0 {
            let start = end.saturating_sub(per_pile);
            dealt.extend_from_slice(&cards[start..end]);
            end = start;
        }
        // The pile, bottom first, that block moves from the top will cut into those shares.
        let mut want: Vec<Card> = Vec::with_capacity(n);
        let mut cuts = Vec::with_capacity(sizes.len());
        let mut at = 0;
        for &size in &sizes {
            let size = size.min(n - at);
            cuts.push(&dealt[at..at + size]);
            at += size;
        }
        for chunk in cuts.iter().rev() {
            want.extend_from_slice(chunk);
        }
        let mut have = cards;
        for t in 0..n {
            if have[t] == want[t] {
                continue;
            }
            let k = (t + 1..n)
                .find(|&k| have[k] == want[t])
                .expect("the same cards, rearranged");
            have.swap(t, k);
            self.push(
                s,
                j,
                Op::Swap {
                    pile,
                    i: t as u16,
                    j: k as u16,
                },
            );
        }
    }

    fn shuffle_pile(&self, s: &mut State, j: &mut Vec<Op>, pile: u16) {
        let mut rng =
            Rng::new(s.seed ^ (s.redeals_used as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let n = s.piles[pile as usize].len();
        for i in (1..n).rev() {
            let k = rng.below(i as u64 + 1) as usize;
            if k != i {
                self.push(
                    s,
                    j,
                    Op::Swap {
                        pile,
                        i: i as u16,
                        j: k as u16,
                    },
                );
            }
        }
    }

    /// Automatic follow-ups: turn up exposed cards, refill emptied piles, deal to an empty
    /// waste when the stock deals by itself.
    pub(crate) fn settle(&self, s: &mut State, j: &mut Vec<Op>) {
        self.settle_with(s, j, &[]);
    }

    pub(crate) fn settle_with(&self, s: &mut State, j: &mut Vec<Op>, initial_emptied: &[u16]) {
        if self.simple_settle {
            if !self.rules.auto_flip {
                return;
            }
            if !self.has_covers && !initial_emptied.is_empty() {
                for &p in initial_emptied {
                    if s.piles[p as usize].last().is_some_and(|c| !c.is_face_up())
                        && (self.group_of(p).kind != PileKind::Stock
                            || self.stock.as_ref().is_some_and(|st| st.face_up))
                    {
                        self.push(s, j, Op::Flip(p));
                    }
                }
                return;
            }
            for p in 0..s.piles.len() as u16 {
                let flip = s.piles[p as usize].last().is_some_and(|c| !c.is_face_up())
                    && (self.group_of(p).kind != PileKind::Stock
                        || self.stock.as_ref().is_some_and(|st| st.face_up))
                    && self.available(s, p);
                if flip {
                    self.push(s, j, Op::Flip(p));
                }
            }
            return;
        }
        let mut emptied: Vec<u16> = initial_emptied.to_vec();
        loop {
            let mut changed = false;
            let mut next_emptied: Vec<u16> = Vec::new();
            for g in self.groups.iter().filter(|g| g.compact) {
                let mut write = g.start;
                for p in g.piles() {
                    let n = s.piles[p as usize].len() as u16;
                    if n == 0 {
                        continue;
                    }
                    if write != p {
                        self.push(
                            s,
                            j,
                            Op::Shift {
                                from: p,
                                to: write,
                                count: n,
                            },
                        );
                        changed = true;
                    }
                    write += 1;
                }
            }
            for g in self.groups.iter().filter(|g| g.compact_face_up) {
                for p in g.start..g.start + g.count.saturating_sub(1) {
                    if s.piles[p as usize].is_empty() && !s.piles[p as usize + 1].is_empty() {
                        let n = self.movable(s, p + 1) as u16;
                        if n > 0 {
                            self.push(
                                s,
                                j,
                                Op::Shift {
                                    from: p + 1,
                                    to: p,
                                    count: n,
                                },
                            );
                            changed = true;
                        }
                    }
                }
            }
            for g in &self.groups {
                if g.refill_from.is_empty() {
                    continue;
                }
                for p in g.piles() {
                    if !s.piles[p as usize].is_empty()
                        || !self.available(s, p)
                        || (g.refill_on_empty && !emptied.contains(&p))
                    {
                        continue;
                    }
                    // The first group in the list with a card to give; within it, the first pile
                    // (or the largest pile when `refill_largest` is set, ties to lowest index).
                    let src = g.refill_from.iter().find_map(|&rg| {
                        let rg = &self.groups[rg as usize];
                        let eligible = |q: u16| {
                            s.piles[q as usize]
                                .last()
                                .is_some_and(|c| c.is_face_up() || rg.kind == PileKind::Stock)
                        };
                        if g.refill_largest {
                            let mut best: Option<(u16, usize)> = None;
                            for q in rg.piles() {
                                if eligible(q) {
                                    let len = s.piles[q as usize].len();
                                    if best.is_none_or(|(_, bl)| len > bl) {
                                        best = Some((q, len));
                                    }
                                }
                            }
                            best.map(|(q, _)| q)
                        } else {
                            rg.piles().find(|&q| eligible(q))
                        }
                    });
                    if let Some(q) = src {
                        if self.group_of(q).kind == PileKind::Stock {
                            self.deal_from_stock(s, j, q, p, 1);
                        } else {
                            self.push(
                                s,
                                j,
                                Op::Shift {
                                    from: q,
                                    to: p,
                                    count: 1,
                                },
                            );
                            if s.piles[q as usize].is_empty() {
                                next_emptied.push(q);
                            }
                        }
                        changed = true;
                    }
                }
            }
            if let Some(st) = self.stock.as_ref().filter(|st| st.auto) {
                let target = self.groups[st.target as usize].start;
                if st.target_kind == PileKind::Waste
                    && s.piles[target as usize].is_empty()
                    && !s.piles[st.pile as usize].is_empty()
                {
                    let n = st.count.min(s.piles[st.pile as usize].len() as u16);
                    self.deal_from_stock(s, j, st.pile, target, n);
                    changed = true;
                }
            }
            if self.rules.auto_flip {
                for p in 0..s.piles.len() as u16 {
                    let flip = s.piles[p as usize].last().is_some_and(|c| !c.is_face_up())
                        && (self.group_of(p).kind != PileKind::Stock
                            || self.stock.as_ref().is_some_and(|st| st.face_up))
                        && self.available(s, p);
                    if flip {
                        self.push(s, j, Op::Flip(p));
                        changed = true;
                    }
                }
            }
            if let Some(h) = &self.hooks {
                for (from, to, count) in h.auto_moves(self, s) {
                    if count > 0 && s.piles[from as usize].len() >= count as usize {
                        self.push(s, j, Op::Shift { from, to, count });
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
            emptied = next_emptied;
        }
    }

    /// Play a move, recording how to undo it in `j`. The move must be legal.
    pub(crate) fn apply(&self, s: &mut State, mv: Move, j: &mut Vec<Op>) {
        for _ in 0..mv.talon {
            let ok = self.talon_step(s, j);
            debug_assert!(ok, "talon step not possible");
        }
        match mv.action {
            Action::Transfer { from, to, count } => {
                self.push(s, j, Op::Shift { from, to, count });
                self.settle_with(s, j, &[from]);
            }
            Action::Deal => {
                let ok = self.talon_step(s, j);
                debug_assert!(ok);
            }
            Action::Remove { rule, n, piles } => {
                let to = self.removal[rule as usize].to;
                for &from in &piles[..n as usize] {
                    self.push(s, j, Op::Shift { from, to, count: 1 });
                }
                self.settle_with(s, j, &piles[..n as usize]);
            }
        }
    }

    /// Play a move on a state. Returns an error if it isn't legal.
    pub fn play(&self, s: &mut State, mv: Move) -> Result<(), IllegalMove> {
        if !self.is_legal(s, mv) {
            return Err(IllegalMove(mv));
        }
        let mut j = Vec::new();
        self.apply(s, mv, &mut j);
        Ok(())
    }

    /// Has the game been won?
    pub fn is_won(&self, s: &State) -> bool {
        if let Some(b) = self.hooks.as_ref().and_then(|h| h.is_won(self, s)) {
            return b;
        }
        match &self.win {
            Win::AllOnFoundations => self.cards_home(s) == self.cards_to_home(),
            Win::Empty(gs) => gs.iter().all(|&g| {
                self.groups[g as usize]
                    .piles()
                    .all(|p| s.piles[p as usize].is_empty())
            }),
            Win::AtMostPiles(gi, n) => {
                self.groups[*gi as usize]
                    .piles()
                    .filter(|&p| !s.piles[p as usize].is_empty())
                    .count()
                    <= *n as usize
                    && self
                        .stock
                        .as_ref()
                        .is_none_or(|st| s.piles[st.pile as usize].is_empty())
            }
            Win::Rows(gi, len, first) => {
                let g = &self.groups[*gi as usize];
                let cols = g.columns.unwrap();
                let _ = first;
                (0..g.count / cols).all(|row| self.row_prefix(s, *gi, row) >= *len as usize)
            }
            Win::Score(n) => self.score(s).is_some_and(|v| v >= *n),
            Win::Count(g, n) => {
                self.groups[*g as usize]
                    .piles()
                    .map(|p| s.piles[p as usize].len())
                    .sum::<usize>()
                    >= *n as usize
            }
        }
    }

    /// Cards on foundations, jokers left out.
    pub(crate) fn cards_home(&self, s: &State) -> usize {
        let piles = self
            .groups
            .iter()
            .filter(|g| g.kind == PileKind::Foundation)
            .flat_map(|g| g.piles());
        if self.jokers == 0 {
            piles.map(|p| s.piles[p as usize].len()).sum()
        } else {
            piles
                .flat_map(|p| &s.piles[p as usize])
                .filter(|c| !c.is_joker())
                .count()
        }
    }

    /// Cards that must reach the foundations to win "all on foundations": every card but the
    /// jokers, which are helpers and may end anywhere.
    pub(crate) fn cards_to_home(&self) -> usize {
        self.total_cards as usize - self.jokers as usize
    }

    /// How many piles from the start of a grid row hold a built sequence (one card each,
    /// starting with the first-column rank).
    pub(crate) fn row_prefix(&self, s: &State, gi: u16, row: u16) -> usize {
        let g = &self.groups[gi as usize];
        let cols = g.columns.unwrap_or(g.count);
        let base = g.start + row * cols;
        let mut n = 0;
        // What the previous card could be (a joker: anything that keeps the row built).
        let mut prev: Option<u64> = None;
        for p in base..base + cols {
            let pile = &s.piles[p as usize];
            let [c] = pile.as_slice() else { break };
            if !c.is_face_up() {
                break;
            }
            let r = self.rule(p);
            let allowed = match prev {
                None => match r.empty {
                    EmptyRule::AfterLeft { first } => self.rank_faces[first as usize],
                    _ => 0,
                },
                Some(set) => self.step_set(r, set, false),
            };
            let id = if c.is_joker() {
                if !r.wild || allowed == 0 {
                    break;
                }
                allowed
            } else {
                if allowed >> c.face() & 1 == 0 {
                    break;
                }
                1 << c.face()
            };
            prev = Some(id);
            n += 1;
        }
        n
    }

    // ----- Dealing --------------------------------------------------------------------------

    /// All the cards of the pack, unshuffled.
    pub fn pack_cards(&self) -> Vec<Card> {
        let p = &self.rules.pack;
        let mut v = Vec::with_capacity(p.len());
        for _ in 0..p.copies {
            for &s in &p.suits {
                for &r in &p.ranks {
                    v.push(Card::new(s, r));
                }
            }
        }
        v.extend((0..p.jokers).map(|_| Card::joker()));
        v
    }

    /// Deal a game from a seed. The same seed always gives the same deal for the same rules.
    pub fn deal(&self, seed: u64) -> State {
        self.try_deal(seed)
            .expect("rules were validated when compiled")
    }

    fn try_deal(&self, seed: u64) -> Result<State, RulesError> {
        let mut deck = self.pack_cards();
        Rng::new(seed).shuffle(&mut deck);
        let mut s = self.deal_from(deck)?;
        s.seed = seed;
        Ok(s)
    }

    /// Deal an explicit card order. `deck[0]` is dealt first.
    pub fn deal_ordered(&self, deck: &[Card]) -> Result<State, RulesError> {
        let d = deck.to_vec();
        let mut expect = self.pack_cards();
        let mut got: Vec<Card> = d.iter().map(|c| c.up()).collect();
        expect.sort();
        got.sort();
        if expect != got {
            return err("deck is not exactly the game's pack");
        }
        self.deal_from(d)
    }

    fn deal_from(&self, deck: Vec<Card>) -> Result<State, RulesError> {
        let mut deck: std::collections::VecDeque<Card> = deck.into_iter().map(|c| c.up()).collect();
        let mut s = State {
            piles: vec![Vec::new(); self.num_piles()],
            redeals_used: 0,
            base_rank: None,
            seed: 0,
        };
        for step in &self.rules.deal {
            let gi = self
                .groups
                .iter()
                .position(|g| g.name == step.to)
                .ok_or_else(|| RulesError(format!("deal: no group named {:?}", step.to)))?;
            let g = &self.groups[gi];
            let piles: Vec<u16> = match &step.piles {
                None => g.piles().collect(),
                Some(ix) => {
                    let mut v = Vec::new();
                    for &i in ix {
                        if i >= g.count {
                            return err(format!("deal: {:?} has no pile {i}", step.to));
                        }
                        v.push(g.start + i);
                    }
                    v
                }
            };
            if piles.is_empty() {
                return err("deal: no piles selected");
            }
            let orient = |c: Card| if step.face_up { c } else { c.down() };
            let mut first: Option<Card> = None;
            let mut put = |s: &mut State, p: u16, c: Card| {
                first.get_or_insert(c);
                s.piles[p as usize].push(orient(c));
            };
            if step.of_base {
                let base = s.base_rank.ok_or_else(|| {
                    RulesError("deal: of_base requires an earlier set_base step".into())
                })?;
                let limit = step.count.map(|c| c as usize * piles.len());
                let mut taken = Vec::new();
                deck.retain(|c| {
                    if c.rank() == base && limit.is_none_or(|m| taken.len() < m) {
                        taken.push(*c);
                        false
                    } else {
                        true
                    }
                });
                for (i, c) in taken.into_iter().enumerate() {
                    put(&mut s, piles[i % piles.len()], c);
                }
            } else if let (Some(ranks), true) = (&step.ranks, step.one_each) {
                for (i, &r) in ranks.iter().enumerate() {
                    let at = deck.iter().position(|c| c.rank() == r).ok_or_else(|| {
                        RulesError(format!("deal: no card of rank {r} left to take"))
                    })?;
                    let c = deck.remove(at).unwrap();
                    put(&mut s, piles[i % piles.len()], c);
                }
            } else if let Some(ranks) = &step.ranks {
                let pile_suit = |p: u16| -> Option<crate::card::Suit> {
                    let r = self.rule(p);
                    if r.empty_suit.is_some() {
                        return r.empty_suit;
                    }
                    let real = r.admits & REAL_FACES;
                    crate::card::Suit::ALL
                        .into_iter()
                        .find(|&s| real == (self.suit_faces[s as usize] & REAL_FACES))
                };
                if let Some(n) = step.count {
                    for _ in 0..n {
                        for &p in &piles {
                            let want_suit = pile_suit(p);
                            let at = deck
                                .iter()
                                .position(|c| {
                                    ranks.contains(&c.rank())
                                        && want_suit.is_none_or(|s| c.suit() == s)
                                })
                                .ok_or_else(|| {
                                    RulesError(format!("deal: no matching card left for {p}"))
                                })?;
                            let c = deck.remove(at).unwrap();
                            put(&mut s, p, c);
                        }
                    }
                } else {
                    let mut taken = Vec::new();
                    deck.retain(|c| {
                        if ranks.contains(&c.rank()) {
                            taken.push(*c);
                            false
                        } else {
                            true
                        }
                    });
                    let has_suit = piles.iter().any(|&p| pile_suit(p).is_some());
                    if has_suit {
                        let mut used = vec![0usize; piles.len()];
                        for c in taken {
                            let slot = (0..piles.len())
                                .filter(|&i| pile_suit(piles[i]) == Some(c.suit()))
                                .min_by_key(|&i| used[i])
                                .unwrap_or(0);
                            used[slot] += 1;
                            put(&mut s, piles[slot], c);
                        }
                    } else {
                        for (i, c) in taken.into_iter().enumerate() {
                            put(&mut s, piles[i % piles.len()], c);
                        }
                    }
                }
            } else if step.rest {
                let mut i = 0;
                while let Some(c) = deck.pop_front() {
                    put(&mut s, piles[i % piles.len()], c);
                    i += 1;
                }
            } else {
                let counts: Vec<u16> = match (&step.counts, step.count) {
                    (Some(c), _) => {
                        if c.len() != piles.len() {
                            return err(format!(
                                "deal to {:?}: counts has the wrong length",
                                step.to
                            ));
                        }
                        c.clone()
                    }
                    (None, Some(n)) => vec![n; piles.len()],
                    (None, None) if step.collect.is_some() => vec![0; piles.len()],
                    (None, None) => {
                        return err(format!(
                            "deal to {:?}: give count, counts, rest or ranks",
                            step.to
                        ))
                    }
                };
                let rows = counts.iter().copied().max().unwrap_or(0);
                for r in 0..rows {
                    for (i, &p) in piles.iter().enumerate() {
                        if counts[i] > r {
                            let c = deck
                                .pop_front()
                                .ok_or_else(|| RulesError("deal: ran out of cards".into()))?;
                            put(&mut s, p, c);
                        }
                    }
                }
            }
            if step.kings_to_bottom {
                for &p in &piles {
                    let pile = &mut s.piles[p as usize];
                    let (mut k, rest): (Vec<Card>, Vec<Card>) =
                        pile.iter().partition(|c| c.rank() == 13);
                    k.extend(rest);
                    *pile = k;
                }
            }
            if let Some(ranks) = &step.collect {
                let mut taken = Vec::new();
                for (p, pile) in s.piles.iter_mut().enumerate() {
                    if piles.contains(&(p as u16)) {
                        continue;
                    }
                    pile.retain(|c| {
                        let hit = ranks.contains(&c.rank());
                        if hit {
                            taken.push(c.up());
                        }
                        !hit
                    });
                }
                for (i, c) in taken.into_iter().enumerate() {
                    s.piles[piles[i % piles.len()] as usize].push(c);
                }
            }
            if step.set_base {
                s.base_rank = first.map(|c| c.rank());
            }
        }
        if !deck.is_empty() {
            return err(format!("deal leaves {} cards undealt", deck.len()));
        }
        let mut j = Vec::new();
        self.settle(&mut s, &mut j);
        Ok(s)
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

/// A removal of the top cards of `piles` under rule `rule`.
fn removal_action(rule: usize, piles: &[u16]) -> Action {
    let mut a = [u16::MAX; 4];
    let mut v = piles.to_vec();
    v.sort_unstable();
    a[..v.len()].copy_from_slice(&v);
    Action::Remove {
        rule: rule as u8,
        n: v.len() as u8,
        piles: a,
    }
}

/// Which piles cover which, within a group, for a layout.
fn layout_covers(layout: &Layout, count: u16) -> Result<Vec<Vec<u16>>, String> {
    match layout {
        Layout::Pyramid { rows } => {
            let rows = *rows;
            if rows * (rows + 1) / 2 != count {
                return Err(format!(
                    "a {rows}-row pyramid has {} piles, not {count}",
                    rows * (rows + 1) / 2
                ));
            }
            let mut v = Vec::new();
            for r in 0..rows {
                for i in 0..=r {
                    if r + 1 < rows {
                        let next = (r + 1) * (r + 2) / 2;
                        v.push(vec![next + i, next + i + 1]);
                    } else {
                        v.push(vec![]);
                    }
                }
            }
            Ok(v)
        }
        Layout::TriPeaks => {
            if count != 28 {
                return Err("TriPeaks has 28 piles".into());
            }
            // Rows: 3 peaks (0..3), 6 (3..9), 9 (9..18), 10 (18..28).
            let mut v = vec![Vec::new(); 28];
            for p in 0..3u16 {
                v[p as usize] = vec![3 + 2 * p, 4 + 2 * p];
            }
            for j in 0..6u16 {
                let (peak, k) = (j / 2, j % 2);
                let below = 9 + peak * 3 + k;
                v[3 + j as usize] = vec![below, below + 1];
            }
            for j in 0..9u16 {
                v[9 + j as usize] = vec![18 + j, 19 + j];
            }
            Ok(v)
        }
        Layout::Grid { .. } => Ok(vec![Vec::new(); count as usize]),
        Layout::Covers { covered_by } => {
            if covered_by.len() != count as usize {
                return Err("covered_by needs one entry per pile".into());
            }
            for c in covered_by.iter().flatten() {
                if *c >= count {
                    return Err(format!("covered_by names pile {c}, which doesn't exist"));
                }
            }
            Ok(covered_by.clone())
        }
    }
}
