//! The isolated prototype's in-memory Herdr snapshot adapter.
//! No connection to the user's running Herdr or to an external command.
use std::{cell::RefCell, collections::BTreeMap, path::Path, rc::Rc};

use gpui_shell::{HostError, HostModule, HostValue};
use serde_json::{Value, json};

pub fn to_host(value: &Value) -> HostValue {
    match value {
        Value::Null => HostValue::Null,
        Value::Bool(v) => HostValue::Bool(*v),
        Value::Number(v) => HostValue::Number(v.as_f64().unwrap_or_default()),
        Value::String(v) => HostValue::Str(v.clone()),
        Value::Array(v) => HostValue::Array(v.iter().map(to_host).collect()),
        Value::Object(v) => {
            HostValue::Object(v.iter().map(|(k, v)| (k.clone(), to_host(v))).collect())
        }
    }
}

pub fn module(home: &Path) -> HostModule {
    let mut module = HostModule::new("herdr");
    // One synthetic space/tab exercises chrome without a repository or live session.
    module = module.function("spaces", |_| {
        Ok(to_host(&json!([{
            "id":"prototype-space", "name":"Prototype", "active":true,
            "machine":null, "folder":null,
            "tabs":[{"id":"prototype-tab", "name":"Prototype", "active":true, "status":"idle"}]
        }])))
    });
    for name in [
        "panes",
        "agents",
        "terminals",
        "machines",
        "keys",
        "layouts",
        "commands",
    ] {
        module = module.function(name, |_| Ok(HostValue::Array(vec![])));
    }
    for name in ["focused", "layout", "placement"] {
        module = module.function(name, |_| Ok(HostValue::Null));
    }
    for name in ["props", "settings"] {
        module = module.function(name, |_| Ok(to_host(&json!({}))));
    }
    module = module.function("window", |_| {
        Ok(to_host(
            &json!({"platform":"linux", "width":800, "height":600, "maximized":false}),
        ))
    });
    module = module.function("updates", |_| {
        Ok(to_host(&json!({"app":{"version":"0.1.15"}})))
    });
    let home = home.to_string_lossy().into_owned();
    module = module.function("home", move |_| Ok(HostValue::Str(home.clone())));
    module = module.function("theme", |_| {
        let mut theme = serde_json::Map::new();
        for key in [
            "background",
            "chrome",
            "surface",
            "raised",
            "hover",
            "selected",
            "border",
            "accent",
            "accent_foreground",
            "foreground",
            "muted",
            "dim",
            "blocked",
            "solid",
            "solid_foreground",
            "running",
            "idle",
            "done",
            "unknown",
            "selection",
        ] {
            let color = match key {
                "background" | "chrome" | "surface" => "#181a1f",
                "raised" | "hover" | "selected" | "border" => "#343840",
                "foreground" | "solid_foreground" | "accent_foreground" => "#eeeeee",
                "accent" | "solid" => "#6d90d8",
                _ => "#999999",
            };
            theme.insert(key.into(), json!(color));
        }
        for i in 0..16 {
            theme.insert(format!("ansi{i}"), json!("#aabbcc"));
        }
        Ok(to_host(&Value::Object(theme)))
    });
    let shared = Rc::new(RefCell::new(BTreeMap::<String, HostValue>::new()));
    let reads = shared.clone();
    module = module.function("shared", move |args| {
        Ok(reads
            .borrow()
            .get(args.string(0)?)
            .cloned()
            .unwrap_or(HostValue::Null))
    });
    module = module.function("setShared", move |args| {
        shared
            .borrow_mut()
            .insert(args.string(0)?.to_owned(), args.value(1)?.clone());
        Ok(HostValue::Null)
    });
    module = module.async_function("nextCommand", |_| {
        Ok(std::future::pending::<Result<HostValue, HostError>>())
    });
    module = module.async_function("knownAgents", |_| {
        Ok(async { Ok(HostValue::Array(vec![])) })
    });
    // Explicit errors: never claim an external operation ran successfully.
    for name in [
        "exec",
        "hc",
        "readTextFile",
        "writeTextFile",
        "watchFolder",
        "pickFolder",
    ] {
        module = module.async_function(name, move |_| {
            Ok(async move {
                Err(HostError::new(format!(
                    "{name}: unavailable in offline headless prototype"
                )))
            })
        });
    }
    for name in [
        "run",
        "openOverlay",
        "closeOverlay",
        "notify",
        "playSound",
        "setAgentOrder",
    ] {
        module = module.function(name, move |_| {
            Err(HostError::new(format!(
                "{name}: no attached client in offline headless prototype"
            )))
        });
    }
    module
}
