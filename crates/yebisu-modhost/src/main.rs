mod capture;
mod host;
mod wire_output;

use anyhow::{Context as _, Result, bail};
use gpui::{AppContext, WindowBounds, WindowOptions, bounds, point, px, size};
use gpui_shell::{Capabilities, ShellRoot, ShellRuntime, policy::Policy};
use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("warn")
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "inspect" {
        println!("{}", wire_output::inspect(std::path::Path::new(&args[1]))?);
        return Ok(());
    }
    if args.len() != 4 {
        bail!("usage: yebisu-modhost <isolated-home> <output-directory> <seconds> <mod,mod,...>");
    }
    let home = PathBuf::from(&args[0]).canonicalize()?;
    let output = PathBuf::from(&args[1]);
    std::fs::create_dir_all(&output)?;
    let seconds: u64 = args[2].parse()?;
    anyhow::ensure!(
        (1..=86400).contains(&seconds),
        "duration must be 1..86400 seconds"
    );
    let names: Vec<String> = args[3].split(',').map(str::to_owned).collect();
    anyhow::ensure!(
        !names.is_empty() && names.len() <= 16,
        "1..16 mods required"
    );
    for name in &names {
        anyhow::ensure!(
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid mod name"
        );
        anyhow::ensure!(
            home.join("mods").join(name).join("main.js").is_file(),
            "missing main.js for {name}"
        );
    }
    let boot = random_id()?;
    let errors = Rc::new(RefCell::new(Vec::<String>::new()));
    let failures = errors.clone();
    gpui::Application::with_platform(gpui_platform::current_platform(true))
        .with_assets(gpui_shell::AppAssets::new(home.clone()))
        .run(move |cx| {
            gpui_shell::init(cx);
            let shared_runtime = ShellRuntime::new(cx).expect("initialize shell runtime");
            for name in names {
                let result = (|| -> Result<()> {
                    let policy = Policy::new()
                        .with_application(name.clone())
                        .with_capabilities(Capabilities::new().storage(true))
                        .with_storage_path(output.join(format!("{name}-storage.json")))
                        .with_host_module(host::module(&home))?;
                    gpui_shell::policy::set_default(policy);
                    let runtime = shared_runtime.clone();
                    let app = runtime.load_application(&home, &format!("mods/{name}/main.js"))?;
                    let (tx, rx) = async_channel::bounded(1);
                    let mut mounted = None;
                    let handle = cx.open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(bounds(
                                point(px(0.), px(0.)),
                                size(px(800.), px(600.)),
                            ))),
                            show: false,
                            ..Default::default()
                        },
                        |window, cx| {
                            let signal = tx.clone();
                            window.set_headless_frame_waker(Rc::new(move || {
                                let _ = signal.try_send(());
                            }));
                            let view = runtime
                                .mount_application(&app, window, cx)
                                .expect("mount script");
                            mounted = Some(view.clone());
                            cx.new(|cx| ShellRoot::new(view.into(), window, cx))
                        },
                    )?;
                    let handle: gpui::AnyWindowHandle = handle.into();
                    let view = mounted.context("script not mounted")?;
                    let output = output.clone();
                    let home = home.clone();
                    let name = name.clone();
                    let incarnation = random_id()?;
                    let boot = boot.clone();
                    let errors = errors.clone();
                    tx.try_send(()).ok();
                    cx.spawn(async move |cx| {
                        let _runtime = runtime;
                        let mut previous: Option<(u64, serde_json::Value)> = None;
                        let mut generation = 0;
                        let mut last = Instant::now() - Duration::from_secs(1);
                        while rx.recv().await.is_ok() {
                            let delay = Duration::from_millis(34).saturating_sub(last.elapsed());
                            if !delay.is_zero() {
                                cx.background_executor().timer(delay).await;
                            }
                            last = Instant::now();
                            let result = handle.update(cx, |_, window, cx| -> Result<bool> {
                                let (_, again) = window.drive_headless_frame(cx);
                                let state = view.read(cx);
                                if let Some(error) = state.build_error() {
                                    bail!("{name}: {error}");
                                }
                                if let Some(snapshot) = state.snapshot() {
                                    let changed = match &previous {
                                        Some((shell_generation, published))
                                            if *shell_generation == snapshot.generation() =>
                                        {
                                            capture::retained_changed(
                                                published,
                                                state,
                                                cx,
                                                window,
                                                capture::Destination {
                                                    home: &home,
                                                    output: &output,
                                                    incarnation: &incarnation,
                                                    revision: generation,
                                                },
                                            )?
                                        }
                                        _ => true,
                                    };
                                    if changed {
                                        generation += 1;
                                        std::fs::write(
                                            output.join(format!("{name}-{generation:04}.tree")),
                                            snapshot.debug_tree(),
                                        )?;
                                        let publish = capture::publish(
                                            snapshot,
                                            state,
                                            cx,
                                            window,
                                            capture::Destination {
                                                home: &home,
                                                output: &output,
                                                incarnation: &incarnation,
                                                revision: generation,
                                            },
                                        )?;
                                        wire_output::write(
                                            &output,
                                            &name,
                                            generation,
                                            &boot,
                                            &incarnation,
                                            &publish,
                                        )?;
                                        previous = Some((snapshot.generation(), publish));
                                    }
                                }
                                Ok(again)
                            });
                            match result {
                                Ok(Ok(true)) => {
                                    tx.try_send(()).ok();
                                }
                                Ok(Ok(false)) => {}
                                other => {
                                    errors.borrow_mut().push(format!("{name}: {other:?}"));
                                    break;
                                }
                            }
                        }
                    })
                    .detach();
                    Ok(())
                })();
                if let Err(error) = result {
                    errors.borrow_mut().push(format!("{name}: {error:#}"));
                }
            }
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_secs(seconds))
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    anyhow::ensure!(
        failures.borrow().is_empty(),
        "render failures: {:?}",
        failures.borrow()
    );
    Ok(())
}

fn random_id() -> Result<String> {
    let mut random = [0u8; 16];
    std::io::Read::read_exact(&mut std::fs::File::open("/dev/urandom")?, &mut random)?;
    Ok(random.iter().map(|b| format!("{b:02x}")).collect())
}
