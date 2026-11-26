//! Difficulty grading.

use std::sync::atomic::AtomicBool;

use dealbreaker::difficulty::{self, calibrate, preset_id};
use dealbreaker::*;

fn freecell() -> Game {
    Game::new(presets::freecell()).unwrap()
}

#[test]
fn every_preset_is_calibrated_and_recognised() {
    for p in presets::PRESETS {
        let g = Game::new((p.rules)()).unwrap();
        assert_eq!(preset_id(&g), Some(p.id));
        if p.id == "accordion" {
            // Almost never won within the default budget: nothing to calibrate from.
            assert_eq!(GradeThresholds::for_preset(p.id), None);
            continue;
        }
        let t = GradeThresholds::for_preset(p.id)
            .unwrap_or_else(|| panic!("{}: no built-in thresholds", p.id));
        assert!(t.medium <= t.hard, "{}: {t:?}", p.id);
        assert_eq!(GradeThresholds::for_game(&g), Some((p.id, t)));
    }
    let t = GradeThresholds::generic();
    assert!(t.medium > 0.0 && t.medium <= t.hard);
}

#[test]
fn search_hints_do_not_hide_the_preset() {
    let mut r = presets::freecell();
    r.search.blocker_weight = 7;
    assert_eq!(preset_id(&Game::new(r).unwrap()), Some("freecell"));
}

#[test]
fn changed_rules_are_not_the_preset() {
    let mut r = presets::freecell();
    r.name = "My FreeCell".into();
    let g = Game::new(r).unwrap();
    assert_eq!(preset_id(&g), None);
    let d = grade(&g, &g.deal(1), &GradeOptions::default());
    assert_eq!(d.source, ThresholdSource::Generic);
    assert_eq!(d.thresholds, GradeThresholds::generic());
}

#[test]
fn thresholds_split_scores() {
    let t = GradeThresholds::new(10.0, 12.0);
    assert_eq!(t.grade(9.99), Grade::Easy);
    assert_eq!(t.grade(10.0), Grade::Medium);
    assert_eq!(t.grade(11.99), Grade::Medium);
    assert_eq!(t.grade(12.0), Grade::Hard);
    assert!(Grade::Easy < Grade::Medium && Grade::Medium < Grade::Hard);
    for g in [Grade::Easy, Grade::Medium, Grade::Hard] {
        assert_eq!(g.label().parse::<Grade>().unwrap(), g);
        assert_eq!(serde_json::to_string(&g).unwrap(), format!("\"{g}\""));
    }
    assert!("tricky".parse::<Grade>().is_err());
}

#[test]
fn winnable_deal_is_graded_from_its_score() {
    let g = freecell();
    let s = g.deal_ordered(&microsoft_deal(1)).unwrap();
    let d = grade(&g, &s, &GradeOptions::default());
    assert_eq!(d.winnable, Some(true));
    assert_eq!(d.source, ThresholdSource::Preset);
    let sc = d.score.unwrap();
    assert_eq!(sc, difficulty::score(d.effort, d.trap_rate));
    assert_eq!(d.grade, d.thresholds.grade(sc));
    assert!(d.effort > 0 && d.steps > 0 && d.probes > 0 && d.traps <= d.probes);
    assert_eq!(d.trap_rate, d.traps as f64 / d.probes as f64);
    // Deterministic.
    assert_eq!(grade(&g, &s, &GradeOptions::default()), d);
}

#[test]
fn custom_thresholds_override_the_preset() {
    let g = freecell();
    let s = g.deal(3);
    let easy = grade(
        &g,
        &s,
        &GradeOptions::default().thresholds(GradeThresholds::new(1e9, 1e9)),
    );
    assert_eq!(easy.grade, Grade::Easy);
    assert_eq!(easy.source, ThresholdSource::Custom);
    let hard = grade(
        &g,
        &s,
        &GradeOptions::default().thresholds(GradeThresholds::new(0.0, 0.0)),
    );
    assert_eq!(hard.grade, Grade::Hard);
    assert_eq!(hard.score, easy.score);
}

#[test]
fn unwinnable_deal_is_hard() {
    let g = Game::new(presets::elevens()).unwrap();
    let d = grade(&g, &g.deal(1), &GradeOptions::default());
    assert_eq!(d.winnable, Some(false));
    assert_eq!(d.grade, Grade::Hard);
    assert_eq!(d.score, None);
    assert_eq!(d.probes, 0);
}

#[test]
fn deal_not_won_in_budget_is_hard() {
    let g = freecell();
    let d = grade(&g, &g.deal(1), &GradeOptions::default().max_states(1));
    assert_eq!(d.winnable, None);
    assert_eq!(d.grade, Grade::Hard);
    assert_eq!(d.score, None);
}

#[test]
fn cancelled_grading_is_unknown() {
    let g = freecell();
    let cancel = AtomicBool::new(true);
    let d = grade_with_cancel(&g, &g.deal(1), &GradeOptions::default(), &cancel);
    assert_eq!(d.winnable, None);
    assert_eq!(d.grade, Grade::Hard);
}

#[test]
fn probes_can_be_turned_off() {
    let g = freecell();
    let d = grade(&g, &g.deal(2), &GradeOptions::default().probe_positions(0));
    assert_eq!((d.probes, d.traps, d.trap_rate), (0, 0, 0.0));
    assert_eq!(d.score, Some(difficulty::score(d.effort, 0.0)));
}

#[test]
fn calibration_puts_a_third_in_each_grade() {
    let g = freecell();
    let graded: Vec<Difficulty> = (1..=12)
        .map(|seed| grade(&g, &g.deal(seed), &GradeOptions::default()))
        .collect();
    let c = calibrate(&graded).unwrap();
    assert_eq!(c.deals, 12);
    assert_eq!(c.winnable + c.unwinnable + c.unknown, 12);
    let mut counts = [0usize; 3];
    for d in &graded {
        if let Some(sc) = d.score {
            counts[c.thresholds.grade(sc) as usize] += 1;
        }
    }
    // Ties could unbalance this; FreeCell scores are effectively continuous.
    let third = c.winnable as usize / 3;
    assert!(
        counts.iter().all(|&n| n.abs_diff(third) <= 1),
        "{counts:?} of {}",
        c.winnable
    );
    assert!(calibrate(&[]).is_none());
}

#[test]
fn every_preset_can_be_graded() {
    // Small budgets: this checks that grading runs everywhere, not the grades themselves.
    let o = GradeOptions::default()
        .max_states(20_000)
        .probe_positions(2)
        .probe_states(200);
    for p in presets::PRESETS {
        let g = Game::new((p.rules)()).unwrap();
        let d = grade(&g, &g.deal(1), &o);
        let want = if p.id == "accordion" {
            ThresholdSource::Generic
        } else {
            ThresholdSource::Preset
        };
        assert_eq!(d.source, want, "{}", p.id);
        assert_eq!(d.score.is_some(), d.winnable == Some(true), "{}", p.id);
        if d.winnable != Some(true) {
            assert_eq!(d.grade, Grade::Hard, "{}", p.id);
        }
    }
}
