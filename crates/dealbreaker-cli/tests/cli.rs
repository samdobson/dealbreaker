use std::process::Command;

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
}
