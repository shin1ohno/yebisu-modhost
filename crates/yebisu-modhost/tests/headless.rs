//! Exercise the actual native headless platform and JS timer -> notify -> snapshot path.
#[test]
fn timer_changes_are_received_and_an_invalid_mod_fails() {
    use std::{fs, process::Command};
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let module = home.join("mods/counter");
    fs::create_dir_all(&module).unwrap();
    fs::write(
        module.join("main.js"),
        include_str!("fixtures/counter/main.js"),
    )
    .unwrap();
    let output = root.path().join("snapshots");
    let result = Command::new(env!("CARGO_BIN_EXE_yebisu-modhost"))
        .args([
            home.to_str().unwrap(),
            output.to_str().unwrap(),
            "2",
            "counter",
        ])
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut received = String::new();
    for file in fs::read_dir(&output).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_some_and(|e| e == "frames") {
            let decoded = Command::new(env!("CARGO_BIN_EXE_yebisu-modhost"))
                .arg("inspect")
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                decoded.status.success(),
                "{}",
                String::from_utf8_lossy(&decoded.stderr)
            );
            received.push_str(&String::from_utf8_lossy(&decoded.stdout));
        }
    }
    // The receiver must reject an otherwise valid capture with a mismatched session.
    let capture = output.join("counter-0001.frames");
    let identity = capture.with_extension("identity.json");
    let mut context: serde_json::Value =
        serde_json::from_slice(&fs::read(&identity).unwrap()).unwrap();
    context["session"]["host"] = serde_json::json!("different-host");
    fs::write(&identity, serde_json::to_vec(&context).unwrap()).unwrap();
    let mismatch = Command::new(env!("CARGO_BIN_EXE_yebisu-modhost"))
        .arg("inspect")
        .arg(&capture)
        .output()
        .unwrap();
    assert!(!mismatch.status.success());
    assert!(
        String::from_utf8_lossy(&mismatch.stderr).contains("identity"),
        "{}",
        String::from_utf8_lossy(&mismatch.stderr)
    );

    assert!(
        received.contains("timer-count=0"),
        "initial JS render missing: {received}"
    );
    assert!(
        received.contains("timer-count=3"),
        "timer notify did not reach capture: {received}"
    );
    assert!(received.contains("sized=true"), "layout callback never ran");
    let bad = Command::new(env!("CARGO_BIN_EXE_yebisu-modhost"))
        .args([
            home.to_str().unwrap(),
            output.to_str().unwrap(),
            "1",
            "absent",
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("missing main.js"));
}
