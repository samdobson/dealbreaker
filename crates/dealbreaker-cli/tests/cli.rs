use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn binary_exits_zero_on_list_and_two_on_bad_command() {
    let bin = env!("CARGO_BIN_EXE_dealbreaker");
    let ok = Command::new(bin).arg("list").output().unwrap();
    assert!(ok.status.success(), "list failed: {ok:?}");
    let stdout = String::from_utf8_lossy(&ok.stdout);
    assert!(stdout.contains("freecell"), "{stdout}");

    let bad = Command::new(bin).arg("no-such-cmd").output().unwrap();
    assert_eq!(bad.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(stderr.contains("unknown command"), "{stderr}");

    let timed = Command::new(bin)
        .args([
            "solve",
            "freecell",
            "--ms",
            "11982",
            "--timeout-ms",
            "0",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(timed.status.success(), "timeout solve failed: {timed:?}");
    let timed_out = String::from_utf8_lossy(&timed.stdout);
    assert!(
        timed_out.contains("\"outcome\": \"gave-up\""),
        "{timed_out}"
    );
}

#[test]
fn serve_command_processes_jsonl_over_stdio() {
    let bin = env!("CARGO_BIN_EXE_dealbreaker");
    let mut child = Command::new(bin)
        .args(["serve", "freecell"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, r#"{{"id":"a","cmd":"deal","ms":1}}"#).unwrap();
        writeln!(stdin, r#"{{"id":"b","cmd":"quit"}}"#).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "serve failed: {output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<serde_json::Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["id"], "a");
    assert_eq!(lines[0]["ok"], true);
    assert!(lines[0]["state"]["piles"].is_object());
    assert_eq!(lines[1]["id"], "b");
    assert_eq!(lines[1]["ok"], true);
}

#[test]
fn binary_json_subcommands_and_error_exit_codes() {
    let bin = env!("CARGO_BIN_EXE_dealbreaker");

    // rules outputs valid Rules JSON.
    let rules_out = Command::new(bin)
        .args(["rules", "freecell"])
        .output()
        .unwrap();
    assert!(rules_out.status.success(), "{rules_out:?}");
    let rules_val: serde_json::Value = serde_json::from_slice(&rules_out.stdout).unwrap();
    assert_eq!(rules_val["name"], "FreeCell");

    // deal outputs valid State JSON.
    let deal_out = Command::new(bin)
        .args(["deal", "freecell", "--ms", "1"])
        .output()
        .unwrap();
    assert!(deal_out.status.success(), "{deal_out:?}");
    let deal_val: serde_json::Value = serde_json::from_slice(&deal_out.stdout).unwrap();
    assert_eq!(deal_val["piles"]["tableau"].as_array().unwrap().len(), 8);

    // solve --json outputs outcome=solved and steps; replay verifies them via temp file.
    let solve_out = Command::new(bin)
        .args([
            "solve",
            "freecell",
            "--ms",
            "1",
            "--max-states",
            "200000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(solve_out.status.success(), "{solve_out:?}");
    let solve_val: serde_json::Value = serde_json::from_slice(&solve_out.stdout).unwrap();
    assert_eq!(solve_val["outcome"], "solved");
    assert!(!solve_val["steps"].as_array().unwrap().is_empty());

    let steps_path = std::env::temp_dir().join(format!(
        "dealbreaker-cli-test-steps-{}.json",
        std::process::id()
    ));
    std::fs::write(&steps_path, &solve_out.stdout).unwrap();
    let replay_out = Command::new(bin)
        .args([
            "replay",
            "freecell",
            "--ms",
            "1",
            "--steps",
            steps_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&steps_path);
    assert!(replay_out.status.success(), "{replay_out:?}");
    let replay_val: serde_json::Value = serde_json::from_slice(&replay_out.stdout).unwrap();
    assert_eq!(replay_val["game"], "FreeCell");
    let positions = replay_val["positions"].as_array().unwrap();
    assert_eq!(
        positions.len(),
        solve_val["steps"].as_array().unwrap().len() + 1
    );
    let last_foundations = positions.last().unwrap()["state"]["piles"]["foundation"]
        .as_array()
        .unwrap();
    assert!(last_foundations
        .iter()
        .all(|f| f.as_array().unwrap().len() == 13));

    // hint outputs a recommended step and win count.
    let hint_out = Command::new(bin)
        .args(["hint", "freecell", "--ms", "5", "--samples", "1"])
        .output()
        .unwrap();
    assert!(hint_out.status.success(), "{hint_out:?}");
    let hint_str = String::from_utf8_lossy(&hint_out.stdout);
    assert!(hint_str.contains("won 1 of 1 guesses"), "{hint_str}");

    // best --json outputs score > 0 for Poker Squares.
    let best_out = Command::new(bin)
        .args([
            "best",
            "poker-squares",
            "--seed",
            "1",
            "--max-states",
            "2000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(best_out.status.success(), "{best_out:?}");
    let best_val: serde_json::Value = serde_json::from_slice(&best_out.stdout).unwrap();
    assert!(best_val["score"].as_u64().unwrap() > 0);

    // bench --json outputs structured summary.
    let bench_out = Command::new(bin)
        .args([
            "bench",
            "freecell",
            "--seeds",
            "1..2",
            "--microsoft",
            "--max-states",
            "200000",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(bench_out.status.success(), "{bench_out:?}");
    let bench_val: serde_json::Value = serde_json::from_slice(&bench_out.stdout).unwrap();
    assert_eq!(bench_val["solved"], 2);

    // Unknown game exits with code 2.
    let bad_game = Command::new(bin)
        .args(["solve", "no-such-game"])
        .output()
        .unwrap();
    assert_eq!(bad_game.status.code(), Some(2));

    // Unknown flag exits with code 2.
    let bad_flag = Command::new(bin)
        .args(["solve", "freecell", "--max-state", "100"])
        .output()
        .unwrap();
    assert_eq!(bad_flag.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&bad_flag.stderr).contains("unknown flag"),
        "{bad_flag:?}"
    );

    // --version prints package version.
    let ver = Command::new(bin).arg("--version").output().unwrap();
    assert!(ver.status.success());
    let ver_str = String::from_utf8_lossy(&ver.stdout);
    assert!(ver_str.contains(env!("CARGO_PKG_VERSION")), "{ver_str}");

    // hint --json outputs structured JSON.
    let hint_json_out = Command::new(bin)
        .args(["hint", "freecell", "--ms", "5", "--samples", "1", "--json"])
        .output()
        .unwrap();
    assert!(hint_json_out.status.success(), "{hint_json_out:?}");
    let hint_val: serde_json::Value = serde_json::from_slice(&hint_json_out.stdout).unwrap();
    assert_eq!(hint_val["game"], "FreeCell");
    assert_eq!(hint_val["hint"]["wins"], 1);
    assert_eq!(hint_val["hint"]["samples"], 1);

    // validate checks rules and optional --state from stdin (`-`).
    let mut val_child = Command::new(bin)
        .args(["validate", "freecell", "--state", "-", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    val_child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&deal_out.stdout)
        .unwrap();
    let val_out = val_child.wait_with_output().unwrap();
    assert!(val_out.status.success(), "{val_out:?}");
    let val_json: serde_json::Value = serde_json::from_slice(&val_out.stdout).unwrap();
    assert_eq!(val_json["ok"], true);

    // Pipe deal JSON into `solve --state - --json`, then pipe into `replay --steps - --json`.
    let mut solve_pipe = Command::new(bin)
        .args([
            "solve",
            "freecell",
            "--state",
            "-",
            "--max-states",
            "200000",
            "--json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    solve_pipe
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&deal_out.stdout)
        .unwrap();
    let solve_pipe_out = solve_pipe.wait_with_output().unwrap();
    assert!(solve_pipe_out.status.success(), "{solve_pipe_out:?}");
    let solve_pipe_val: serde_json::Value = serde_json::from_slice(&solve_pipe_out.stdout).unwrap();
    assert_eq!(solve_pipe_val["outcome"], "solved");

    let mut replay_pipe = Command::new(bin)
        .args(["replay", "freecell", "--ms", "1", "--steps", "-", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    replay_pipe
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&solve_pipe_out.stdout)
        .unwrap();
    let replay_pipe_out = replay_pipe.wait_with_output().unwrap();
    assert!(replay_pipe_out.status.success(), "{replay_pipe_out:?}");
}

#[test]
fn serve_handles_unknown_hint_cards_and_errors() {
    let bin = env!("CARGO_BIN_EXE_dealbreaker");
    let deal_out = Command::new(bin)
        .args(["deal", "klondike", "--seed", "2"])
        .output()
        .unwrap();
    assert!(deal_out.status.success());
    let mut state: serde_json::Value = serde_json::from_slice(&deal_out.stdout).unwrap();
    for piles in state["piles"].as_object_mut().unwrap().values_mut() {
        for pile in piles.as_array_mut().unwrap() {
            for card in pile.as_array_mut().unwrap() {
                if card.as_str().is_some_and(|c| c.starts_with('~')) {
                    *card = serde_json::Value::String("?".into());
                }
            }
        }
    }

    let mut child = Command::new(bin)
        .args(["serve", "klondike"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "not valid json").unwrap();
        writeln!(
            stdin,
            r#"{{"id":"h","cmd":"hint","state":{state},"options":{{"samples":2,"seed":1,"solve":{{"max_states":5000}}}}}}"#
        )
        .unwrap();
        writeln!(stdin, r#"{{"id":"q","cmd":"quit"}}"#).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["ok"], false);
    assert_eq!(lines[1]["id"], "h");
    assert_eq!(lines[1]["ok"], true);
    assert_eq!(lines[2]["id"], "q");
    assert_eq!(lines[2]["ok"], true);
}

#[test]
fn grade_single_deal_many_deals_and_serve() {
    let bin = env!("CARGO_BIN_EXE_dealbreaker");
    let run = |args: &[&str]| {
        let out = Command::new(bin).args(args).output().unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };

    let (code, out, _) = run(&["grade", "freecell", "--ms", "1"]);
    assert_eq!(code, Some(0));
    assert!(
        ["easy (", "medium (", "hard ("]
            .iter()
            .any(|g| out.starts_with(g)),
        "{out}"
    );

    let (code, out, _) = run(&["grade", "freecell", "--ms", "1", "--json"]);
    assert_eq!(code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["game"], "FreeCell");
    assert_eq!(v["winnable"], true);
    assert_eq!(v["source"], "preset");

    let (code, out, _) = run(&[
        "grade",
        "freecell",
        "--ms",
        "1",
        "--thresholds",
        "0,0",
        "--json",
    ]);
    assert_eq!(code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["grade"], "hard");
    assert_eq!(v["source"], "custom");

    let (code, out, _) = run(&[
        "grade",
        "freecell",
        "--seeds",
        "1..6",
        "--calibrate",
        "--jobs",
        "2",
        "--json",
        "--verbose",
    ]);
    assert_eq!(code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["deals"], 6);
    assert_eq!(v["preset"], "freecell");
    assert_eq!(v["grades"].as_array().unwrap().len(), 6);
    assert_eq!(
        v["easy"].as_u64().unwrap() + v["medium"].as_u64().unwrap() + v["hard"].as_u64().unwrap(),
        6
    );

    let (code, out, _) = run(&["grade", "elevens", "--seeds", "1..3"]);
    assert_eq!(code, Some(0));
    assert!(out.contains("3 hard"), "{out}");

    for bad in [
        &["grade", "freecell", "--thresholds", "5"][..],
        &["grade", "freecell", "--thresholds", "9,3"],
        &["grade", "freecell", "--calibrate"],
        &["grade", "freecell", "--seeds", "1..2", "--seed", "3"],
        &[
            "grade",
            "freecell",
            "--seeds",
            "1..2",
            "--calibrate",
            "--thresholds",
            "1,2",
        ],
        &["grade", "freecell", "--samples", "2"],
    ] {
        let (code, _, err) = run(bad);
        assert_eq!(code, Some(2), "{bad:?}");
        assert!(err.starts_with("error:"), "{bad:?}: {err}");
    }

    let mut child = Command::new(bin)
        .args(["serve", "freecell"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        let deal = run(&["deal", "freecell", "--ms", "1"]).1;
        let deal: serde_json::Value = serde_json::from_str(&deal).unwrap();
        writeln!(stdin, r#"{{"id":1,"cmd":"grade","state":{deal}}}"#).unwrap();
        writeln!(
            stdin,
            r#"{{"id":2,"cmd":"grade","state":{deal},"options":{{"thresholds":{{"medium":1000,"hard":2000}}}}}}"#
        )
        .unwrap();
        writeln!(stdin, r#"{{"id":3,"cmd":"grade"}}"#).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["difficulty"]["winnable"], true);
    assert_eq!(lines[1]["difficulty"]["grade"], "easy");
    assert_eq!(lines[2]["ok"], false);
}
