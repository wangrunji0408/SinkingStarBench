// The original engine uses raw guest addresses throughout its unsafe routines.
#![allow(unsafe_op_in_unsafe_fn)]

include!("engine.rs");
mod movie;
mod overworld;
mod render;

mod cli;
use clap::{CommandFactory, Parser};
use std::{env, fs, io::Write, path::Path, time::Instant};

fn main() {
    let args = cli::Cli::parse();
    if args.version {
        println!(
            "{}",
            serde_json::json!({"game_build":env!("STAR_GAME_BUILD"),"source_sha256":env!("STAR_SOURCE_SHA256")})
        );
        return;
    }
    use cli::Command::*;
    match args.command {
        None => {
            cli::Cli::command().print_long_help().unwrap();
            println!();
        }
        Some(ListLevels) => {
            let mut out = std::io::stdout().lock();
            let result = assets::INDEX
                .iter()
                .filter_map(|e| e.0.strip_prefix("scene/"))
                .try_for_each(|level| writeln!(out, "{level}"));
            if let Err(e) = result {
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    eprintln!("sinking-star list-levels: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(Render(args)) => {
            if let Err(e) = run_render(args) {
                eprintln!("sinking-star render: {e}");
                std::process::exit(1);
            }
        }
        Some(Replay {
            level,
            input,
            state,
            trace,
            instant_solve,
            inspect_radius,
        }) => {
            if !has_scene(&level) {
                cli::Cli::command()
                    .error(
                        clap::error::ErrorKind::InvalidValue,
                        format!("unknown scene {level:?}"),
                    )
                    .exit();
            }
            unsafe {
                if env::var_os("STAR_BATCH").is_some() {
                    install_batch_signals();
                }
                if instant_solve {
                    if level != "overworld" {
                        eprintln!("--instant-solve requires overworld");
                        std::process::exit(2);
                    }
                    overworld::enable();
                }
                if inspect_radius.is_some() && level != "overworld" {
                    eprintln!("--inspect-radius requires overworld");
                    std::process::exit(2);
                }
                replay(&level, input, trace.as_deref(), inspect_radius, state);
            }
        }
    }
}

unsafe fn install_batch_signals() {
    unsafe extern "C" {
        fn signal(n: i32, handler: extern "C" fn(i32)) -> usize;
        fn _exit(x: i32) -> !;
    }
    extern "C" fn fail(n: i32) {
        unsafe { _exit(128 + n) }
    }
    for s in [4, 5, 6, 7, 10, 11] {
        signal(s, fail);
    }
}

unsafe fn replay(
    key: &str,
    input: routes::Route,
    trace_path: Option<&Path>,
    _radius: Option<u8>,
    state: bool,
) {
    let route = input
        .read()
        .map_err(|e| e.to_string())
        .and_then(|b| routes::parse(&b))
        .unwrap_or_else(|error| {
            println!("{}", serde_json::json!({"error":error}));
            std::process::exit(2)
        });
    let (level, ranges) = init_world(key, trace_path.is_some());
    check_mapping();
    let mut frames = 0;
    let mut ctrl = false;
    let mut counts = Vec::<usize>::new();
    let mut trace = trace_path.map(|path| std::io::BufWriter::new(fs::File::create(path).unwrap()));
    let tail = [
        (W + layout::TRANSITION, 1),
        (level + layout::PHASE_FLAGS, 4),
        (W + layout::INPUT_QUEUE, 24),
        (W + layout::ACTIVE_FAMILY, 8),
        (W + 0x67d8, 8),
        (W + 0x67f0, 8),
        (W + 0x4178, 4),
    ];
    let start = Instant::now();
    let result = play_route(&route, |step| {
        match step {
            ReplayStep::Input(_, key) => {
                op(key, &mut ctrl);
                return Ok(());
            }
            ReplayStep::Frame(tick) => {
                advance(ctrl);
                frames = tick;
                return Ok(());
            }
            ReplayStep::Boundary => {}
        }
        if let Some(f) = trace.as_mut() {
            let q = W + layout::INPUT_QUEUE;
            let n = get::<usize>(q);
            if n > 0 {
                f.write_all(std::slice::from_raw_parts(
                    get::<usize>(q + 8) as *const u8,
                    n * 28,
                ))
                .unwrap();
            }
            f.write_all(&frames.to_le_bytes()).unwrap();
            f.write_all(&(*guest_cursor as u64).to_le_bytes()).unwrap();
            counts.push(n);
            for &(a, n) in ranges.iter().chain(tail.iter()) {
                f.write_all(std::slice::from_raw_parts(a as *const u8, n))
                    .unwrap();
            }
        }
        Ok(())
    });
    if let Err(error) = result {
        println!("{}", serde_json::json!({"error":error.to_string()}));
        std::process::exit(2);
    }
    let elapsed = start.elapsed().as_nanos();
    if let Some(path) = trace_path {
        trace.as_mut().unwrap().flush().unwrap();
        let ranges = ranges.iter().chain(tail.iter()).collect::<Vec<_>>();
        let layout = serde_json::json!({"version":1,"ranges":ranges,"queue_counts":counts});
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(".json");
        fs::write(sidecar, layout.to_string()).unwrap();
    }
    let mut result = serde_json::json!({"frames":frames,"won":won(),"target_level":target_level(),"replay_ns":elapsed as u64});
    overworld::report(&mut result, false);
    if let Some(radius) = _radius {
        result["map_state"] = overworld::inspect(radius);
    }
    if state {
        result["state"] = state::snapshot(key, frames);
    }
    println!("{result}");
}

fn emit_render_data(
    key: &str,
    route: &[routes::Key],
    mode: &str,
    fps: u32,
    out: &mut impl Write,
) -> std::io::Result<()> {
    unsafe {
        movie::enable();
        init_world(key, false);
        movie::emit(key, route, mode, fps, |j| {
            serde_json::to_writer(&mut *out, &j)?;
            out.write_all(b"\n")
        })
    }
    .and_then(|()| out.flush())
}

fn run_render(args: cli::Render) -> render::io::Res<()> {
    if args.instant_solve {
        if args.level.as_deref() != Some("overworld") {
            return Err("--instant-solve requires overworld".into());
        }
        overworld::enable();
    }
    if args.version {
        println!("{}", render::io::source());
        return Ok(());
    }
    if let Some(mode) = args.stream_mode() {
        let key = args.level.as_deref().unwrap();
        if !has_scene(key) {
            return Err(format!("unknown scene {key:?}").into());
        }
        let route = routes::parse(&args.input.read()?)?;
        use std::io::BufWriter;
        let mut out = BufWriter::new(std::io::stdout().lock());
        let result = emit_render_data(
            key,
            &route,
            mode,
            args.fps.unwrap_or(cli::DEFAULT_FPS),
            &mut out,
        );
        if let Err(e) = result {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(e.into());
            }
        }
        return Ok(());
    }
    render::run(args)
}
