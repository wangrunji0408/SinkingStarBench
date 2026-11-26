// Exercise the original scheduler at even and odd victory ticks and on a non-win.
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn timeline(level: &str, keys: &[u8]) -> Vec<Value> {
    timeline_with_options(level, keys, &[])
}

fn timeline_with_options(level: &str, keys: &[u8], options: &[&str]) -> Vec<Value> {
    stream(level, keys, "--movie-stream", options)
}

fn stream(level: &str, keys: &[u8], mode: &str, options: &[&str]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
        .args(["render", level, mode])
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(keys).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    out.stdout
        .split(|&c| c == b'\n')
        .filter(|line| !line.is_empty())
        .map(|l| serde_json::from_slice(l).unwrap())
        .collect()
}

#[test]
fn custom_fps_keeps_the_final_frame_between_samples() {
    for (fps, frames) in [("20", 44), ("1", 4)] {
        let rows = timeline_with_options(
            "heroes1_1",
            include_bytes!("../solutions/heroes1_1.txt"),
            &["--fps", fps],
        );
        let end = rows.last().unwrap();
        assert_eq!(end["ticks"], 106);
        assert_eq!(end["render_ticks"], 127);
        assert_eq!(end["settled"], true);
        assert_eq!(end["frames"], frames);
        assert_eq!(rows[rows.len() - 2]["tick"], 127);
    }
}

#[test]
fn fps_must_divide_sixty() {
    for fps in ["0", "24", "59", "120"] {
        let out = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
            .args(["render", "heroes1_1", "--fps", fps])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&out.stderr).contains("positive divisor of 60"));
    }
}
#[test]
fn victory_tail_ends_when_the_scheduler_unblocks() {
    for (level, keys, win_tick, settled_tick) in [
        (
            "heroes1_1",
            include_bytes!("../solutions/heroes1_1.txt").as_slice(),
            106,
            127,
        ),
        (
            "heroes2_2b",
            include_bytes!("../solutions/heroes2_2b.txt").as_slice(),
            133,
            152,
        ),
    ] {
        let rows = timeline(level, keys);
        let end = rows.last().unwrap();
        assert_eq!(end["ticks"], win_tick);
        assert_eq!(end["render_ticks"], settled_tick);
        assert_eq!(end["settled"], true);
        let frames = &rows[1..rows.len() - 1];
        let win = frames
            .iter()
            .position(|f| f["tick"] == win_tick && f["won"] == true)
            .unwrap();
        let tail = &frames[win + 1..];
        assert_eq!(tail.len() as u64, (settled_tick - win_tick + 1) / 2);
        for (i, f) in tail.iter().enumerate() {
            assert_eq!(f["tick"], (win_tick + (i as u64 + 1) * 2).min(settled_tick))
        }
    }
}

#[test]
fn first_level_reaches_the_exit_after_victory() {
    let rows = timeline("heroes1_1", include_bytes!("../solutions/heroes1_1.txt"));
    let hero = rows[0]["initial"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["type"] == "Guy")
        .unwrap();
    let mut position = &hero["f"];
    let mut samples = Vec::new();
    for frame in &rows[1..rows.len() - 1] {
        if let Some(entity) = frame["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entity| entity["id"] == hero["id"])
        {
            position = &entity["f"];
        }
        let x = position["position.x"].as_f64().unwrap();
        let y = position["position.y"].as_f64().unwrap();
        samples.push((frame["tick"].as_u64().unwrap(), [x, y]));
    }
    let at_win = samples.iter().find(|(tick, _)| *tick == 106).unwrap().1;
    let (tick, settled) = *samples.last().unwrap();
    assert_eq!(tick, 127);
    assert!((at_win[1] - 2.).abs() > 0.5);
    assert!((settled[0] - 10.).abs() < 0.02 && (settled[1] - 2.).abs() < 0.02);
}
#[test]
fn unfinished_route_completes_pending_movement_before_extra_wait() {
    for (route, ticks, frames) in [(b"R".as_slice(), 9, 6), (b"RT8", 17, 10)] {
        let rows = timeline("heroes1_1", route);
        let end = rows.last().unwrap();
        assert_eq!(end["won"], false);
        assert_eq!(end.get("target_level"), Some(&Value::Null));
        assert_eq!(end["ticks"], ticks);
        assert_eq!(end["render_ticks"], ticks);
        assert!(end["settled"].is_null());
        assert_eq!(end["frames"], frames);
    }
}

#[test]
fn named_exit_stops_at_transition_without_a_victory_tail() {
    let rows = stream(
        "intro",
        include_bytes!("../solutions/intro.txt"),
        "--movie-keys",
        &[],
    );
    let end = rows.last().unwrap();
    assert_eq!(end["won"], false);
    assert_eq!(end["target_level"], "overworld");
    assert_eq!(end["render_ticks"], end["ticks"]);
    assert!(end["settled"].is_null());
}

#[test]
fn replay_samples_dynamically_spawned_water_effects() {
    let rows = timeline(
        "lily_pull_intro",
        include_bytes!("../solutions/lily_pull_intro.txt"),
    );
    assert!(
        rows.iter()
            .filter_map(|f| f["changes"].as_array())
            .flatten()
            .any(|e| e["type"] == "Turbulence")
    );
    let end = rows.last().unwrap();
    assert_eq!(end["won"], true);
    assert_eq!(end["frames"], rows.len() - 2);
}

#[test]
fn overworld_render_initializes_and_advances() {
    // A missing layer marker can leave the original wraparound ray query
    // spinning inside START_WORLD, before the renderer receives its first frame.
    let mut child = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
        .args(["render", "overworld", "--movie-keys"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"T2").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("overworld initialization did not finish");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let end: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(end["render_ticks"], 2);
    assert_eq!(end["won"], false);
}

#[test]
fn screenshot_stream_preserves_initial_effects_and_all_movie_samples() {
    let movie = timeline("heroes1_1", b"RT4");
    let image = stream("heroes1_1", b"RT4", "--movie-image-stream", &[]);
    assert_eq!(
        image[0]["initialVisualEvents"],
        movie[0]["initial"]["initialVisualEvents"]
    );
    assert_eq!(image[0]["frame"], movie[1]);
    assert_eq!(image[1..], movie[2..]);
}

#[test]
fn completing_an_overworld_entry_updates_its_tint_and_removes_only_its_thawed_ice() {
    let moves = b"SUT12UT12UT12UT12UT12RT12RT12UT12UT12UT12UT12UT12UT12LT12LT9VT3UT6VT6UT6VT6UT6V";
    let rows = timeline_with_options("overworld", moves, &["--instant-solve", "--fps", "1"]);
    let states = |uid: u64| {
        rows.iter()
            .filter_map(|r| r["changes"].as_array())
            .flatten()
            .filter(|e| e["id"] == uid)
            .collect::<Vec<_>>()
    };
    let ice = states(111254912); // (80,86), unlocked by heroes1_4.
    assert_eq!(ice.first().unwrap()["exists"], true);
    assert_eq!(ice.last().unwrap()["exists"], false);
    assert_eq!(states(111254914).last().unwrap()["exists"], true); // next, still locked
    let star = states(14680953);
    let f = &star.last().unwrap()["f"];
    assert_eq!(f["solved_t_target"], 1.);
    assert_eq!(f["mesh"], "GAME_OVER_Entry_Solved_A");
    for axis in ["x", "y", "z", "w"] {
        assert_eq!(
            f[format!("tint_color_a.{axis}")],
            f[format!("color_solved.{axis}")]
        );
    }
}
