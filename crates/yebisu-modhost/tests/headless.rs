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

/// The runner's lifetime when a test ends on an observed publication instead.
/// A passing run stops as soon as its condition holds; only a failing one
/// waits this long.
const RUN_BOUND_SECONDS: &str = "30";

fn inspect_capture(path: &std::path::Path) -> serde_json::Value {
    let received = std::process::Command::new(env!("CARGO_BIN_EXE_yebisu-modhost"))
        .arg("inspect")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        received.status.success(),
        "{}: {}",
        path.display(),
        String::from_utf8_lossy(&received.stderr)
    );
    serde_json::from_slice(&received.stdout).unwrap()
}

/// Run one mod and read its publications as they arrive, until `done`
/// accepts the ones received so far. The test therefore waits on what the
/// receiver observes, never on how long the runner takes to get there.
fn captures_until(
    script: &str,
    done: impl Fn(&[serde_json::Value]) -> bool,
) -> Vec<serde_json::Value> {
    use std::{fs, process::Command, time::Duration};
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let module = home.join("mods/input");
    fs::create_dir_all(&module).unwrap();
    fs::write(module.join("main.js"), script).unwrap();
    let output = root.path().join("snapshots");
    let stderr = root.path().join("stderr.log");
    let mut runner = Command::new(env!("CARGO_BIN_EXE_yebisu-modhost"))
        .args([
            home.to_str().unwrap(),
            output.to_str().unwrap(),
            RUN_BOUND_SECONDS,
            "input",
        ])
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .stdout(std::process::Stdio::null())
        .stderr(fs::File::create(&stderr).unwrap())
        .spawn()
        .unwrap();
    let mut captures = Vec::new();
    loop {
        // Read the exit status before the directory: after an exit, this
        // scan sees every publication the runner wrote.
        let exited = runner.try_wait().unwrap();
        let mut paths: Vec<_> = match fs::read_dir(&output) {
            Ok(entries) => entries
                .map(|f| f.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "frames"))
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => panic!("{error}"),
        };
        paths.sort();
        // Frames files are renamed into place, so each one is complete.
        for path in paths.iter().skip(captures.len()) {
            // Inspect the actual wire bytes, without the diagnostic JSON sidecar.
            fs::remove_file(path.with_extension("publish.json")).unwrap();
            captures.push(inspect_capture(path));
        }
        if done(&captures) {
            // An exited runner was already reaped by try_wait.
            if exited.is_none() {
                runner.kill().unwrap();
                runner.wait().unwrap();
            }
            return captures;
        }
        if let Some(status) = exited {
            panic!(
                "runner ended ({status}) before the expected publication; received {captures:#?}\n{}",
                fs::read_to_string(&stderr).unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn retained_input(capture: &serde_json::Value) -> &serde_json::Value {
    capture["retained_entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["ref"]["kind"] == "Input")
        .unwrap()
}

#[test]
fn retained_input_subscriptions_publish_without_parent_notify() {
    use serde_json::json;
    let all = json!(["change", "submit"]);
    // Without a frame request from the subscription itself, nothing after the
    // first frame publishes these events, and the run ends unmet.
    let captures = captures_until(include_str!("fixtures/subscriptions/main.js"), |captures| {
        captures
            .last()
            .is_some_and(|c| retained_input(c)["subscriptions"]["events"] == all)
    });
    let inputs: Vec<_> = captures.iter().map(retained_input).collect();
    // The fixture starts changing the Input only after the first frame was
    // laid out, so the first publication shows it untouched.
    assert_eq!(inputs[0]["subscriptions"]["events"], json!([]));
    assert_eq!(inputs[0]["config"]["masked"], json!(false));
    let all = all.as_array().unwrap();
    let mut previous_rev = 0;
    for (index, (capture, input)) in captures.iter().zip(&inputs).enumerate() {
        let events = input["subscriptions"]["events"].as_array().unwrap();
        assert_eq!(
            events[..],
            all[..events.len()],
            "events are registered in order, once each"
        );
        assert_eq!(input["config"], inputs[0]["config"]);
        assert_eq!(input["state"], inputs[0]["state"]);
        assert_eq!(input["ref"], inputs[0]["ref"]);
        assert_eq!(capture["texts"], json!(["render=1"]));
        assert_eq!(input["rev"], capture["state_rev"]);
        let revision = capture["state_rev"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert!(revision > previous_rev);
        previous_rev = revision;
        // Config and state never change here, so every publication must add
        // an event: the masked setter's wake and the duplicate subscription
        // must not publish on their own.
        if index > 0 {
            let before = inputs[index - 1]["subscriptions"]["events"]
                .as_array()
                .unwrap();
            assert!(
                events.len() > before.len(),
                "publication {index} repeats the previous Input: {captures:#?}"
            );
        }
    }
}

#[test]
fn empty_virtual_list_is_received_with_its_view_generation() {
    use serde_json::json;
    let captures = captures_until(include_str!("fixtures/virtual_list/main.js"), |captures| {
        captures.iter().any(|c| c["texts"] == json!(["n=1"]))
    });
    assert_eq!(captures[0]["texts"], json!(["n=0"]));
    assert_eq!(captures.last().unwrap()["texts"], json!(["n=1"]));
    for capture in &captures {
        // div, VirtualList and the text node.
        assert_eq!(capture["nodes"], 3);
        let generation = &capture["generation"];
        assert_eq!(
            capture["list_manifests"],
            json!([{
                "list_id": "probe-list",
                "content_rev": generation,
                "kind": "virtual",
                "item_count": 0,
                "sizes_or_estimate": {"kind": "Sizes", "value": []},
                "handler_generation": generation,
            }])
        );
    }
}

#[test]
fn unicode_input_selections_are_received_as_utf8_bytes() {
    let captures = captures_until(include_str!("fixtures/unicode/main.js"), |c| !c.is_empty());
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
    use serde_json::json;
    let fixture = include_str!("fixtures/hostset/main.js");
    assert!(fixture.contains("/*SETTER*/"));
    let rev = |v: &serde_json::Value| v.as_str().unwrap().parse::<u64>().unwrap();
    // Each setter runs alone: if it did not request a frame itself, nothing
    // else would publish its change and the run would end unmet.
    for (setter, field, expected) in [
        (
            r#"this.input.set_value("after😀")"#,
            "/state/text",
            json!("after😀"),
        ),
        ("this.input.set_masked(true)", "/config/masked", json!(true)),
        (
            "this.input.set_selected_range({start: 0, end: 5})",
            "/state/selection/end",
            json!(5),
        ),
    ] {
        let captures = captures_until(&fixture.replace("/*SETTER*/", setter), |captures| {
            captures
                .last()
                .is_some_and(|c| retained_input(c).pointer(field) == Some(&expected))
        });
        assert_eq!(captures.len(), 2, "{setter}: {captures:#?}");
        let (before, after) = (retained_input(&captures[0]), retained_input(&captures[1]));
        assert_eq!(before["state"]["text"], "before", "{setter}");
        assert_eq!(before["config"]["masked"], false, "{setter}");
        if setter.contains("set_value") {
            assert_eq!(after["state"]["selection"]["end"], "after😀".len());
        }
        if setter.contains("set_selected_range") {
            assert_eq!(after["state"]["selection"]["start"], 0);
        }
        for capture in &captures {
            assert_eq!(
                capture["texts"],
                json!(["render=1"]),
                "{setter}: parent unexpectedly rebuilt"
            );
            assert_eq!(retained_input(capture)["rev"], capture["state_rev"]);
        }
        assert!(rev(&captures[1]["state_rev"]) > rev(&captures[0]["state_rev"]));
        assert!(rev(&after["rev"]) > rev(&before["rev"]));
        assert_eq!(after["ref"], before["ref"]);
    }
}
