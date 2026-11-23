//! `dealbreaker`: solve patience games from the command line.

use std::process::ExitCode;
use std::time::Instant;

use dealbreaker::{
    hint, maximize, microsoft_deal, presets, solve, Game, HintOptions, Outcome, Rules, SearchHints,
    SolveOptions, State, Strategy,
};

const USAGE: &str = "\
dealbreaker: solve patience (solitaire) games

USAGE:
    dealbreaker list
    dealbreaker rules <GAME>
    dealbreaker deal  <GAME> [DEAL]
    dealbreaker solve <GAME> [DEAL] [SEARCH] [--json]
    dealbreaker hint  <GAME> [DEAL] [SEARCH] [--samples N]
    dealbreaker replay <GAME> [DEAL] --steps FILE [--legal] [--json]
    dealbreaker best  <GAME> [DEAL] [SEARCH] [--json]
    dealbreaker bench <GAME> --seeds A..B [--microsoft] [--best] [SEARCH] [--json] [--verbose]

`replay` plays a list of steps (a JSON array, or `solve --json` output) from a deal and prints
the position after each one; `--legal` adds every legal step at each position.

`best` finds the highest score for games played for points; `bench --best` reports scores.

GAME is a preset id (see `list`) or a path to a rules JSON file.

DEAL (default: --seed 1):
    --seed N          deal from a seed
    --ms N            Microsoft FreeCell deal number (FreeCell-layout games)
    --state FILE      a position as JSON (`deal` prints the format)

SEARCH:
    --max-states N    budget of distinct positions (default 2000000)
    --timeout-ms N    wall-clock time limit in milliseconds
    --strategy S      auto | dfs | best-first (default auto)
    --no-autoplay     don't play safe foundation moves automatically
    --no-shorten      don't shorten winning lines (faster when only solvability matters)
    --search JSON     override the game's search hints, e.g. '{\"blocker_weight\": 4}'
                      (speed only: verdicts never change)
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

struct Args {
    rest: Vec<String>,
    flags: Vec<(String, Option<String>)>,
}

impl Args {
    fn parse(v: &[String]) -> Args {
        let takes_value = [
            "--seed",
            "--ms",
            "--state",
            "--max-states",
            "--timeout-ms",
            "--strategy",
            "--seeds",
            "--samples",
            "--search",
            "--steps",
        ];
        let mut rest = Vec::new();
        let mut flags = Vec::new();
        let mut i = 0;
        while i < v.len() {
            let a = &v[i];
            if let Some((k, val)) = a.split_once('=').filter(|_| a.starts_with("--")) {
                flags.push((k.to_string(), Some(val.to_string())));
            } else if takes_value.contains(&a.as_str()) {
                flags.push((a.clone(), v.get(i + 1).cloned()));
                i += 1;
            } else if a.starts_with("--") {
                flags.push((a.clone(), None));
            } else {
                rest.push(a.clone());
            }
            i += 1;
        }
        Args { rest, flags }
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

fn load_game(spec: &str, a: &Args) -> Result<Game, String> {
    let mut rules: Rules = match presets::preset(spec) {
        Some(r) => r,
        None => {
            let text = std::fs::read_to_string(spec).map_err(|e| {
                format!("{spec:?} is not a preset (see `list`) or a readable file: {e}")
            })?;
            Rules::from_json(&text).map_err(|e| format!("{spec}: {e}"))?
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
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
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

fn run(argv: &[String]) -> Result<(), String> {
    let a = Args::parse(argv);
    let cmd = a.rest.first().map(String::as_str).unwrap_or("help");
    let game_arg = || {
        a.rest
            .get(1)
            .ok_or_else(|| format!("`{cmd}` needs a game\n\n{USAGE}"))
    };
    match cmd {
        "list" => {
            for p in presets::PRESETS {
                println!("{:<20} {}", p.id, (p.rules)().name);
            }
        }
        "rules" => {
            let g = load_game(game_arg()?, &a)?;
            println!("{}", g.rules().to_json());
        }
        "deal" => {
            let g = load_game(game_arg()?, &a)?;
            let s = load_deal(&g, &a)?;
            println!("{}", g.state_to_json(&s));
        }
        "solve" => {
            let g = load_game(game_arg()?, &a)?;
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
            let g = load_game(game_arg()?, &a)?;
            let mut s = load_deal(&g, &a)?;
            let path = a.get("--steps").ok_or("`replay` needs --steps FILE")?;
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
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
            let g = load_game(game_arg()?, &a)?;
            let s = load_deal(&g, &a)?;
            let mut o = HintOptions::default().solve(search_opts(&a)?);
            if a.get("--max-states").is_none() {
                o.solve.max_states = 100_000;
            }
            if let Some(n) = a.num("--samples")? {
                o.samples = n;
            }
            match hint(&g, &s, &o) {
                Some(h) => println!("{} (won {} of {} guesses)", h.step, h.wins, h.samples),
                None => println!("no winning line found"),
            }
        }
        "best" => {
            let g = load_game(game_arg()?, &a)?;
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
        "bench" if a.has("--best") => {
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
            let (mut total, mut proven, mut states) = (0u64, 0u32, 0u64);
            let (mut lowest, mut highest) = (u32::MAX, 0u32);
            let t = Instant::now();
            for seed in lo..=hi {
                let s = g.deal(seed);
                let t1 = Instant::now();
                let r = maximize(&g, &s, &o);
                total += r.score as u64;
                proven += r.proven as u32;
                states += r.states;
                lowest = lowest.min(r.score);
                highest = highest.max(r.score);
                if a.has("--verbose") {
                    println!(
                        "{seed}: {} points{} ({} states, {:.3}s)",
                        r.score,
                        if r.proven { " (proven)" } else { "" },
                        r.states,
                        t1.elapsed().as_secs_f64()
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
            let g = load_game(game_arg()?, &a)?;
            let range = a.get("--seeds").unwrap_or("1..100");
            let (lo, hi) = range
                .split_once("..")
                .and_then(|(l, h)| Some((l.parse::<u64>().ok()?, h.parse::<u64>().ok()?)))
                .ok_or_else(|| format!("--seeds wants A..B, got {range}"))?;
            let o = search_opts(&a)?;
            let ms = a.has("--microsoft");
            let (mut solved, mut unsolvable, mut gaveup) = (0, 0, 0);
            let mut states = 0u64;
            let mut steps_total = 0usize;
            let t = Instant::now();
            let mut worst = (0f64, 0u64);
            for seed in lo..=hi {
                let s = if ms {
                    g.deal_ordered(&microsoft_deal(seed as u32))
                        .map_err(|e| e.to_string())?
                } else {
                    g.deal(seed)
                };
                let t1 = Instant::now();
                let r = solve(&g, &s, &o);
                let dt = t1.elapsed().as_secs_f64();
                if dt > worst.0 {
                    worst = (dt, seed);
                }
                states += r.states;
                match &r.outcome {
                    Outcome::Solved(m) => {
                        solved += 1;
                        steps_total += g.steps(&s, m).len();
                    }
                    Outcome::Unsolvable => unsolvable += 1,
                    Outcome::GaveUp => gaveup += 1,
                }
                if a.has("--verbose") {
                    println!(
                        "{seed}: {} ({} states, {dt:.3}s)",
                        r.outcome.label(),
                        r.states
                    );
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
        "help" | "--help" | "-h" => print!("{USAGE}"),
        other => return Err(format!("unknown command {other:?}\n\n{USAGE}")),
    }
    Ok(())
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
        ]));
        assert_eq!(a.rest, ["solve", "freecell"]);
        assert_eq!(a.get("--seed"), Some("5"));
        assert_eq!(a.num::<u64>("--seed").unwrap(), Some(5));
        assert!(a.has("--json"));
        assert!(!a.has("--verbose"));

        let bad = Args::parse(&argv(&["--seed", "nope"]));
        assert!(bad
            .num::<u64>("--seed")
            .unwrap_err()
            .contains("not a number"));

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
    }
}
