//! The first colony, unattended on the dev embark: everyone alive after 30 days, nobody stuck,
//! the hut built, and the same story on every run. Runs the real binary (`--sim-snapshot`, a
//! couple of seconds once the dev region is in the chunk cache, ~15 s the first time).

use std::process::Command;

fn run(dir: &std::path::Path) -> (String, String) {
    let prefix = dir.join("colony");
    let out = Command::new(env!("CARGO_BIN_EXE_planet_generator"))
        .args(["--sim-snapshot", prefix.to_str().unwrap()])
        .output()
        .expect("run planet_generator");
    assert!(out.status.success(), "planet_generator failed: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let log = std::fs::read_to_string(dir.join("colony_log.txt")).expect("colony log");
    (stdout, log)
}

#[test]
fn seven_settlers_live_thirty_days_unattended() {
    let dir = std::env::temp_dir().join(format!("colony_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (out, log) = run(&dir);
    let summary = out.lines().find(|l| l.starts_with("Colony after 30 days")).expect("summary line");
    assert!(summary.contains("7 of 7 alive"), "someone died: {summary}\n{log}");
    assert!(summary.contains(" 0 times a settler found no way"), "settlers got stuck: {summary}");
    assert!(log.contains("finishes the hut"), "no hut after 30 days:\n{log}");
    assert!(!log.contains("is starving"), "someone starved:\n{log}");
    // Deterministic: a second run tells the same story.
    let dir2 = dir.join("again");
    std::fs::create_dir_all(&dir2).unwrap();
    let (_, log2) = run(&dir2);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(log, log2, "two runs of the same colony diverged");
}
