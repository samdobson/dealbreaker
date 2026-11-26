//! `dealbreaker`: solve patience games from the command line.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::time::Instant;

use dealbreaker::difficulty::{self, GradeThresholds};
use dealbreaker::{
    grade, hint, maximize, microsoft_deal, presets, solve, Card, Difficulty, Game, GradeOptions,
    HintOptions, Outcome, Rules, SearchHints, SolveOptions, State, StateData, Step, Strategy,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
dealbreaker: solve patience (solitaire) games

USAGE:
    dealbreaker list
    dealbreaker rules <GAME>
    dealbreaker validate <GAME> [--state FILE] [--json]
    dealbreaker deal  <GAME> [DEAL]
    dealbreaker solve <GAME> [DEAL] [SEARCH] [--json]
    dealbreaker hint  <GAME> [DEAL] [SEARCH] [--samples N] [--json]
    dealbreaker replay <GAME> [DEAL] --steps FILE [--legal] [--json]
    dealbreaker best  <GAME> [DEAL] [SEARCH] [--json]
    dealbreaker grade <GAME> [DEAL] [GRADE] [--json]
    dealbreaker grade <GAME> --seeds A..B [--microsoft] [--jobs N] [--calibrate] [GRADE] [--json] [--verbose]
    dealbreaker bench <GAME> --seeds A..B [--microsoft] [--best] [--jobs N] [SEARCH] [--json] [--verbose]
    dealbreaker serve <GAME> [--search JSON]
    dealbreaker --version

`validate` checks that a rules file (and optional `--state` JSON) is valid.

`replay` plays a list of steps (a JSON array, or `solve --json` output) from a deal and prints
the position after each one; `--legal` adds every legal step at each position.

`best` finds the highest score for games played for points; `bench --best` reports scores.

`grade` rates a deal easy, medium or hard (unwinnable deals are hard). With `--seeds`, it
grades many deals and reports how they spread; `--calibrate` also fits thresholds to them, a
third of the winnable deals per grade, and grades with those.

`serve` loads GAME once, reads one JSON request per line from stdin (`deal`, `solve`, `best`,
`hint`, `grade`, `legal_steps`, `play_step`, `validate`, `is_won`, `score`, `rules`, `quit`), and writes
one JSON response line per request to stdout.

GAME is a preset id (see `list`), a path to a rules JSON file, or `-` for stdin.

DEAL (default: --seed 1):
    --seed N          deal from a seed
    --ms N            Microsoft FreeCell deal number (FreeCell-layout games)
    --state FILE      a position as JSON (`deal` prints the format; `-` for stdin)

SEARCH:
    --max-states N    budget of distinct positions (default 2000000)
    --timeout-ms N    wall-clock time limit in milliseconds
    --strategy S      auto | dfs | best-first (default auto)
    --no-autoplay     don't play safe foundation moves automatically
    --no-shorten      don't shorten winning lines (faster when only solvability matters)
    --search JSON     override the game's search hints, e.g. '{\"blocker_weight\": 4}'
                      (speed only: verdicts never change)

GRADE:
    --max-states N        main search budget (default 500000)
    --probe-positions N   positions along the winning line where every move is tried (default 10)
    --probe-states N      budget for each move tried (default 3000)
    --thresholds M,H      score cut-offs: below M easy, below H medium (default: the game's own)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

const VALUE_FLAGS: &[&str] = &[
    "--seed",
    "--ms",
    "--state",
    "--max-states",
    "--timeout-ms",
    "--strategy",
    "--seeds",
    "--jobs",
    "--samples",
    "--search",
    "--steps",
    "--probe-positions",
    "--probe-states",
    "--thresholds",
];

const BOOL_FLAGS: &[&str] = &[
    "--json",
    "--legal",
    "--no-autoplay",
    "--no-shorten",
    "--microsoft",
    "--best",
    "--verbose",
    "--calibrate",
    "--help",
    "-h",
    "--version",
    "-V",
];

#[derive(Debug)]
struct Args {
    rest: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(v: &[String]) -> Result<Args, String> {
        let mut rest = Vec::new();
        let mut flags = Vec::new();
        let mut i = 0;
        while i < v.len() {
            let a = &v[i];
            if a == "-" {
                rest.push(a.clone());
            } else if let Some((k, val)) = a.split_once('=').filter(|_| a.starts_with("--")) {
                if VALUE_FLAGS.contains(&k) {
                    flags.push((k.to_string(), Some(val.to_string())));
                } else if BOOL_FLAGS.contains(&k) {
                    return Err(format!("{k} does not take a value"));
                } else {
                    return Err(format!("unknown flag {k:?}\n\n{USAGE}"));
                }
            } else if VALUE_FLAGS.contains(&a.as_str()) {
                let val = v
                    .get(i + 1)
                    .ok_or_else(|| format!("{a} requires a value"))?;
                flags.push((a.clone(), Some(val.clone())));
                i += 1;
            } else if BOOL_FLAGS.contains(&a.as_str()) {
                flags.push((a.clone(), None));
            } else if a.starts_with('-') {
                return Err(format!("unknown flag {a:?}\n\n{USAGE}"));
            } else {
                rest.push(a.clone());
            }
            i += 1;
        }
        Ok(Args { rest, flags })
    }

    fn check_allowed(&self, cmd: &str, allowed: &[&str]) -> Result<(), String> {
        for (f, _) in &self.flags {
            if !allowed.contains(&f.as_str()) {
                return Err(format!("unknown flag {f:?} for `{cmd}`\n\n{USAGE}"));
            }
        }
        Ok(())
    }

    fn get(&self, k: &str) -> Option<&str> {
        self.flags
            .iter()
            .rev()
            .find(|(f, _)| f == k)
            .and_then(|(_, v)| v.as_deref())
    }

    fn has(&self, k: &str) -> bool {
        self.flags.iter().any(|(f, _)| f == k)
    }

    fn num<T: std::str::FromStr>(&self, k: &str) -> Result<Option<T>, String> {
        self.get(k)
            .map(|v| v.parse().map_err(|_| format!("{k}: not a number: {v}")))
            .transpose()
    }
}

fn read_input(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf).map_err(|e| format!("stdin: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
    }
}

fn load_game(spec: &str, a: &Args) -> Result<Game, String> {
    let mut rules: Rules = if spec == "-" {
        let text = read_input("-")?;
        Rules::from_json(&text).map_err(|e| format!("stdin: {e}"))?
    } else {
        match presets::preset(spec) {
            Some(r) => r,
            None => {
                let text = std::fs::read_to_string(spec).map_err(|e| {
                    format!("{spec:?} is not a preset (see `list`) or a readable file: {e}")
                })?;
                Rules::from_json(&text).map_err(|e| format!("{spec}: {e}"))?
            }
        }
    };
    if let Some(json) = a.get("--search") {
        rules.search = merge_search(&rules.search, json)?;
    }
    Game::new(rules).map_err(|e| e.to_string())
}

/// The game's search hints with the fields given in `json` replaced.
fn merge_search(base: &SearchHints, json: &str) -> Result<SearchHints, String> {
    let bad = |e: serde_json::Error| format!("--search: {e}");
    let mut v = serde_json::to_value(base).map_err(bad)?;
    let patch: serde_json::Value = serde_json::from_str(json).map_err(bad)?;
    let (Some(obj), Some(p)) = (v.as_object_mut(), patch.as_object()) else {
        return Err("--search takes a JSON object, e.g. '{\"blocker_weight\": 4}'".into());
    };
    for (k, val) in p {
        obj.insert(k.clone(), val.clone());
    }
    serde_json::from_value(v).map_err(bad)
}

fn load_deal(game: &Game, a: &Args) -> Result<State, String> {
    if let Some(path) = a.get("--state") {
        let text = read_input(path)?;
        return game.state_from_json(&text).map_err(|e| e.to_string());
    }
    if let Some(n) = a.num::<u32>("--ms")? {
        return game
            .deal_ordered(&microsoft_deal(n))
            .map_err(|e| e.to_string());
    }
    Ok(game.deal(a.num::<u64>("--seed")?.unwrap_or(1)))
}

/// Steps from a JSON array, or from the `steps` field of `solve --json` output.
fn parse_steps(text: &str) -> Result<Vec<dealbreaker::Step>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let list = match v {
        serde_json::Value::Array(_) => v,
        serde_json::Value::Object(mut o) => match o.remove("steps") {
            Some(s @ serde_json::Value::Array(_)) => s,
            _ => return Err("no \"steps\" array (was the deal solved?)".into()),
        },
        _ => return Err("expected an array of steps".into()),
    };
    serde_json::from_value(list).map_err(|e| e.to_string())
}

fn search_opts(a: &Args) -> Result<SolveOptions, String> {
    let mut o = SolveOptions::default();
    if let Some(n) = a.num("--max-states")? {
        o.max_states = n;
    }
    if let Some(ms) = a.num("--timeout-ms")? {
        o.timeout_ms = Some(ms);
    }
    if let Some(s) = a.get("--strategy") {
        o.strategy = match s {
            "auto" => Strategy::Auto,
            "dfs" | "depth-first" => Strategy::DepthFirst,
            "bfs" | "best-first" => Strategy::BestFirst,
            _ => return Err(format!("unknown strategy {s:?}")),
        };
    }
    if a.has("--no-autoplay") {
        o.autoplay = false;
    }
    if a.has("--no-shorten") {
        o.shorten = false;
    }
    Ok(o)
}

fn grade_opts(a: &Args) -> Result<GradeOptions, String> {
    let mut o = GradeOptions::default();
    if let Some(n) = a.num("--max-states")? {
        o.max_states = n;
    }
    if let Some(n) = a.num("--probe-positions")? {
        o.probe_positions = n;
    }
    if let Some(n) = a.num("--probe-states")? {
        o.probe_states = n;
    }
    if let Some(t) = a.get("--thresholds") {
        let (m, h) = t
            .split_once(',')
            .and_then(|(m, h)| Some((m.trim().parse().ok()?, h.trim().parse().ok()?)))
            .ok_or_else(|| format!("--thresholds wants M,H (two numbers), got {t}"))?;
        if m > h {
            return Err(format!("--thresholds: {m} is above {h}"));
        }
        o.thresholds = Some(GradeThresholds::new(m, h));
    }
    Ok(o)
}

fn seed_range(a: &Args) -> Result<Vec<u64>, String> {
    let range = a.get("--seeds").unwrap_or("1..100");
    let (lo, hi) = range
        .split_once("..")
        .and_then(|(l, h)| Some((l.parse::<u64>().ok()?, h.parse::<u64>().ok()?)))
        .ok_or_else(|| format!("--seeds wants A..B, got {range}"))?;
    Ok((lo..=hi).collect())
}

/// One line describing a grade.
fn describe(d: &Difficulty) -> String {
    let why = match d.winnable {
        Some(true) => format!(
            "score {:.2}: effort {} states, {} of {} moves tried are traps, {} steps",
            d.score.unwrap_or(0.0),
            d.effort,
            d.traps,
            d.probes,
            d.steps
        ),
        Some(false) => format!("unwinnable ({} states)", d.effort),
        None => format!("no win found in {} states", d.effort),
    };
    format!("{} ({why})", d.grade)
}

fn grade_json(d: &Difficulty) -> serde_json::Value {
    serde_json::to_value(d).unwrap()
}

fn run(argv: &[String]) -> Result<(), String> {
    let a = Args::parse(argv)?;
    if a.has("--help") || a.has("-h") {
        print!("{USAGE}");
        return Ok(());
    }
    if a.has("--version") || a.has("-V") {
        println!("dealbreaker {VERSION}");
        return Ok(());
    }
    let cmd = a.rest.first().map(String::as_str).unwrap_or("help");
    let game_arg = || {
        a.rest
            .get(1)
            .ok_or_else(|| format!("`{cmd}` needs a game\n\n{USAGE}"))
    };
    const DEAL_FLAGS: &[&str] = &["--seed", "--ms", "--state", "--search"];
    match cmd {
        "list" => {
            a.check_allowed("list", &[])?;
            for p in presets::PRESETS {
                println!("{:<20} {}", p.id, (p.rules)().name);
            }
        }
        "rules" => {
            a.check_allowed("rules", &["--search"])?;
            let g = load_game(game_arg()?, &a)?;
            println!("{}", g.rules().to_json());
        }
        "validate" => {
            a.check_allowed("validate", &["--state", "--search", "--json"])?;
            let spec = game_arg()?;
            if spec == "-" && a.get("--state") == Some("-") {
                return Err("cannot read both game rules and --state from stdin".into());
            }
            let g = load_game(spec, &a)?;
            if let Some(path) = a.get("--state") {
                let text = read_input(path)?;
                g.state_from_json(&text).map_err(|e| e.to_string())?;
            }
            if a.has("--json") {
                println!("{}", serde_json::json!({ "ok": true, "game": g.name() }));
            } else {
                println!("ok ({})", g.name());
            }
        }
        "deal" => {
            a.check_allowed("deal", DEAL_FLAGS)?;
            let g = load_game(game_arg()?, &a)?;
            let s = load_deal(&g, &a)?;
            println!("{}", g.state_to_json(&s));
        }
        "solve" => {
            a.check_allowed(
                "solve",
                &[
                    "--seed",
                    "--ms",
                    "--state",
                    "--max-states",
                    "--timeout-ms",
                    "--strategy",
                    "--no-autoplay",
                    "--no-shorten",
                    "--search",
                    "--json",
                ],
            )?;
            let spec = game_arg()?;
            if spec == "-" && a.get("--state") == Some("-") {
                return Err("cannot read both game rules and --state from stdin".into());
            }
            let g = load_game(spec, &a)?;
            let s = load_deal(&g, &a)?;
            let t = Instant::now();
            let r = solve(&g, &s, &search_opts(&a)?);
            let secs = t.elapsed().as_secs_f64();
            if a.has("--json") {
                let steps = match &r.outcome {
                    Outcome::Solved(m) => Some(g.steps(&s, m)),
                    _ => None,
                };
                let out = serde_json::json!({
                    "game": g.name(),
                    "outcome": r.outcome.label(),
                    "states": r.states,
                    "seconds": secs,
                    "steps": steps,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                print!("{}", g.render(&s));
                println!();
                match &r.outcome {
                    Outcome::Solved(m) => {
                        let steps = g.steps(&s, m);
                        for (i, st) in steps.iter().enumerate() {
                            println!("{:>4}. {st}", i + 1);
                        }
                        println!(
                            "\nsolved in {} steps ({} states, {secs:.3}s)",
                            steps.len(),
                            r.states
                        );
                    }
                    Outcome::Unsolvable => println!("unsolvable ({} states, {secs:.3}s)", r.states),
                    Outcome::GaveUp => println!("gave up after {} states ({secs:.3}s)", r.states),
                }
            }
        }
        "replay" => {
            a.check_allowed(
                "replay",
                &[
                    "--seed", "--ms", "--state", "--steps", "--legal", "--json", "--search",
                ],
            )?;
            let spec = game_arg()?;
            let path = a.get("--steps").ok_or("`replay` needs --steps FILE")?;
            let stdin_uses = [Some(spec.as_str()), a.get("--state"), Some(path)]
                .into_iter()
                .filter(|&p| p == Some("-"))
                .count();
            if stdin_uses > 1 {
                return Err("only one input may be read from stdin (`-`)".into());
            }
            let g = load_game(spec, &a)?;
            let mut s = load_deal(&g, &a)?;
            let text = read_input(path)?;
            let steps = parse_steps(&text).map_err(|e| format!("{path}: {e}"))?;
            let legal = a.has("--legal");
            let json = a.has("--json");
            let mut positions = Vec::new();
            for i in 0..=steps.len() {
                if i > 0 {
                    g.play_step(&mut s, &steps[i - 1])
                        .map_err(|e| format!("step {} is not legal: {e}", i))?;
                }
                if json {
                    let state: serde_json::Value =
                        serde_json::from_str(&g.state_to_json(&s)).map_err(|e| e.to_string())?;
                    let mut p = serde_json::json!({
                        "index": i,
                        "step": if i > 0 { Some(&steps[i - 1]) } else { None },
                        "state": state,
                    });
                    if legal {
                        p["legal"] = serde_json::json!(g.legal_steps(&s));
                    }
                    positions.push(p);
                } else {
                    if i > 0 {
                        println!("{:>4}. {}", i, steps[i - 1]);
                    }
                    print!("{}", g.render(&s));
                    if legal {
                        for st in g.legal_steps(&s) {
                            println!("  legal: {st}");
                        }
                    }
                    println!();
                }
            }
            if json {
                let out = serde_json::json!({ "game": g.name(), "positions": positions });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            }
        }
        "hint" => {
            a.check_allowed(
                "hint",
                &[
                    "--seed",
                    "--ms",
                    "--state",
                    "--max-states",
                    "--timeout-ms",
                    "--strategy",
                    "--no-autoplay",
                    "--no-shorten",
                    "--search",
                    "--samples",
                    "--json",
                ],
            )?;
            let spec = game_arg()?;
            if spec == "-" && a.get("--state") == Some("-") {
                return Err("cannot read both game rules and --state from stdin".into());
            }
            let g = load_game(spec, &a)?;
            let s = if let Some(path) = a.get("--state") {
                let text = read_input(path)?;
                g.hint_state_from_json(&text).map_err(|e| e.to_string())?
            } else {
                load_deal(&g, &a)?
            };
            let mut o = HintOptions::default().solve(search_opts(&a)?);
            if a.get("--max-states").is_none() {
                o.solve.max_states = 100_000;
            }
            if let Some(n) = a.num("--samples")? {
                o.samples = n;
            }
            let res = hint(&g, &s, &o);
            if a.has("--json") {
                let out = serde_json::json!({
                    "game": g.name(),
                    "hint": res,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                match res {
                    Some(h) => println!("{} (won {} of {} guesses)", h.step, h.wins, h.samples),
                    None => println!("no winning line found"),
                }
            }
        }
        "best" => {
            a.check_allowed(
                "best",
                &[
                    "--seed",
                    "--ms",
                    "--state",
                    "--max-states",
                    "--timeout-ms",
                    "--strategy",
                    "--no-autoplay",
                    "--no-shorten",
                    "--search",
                    "--json",
                ],
            )?;
            let spec = game_arg()?;
            if spec == "-" && a.get("--state") == Some("-") {
                return Err("cannot read both game rules and --state from stdin".into());
            }
            let g = load_game(spec, &a)?;
            if !g.is_scored() {
                return Err(format!("{} isn't played for points", g.name()));
            }
            let s = load_deal(&g, &a)?;
            let t = Instant::now();
            let r = maximize(&g, &s, &search_opts(&a)?);
            let secs = t.elapsed().as_secs_f64();
            let steps = g.steps(&s, &r.moves);
            if a.has("--json") {
                let out = serde_json::json!({
                    "game": g.name(), "score": r.score, "proven": r.proven,
                    "states": r.states, "seconds": secs, "steps": steps,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                let mut end = s.clone();
                for &m in &r.moves {
                    g.play(&mut end, m).map_err(|e| e.to_string())?;
                }
                for (i, st) in steps.iter().enumerate() {
                    println!("{:>4}. {st}", i + 1);
                }
                println!();
                print!("{}", g.render(&end));
                println!(
                    "\n{} points{} ({} states, {secs:.3}s)",
                    r.score,
                    if r.proven {
                        ", the most possible"
                    } else {
                        ", best found"
                    },
                    r.states
                );
            }
        }
        "grade" => {
            const GRADE_FLAGS: &[&str] = &[
                "--seed",
                "--ms",
                "--state",
                "--seeds",
                "--microsoft",
                "--jobs",
                "--calibrate",
                "--max-states",
                "--probe-positions",
                "--probe-states",
                "--thresholds",
                "--search",
                "--json",
                "--verbose",
            ];
            a.check_allowed("grade", GRADE_FLAGS)?;
            let spec = game_arg()?;
            if spec == "-" && a.get("--state") == Some("-") {
                return Err("cannot read both game rules and --state from stdin".into());
            }
            let g = load_game(spec, &a)?;
            let mut o = grade_opts(&a)?;
            if !a.has("--seeds") {
                for f in ["--microsoft", "--jobs", "--calibrate", "--verbose"] {
                    if a.has(f) {
                        return Err(format!("{f} needs --seeds"));
                    }
                }
                let s = load_deal(&g, &a)?;
                let t = Instant::now();
                let d = grade(&g, &s, &o);
                let secs = t.elapsed().as_secs_f64();
                if a.has("--json") {
                    let mut out = grade_json(&d);
                    out["game"] = serde_json::json!(g.name());
                    out["seconds"] = serde_json::json!(secs);
                    println!("{}", serde_json::to_string_pretty(&out).unwrap());
                } else {
                    println!("{} ({secs:.3}s)", describe(&d));
                }
                return Ok(());
            }
            for f in ["--seed", "--ms", "--state"] {
                if a.has(f) {
                    return Err(format!("{f} can't be combined with --seeds"));
                }
            }
            if a.has("--calibrate") && o.thresholds.is_some() {
                return Err("--calibrate and --thresholds can't be combined".into());
            }
            let seeds = seed_range(&a)?;
            let ms = a.has("--microsoft");
            let t = Instant::now();
            let graded = run_bench(&seeds, a.num::<usize>("--jobs")?, |&seed| {
                let s = if ms {
                    g.deal_ordered(&microsoft_deal(seed as u32))
                        .map_err(|e| e.to_string())?
                } else {
                    g.deal(seed)
                };
                Ok::<_, String>(grade(&g, &s, &o))
            })?
            .into_iter()
            .collect::<Result<Vec<Difficulty>, String>>()?;
            let secs = t.elapsed().as_secs_f64();
            let cal = if a.has("--calibrate") {
                let c = difficulty::calibrate(&graded)
                    .ok_or("--calibrate: none of the deals was won")?;
                o.thresholds = Some(c.thresholds);
                Some(c)
            } else {
                None
            };
            // Regrade from the scores (thresholds may have changed).
            let graded: Vec<Difficulty> = graded
                .into_iter()
                .map(|mut d| {
                    if let Some(t) = o.thresholds {
                        d.thresholds = t;
                        d.source = dealbreaker::ThresholdSource::Custom;
                        if let Some(sc) = d.score {
                            d.grade = t.grade(sc);
                        }
                    }
                    d
                })
                .collect();
            let count = |gr: dealbreaker::Grade| graded.iter().filter(|d| d.grade == gr).count();
            let unwinnable = graded.iter().filter(|d| d.winnable == Some(false)).count();
            let unknown = graded.iter().filter(|d| d.winnable.is_none()).count();
            let (easy, medium, hard) = (
                count(dealbreaker::Grade::Easy),
                count(dealbreaker::Grade::Medium),
                count(dealbreaker::Grade::Hard),
            );
            let thresholds = o.thresholds.or(graded.first().map(|d| d.thresholds));
            if a.has("--json") {
                let mut out = serde_json::json!({
                    "game": g.name(),
                    "preset": difficulty::preset_id(&g),
                    "deals": graded.len(),
                    "easy": easy, "medium": medium, "hard": hard,
                    "unwinnable": unwinnable, "unknown": unknown,
                    "thresholds": thresholds,
                    "seconds": secs,
                });
                if a.has("--verbose") {
                    out["grades"] = graded
                        .iter()
                        .zip(&seeds)
                        .map(|(d, seed)| {
                            let mut v = grade_json(d);
                            v["seed"] = serde_json::json!(seed);
                            v
                        })
                        .collect();
                }
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                if a.has("--verbose") {
                    for (d, seed) in graded.iter().zip(&seeds) {
                        println!("{seed}: {}", describe(d));
                    }
                }
                let n = graded.len().max(1) as f64;
                let pct = |k: usize| 100.0 * k as f64 / n;
                println!(
                    "{}: {} deals: {easy} easy ({:.0}%), {medium} medium ({:.0}%), {hard} hard \
                     ({:.0}%; {unwinnable} unwinnable, {unknown} not won in budget); {:.3}s/deal",
                    g.name(),
                    graded.len(),
                    pct(easy),
                    pct(medium),
                    pct(hard),
                    secs / n,
                );
                if let (Some(c), Some(t)) = (cal, thresholds) {
                    println!(
                        "calibrated from {} winnable deals: --thresholds {:.2},{:.2}",
                        c.winnable, t.medium, t.hard
                    );
                }
            }
        }
        "bench" if a.has("--best") => {
            a.check_allowed(
                "bench",
                &[
                    "--seeds",
                    "--microsoft",
                    "--best",
                    "--jobs",
                    "--max-states",
                    "--timeout-ms",
                    "--strategy",
                    "--no-autoplay",
                    "--no-shorten",
                    "--search",
                    "--json",
                    "--verbose",
                ],
            )?;
            let g = load_game(game_arg()?, &a)?;
            if !g.is_scored() {
                return Err(format!("{} isn't played for points", g.name()));
            }
            let range = a.get("--seeds").unwrap_or("1..100");
            let (lo, hi) = range
                .split_once("..")
                .and_then(|(l, h)| Some((l.parse::<u64>().ok()?, h.parse::<u64>().ok()?)))
                .ok_or_else(|| format!("--seeds wants A..B, got {range}"))?;
            let o = search_opts(&a)?;
            let jobs = a.num::<usize>("--jobs")?;
            let seeds: Vec<u64> = (lo..=hi).collect();
            let t = Instant::now();
            let run_one = |&seed: &u64| {
                let s = g.deal(seed);
                let t1 = Instant::now();
                let r = maximize(&g, &s, &o);
                (
                    seed,
                    r.score,
                    r.proven,
                    r.states,
                    t1.elapsed().as_secs_f64(),
                )
            };
            let results: Vec<_> = run_bench(&seeds, jobs, run_one)?;
            let (mut total, mut proven, mut states) = (0u64, 0u32, 0u64);
            let (mut lowest, mut highest) = (u32::MAX, 0u32);
            for (seed, score, is_proven, st, dt) in results {
                total += score as u64;
                proven += is_proven as u32;
                states += st;
                lowest = lowest.min(score);
                highest = highest.max(score);
                if a.has("--verbose") {
                    println!(
                        "{seed}: {score} points{} ({st} states, {dt:.3}s)",
                        if is_proven { " (proven)" } else { "" },
                    );
                }
            }
            let n = hi - lo + 1;
            let secs = t.elapsed().as_secs_f64();
            let mean = total as f64 / n as f64;
            if a.has("--json") {
                let out = serde_json::json!({
                    "game": g.name(), "deals": n, "mean_score": mean, "lowest": lowest,
                    "highest": highest, "proven": proven, "states": states, "seconds": secs,
                });
                println!("{out}");
            } else {
                println!(
                    "{}: {n} deals: mean {mean:.1} points (lowest {lowest}, highest {highest}), \
                     {proven} proven best; {:.0} states/deal, {:.3}s/deal",
                    g.name(),
                    states as f64 / n as f64,
                    secs / n as f64,
                );
            }
        }
        "bench" => {
            a.check_allowed(
                "bench",
                &[
                    "--seeds",
                    "--microsoft",
                    "--best",
                    "--jobs",
                    "--max-states",
                    "--timeout-ms",
                    "--strategy",
                    "--no-autoplay",
                    "--no-shorten",
                    "--search",
                    "--json",
                    "--verbose",
                ],
            )?;
            let g = load_game(game_arg()?, &a)?;
            let range = a.get("--seeds").unwrap_or("1..100");
            let (lo, hi) = range
                .split_once("..")
                .and_then(|(l, h)| Some((l.parse::<u64>().ok()?, h.parse::<u64>().ok()?)))
                .ok_or_else(|| format!("--seeds wants A..B, got {range}"))?;
            let o = search_opts(&a)?;
            let jobs = a.num::<usize>("--jobs")?;
            let ms = a.has("--microsoft");
            let seeds: Vec<u64> = (lo..=hi).collect();
            let t = Instant::now();
            let run_one = |&seed: &u64| -> Result<(u64, &'static str, u64, usize, f64), String> {
                let s = if ms {
                    g.deal_ordered(&microsoft_deal(seed as u32))
                        .map_err(|e| e.to_string())?
                } else {
                    g.deal(seed)
                };
                let t1 = Instant::now();
                let r = solve(&g, &s, &o);
                let dt = t1.elapsed().as_secs_f64();
                let steps = match &r.outcome {
                    Outcome::Solved(m) => g.steps(&s, m).len(),
                    _ => 0,
                };
                Ok((seed, r.outcome.label(), r.states, steps, dt))
            };
            let raw: Vec<Result<_, String>> = run_bench(&seeds, jobs, run_one)?;
            let (mut solved, mut unsolvable, mut gaveup) = (0, 0, 0);
            let mut states = 0u64;
            let mut steps_total = 0usize;
            let mut worst = (0f64, 0u64);
            for item in raw {
                let (seed, label, st, steps, dt) = item?;
                if dt > worst.0 {
                    worst = (dt, seed);
                }
                states += st;
                match label {
                    "solved" => {
                        solved += 1;
                        steps_total += steps;
                    }
                    "unsolvable" => unsolvable += 1,
                    _ => gaveup += 1,
                }
                if a.has("--verbose") {
                    println!("{seed}: {label} ({st} states, {dt:.3}s)");
                }
            }
            let n = hi - lo + 1;
            let secs = t.elapsed().as_secs_f64();
            if a.has("--json") {
                let out = serde_json::json!({
                    "game": g.name(), "deals": n, "solved": solved, "unsolvable": unsolvable,
                    "gave_up": gaveup, "states": states, "seconds": secs,
                    "mean_steps": if solved > 0 { steps_total as f64 / solved as f64 } else { 0.0 },
                });
                println!("{out}");
            } else {
                println!(
                    "{}: {n} deals: {solved} solved, {unsolvable} unsolvable, {gaveup} gave up; \
                     {:.0} states/deal, {:.1} steps/solution, {:.3}s/deal (slowest: seed {} {:.2}s)",
                    g.name(),
                    states as f64 / n as f64,
                    if solved > 0 { steps_total as f64 / solved as f64 } else { 0.0 },
                    secs / n as f64,
                    worst.1,
                    worst.0,
                );
            }
        }
        "serve" => {
            a.check_allowed("serve", &["--search"])?;
            let g = load_game(game_arg()?, &a)?;
            let stdin = io::stdin();
            let stdout = io::stdout();
            serve_io(&g, stdin.lock(), stdout.lock())?;
        }
        "version" => {
            a.check_allowed("version", &[])?;
            println!("dealbreaker {VERSION}");
        }
        "help" => print!("{USAGE}"),
        other => return Err(format!("unknown command {other:?}\n\n{USAGE}")),
    }
    Ok(())
}

/// Run a benchmark closure over `seeds`, in parallel when the `parallel` feature is enabled
/// (unless `--jobs 1` is passed). Results are returned in input seed order.
fn run_bench<T, F>(seeds: &[u64], jobs: Option<usize>, f: F) -> Result<Vec<T>, String>
where
    T: Send,
    F: Fn(&u64) -> T + Sync + Send,
{
    if jobs == Some(0) {
        return Err("--jobs must be at least 1".into());
    }
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        if jobs == Some(1) {
            return Ok(seeds.iter().map(f).collect());
        }
        if let Some(n) = jobs {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build()
                .map_err(|e| format!("failed to build thread pool: {e}"))?;
            return Ok(pool.install(|| seeds.par_iter().map(f).collect()));
        }
        Ok(seeds.par_iter().map(f).collect())
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = jobs;
        Ok(seeds.iter().map(f).collect())
    }
}

/// Long-lived JSONL stdio loop: one JSON request per line, one JSON response per line.
fn serve_io<R: BufRead, W: Write>(game: &Game, reader: R, mut writer: W) -> Result<(), String> {
    for line in reader.lines() {
        let line = line.map_err(|e| format!("stdin read error: {e}"))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (resp, quit) = handle_serve_line(game, trimmed);
        serde_json::to_writer(&mut writer, &resp)
            .map_err(|e| format!("stdout write error: {e}"))?;
        writer
            .write_all(b"\n")
            .and_then(|()| writer.flush())
            .map_err(|e| format!("stdout write error: {e}"))?;
        if quit {
            break;
        }
    }
    Ok(())
}

fn handle_serve_line(game: &Game, line: &str) -> (serde_json::Value, bool) {
    let val: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            return (
                serde_json::json!({ "ok": false, "error": format!("invalid JSON: {e}") }),
                false,
            );
        }
    };
    let id = val.get("id").cloned();
    let wrap_err = |msg: String| {
        let mut out = serde_json::json!({ "ok": false, "error": msg });
        if let Some(ref id) = id {
            out["id"] = id.clone();
        }
        out
    };
    let Some(obj) = val.as_object() else {
        return (wrap_err("request must be a JSON object".into()), false);
    };
    let Some(cmd) = obj.get("cmd").and_then(serde_json::Value::as_str) else {
        return (wrap_err("missing string field \"cmd\"".into()), false);
    };
    let parse_state = |obj: &serde_json::Map<String, serde_json::Value>| -> Result<State, String> {
        let v = obj
            .get("state")
            .ok_or_else(|| "missing \"state\"".to_string())?;
        let data: StateData =
            serde_json::from_value(v.clone()).map_err(|e| format!("invalid state: {e}"))?;
        game.state_from_data(&data).map_err(|e| e.to_string())
    };
    let mut quit = false;
    let res: Result<serde_json::Value, String> = match cmd {
        "quit" => {
            quit = true;
            Ok(serde_json::json!({ "ok": true }))
        }
        "rules" => Ok(serde_json::json!({ "ok": true, "rules": game.rules() })),
        "deal" => (|| {
            let state = if let Some(ms) = obj.get("ms") {
                let n = ms
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or_else(|| "\"ms\" must be a non-negative 32-bit integer".to_string())?;
                game.deal_ordered(&microsoft_deal(n))
                    .map_err(|e| e.to_string())?
            } else if let Some(cards_val) = obj.get("cards") {
                let names: Vec<String> = serde_json::from_value(cards_val.clone())
                    .map_err(|e| format!("invalid \"cards\": {e}"))?;
                let cards: Vec<Card> = names
                    .iter()
                    .map(|c| c.parse::<Card>().map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
                game.deal_ordered(&cards).map_err(|e| e.to_string())?
            } else {
                let seed = match obj.get("seed") {
                    Some(v) => v
                        .as_u64()
                        .ok_or_else(|| "\"seed\" must be a non-negative integer".to_string())?,
                    None => 1,
                };
                game.deal(seed)
            };
            Ok(serde_json::json!({ "ok": true, "state": game.state_to_data(&state) }))
        })(),
        "validate" => parse_state(obj).map(|_| serde_json::json!({ "ok": true })),
        "is_won" => {
            parse_state(obj).map(|s| serde_json::json!({ "ok": true, "won": game.is_won(&s) }))
        }
        "score" => {
            parse_state(obj).map(|s| serde_json::json!({ "ok": true, "score": game.score(&s) }))
        }
        "legal_steps" => parse_state(obj)
            .map(|s| serde_json::json!({ "ok": true, "steps": game.legal_steps(&s) })),
        "play_step" => (|| {
            let mut s = parse_state(obj)?;
            let step_val = obj
                .get("step")
                .ok_or_else(|| "missing \"step\"".to_string())?;
            let step: Step = serde_json::from_value(step_val.clone())
                .map_err(|e| format!("invalid step: {e}"))?;
            game.play_step(&mut s, &step).map_err(|e| e.to_string())?;
            let mut out = serde_json::json!({
                "ok": true,
                "state": game.state_to_data(&s),
                "won": game.is_won(&s),
            });
            if let Some(sc) = game.score(&s) {
                out["score"] = serde_json::json!(sc);
            }
            Ok(out)
        })(),
        "solve" => (|| {
            let s = parse_state(obj)?;
            let opts: SolveOptions = match obj.get("options") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid options: {e}"))?,
                None => SolveOptions::default(),
            };
            let r = solve(game, &s, &opts);
            let mut out = serde_json::json!({
                "ok": true,
                "outcome": r.outcome.label(),
                "states": r.states,
            });
            if let Outcome::Solved(moves) = &r.outcome {
                out["steps"] = serde_json::json!(game.steps(&s, moves));
            }
            Ok(out)
        })(),
        "best" => (|| {
            if !game.is_scored() {
                return Err(format!("{} isn't played for points", game.name()));
            }
            let s = parse_state(obj)?;
            let opts: SolveOptions = match obj.get("options") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid options: {e}"))?,
                None => SolveOptions::default(),
            };
            let r = maximize(game, &s, &opts);
            Ok(serde_json::json!({
                "ok": true,
                "score": r.score,
                "proven": r.proven,
                "states": r.states,
                "steps": game.steps(&s, &r.moves),
            }))
        })(),
        "hint" => (|| {
            let v = obj
                .get("state")
                .ok_or_else(|| "missing \"state\"".to_string())?;
            let data: StateData =
                serde_json::from_value(v.clone()).map_err(|e| format!("invalid state: {e}"))?;
            let s = game
                .hint_state_from_data(&data)
                .map_err(|e| e.to_string())?;
            let opts: HintOptions = match obj.get("options") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid options: {e}"))?,
                None => HintOptions::default(),
            };
            Ok(serde_json::json!({ "ok": true, "hint": hint(game, &s, &opts) }))
        })(),
        "grade" => (|| {
            let s = parse_state(obj)?;
            let opts: GradeOptions = match obj.get("options") {
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid options: {e}"))?,
                None => GradeOptions::default(),
            };
            Ok(serde_json::json!({ "ok": true, "difficulty": grade(game, &s, &opts) }))
        })(),
        other => Err(format!("unknown cmd {other:?}")),
    };
    let mut out = match res {
        Ok(v) => v,
        Err(e) => wrap_err(e),
    };
    if let Some(id) = id {
        out["id"] = id;
    }
    (out, quit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const STEP: &str = r#"{"type":"deal"}"#;
    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempFile {
        path: PathBuf,
    }

    impl TempFile {
        fn new(name: &str, contents: &str) -> Self {
            let id = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "dealbreaker_cli_test_{}_{}_{}",
                std::process::id(),
                id,
                name
            ));
            std::fs::write(&path, contents).unwrap();
            Self { path }
        }

        fn str(&self) -> &str {
            self.path.to_str().unwrap()
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn steps_from_an_array_or_solve_output() {
        let array = format!("[{STEP},{STEP}]");
        assert_eq!(parse_steps(&array).unwrap().len(), 2);
        let solved = format!(r#"{{"outcome":"solved","steps":[{STEP}]}}"#);
        assert_eq!(parse_steps(&solved).unwrap().len(), 1);
    }

    #[test]
    fn unsolved_output_has_no_steps() {
        let err = parse_steps(r#"{"outcome":"gave-up","steps":null}"#).unwrap_err();
        assert!(err.contains("steps"), "{err}");
        assert!(parse_steps("3").is_err());
        assert!(parse_steps(r#"[{"type":"nonsense"}]"#).is_err());
    }

    #[test]
    fn args_parsing_and_search_merge() {
        let a = Args::parse(&argv(&[
            "solve", "freecell", "--seed=3", "--seed", "5", "--json",
        ]))
        .unwrap();
        assert_eq!(a.rest, ["solve", "freecell"]);
        assert_eq!(a.get("--seed"), Some("5"));
        assert_eq!(a.num::<u64>("--seed").unwrap(), Some(5));
        assert!(a.has("--json"));
        assert!(!a.has("--verbose"));

        let bad = Args::parse(&argv(&["--seed", "nope"])).unwrap();
        assert!(bad
            .num::<u64>("--seed")
            .unwrap_err()
            .contains("not a number"));

        // Unknown flags, missing values, and values on bool flags are rejected.
        assert!(Args::parse(&argv(&["solve", "freecell", "--bogus"]))
            .unwrap_err()
            .contains("unknown flag"));
        assert!(Args::parse(&argv(&["solve", "freecell", "--seed"]))
            .unwrap_err()
            .contains("requires a value"));
        assert!(Args::parse(&argv(&["solve", "freecell", "--json=true"]))
            .unwrap_err()
            .contains("does not take a value"));
        assert!(run(&argv(&["solve", "freecell", "--samples", "4"]))
            .unwrap_err()
            .contains("unknown flag"));
        run(&argv(&["--version"])).unwrap();
        run(&argv(&["-V"])).unwrap();
        run(&argv(&["version"])).unwrap();
        run(&argv(&["solve", "--help"])).unwrap();
        run(&argv(&["validate", "freecell"])).unwrap();
        run(&argv(&["validate", "freecell", "--json"])).unwrap();
        run(&argv(&[
            "hint",
            "freecell",
            "--ms",
            "5",
            "--samples",
            "1",
            "--json",
        ]))
        .unwrap();

        let base = SearchHints::default();
        let merged = merge_search(&base, r#"{"blocker_weight": 4}"#).unwrap();
        assert_ne!(
            serde_json::to_value(&merged).unwrap(),
            serde_json::to_value(&base).unwrap()
        );
        assert!(merge_search(&base, "[]")
            .unwrap_err()
            .contains("JSON object"));
        assert!(merge_search(&base, "{bad").is_err());
    }

    #[test]
    fn cli_commands_happy_paths() {
        run(&argv(&[])).unwrap();
        run(&argv(&["help"])).unwrap();
        run(&argv(&["--help"])).unwrap();
        run(&argv(&["-h"])).unwrap();
        run(&argv(&["list"])).unwrap();
        run(&argv(&["rules", "freecell"])).unwrap();

        let rules_json = presets::freecell().to_json();
        let rules_file = TempFile::new("rules.json", &rules_json);
        run(&argv(&["rules", rules_file.str()])).unwrap();

        run(&argv(&["deal", "freecell", "--seed", "2"])).unwrap();
        run(&argv(&["deal", "freecell", "--ms", "1"])).unwrap();

        let g = Game::new(presets::freecell()).unwrap();
        let state_json = g.state_to_json(&g.deal_ordered(&microsoft_deal(1)).unwrap());
        let state_file = TempFile::new("state.json", &state_json);
        run(&argv(&["deal", "freecell", "--state", state_file.str()])).unwrap();

        run(&argv(&[
            "solve",
            "freecell",
            "--ms",
            "1",
            "--max-states",
            "200000",
            "--json",
        ]))
        .unwrap();
        run(&argv(&[
            "solve",
            "freecell",
            "--ms",
            "1",
            "--strategy",
            "dfs",
            "--no-autoplay",
            "--no-shorten",
            "--max-states",
            "200000",
        ]))
        .unwrap();
        run(&argv(&[
            "solve",
            "freecell",
            "--ms",
            "1",
            "--strategy",
            "best-first",
            "--search",
            r#"{"blocker_weight":4}"#,
            "--max-states",
            "5",
        ]))
        .unwrap();

        let s1 = g.deal_ordered(&microsoft_deal(1)).unwrap();
        let r = solve(&g, &s1, &SolveOptions::default().max_states(200_000));
        let Outcome::Solved(moves) = &r.outcome else {
            panic!("expected ms deal 1 to solve");
        };
        let steps_json = serde_json::to_string(&g.steps(&s1, moves)).unwrap();
        let steps_file = TempFile::new("steps.json", &steps_json);
        run(&argv(&[
            "replay",
            "freecell",
            "--ms",
            "1",
            "--steps",
            steps_file.str(),
        ]))
        .unwrap();
        run(&argv(&[
            "replay",
            "freecell",
            "--ms",
            "1",
            "--steps",
            steps_file.str(),
            "--legal",
            "--json",
        ]))
        .unwrap();

        run(&argv(&["hint", "freecell", "--ms", "5", "--samples", "1"])).unwrap();

        run(&argv(&[
            "best",
            "poker-squares",
            "--seed",
            "1",
            "--max-states",
            "2000",
            "--json",
        ]))
        .unwrap();
        run(&argv(&[
            "best",
            "poker-squares",
            "--seed",
            "1",
            "--max-states",
            "2000",
        ]))
        .unwrap();

        run(&argv(&[
            "bench",
            "freecell",
            "--seeds",
            "1..2",
            "--microsoft",
            "--max-states",
            "200000",
            "--verbose",
            "--json",
        ]))
        .unwrap();
        run(&argv(&[
            "bench",
            "freecell",
            "--seeds",
            "1..2",
            "--max-states",
            "10",
        ]))
        .unwrap();
        run(&argv(&[
            "bench",
            "poker-squares",
            "--best",
            "--seeds",
            "1..2",
            "--max-states",
            "1000",
            "--verbose",
            "--json",
        ]))
        .unwrap();
        run(&argv(&[
            "bench",
            "poker-squares",
            "--best",
            "--seeds",
            "1..2",
            "--max-states",
            "1000",
        ]))
        .unwrap();
    }

    #[test]
    fn cli_commands_error_paths() {
        assert!(run(&argv(&["bogus"]))
            .unwrap_err()
            .contains("unknown command"));
        assert!(run(&argv(&["solve"])).unwrap_err().contains("needs a game"));
        assert!(run(&argv(&["rules", "no-such-game-or-file"])).is_err());
        assert!(run(&argv(&["deal", "spider", "--ms", "1"])).is_err());
        assert!(run(&argv(&["solve", "freecell", "--strategy", "bogus"]))
            .unwrap_err()
            .contains("unknown strategy"));
        assert!(run(&argv(&["replay", "freecell"]))
            .unwrap_err()
            .contains("--steps"));
        let bad_steps = TempFile::new("bad_steps.json", r#"[{"type":"deal"}]"#);
        assert!(run(&argv(&[
            "replay",
            "freecell",
            "--ms",
            "1",
            "--steps",
            bad_steps.str(),
        ]))
        .unwrap_err()
        .contains("not legal"));
        assert!(run(&argv(&["best", "freecell"]))
            .unwrap_err()
            .contains("points"));
        assert!(run(&argv(&["bench", "freecell", "--best"]))
            .unwrap_err()
            .contains("points"));
        assert!(run(&argv(&["bench", "freecell", "--seeds", "bad"]))
            .unwrap_err()
            .contains("A..B"));
        assert!(run(&argv(&[
            "bench", "freecell", "--seeds", "1..2", "--jobs", "0"
        ]))
        .unwrap_err()
        .contains("--jobs"));
        run(&argv(&[
            "bench",
            "freecell",
            "--seeds",
            "1..2",
            "--jobs",
            "2",
            "--max-states",
            "5000",
        ]))
        .unwrap();
    }

    #[test]
    fn serve_io_handles_commands_and_errors() {
        let fc = Game::new(presets::freecell()).unwrap();
        let deal = fc.deal_ordered(&microsoft_deal(1)).unwrap();
        let state_json = serde_json::to_string(&fc.state_to_data(&deal)).unwrap();
        let legal = fc.legal_steps(&deal);
        let step_json = serde_json::to_string(&legal[0]).unwrap();

        let input = format!(
            "\n\
             not json\n\
             {{\"id\":1,\"cmd\":\"rules\"}}\n\
             {{\"id\":2,\"cmd\":\"deal\",\"ms\":1}}\n\
             {{\"id\":3,\"cmd\":\"validate\",\"state\":{state_json}}}\n\
             {{\"id\":4,\"cmd\":\"legal_steps\",\"state\":{state_json}}}\n\
             {{\"id\":5,\"cmd\":\"play_step\",\"state\":{state_json},\"step\":{step_json}}}\n\
             {{\"id\":6,\"cmd\":\"solve\",\"state\":{state_json},\"options\":{{\"max_states\":50000}}}}\n\
             {{\"id\":7,\"cmd\":\"hint\",\"state\":{state_json},\"options\":{{\"samples\":1}}}}\n\
             {{\"id\":8,\"cmd\":\"is_won\",\"state\":{state_json}}}\n\
             {{\"id\":9,\"cmd\":\"score\",\"state\":{state_json}}}\n\
             {{\"id\":10,\"cmd\":\"best\",\"state\":{state_json}}}\n\
             {{\"id\":11,\"cmd\":\"bogus\"}}\n\
             {{\"id\":12,\"cmd\":\"quit\"}}\n\
             {{\"id\":13,\"cmd\":\"rules\"}}\n"
        );
        let mut out = Vec::new();
        serve_io(&fc, input.as_bytes(), &mut out).unwrap();
        let lines: Vec<serde_json::Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        // 13 non-empty lines before quit (line 13 after quit is not processed).
        assert_eq!(lines.len(), 13);
        assert_eq!(lines[0]["ok"], false);
        assert_eq!(lines[1]["id"], 1);
        assert_eq!(lines[1]["ok"], true);
        assert!(lines[1]["rules"]["name"].is_string());
        assert_eq!(lines[2]["ok"], true);
        assert!(lines[2]["state"]["piles"].is_object());
        assert_eq!(lines[3]["ok"], true);
        assert_eq!(lines[4]["ok"], true);
        assert!(!lines[4]["steps"].as_array().unwrap().is_empty());
        assert_eq!(lines[5]["ok"], true);
        assert_eq!(lines[5]["won"], false);
        assert_eq!(lines[6]["ok"], true);
        assert_eq!(lines[6]["outcome"], "solved");
        assert!(!lines[6]["steps"].as_array().unwrap().is_empty());
        assert_eq!(lines[7]["ok"], true);
        assert!(lines[7]["hint"].is_object());
        assert_eq!(lines[8]["won"], false);
        assert!(lines[9]["score"].is_null());
        assert_eq!(lines[10]["ok"], false);
        assert_eq!(lines[11]["ok"], false);
        assert_eq!(lines[12]["id"], 12);
        assert_eq!(lines[12]["ok"], true);

        // Scored game: `best` works.
        let ps = Game::new(presets::poker_squares()).unwrap();
        let ps_state = serde_json::to_string(&ps.state_to_data(&ps.deal(1))).unwrap();
        let ps_in = format!(
            "{{\"cmd\":\"best\",\"state\":{ps_state},\"options\":{{\"max_states\":1000}}}}\n"
        );
        let mut ps_out = Vec::new();
        serve_io(&ps, ps_in.as_bytes(), &mut ps_out).unwrap();
        let ps_resp: serde_json::Value = serde_json::from_slice(&ps_out).unwrap();
        assert_eq!(ps_resp["ok"], true);
        assert!(ps_resp["score"].as_u64().unwrap() > 0);
    }
}
