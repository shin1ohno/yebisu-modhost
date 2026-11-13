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

fn input_captures(script: &str) -> Vec<serde_json::Value> {
    use std::{fs, process::Command};
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let module = home.join("mods/input");
    fs::create_dir_all(&module).unwrap();
    fs::write(module.join("main.js"), script).unwrap();
    let output = root.path().join("snapshots");
    let run = Command::new(env!("CARGO_BIN_EXE_yebisu-modhost"))
        .args([
            home.to_str().unwrap(),
            output.to_str().unwrap(),
            "2",
            "input",
        ])
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let mut paths: Vec<_> = fs::read_dir(&output)
        .unwrap()
        .map(|f| f.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "frames"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no Input publication received");
    paths
        .iter()
        .map(|path| {
            let received = Command::new(env!("CARGO_BIN_EXE_yebisu-modhost"))
                .arg("inspect")
                .arg(path)
                .output()
                .unwrap();
            assert!(
                received.status.success(),
                "{}",
                String::from_utf8_lossy(&received.stderr)
            );
            serde_json::from_slice(&received.stdout).unwrap()
        })
        .collect()
}

#[test]
fn unicode_input_selections_are_received_as_utf8_bytes() {
    let captures = input_captures(include_str!("fixtures/unicode/main.js"));
    let inputs: Vec<_> = captures[0]["retained_entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["ref"]["kind"] == "Input")
        .collect();
    for text in ["abc", "あ", "éab", "あ😀é"] {
        let state = &inputs.iter().find(|e| e["state"]["text"] == text).unwrap()["state"];
        assert_eq!(state["selection"]["start"], text.len(), "{text}");
        assert_eq!(state["selection"]["end"], text.len(), "{text}");
    }
}

#[test]
fn retained_input_host_set_publishes_without_parent_notify() {
    let captures = input_captures(include_str!("fixtures/hostset/main.js"));
    let input = |capture: &serde_json::Value| {
        capture["retained_entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["ref"]["kind"] == "Input")
            .unwrap()
            .clone()
    };
    let before = &captures[0];
    assert_eq!(input(before)["state"]["text"], "before");
    let after = captures
        .iter()
        .find(|c| input(c)["state"]["text"] == "after😀")
        .expect("retained Input update never reached the wire receiver");
    assert_eq!(before["texts"], serde_json::json!(["render=1"]));
    assert_eq!(
        after["texts"], before["texts"],
        "parent unexpectedly rebuilt"
    );
    let rev = |v: &serde_json::Value| v.as_str().unwrap().parse::<u64>().unwrap();
    assert!(rev(&after["state_rev"]) > rev(&before["state_rev"]));
    assert!(rev(&input(after)["rev"]) > rev(&input(before)["rev"]));
    assert_eq!(input(after)["rev"], after["state_rev"]);
    assert_eq!(input(after)["ref"], input(before)["ref"]);
    assert_eq!(input(after)["state"]["selection"]["end"], "after😀".len());
    let masked = captures
        .iter()
        .find(|c| input(c)["config"]["masked"] == true)
        .expect("retained Input config update never reached the receiver");
    assert!(rev(&masked["state_rev"]) > rev(&after["state_rev"]));
    let selected = captures
        .iter()
        .find(|c| input(c)["state"]["selection"]["end"] == 5)
        .expect("retained Input selection update never reached the receiver");
    assert_eq!(input(selected)["state"]["selection"]["start"], 0);
    assert!(rev(&selected["state_rev"]) > rev(&masked["state_rev"]));
    for capture in &captures {
        assert_eq!(
            capture["texts"], before["texts"],
            "parent unexpectedly rebuilt"
        );
        assert_eq!(input(capture)["rev"], capture["state_rev"]);
    }
    assert_eq!(
        captures.len(),
        4,
        "unchanged frames must not create more publications"
    );
}
