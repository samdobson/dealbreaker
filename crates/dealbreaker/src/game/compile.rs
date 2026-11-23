//! Compiling [`Rules`] into lookup tables and validating rule consistency.

use crate::card::{Card, FACES, JOKER, REAL_FACES};
use crate::rules::*;

use super::{err, Game, Group, PileRules, Redeal, Removal, RulesError, Stock, Win};

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
