//! Compiling [`Rules`] into lookup tables and validating rule consistency.

use crate::card::{Card, FACES, JOKER, REAL_FACES};
use crate::rules::*;

use super::{
    err, CompiledChooseBase, Game, Group, PileRules, Redeal, Removal, RulesError, Stock, Win,
};

struct CompiledPack {
    rank_idx: [u8; 14],
    in_pack: [bool; FACES],
    rank_faces: [u64; 14],
    suit_faces: [u64; 4],
    nranks: u8,
    total_cards: u16,
    first_rank: u8,
    last_rank: u8,
}

fn compile_pack(pack: &Pack) -> Result<CompiledPack, RulesError> {
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
    let first_rank = pack.ranks[0];
    let last_rank = *pack.ranks.last().expect("pack.ranks is non-empty");
    Ok(CompiledPack {
        rank_idx,
        in_pack,
        rank_faces,
        suit_faces,
        nranks,
        total_cards: total as u16,
        first_rank,
        last_rank,
    })
}

fn find_group(rules: &Rules, name: &str) -> Result<u16, RulesError> {
    rules
        .groups
        .iter()
        .position(|g| g.name == name)
        .map(|i| i as u16)
        .ok_or_else(|| RulesError(format!("no group named {name:?}")))
}

fn resolve_group_mask(rules: &Rules, names: &[String]) -> Result<u64, RulesError> {
    let mut m = 0u64;
    for n in names {
        m |= 1 << find_group(rules, n)?;
    }
    Ok(m)
}

fn compile_pile_rules(
    rules: &Rules,
    g: &GroupRules,
    pile_index: u16,
    pack: &CompiledPack,
    autoplay_globally_allowed: bool,
) -> Result<PileRules, RulesError> {
    // Later overrides win over earlier ones; any override wins over the group.
    let mut build = g.build.clone();
    let mut empty = g.empty.clone();
    let mut moves = g.moves;
    let mut max_cards = g.max_cards;
    let mut min_cards = g.min_cards;
    let mut max_accept = g.max_accept;
    let mut empty_max_accept = g.empty_max_accept;
    let mut non_empty_max_accept = g.non_empty_max_accept;
    let mut suit = g.suit;
    let mut empty_suit = g.empty_suit;
    let mut color = g.color;
    for o in g.piles.iter().filter(|o| o.piles.contains(&pile_index)) {
        build = o.build.clone().or(build);
        empty = o.empty.clone().or(empty);
        moves = o.moves.or(moves);
        max_cards = o.max_cards.or(max_cards);
        min_cards = o.min_cards.or(min_cards);
        max_accept = o.max_accept.or(max_accept);
        empty_max_accept = o.empty_max_accept.or(empty_max_accept);
        non_empty_max_accept = o.non_empty_max_accept.or(non_empty_max_accept);
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
    if build.step == 0 || build.step >= pack.nranks.max(2) {
        return err(format!("{:?}: build step out of range", g.name));
    }
    let empty = empty.unwrap_or(match g.kind {
        PileKind::Foundation => EmptyRule::Rank(if build.dir == Dir::Down {
            pack.last_rank
        } else {
            pack.first_rank
        }),
        PileKind::Tableau | PileKind::Cell => EmptyRule::Any,
        _ => EmptyRule::None,
    });
    let moves = moves.unwrap_or(match g.kind {
        PileKind::Foundation | PileKind::Stock => MoveUnit::None,
        _ => MoveUnit::Top,
    });
    let max_cards = max_cards.unwrap_or(match g.kind {
        PileKind::Cell => 1,
        _ if g.complete_runs => pack.nranks as u16,
        _ => u16::MAX,
    });
    let max_accept = max_accept.unwrap_or(match g.kind {
        PileKind::Foundation if g.complete_runs => pack.nranks as u16,
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
    let after_pile = match &empty {
        EmptyRule::AfterPile { group, offset } => {
            let ref_gi = find_group(rules, group)?;
            if rules.groups[ref_gi as usize].count != g.count {
                return err(format!(
                    "{:?}: after_pile group {:?} must have the same pile count",
                    g.name, group
                ));
            }
            Some((ref_gi, *offset))
        }
        _ => None,
    };
    // Competing foundations with different rules are filtered out after `fits` is compiled.
    let autoplay = autoplay_globally_allowed
        && g.kind == PileKind::Foundation
        && !g.complete_runs
        && !g.requires_rank_on_prev
        && !g.requires_prev_non_empty
        && !g.match_longest_rank
        && !g.lock_build_dir
        && !g.lock_pile_dir
        && !g.unique_suits
        && g.rank_sequence.is_empty()
        && g.unrestricted_from.is_none()
        && g.final_from_same_index.is_none()
        && build.by == BuildBy::Suit
        && matches!(build.dir, Dir::Up | Dir::Down)
        && (!build.wrap || max_cards <= pack.nranks as u16)
        && build.blocked_ranks.is_empty()
        && matches!(
            empty,
            EmptyRule::Rank(_) | EmptyRule::DealtBase | EmptyRule::DealtBaseOffset(_)
        );
    Ok(PileRules {
        build,
        empty,
        moves,
        max_cards,
        min_cards: min_cards.unwrap_or(0),
        max_accept,
        empty_max_accept: empty_max_accept.unwrap_or(max_accept),
        non_empty_max_accept: non_empty_max_accept.unwrap_or(max_accept),
        complete_runs: g.complete_runs,
        autoplay,
        sink: moves == MoveUnit::None
            && g.kind != PileKind::Stock
            && !g.requires_rank_on_prev
            && !g.match_longest_rank
            && !g.lock_build_dir
            && !g.lock_pile_dir
            && g.rank_sequence.is_empty()
            && g.max_same_rank.is_none(),
        admits,
        empty_suit,
        fits: [0; FACES],
        wild: g.wild.unwrap_or(g.kind != PileKind::Foundation),
        after_pile,
    })
}

struct CompiledGroups {
    groups: Vec<Group>,
    pile_group: Vec<u16>,
    pile_rules: Vec<PileRules>,
    start: u16,
}

fn compile_groups(rules: &Rules, pack: &CompiledPack) -> Result<CompiledGroups, RulesError> {
    if rules.groups.len() > 64 {
        return err("at most 64 groups");
    }
    let stock_redeals_non_waste = rules.stock.as_ref().is_some_and(|sr| {
        (sr.redeals > 0 || sr.unlimited_redeals)
            && rules
                .groups
                .iter()
                .any(|gr| gr.name == sr.to && gr.kind != PileKind::Waste)
    });
    let stock_sensitive_to_empty = rules.stock.as_ref().is_some_and(|sr| {
        sr.require_no_empty
            || sr.require_no_empty_group.is_some()
            || sr.skip_empty
            || sr.first_empty
            || sr.deal_empty_first
    });
    // With a tableau redeal, which cards are still out decides how the rest are regathered and
    // dealt, so no foundation play is free of consequences. A joker can stand on any card, so
    // with jokers about no card is ever done with.
    let autoplay_globally_allowed = rules.safe_autoplay
        && rules.redeal.is_none()
        && !stock_redeals_non_waste
        && !stock_sensitive_to_empty
        && rules.pack.jokers == 0
        && !rules.groups.iter().any(|gr| gr.no_build_single_while_stock);

    let mut groups = Vec::new();
    let mut pile_group = Vec::new();
    let mut pile_rules = Vec::new();
    let mut start = 0u16;
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
            pile_rules.push(compile_pile_rules(
                rules,
                g,
                i,
                pack,
                autoplay_globally_allowed,
            )?);
            pile_group.push(gi as u16);
        }
        let accepts_from = match &g.accepts_from {
            None => u64::MAX,
            Some(names) => resolve_group_mask(rules, names)?,
        };
        let empty_accepts_from = match &g.empty_accepts_from {
            None => accepts_from,
            Some(names) => resolve_group_mask(rules, names)?,
        };
        let unrestricted_from = match &g.unrestricted_from {
            None => 0u64,
            Some(names) => resolve_group_mask(rules, names)?,
        };
        let refill_count = g.refill_count.unwrap_or(1);
        if refill_count == 0 {
            return err(format!("{:?}: refill_count must be at least 1", g.name));
        }
        let refill_from = g
            .refill_from
            .iter()
            .map(|n| find_group(rules, n))
            .collect::<Result<Vec<_>, _>>()?;
        let final_from_same_index = match &g.final_from_same_index {
            None => None,
            Some(name) => {
                let ref_gi = find_group(rules, name)?;
                if rules.groups[ref_gi as usize].count != g.count {
                    return err(format!(
                        "{:?}: final_from_same_index group {:?} must have the same pile count",
                        g.name, name
                    ));
                }
                Some(ref_gi)
            }
        };
        let moves_when_any_empty = match &g.moves_when_any_empty {
            None => None,
            Some(name) => Some(find_group(rules, name)?),
        };
        let moves_when_same_index_empty = match &g.moves_when_same_index_empty {
            None => None,
            Some(name) => {
                let ref_gi = find_group(rules, name)?;
                if rules.groups[ref_gi as usize].count != g.count {
                    return err(format!(
                        "{:?}: moves_when_same_index_empty group {:?} must have the same pile count",
                        g.name, name
                    ));
                }
                Some(ref_gi)
            }
        };
        let has_after_pile = (start as usize..start as usize + g.count as usize)
            .any(|p| pile_rules[p].after_pile.is_some());
        let mut grp = Group {
            name: g.name.clone(),
            kind: g.kind,
            start,
            count: g.count,
            accepts_from,
            empty_accepts_from,
            refill_from,
            refill_count,
            symmetric: false,
            symmetric_when_no_stock: false,
            symmetric_when_no_redeal: false,
            sym_classes: Vec::new(),
            columns: match g.layout {
                Some(Layout::Grid { columns }) => Some(columns),
                _ => None,
            },
            hidden: false,
            reach: g.reach.clone(),
            reach_both_ways: g.reach_both_ways,
            replace_on_drop: None,
            compact: g.compact,
            requires_rank_on_prev: g.requires_rank_on_prev,
            match_longest_rank: g.match_longest_rank,
            accept_only_when_empty: g.accept_only_when_empty,
            accept_when_stock_empty: g.accept_when_stock_empty,
            compact_face_up: g.compact_face_up,
            refill_largest: g.refill_largest,
            refill_on_empty: g.refill_on_empty,
            requires_prev_non_empty: g.requires_prev_non_empty,
            max_same_rank: g.max_same_rank,
            moves_when_stock_empty: g.moves_when_stock_empty,
            moves_when_any_empty,
            moves_when_same_index_empty,
            flip_only_last: g.flip_only_last,
            final_from_same_index,
            lock_build_dir: g.lock_build_dir,
            no_flip: g.no_flip,
            refill_same_index: g.refill_same_index,
            refill_nearest: g.refill_nearest,
            require_valid_base: g.require_valid_base,
            swap_bases: g.swap_bases,
            supermove_complete_run: g.supermove_complete_run,
            unrestricted_from,
            lock_pile_dir: g.lock_pile_dir,
            rank_sequence: g.rank_sequence.clone(),
            unique_suits: g.unique_suits,
            no_build_single_while_stock: g.no_build_single_while_stock,
            refill_first_face_down: g.refill_first_face_down,
        };
        grp.symmetric = g.layout.is_none()
            && g.piles.is_empty()
            && !grp.has_positional_rules()
            && !has_after_pile;
        groups.push(grp);
        start = start
            .checked_add(g.count)
            .ok_or_else(|| RulesError("too many piles".into()))?;
    }
    Ok(CompiledGroups {
        groups,
        pile_group,
        pile_rules,
        start,
    })
}

fn compile_redeal(
    rules: &Rules,
    groups: &mut Vec<Group>,
    pile_group: &mut Vec<u16>,
    pile_rules: &mut Vec<PileRules>,
    start: &mut u16,
) -> Result<Option<Redeal>, RulesError> {
    let Some(rd) = &rules.redeal else {
        return Ok(None);
    };
    if rules.stock.is_some() {
        return err("a game can't have both stock rules and a tableau redeal");
    }
    let gi = find_group(rules, &rd.group)?;
    if groups[gi as usize].symmetric && groups[gi as usize].count > 1 {
        groups[gi as usize].symmetric_when_no_redeal = true;
    }
    groups[gi as usize].symmetric = false;
    if matches!(
        rd.how,
        RedealHow::GridShuffle | RedealHow::GridShuffleRandomGaps
    ) && groups[gi as usize].columns.is_none()
    {
        return err("grid_shuffle redeals need a grid layout");
    }
    if let RedealHow::Regroup { per_pile: 0 } | RedealHow::Shuffle { per_pile: 0 } = rd.how {
        return err("redeal per_pile must be at least 1");
    }
    // A redeal gathers cards on a hidden scratch pile.
    groups.push(Group::hidden_scratch("__gather", *start));
    pile_rules.push(PileRules {
        build: Build::default(),
        empty: EmptyRule::None,
        moves: MoveUnit::None,
        max_cards: u16::MAX,
        min_cards: 0,
        max_accept: 0,
        empty_max_accept: 0,
        non_empty_max_accept: 0,
        complete_runs: false,
        autoplay: false,
        sink: false,
        admits: 0,
        empty_suit: None,
        fits: [0; FACES],
        wild: false,
        after_pile: None,
    });
    pile_group.push(groups.len() as u16 - 1);
    *start += 1;
    Ok(Some(Redeal {
        group: gi,
        times: if rd.unlimited { u32::MAX } else { rd.times },
        how: rd.how.clone(),
        gather: *start - 1,
        refill: match &rd.how {
            RedealHow::Compact {
                refill_from: Some(n),
            } => Some(find_group(rules, n)?),
            _ => None,
        },
    }))
}

fn compile_covers(
    rules: &Rules,
    groups: &[Group],
    npiles: usize,
) -> Result<(Vec<Vec<u16>>, bool), RulesError> {
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
        let local =
            layout_covers(layout, g.count).map_err(|e| RulesError(format!("{:?}: {e}", g.name)))?;
        for (i, cov) in local.into_iter().enumerate() {
            covered_by[base as usize + i] = cov.into_iter().map(|c| base + c).collect();
        }
    }
    Ok((covered_by, has_covers))
}

fn compile_stock(
    rules: &Rules,
    groups: &mut [Group],
) -> Result<(Option<Stock>, Option<u16>), RulesError> {
    let stock_groups: Vec<usize> = groups
        .iter()
        .enumerate()
        .filter(|(_, g)| g.kind == PileKind::Stock)
        .map(|(i, _)| i)
        .collect();
    if stock_groups.len() > 1 {
        return err("at most one stock group");
    }
    let stock_pile = stock_groups.first().map(|&sg| groups[sg].start);
    let stock = match (&rules.stock, stock_groups.first()) {
        (Some(sr), Some(&sg)) => {
            let target = find_group(rules, &sr.to)?;
            let tk = groups[target as usize].kind;
            if matches!(tk, PileKind::Stock) {
                return err("the stock can't deal to itself");
            }
            if sr.count == 0 {
                return err("stock count must be at least 1");
            }
            if sr.count_per_round.contains(&0) {
                return err("stock count_per_round entries must be at least 1");
            }
            let final_to = match &sr.final_to {
                None => None,
                Some(name) => {
                    let ft = find_group(rules, name)?;
                    let ftk = groups[ft as usize].kind;
                    if matches!(ftk, PileKind::Stock) {
                        return err("stock final_to can't be the stock");
                    }
                    if ftk != PileKind::Waste && ftk != PileKind::Foundation {
                        if groups[ft as usize].symmetric && groups[ft as usize].count > 1 {
                            groups[ft as usize].symmetric_when_no_stock = true;
                        }
                        groups[ft as usize].symmetric = false;
                    }
                    Some(ft)
                }
            };
            let targets = match &sr.piles {
                None => groups[target as usize].piles().collect(),
                Some(ix) => resolve_group_piles(&groups[target as usize], ix, "stock piles")?,
            };
            let piles_per_round = sr
                .piles_per_round
                .iter()
                .map(|ix| {
                    resolve_group_piles(&groups[target as usize], ix, "stock piles_per_round")
                })
                .collect::<Result<Vec<_>, _>>()?;
            let final_piles = match &sr.final_piles {
                None => None,
                Some(ix) => Some(resolve_group_piles(
                    &groups[target as usize],
                    ix,
                    "stock final_piles",
                )?),
            };
            let final_split_to = match &sr.final_split_to {
                None => None,
                Some(name) => {
                    let ft = find_group(rules, name)?;
                    if matches!(groups[ft as usize].kind, PileKind::Stock) {
                        return err("stock final_split_to can't be the stock");
                    }
                    Some(groups[ft as usize].start)
                }
            };
            let require_no_empty_group = match &sr.require_no_empty_group {
                None => None,
                Some(name) => Some(find_group(rules, name)?),
            };
            if sr.skip_empty && (sr.require_no_empty || sr.first_empty) {
                return err(
                    "stock skip_empty can't be combined with require_no_empty or first_empty",
                );
            }
            // A group that receives one card per pile from the stock isn't symmetric while the
            // stock can still deal: which pile gets which card depends on its position. Once the
            // stock is empty (and cannot redeal), the group becomes symmetric.
            if tk != PileKind::Waste && tk != PileKind::Foundation {
                if groups[target as usize].symmetric && groups[target as usize].count > 1 {
                    groups[target as usize].symmetric_when_no_stock = true;
                }
                groups[target as usize].symmetric = false;
            }
            if let Some(ft) = final_to {
                if groups[ft as usize].symmetric && groups[ft as usize].count > 1 {
                    groups[ft as usize].symmetric_when_no_stock = true;
                }
                groups[ft as usize].symmetric = false;
            }
            let s = Stock {
                pile: groups[sg].start,
                target,
                target_kind: tk,
                count: sr.count,
                count_per_round: sr.count_per_round.clone(),
                final_to,
                targets,
                piles_per_round,
                final_piles,
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
                deal_per_pile: sr.deal_per_pile,
                redeal_reverse: sr.redeal_reverse,
                redeal_shuffle: sr.redeal_shuffle,
                redeal_deal_all_alternating: sr.redeal_deal_all_alternating,
                deal_empty_first: sr.deal_empty_first,
                divert_ranks: sr.divert_ranks.clone(),
                final_split_to,
                require_no_empty_group,
            };
            if s.redeals > 0 && s.target_kind == PileKind::Foundation {
                return err("redeals can't gather from a foundation group");
            }
            Some(s)
        }
        (None, _) => None,
        (Some(_), None) => return err("stock rules given but no stock group"),
    };
    Ok((stock, stock_pile))
}

/// Groups that refill others or are referenced by index (`after_pile` / `final_from_same_index` /
/// `moves_when_same_index_empty`) are read by pile position, so their order matters.
fn mark_positional_sources(groups: &mut [Group], pile_rules: &[PileRules]) -> Vec<bool> {
    let mut is_refill_source = vec![false; groups.len()];
    let mut mark = |groups: &mut [Group], gi: usize, require_multi: bool| {
        if !require_multi || groups[gi].count > 1 {
            groups[gi].symmetric = false;
            groups[gi].symmetric_when_no_stock = false;
            groups[gi].symmetric_when_no_redeal = false;
            is_refill_source[gi] = true;
        }
    };
    for i in 0..groups.len() {
        for r in groups[i].refill_from.clone() {
            mark(groups, r as usize, true);
        }
        if let Some(r) = groups[i].final_from_same_index {
            mark(groups, r as usize, true);
        }
        if let Some(r) = groups[i].moves_when_same_index_empty {
            mark(groups, r as usize, true);
        }
        for p in groups[i].piles() {
            if let Some((r, _)) = pile_rules[p as usize].after_pile {
                mark(groups, r as usize, true);
                mark(groups, i, false);
            }
        }
    }
    is_refill_source
}

fn compile_removals(rules: &Rules, groups: &[Group]) -> Result<Vec<Removal>, RulesError> {
    let mut removal = Vec::new();
    for r in &rules.removal {
        match &r.kind {
            RemovalKind::BetweenMatch { max_len } => {
                if *max_len == 0 || *max_len > 4 {
                    return err("between_match max_len must be 1..=4");
                }
            }
            RemovalKind::ContiguousSumMultiple { divisor, max_len } => {
                if *divisor == 0 {
                    return err("contiguous_sum_multiple divisor must be at least 1");
                }
                if *max_len == 0 || *max_len > 4 {
                    return err("contiguous_sum_multiple max_len must be 1..=4");
                }
            }
            _ => {}
        }
        let from = resolve_group_mask(rules, &r.from)?;
        let adjacent = match r.adjacent {
            None => None,
            Some(a) => {
                let [name] = r.from.as_slice() else {
                    return err("adjacent removal takes cards from exactly one grid group");
                };
                let gi = find_group(rules, name)?;
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
            to: groups[find_group(rules, &r.to)? as usize].start,
            adjacent,
        });
    }
    Ok(removal)
}

fn compile_win(
    rules: &Rules,
    groups: &[Group],
    pile_rules: &[PileRules],
) -> Result<Win, RulesError> {
    match &rules.win {
        WinRule::AllOnFoundations => {
            if !groups.iter().any(|g| g.kind == PileKind::Foundation) {
                return err("win is all_on_foundations but there are no foundations");
            }
            Ok(Win::AllOnFoundations)
        }
        WinRule::Empty(names) => Ok(Win::Empty(
            names
                .iter()
                .map(|n| find_group(rules, n))
                .collect::<Result<_, _>>()?,
        )),
        WinRule::Count { group, cards } => Ok(Win::Count(find_group(rules, group)?, *cards)),
        WinRule::CountAndSpread {
            group,
            cards,
            spread,
        } => Ok(Win::CountAndSpread(
            find_group(rules, group)?,
            *cards,
            find_group(rules, spread)?,
        )),
        WinRule::AtMostPiles { group, piles } => {
            Ok(Win::AtMostPiles(find_group(rules, group)?, *piles))
        }
        WinRule::Rows { group, length } | WinRule::RowsBuilt { group, length, .. } => {
            let gi = find_group(rules, group)?;
            let g = &groups[gi as usize];
            let cols = g.columns.ok_or_else(|| {
                RulesError(format!("win by rows needs {group:?} to have a grid layout"))
            })?;
            let (EmptyRule::AfterLeft { first } | EmptyRule::AfterNeighbor { first }) =
                pile_rules[g.start as usize].empty
            else {
                return err(format!(
                    "win by rows needs {group:?} to use the after_left or after_neighbor empty rule"
                ));
            };
            let len = length.unwrap_or(cols - 1);
            if len > cols {
                return err("rows win: length longer than a row");
            }
            Ok(Win::Rows(gi, len, first))
        }
        WinRule::Score { at_least } => {
            if rules.score.is_none() {
                return err("win by score needs a score rule");
            }
            Ok(Win::Score(*at_least))
        }
        WinRule::AllFull { group } => Ok(Win::AllFull(find_group(rules, group)?)),
    }
}

fn can_compress_talon(
    stock: &Option<Stock>,
    waste: Option<u16>,
    groups: &[Group],
    pile_group: &[u16],
    pile_rules: &[PileRules],
    removal: &[Removal],
) -> bool {
    match (stock, waste) {
        (Some(s), Some(w))
            if s.target_kind == PileKind::Waste
                && !s.face_up
                && !groups.iter().any(|g| g.depends_on_stock_empty()) =>
        {
            let wg = &groups[s.target as usize];
            let sg = pile_group[s.pile as usize];
            debug_assert_eq!(w, wg.start);
            let feeds_refill = groups
                .iter()
                .any(|g| g.refill_from.contains(&s.target) || g.refill_from.contains(&sg));
            let stock_playable = pile_rules[s.pile as usize].moves != MoveUnit::None
                || removal.iter().any(|r| r.from & (1 << sg) != 0);
            !feeds_refill
                && !stock_playable
                && (pile_rules[w as usize].moves != MoveUnit::None
                    || removal.iter().any(|r| r.from & (1 << s.target) != 0))
        }
        _ => false,
    }
}

impl Game {
    /// Check and compile rules.
    pub fn new(rules: Rules) -> Result<Game, RulesError> {
        let pack = compile_pack(&rules.pack)?;
        let CompiledGroups {
            mut groups,
            mut pile_group,
            mut pile_rules,
            mut start,
        } = compile_groups(&rules, &pack)?;
        let redeal = compile_redeal(
            &rules,
            &mut groups,
            &mut pile_group,
            &mut pile_rules,
            &mut start,
        )?;
        let npiles = start as usize;
        for (gi, g) in rules.groups.iter().enumerate() {
            if let Some(name) = &g.replace_on_drop {
                let dgi = find_group(&rules, name)?;
                groups[gi].replace_on_drop = Some(groups[dgi as usize].start);
            }
        }
        let (covered_by, has_covers) = compile_covers(&rules, &groups, npiles)?;
        let (stock, stock_pile) = compile_stock(&rules, &mut groups)?;
        let is_refill_source = mark_positional_sources(&mut groups, &pile_rules);
        let removal = compile_removals(&rules, &groups)?;
        let win = compile_win(&rules, &groups, &pile_rules)?;

        let waste = groups
            .iter()
            .find(|g| g.kind == PileKind::Waste)
            .map(|g| g.start);
        let compress_talon =
            can_compress_talon(&stock, waste, &groups, &pile_group, &pile_rules, &removal);
        let choose_base = match &rules.choose_base {
            None => None,
            Some(cb) => Some(CompiledChooseBase {
                from_group: find_group(&rules, &cb.from)?,
                refill_from_stock: cb.refill_from_stock,
                deal_waste: cb.deal_waste,
            }),
        };
        let simple_settle =
            !groups.iter().any(|g| g.needs_settle()) && !stock.as_ref().is_some_and(|st| st.auto);
        let copies = rules.pack.copies;
        let jokers = rules.pack.jokers;
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
            row_win_fits: None,
            rank_idx: pack.rank_idx,
            nranks: pack.nranks,
            copies,
            total_cards: pack.total_cards,
            jokers,
            in_pack: pack.in_pack,
            rank_faces: pack.rank_faces,
            suit_faces: pack.suit_faces,
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
            choose_base,
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

        game.compile_fits();
        game.compile_autoplay_and_dependents();
        game.compile_sym_classes(&is_refill_source);
        game.compile_move_gen_tables();

        // Validate the deal by dealing once.
        game.try_deal(0)?;
        Ok(game)
    }

    fn compile_fits(&mut self) {
        for p in 0..self.pile_rules.len() {
            let mut t = [0u64; FACES];
            let pr = &self.pile_rules[p];
            for (u, row) in t.iter_mut().enumerate() {
                for o in 0..FACES {
                    if pr.admits >> o & 1 != 0
                        && self.fits(
                            &pr.build,
                            Card::from_face(u as u8),
                            Card::from_face(o as u8),
                        )
                    {
                        *row |= 1 << o;
                    }
                }
            }
            self.pile_rules[p].fits = t;
        }
        if let WinRule::RowsBuilt { build: b, .. } = &self.rules.win {
            let mut t = [0u64; FACES];
            for (u, row) in t.iter_mut().enumerate() {
                for o in 0..FACES {
                    if self.fits(b, Card::from_face(u as u8), Card::from_face(o as u8)) {
                        *row |= 1 << o;
                    }
                }
            }
            self.row_win_fits = Some(t);
        }
    }

    fn compile_autoplay_and_dependents(&mut self) {
        // Disable autoplay on any foundation pile that can compete for the same faces with
        // another foundation pile having a different build/empty/admits rule (e.g. an Up and a
        // Down foundation of the same suit whose reachable rank sets overlap).
        let foundation_reachable = |p: usize| -> u64 {
            let pr = &self.pile_rules[p];
            if pr.max_accept == 0 || pr.max_cards == 0 {
                return 0;
            }
            let real = pr.admits & REAL_FACES;
            let start_mask = match &pr.empty {
                EmptyRule::None => 0,
                EmptyRule::Any | EmptyRule::DealtBase | EmptyRule::DealtBaseOffset(_) => {
                    return real;
                }
                EmptyRule::Rank(r) => real & self.rank_faces[*r as usize],
                EmptyRule::Ranks(rs) => rs
                    .iter()
                    .fold(0u64, |m, &r| m | (real & self.rank_faces[r as usize])),
                EmptyRule::AfterLeft { .. }
                | EmptyRule::AfterNeighbor { .. }
                | EmptyRule::AfterPile { .. } => return real,
            };
            let mut cur = start_mask;
            if let Some(es) = pr.empty_suit {
                cur &= self.suit_faces[es as usize];
            }
            let mut seen = cur;
            let steps = (pr.max_cards as usize)
                .saturating_sub(1)
                .min(self.nranks as usize);
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
        let found_piles: Vec<usize> = (0..self.num_piles())
            .filter(|&p| self.pile_kind(p as u16) == PileKind::Foundation)
            .collect();
        let found_reach: Vec<u64> = found_piles
            .iter()
            .map(|&p| foundation_reachable(p))
            .collect();
        for (i, &p) in found_piles.iter().enumerate() {
            if !self.pile_rules[p].autoplay {
                continue;
            }
            let rp = found_reach[i];
            let conflicts = found_piles.iter().enumerate().any(|(j, &q)| {
                i != j
                    && (rp & found_reach[j]) != 0
                    && !self.pile_rules[p].same_foundation_policy(&self.pile_rules[q])
            });
            if conflicts {
                self.pile_rules[p].autoplay = false;
            }
        }

        // Which faces can be built on which, outside foundations: used to decide when playing
        // a card to a foundation can never hurt. Unrestricted dump piles (any card on any card,
        // empty accepts any, no card cap) never create a reason to hold a card off a foundation.
        let mut all_real_in_pack = 0u64;
        for (f, &present) in self.in_pack.iter().enumerate().take(JOKER as usize) {
            if present {
                all_real_in_pack |= 1 << f;
            }
        }
        let is_dump_pile: Vec<bool> = (0..self.num_piles())
            .map(|p| {
                let pr = &self.pile_rules[p];
                let gr = &self.groups[self.pile_group[p] as usize];
                pr.build.by == BuildBy::Any
                    && pr.build.dir == Dir::Any
                    && pr.build.blocked_ranks.is_empty()
                    && pr.empty == EmptyRule::Any
                    && pr.empty_suit.is_none()
                    && pr.max_cards == u16::MAX
                    && pr.empty_max_accept >= pr.max_accept
                    && pr.non_empty_max_accept >= pr.max_accept
                    && !gr.accept_only_when_empty
                    && !gr.requires_rank_on_prev
                    && !gr.requires_prev_non_empty
                    && !gr.match_longest_rank
                    && gr.max_same_rank.is_none()
                    && (pr.admits & all_real_in_pack) == all_real_in_pack
            })
            .collect();
        self.any_autoplay = self.pile_rules.iter().any(|r| r.autoplay);
        for f in 0..FACES {
            for o in 0..FACES {
                if !self.in_pack[f] || !self.in_pack[o] {
                    continue;
                }
                let dep = (0..self.num_piles()).any(|p| {
                    self.pile_kind(p as u16) != PileKind::Foundation
                        && !is_dump_pile[p]
                        && self.pile_rules[p].fits[f] >> o & 1 != 0
                });
                if dep {
                    self.dependents[f].push(o as u8);
                }
            }
        }
    }

    /// Equivalence-class symmetry for groups that are non-uniform only because of per-pile
    /// overrides (e.g. foundations with both up and down sub-sets, or paired suit piles).
    fn compile_sym_classes(&mut self, is_refill_source: &[bool]) {
        let mut next_sym_class = 0u16;
        for gi in 0..self.groups.len() {
            let gr = &self.groups[gi];
            if gr.symmetric
                || gr.symmetric_when_no_stock
                || gr.symmetric_when_no_redeal
                || gr.count <= 1
                || gr.hidden
                || gr.columns.is_some()
                || gr.has_positional_rules()
                || is_refill_source[gi]
                || self
                    .redeal
                    .as_ref()
                    .is_some_and(|rd| rd.group as usize == gi)
                || self.stock.as_ref().is_some_and(|st| {
                    (st.target as usize == gi
                        && st.target_kind != PileKind::Waste
                        && st.target_kind != PileKind::Foundation)
                        || st.final_to == Some(gi as u16)
                })
                || gr.piles().any(|p| !self.covered_by[p as usize].is_empty())
            {
                continue;
            }
            let mut classes: Vec<Vec<u16>> = Vec::new();
            for p in gr.piles() {
                if let Some(cls) = classes.iter_mut().find(|c| {
                    self.pile_rules[c[0] as usize].same_policy(&self.pile_rules[p as usize])
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
                            self.pile_sym_class[p as usize] = next_sym_class;
                        }
                        next_sym_class += 1;
                    }
                }
                self.groups[gi].sym_classes = classes;
            }
        }
        self.num_sym_classes = next_sym_class as usize;
    }

    fn compile_move_gen_tables(&mut self) {
        // `RemovalKind::LowerOfSuit` is soundly forced when nothing can build on non-foundation
        // piles, there is only one removal rule, no redeal/jokers/score, and emptying a pile
        // cannot block or alter a stock deal or trigger a refill/compaction.
        self.force_lower_of_suit = self.rules.safe_autoplay
            && self.jokers == 0
            && self.redeal.is_none()
            && self.score.is_none()
            && self.removal.len() == 1
            && matches!(self.removal[0].kind, RemovalKind::LowerOfSuit)
            && self
                .stock
                .as_ref()
                .is_none_or(|st| !st.sensitive_to_empty_piles())
            && self.groups.iter().all(|gr| {
                gr.kind == PileKind::Foundation
                    || (!gr.needs_settle()
                        && gr
                            .piles()
                            .all(|p| self.pile_rules[p as usize].build.by == BuildBy::None))
            });

        // Precompute movable sources and valid target piles per source group for fast move-gen.
        self.has_supermove = self
            .pile_rules
            .iter()
            .any(|r| r.moves == MoveUnit::Supermove);
        self.movable_piles = (0..self.num_piles() as u16)
            .filter(|&p| self.pile_rules[p as usize].moves != MoveUnit::None)
            .collect();
        let ngroups = self.groups.len();
        let npiles = self.num_piles() as u16;
        self.targets_for_group = (0..ngroups)
            .map(|fg| {
                let from_kind = self.groups[fg].kind;
                (0..npiles)
                    .filter(|&to| {
                        let tgi = self.pile_group[to as usize] as usize;
                        let tg = &self.groups[tgi];
                        let tr = &self.pile_rules[to as usize];
                        let allowed_from = tg.accepts_from
                            | tg.empty_accepts_from
                            | tg.unrestricted_from
                            | tg.final_from_same_index.map_or(0, |r| 1u64 << r);
                        if (tg.kind == PileKind::Stock
                            && tr.empty == EmptyRule::None
                            && tr.build.by == BuildBy::None)
                            || allowed_from & (1 << fg) == 0
                        {
                            return false;
                        }
                        if from_kind == PileKind::Foundation && tg.kind == PileKind::Foundation {
                            return false;
                        }
                        if tg.reach.is_some() && tgi != fg {
                            return false;
                        }
                        tr.complete_runs
                            || !tg.rank_sequence.is_empty()
                            || tg.unrestricted_from & (1 << fg) != 0
                            || tr.empty != EmptyRule::None
                            || tr.build.by != BuildBy::None
                    })
                    .collect()
            })
            .collect();
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
                EmptyRule::AfterLeft { .. } | EmptyRule::AfterNeighbor { .. }
            )
            && !g.has_positional_rules()
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

/// Resolve a non-empty list of group-local pile indices into global pile indices.
fn resolve_group_piles(group: &Group, indices: &[u16], ctx: &str) -> Result<Vec<u16>, RulesError> {
    if indices.is_empty() {
        return err(format!("{ctx} must not be empty"));
    }
    let mut out = Vec::with_capacity(indices.len());
    for &i in indices {
        if i >= group.count {
            return err(format!("{ctx}: {:?} has no pile {i}", group.name));
        }
        out.push(group.start + i);
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets;

    #[test]
    fn compile_rejects_invalid_rules() {
        let mut r = presets::freecell();
        r.pack.copies = 0;
        assert!(Game::new(r).unwrap_err().0.contains("empty"));

        let mut r = presets::freecell();
        r.pack.ranks = vec![0];
        assert!(Game::new(r).unwrap_err().0.contains("1..=13"));

        let mut r = presets::freecell();
        r.pack.ranks = vec![1, 1];
        assert!(Game::new(r).unwrap_err().0.contains("twice"));

        let mut r = presets::freecell();
        r.groups[0].name = r.groups[1].name.clone();
        assert!(Game::new(r).unwrap_err().0.contains("two groups"));

        let mut r = presets::freecell();
        r.groups[0].count = 0;
        assert!(Game::new(r).unwrap_err().0.contains("no piles"));

        let mut r = presets::klondike1();
        r.groups[0].count = 2;
        assert!(Game::new(r).unwrap_err().0.contains("exactly one pile"));

        let mut r = presets::freecell();
        r.groups[2].complete_runs = true;
        assert!(Game::new(r).unwrap_err().0.contains("complete_runs"));

        let mut r = presets::freecell();
        r.groups[0].piles.push(PileOverride::on(&[99]));
        assert!(Game::new(r).unwrap_err().0.contains("doesn't exist"));

        let mut r = presets::freecell();
        r.groups[2].layout = Some(Layout::Covers {
            covered_by: vec![vec![]],
        });
        assert!(Game::new(r).unwrap_err().0.contains("covered_by"));

        let mut r = presets::freecell();
        let count = r.groups[2].count as usize;
        let mut bad = vec![vec![]; count];
        bad[0] = vec![99];
        r.groups[2].layout = Some(Layout::Covers { covered_by: bad });
        assert!(Game::new(r).unwrap_err().0.contains("doesn't exist"));
    }

    #[test]
    fn compile_rejects_invalid_stock_redeal_removal_win_and_score_rules() {
        // Build step out of range.
        let mut r = presets::freecell();
        r.groups[2].build.as_mut().unwrap().step = 0;
        assert!(Game::new(r).unwrap_err().0.contains("build step"));

        // after_pile with mismatched group pile count.
        let mut r = presets::freecell();
        r.groups[2].empty = Some(EmptyRule::AfterPile {
            group: "cell".into(),
            offset: 1,
        });
        assert!(Game::new(r).unwrap_err().0.contains("same pile count"));

        // refill_count == 0.
        let mut r = presets::freecell();
        r.groups[2].refill_count = Some(0);
        assert!(Game::new(r).unwrap_err().0.contains("refill_count"));

        // final_from_same_index with mismatched pile count.
        let mut r = presets::freecell();
        r.groups[2].final_from_same_index = Some("cell".into());
        assert!(Game::new(r).unwrap_err().0.contains("same pile count"));

        // moves_when_same_index_empty with mismatched pile count.
        let mut r = presets::freecell();
        r.groups[2].moves_when_same_index_empty = Some("cell".into());
        assert!(Game::new(r).unwrap_err().0.contains("same pile count"));

        // Stock rules + tableau redeal together.
        let mut r = presets::klondike1();
        r.redeal = Some(RedealRules::new("tableau", RedealHow::RotateBottom));
        assert!(Game::new(r)
            .unwrap_err()
            .0
            .contains("both stock rules and a tableau redeal"));

        // GridShuffle redeal without grid layout.
        let mut r = presets::la_belle_lucie();
        r.redeal.as_mut().unwrap().how = RedealHow::GridShuffle;
        assert!(Game::new(r).unwrap_err().0.contains("grid layout"));

        // Redeal per_pile == 0.
        let mut r = presets::la_belle_lucie();
        r.redeal.as_mut().unwrap().how = RedealHow::Shuffle { per_pile: 0 };
        assert!(Game::new(r).unwrap_err().0.contains("per_pile"));

        // Grid layout with 0 columns or non-dividing pile count.
        let mut r = presets::freecell();
        r.groups[2].layout = Some(Layout::Grid { columns: 0 });
        assert!(Game::new(r).unwrap_err().0.contains("whole rows"));
        let mut r = presets::freecell();
        r.groups[2].layout = Some(Layout::Grid { columns: 3 });
        assert!(Game::new(r).unwrap_err().0.contains("whole rows"));

        // Pyramid / TriPeaks pile count mismatches.
        let mut r = presets::freecell();
        r.groups[2].layout = Some(Layout::Pyramid { rows: 3 });
        assert!(Game::new(r).unwrap_err().0.contains("pyramid"));
        let mut r = presets::freecell();
        r.groups[2].layout = Some(Layout::TriPeaks);
        assert!(Game::new(r).unwrap_err().0.contains("28 piles"));

        // Stock validation errors.
        let mut r = presets::klondike1();
        r.stock.as_mut().unwrap().to = "stock".into();
        assert!(Game::new(r).unwrap_err().0.contains("deal to itself"));

        let mut r = presets::klondike1();
        r.stock.as_mut().unwrap().count = 0;
        assert!(Game::new(r).unwrap_err().0.contains("stock count"));

        let mut r = presets::klondike1();
        r.stock.as_mut().unwrap().count_per_round = vec![1, 0];
        assert!(Game::new(r).unwrap_err().0.contains("count_per_round"));

        let mut r = presets::klondike1();
        r.stock.as_mut().unwrap().final_to = Some("stock".into());
        assert!(Game::new(r).unwrap_err().0.contains("final_to"));

        let mut r = presets::klondike1();
        r.stock.as_mut().unwrap().skip_empty = true;
        r.stock.as_mut().unwrap().require_no_empty = true;
        assert!(Game::new(r).unwrap_err().0.contains("skip_empty"));

        let mut r = presets::freecell();
        r.stock = Some(StockRules::to("tableau"));
        assert!(Game::new(r).unwrap_err().0.contains("no stock group"));

        let mut r = presets::klondike1();
        r.stock.as_mut().unwrap().to = "foundation".into();
        r.stock.as_mut().unwrap().redeals = 1;
        assert!(Game::new(r)
            .unwrap_err()
            .0
            .contains("gather from a foundation"));

        // Removal validation errors.
        let mut r = presets::pyramid();
        r.removal[0].kind = RemovalKind::BetweenMatch { max_len: 0 };
        assert!(Game::new(r).unwrap_err().0.contains("between_match"));

        let mut r = presets::pyramid();
        r.removal[0].kind = RemovalKind::ContiguousSumMultiple {
            divisor: 0,
            max_len: 2,
        };
        assert!(Game::new(r).unwrap_err().0.contains("divisor"));

        let mut r = presets::pyramid();
        r.removal[0].kind = RemovalKind::ContiguousSumMultiple {
            divisor: 10,
            max_len: 5,
        };
        assert!(Game::new(r).unwrap_err().0.contains("max_len"));

        let mut r = presets::pyramid();
        r.removal[0].adjacent = Some(Adjacency::Eight);
        assert!(Game::new(r)
            .unwrap_err()
            .0
            .contains("exactly one grid group"));

        let mut r = presets::pyramid();
        r.removal[0].from = vec!["tableau".into()];
        r.removal[0].adjacent = Some(Adjacency::Eight);
        assert!(Game::new(r).unwrap_err().0.contains("grid layout"));

        // WinRule validation errors.
        let mut r = presets::poker_squares();
        r.win = WinRule::AllOnFoundations;
        assert!(Game::new(r).unwrap_err().0.contains("no foundations"));

        let mut r = presets::freecell();
        r.win = WinRule::Score { at_least: 10 };
        assert!(Game::new(r).unwrap_err().0.contains("score rule"));

        let mut r = presets::montana();
        r.win = WinRule::Rows {
            group: "tableau".into(),
            length: Some(99),
        };
        assert!(Game::new(r).unwrap_err().0.contains("longer than a row"));

        // ScoreRules validation errors.
        let mut r = presets::poker_squares();
        r.groups[2].layout = None;
        assert!(Game::new(r).unwrap_err().0.contains("grid layout"));

        let mut r = presets::cribbage_squares();
        if let Some(ScoreRules {
            hands: Hands::Cribbage(ref mut cr),
            ..
        }) = r.score
        {
            cr.starter = Some("tableau".into());
        }
        assert!(Game::new(r).unwrap_err().0.contains("starter"));
    }

    #[test]
    fn min_cards_override_splits_symmetry_classes() {
        let mut ov = PileOverride::on(&[0]);
        ov.min_cards = Some(1);
        let r = Rules::new(
            "min_cards_sym",
            Pack::new(
                &[crate::card::Suit::Spades, crate::card::Suit::Hearts],
                &[1, 2, 3],
            ),
            vec![
                GroupRules::new("foundation", PileKind::Foundation, 2),
                GroupRules::new("tableau", PileKind::Tableau, 3)
                    .build(BuildBy::Alternate, Dir::Down)
                    .moves(MoveUnit::Top)
                    .pile(ov),
            ],
            vec![DealStep::to("tableau").rest()],
        );
        let g = Game::new(r).unwrap();
        let tab = g.groups.iter().find(|gr| gr.name == "tableau").unwrap();
        assert!(!tab.symmetric);
        assert_eq!(tab.sym_classes.len(), 2);
        // Pile 0 (min_cards=1) must not share a symmetry class with piles 1 and 2 (min_cards=0).
        let p0 = tab.start;
        assert_eq!(g.pile_sym_class[p0 as usize], u16::MAX);
        assert_ne!(g.pile_sym_class[(p0 + 1) as usize], u16::MAX);
        assert_eq!(
            g.pile_sym_class[(p0 + 1) as usize],
            g.pile_sym_class[(p0 + 2) as usize]
        );
    }
}
