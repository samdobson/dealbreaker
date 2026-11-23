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
