//! Reproducible, parallel solver benchmark and solve-rate baseline across presets or custom rules.
//!
//! Run with:
//! ```sh
//! cargo bench -p dealbreaker
//! cargo bench -p dealbreaker -- --seeds 20 --max-states 100000 --save /tmp/base.json
//! cargo bench -p dealbreaker -- --seeds 20 --max-states 100000 --compare /tmp/base.json
//! cargo bench -p dealbreaker -- --filter spider --seeds 50
//! cargo bench -p dealbreaker -- --rules /path/to/rules_or_dir --seeds 20 --max-states 200000
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use dealbreaker::{
    maximize, microsoft_deal, presets, solve, Game, Outcome, Rules, SolveOptions, State,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EntryReport {
    label: String,
    deals: u64,
    solved: u64,
    unsolvable: u64,
    gave_up: u64,
    scored: bool,
    mean_score: f64,
    total_states: u64,
    mean_ms: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BaselineReport {
    seeds: u64,
    max_states: u64,
    wall_seconds: f64,
    entries: Vec<EntryReport>,
}

struct Config {
    seeds: u64,
    max_states: u64,
    filter: Option<String>,
    rules_paths: Vec<PathBuf>,
    threads: usize,
    save: Option<PathBuf>,
    compare: Option<PathBuf>,
}

fn parse_args() -> Config {
    let mut cfg = Config {
        seeds: 20,
        max_states: 100_000,
        filter: None,
        rules_paths: Vec::new(),
        threads: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
        save: None,
        compare: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bench" => {}
            "--seeds" => {
                cfg.seeds = args
                    .next()
                    .expect("--seeds requires a value")
                    .parse()
                    .expect("invalid --seeds");
            }
            "--max-states" => {
                cfg.max_states = args
                    .next()
                    .expect("--max-states requires a value")
                    .parse()
                    .expect("invalid --max-states");
            }
            "--filter" => {
                cfg.filter = Some(args.next().expect("--filter requires a value"));
            }
            "--rules" => {
                let val = args.next().expect("--rules requires a path");
                if let Some(list_path) = val.strip_prefix('@') {
                    let text = fs::read_to_string(list_path).expect("failed to read rules list");
                    for line in text.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() && !trimmed.starts_with('#') {
                            cfg.rules_paths.push(PathBuf::from(trimmed));
                        }
                    }
                } else {
                    let p = PathBuf::from(val);
                    if p.is_dir() {
                        let mut files: Vec<PathBuf> = fs::read_dir(&p)
                            .expect("failed to read rules dir")
                            .filter_map(|e| e.ok().map(|e| e.path()))
                            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
                            .collect();
                        files.sort();
                        cfg.rules_paths.extend(files);
                    } else {
                        cfg.rules_paths.push(p);
                    }
                }
            }
            "--threads" => {
                cfg.threads = args
                    .next()
                    .expect("--threads requires a value")
                    .parse()
                    .expect("invalid --threads");
            }
            "--save" => {
                cfg.save = Some(PathBuf::from(args.next().expect("--save requires a path")));
            }
            "--compare" => {
                cfg.compare = Some(PathBuf::from(
                    args.next().expect("--compare requires a path"),
                ));
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: solver [--seeds N] [--max-states N] [--filter SUBSTR] \
                     [--rules PATH|DIR|@LIST] [--threads N] [--save JSON] [--compare JSON]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    cfg
}

enum DealSpec {
    Seed(u64),
    Microsoft(u32),
}

struct BenchSuite {
    label: String,
    game: Game,
    deals: Vec<DealSpec>,
    max_states: u64,
    scored: bool,
    assert_unsolvable: bool,
}

#[derive(Default)]
struct SuiteAccum {
    solved: u64,
    unsolvable: u64,
    gave_up: u64,
    total_score: u64,
    total_states: u64,
    total_ns: u128,
}

fn load_rules_file(path: &Path) -> (String, Rules) {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let rules: Rules =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom");
    let label = if rules.name.is_empty() {
        stem.to_string()
    } else {
        format!("{stem}: {}", rules.name)
    };
    (label, rules)
}

fn build_suites(cfg: &Config) -> Vec<BenchSuite> {
    let mut suites = Vec::new();
    let matches_filter = |label: &str| {
        cfg.filter
            .as_ref()
            .is_none_or(|f| label.to_lowercase().contains(&f.to_lowercase()))
    };

    if !cfg.rules_paths.is_empty() {
        for path in &cfg.rules_paths {
            let (label, rules) = load_rules_file(path);
            if !matches_filter(&label) {
                continue;
            }
            let game = Game::new(rules).unwrap_or_else(|e| panic!("compile {label}: {e}"));
            let scored = game.is_scored();
            let deals: Vec<DealSpec> = (1..=cfg.seeds).map(DealSpec::Seed).collect();
            suites.push(BenchSuite {
                label,
                game,
                deals,
                max_states: cfg.max_states,
                scored,
                assert_unsolvable: false,
            });
        }
        return suites;
    }

    // Special Microsoft FreeCell benchmarks.
    if matches_filter("freecell (ms #1..#20)") {
        suites.push(BenchSuite {
            label: "freecell (ms #1..#20)".to_string(),
            game: Game::new(presets::freecell()).expect("freecell"),
            deals: (1..=20).map(DealSpec::Microsoft).collect(),
            max_states: 500_000,
            scored: false,
            assert_unsolvable: false,
        });
    }
    if matches_filter("freecell (#11982 proof)") {
        suites.push(BenchSuite {
            label: "freecell (#11982 proof)".to_string(),
            game: Game::new(presets::freecell()).expect("freecell"),
            deals: vec![DealSpec::Microsoft(11982)],
            max_states: 2_000_000,
            scored: false,
            assert_unsolvable: true,
        });
    }

    // All built-in presets.
    for p in presets::PRESETS {
        if !matches_filter(p.id) {
            continue;
        }
        let game = Game::new((p.rules)()).unwrap_or_else(|e| panic!("preset {}: {e}", p.id));
        let scored = game.is_scored();
        let (n_seeds, max_st) = if scored {
            (cfg.seeds.min(5), cfg.max_states.min(50_000))
        } else {
            (cfg.seeds, cfg.max_states)
        };
        suites.push(BenchSuite {
            label: p.id.to_string(),
            game,
            deals: (1..=n_seeds).map(DealSpec::Seed).collect(),
            max_states: max_st,
            scored,
            assert_unsolvable: false,
        });
    }
    suites
}

fn deal_state(game: &Game, spec: &DealSpec) -> State {
    match *spec {
        DealSpec::Seed(seed) => game.deal(seed),
        DealSpec::Microsoft(n) => game
            .deal_ordered(&microsoft_deal(n))
            .expect("microsoft deal"),
    }
}

fn run_suites(suites: &[BenchSuite], threads: usize) -> Vec<EntryReport> {
    let tasks: Vec<(usize, usize)> = suites
        .iter()
        .enumerate()
        .flat_map(|(si, s)| (0..s.deals.len()).map(move |di| (si, di)))
        .collect();
    let accums: Vec<Mutex<SuiteAccum>> = (0..suites.len())
        .map(|_| Mutex::new(SuiteAccum::default()))
        .collect();
    let cursor = AtomicUsize::new(0);
    let n_threads = threads.max(1).min(tasks.len().max(1));

    std::thread::scope(|scope| {
        for _ in 0..n_threads {
            scope.spawn(|| loop {
                let idx = cursor.fetch_add(1, Ordering::Relaxed);
                if idx >= tasks.len() {
                    break;
                }
                let (si, di) = tasks[idx];
                let suite = &suites[si];
                let state = deal_state(&suite.game, &suite.deals[di]);
                let opts = SolveOptions::default().max_states(suite.max_states);
                let t0 = Instant::now();
                if suite.scored {
                    let res = maximize(&suite.game, &state, &opts);
                    let ns = t0.elapsed().as_nanos();
                    let mut acc = accums[si].lock().unwrap();
                    acc.total_score += u64::from(res.score);
                    acc.total_states += res.states;
                    acc.total_ns += ns;
                } else {
                    let res = solve(&suite.game, &state, &opts);
                    let ns = t0.elapsed().as_nanos();
                    if suite.assert_unsolvable {
                        assert_eq!(
                            res.outcome,
                            Outcome::Unsolvable,
                            "{} expected Unsolvable",
                            suite.label
                        );
                    }
                    let mut acc = accums[si].lock().unwrap();
                    match res.outcome {
                        Outcome::Solved(_) => acc.solved += 1,
                        Outcome::Unsolvable => acc.unsolvable += 1,
                        Outcome::GaveUp => acc.gave_up += 1,
                    }
                    acc.total_states += res.states;
                    acc.total_ns += ns;
                }
            });
        }
    });

    suites
        .iter()
        .zip(accums)
        .map(|(suite, m)| {
            let acc = m.into_inner().unwrap();
            let deals = suite.deals.len() as u64;
            EntryReport {
                label: suite.label.clone(),
                deals,
                solved: acc.solved,
                unsolvable: acc.unsolvable,
                gave_up: acc.gave_up,
                scored: suite.scored,
                mean_score: if suite.scored && deals > 0 {
                    acc.total_score as f64 / deals as f64
                } else {
                    0.0
                },
                total_states: acc.total_states,
                mean_ms: if deals > 0 {
                    acc.total_ns as f64 / 1_000_000.0 / deals as f64
                } else {
                    0.0
                },
            }
        })
        .collect()
}

fn print_report(report: &BaselineReport, baseline: Option<&BaselineReport>) {
    let base_map: BTreeMap<&str, &EntryReport> = baseline
        .map(|b| b.entries.iter().map(|e| (e.label.as_str(), e)).collect())
        .unwrap_or_default();

    println!(
        "Dealbreaker solver benchmark (seeds={}, max_states={})",
        report.seeds, report.max_states
    );
    let width = if baseline.is_some() { 116 } else { 90 };
    println!("{}", "-".repeat(width));

    let mut tot_deals = 0u64;
    let mut tot_solved = 0u64;
    let mut tot_unsolv = 0u64;
    let mut tot_gu = 0u64;
    let mut tot_states = 0u64;
    let mut tot_cpu_ms = 0.0f64;

    let mut base_solved = 0u64;
    let mut base_unsolv = 0u64;
    let mut base_gu = 0u64;
    let mut base_states = 0u64;
    let mut base_cpu_ms = 0.0f64;

    let mut regressions = Vec::new();
    let mut improvements = Vec::new();

    for e in &report.entries {
        if !e.scored {
            tot_deals += e.deals;
            tot_solved += e.solved;
            tot_unsolv += e.unsolvable;
            tot_gu += e.gave_up;
            tot_states += e.total_states;
            tot_cpu_ms += e.mean_ms * e.deals as f64;
        }
        let label = if e.label.len() > 28 {
            &e.label[..28]
        } else {
            &e.label
        };
        let delta_str = if let Some(b) = base_map.get(e.label.as_str()) {
            if !e.scored {
                base_solved += b.solved;
                base_unsolv += b.unsolvable;
                base_gu += b.gave_up;
                base_states += b.total_states;
                base_cpu_ms += b.mean_ms * b.deals as f64;
                let ds = e.solved as i64 - b.solved as i64;
                let ms_pct = if b.mean_ms > 0.01 {
                    (e.mean_ms - b.mean_ms) / b.mean_ms * 100.0
                } else {
                    0.0
                };
                if ds < 0 {
                    regressions.push(format!("{}: solved {:+}", e.label, ds));
                } else if ds > 0 {
                    improvements.push(format!("{}: solved {:+}", e.label, ds));
                }
                format!(" | Δsolv {:>+3} Δms {:>+6.1}%", ds, ms_pct)
            } else {
                let dsc = e.mean_score - b.mean_score;
                format!(" | Δscore {:>+6.1}", dsc)
            }
        } else {
            String::new()
        };

        if e.scored {
            println!(
                "{:<28} {:>3} deals | mean score {:>13.1} | {:>7.2} ms/deal | {:>9} states{}",
                label, e.deals, e.mean_score, e.mean_ms, e.total_states, delta_str
            );
        } else {
            println!(
                "{:<28} {:>3} deals | {:>3} solv {:>3} uns {:>3} gu | {:>7.2} ms/deal | {:>9} states{}",
                label, e.deals, e.solved, e.unsolvable, e.gave_up, e.mean_ms, e.total_states, delta_str
            );
        }
    }

    println!("{}", "-".repeat(width));
    let sr = if tot_deals > 0 {
        tot_solved as f64 / tot_deals as f64 * 100.0
    } else {
        0.0
    };
    let gur = if tot_deals > 0 {
        tot_gu as f64 / tot_deals as f64 * 100.0
    } else {
        0.0
    };
    println!(
        "TOTAL (non-scored): {} deals | {} solved ({:.1}%) | {} uns | {} gu ({:.1}%) | {} states | {:.2}s cpu | {:.2}s wall",
        tot_deals,
        tot_solved,
        sr,
        tot_unsolv,
        tot_gu,
        gur,
        tot_states,
        tot_cpu_ms / 1000.0,
        report.wall_seconds
    );
    if baseline.is_some() && base_states > 0 {
        let ds = tot_solved as i64 - base_solved as i64;
        let du = tot_unsolv as i64 - base_unsolv as i64;
        let dgu = tot_gu as i64 - base_gu as i64;
        let st_pct = (tot_states as f64 - base_states as f64) / base_states as f64 * 100.0;
        let cpu_pct = (tot_cpu_ms - base_cpu_ms) / base_cpu_ms.max(1e-9) * 100.0;
        println!(
            "DELTA vs baseline:  Δsolved {:+} | Δuns {:+} | Δgu {:+} | Δstates {:+.1}% | Δcpu {:+.1}%",
            ds, du, dgu, st_pct, cpu_pct
        );
        if !improvements.is_empty() {
            println!(
                "IMPROVED ({}): {}",
                improvements.len(),
                improvements.join(", ")
            );
        }
        if !regressions.is_empty() {
            println!(
                "REGRESSED ({}): {}",
                regressions.len(),
                regressions.join(", ")
            );
        }
    }
}

fn main() {
    let cfg = parse_args();
    let suites = build_suites(&cfg);
    let baseline = cfg.compare.as_ref().map(|p| {
        let text = fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str::<BaselineReport>(&text)
            .unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    });

    let t0 = Instant::now();
    let entries = run_suites(&suites, cfg.threads);
    let wall_seconds = t0.elapsed().as_secs_f64();

    let report = BaselineReport {
        seeds: cfg.seeds,
        max_states: cfg.max_states,
        wall_seconds,
        entries,
    };

    print_report(&report, baseline.as_ref());

    if let Some(save_path) = &cfg.save {
        let json = serde_json::to_string_pretty(&report).expect("serialize baseline");
        fs::write(save_path, json).unwrap_or_else(|e| panic!("write {}: {e}", save_path.display()));
        println!("Saved baseline to {}", save_path.display());
    }
}
