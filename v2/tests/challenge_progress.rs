use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

const INTRO: &[u8] = include_bytes!("../solutions/intro.txt");
const OVERWORLD: &[u8] = include_bytes!("../solutions/overworld.txt");

fn map_through_entry(n: usize) -> &'static [u8] {
    let last = OVERWORLD
        .iter()
        .enumerate()
        .filter(|&(_, &op)| op == b'V')
        .nth(n - 1)
        .unwrap()
        .0;
    &OVERWORLD[..=last]
}

fn with_key(key: &str, route: &[u8]) -> Vec<u8> {
    [format!("# {key}\n").as_bytes(), route].concat()
}

struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("star-challenge-{name}-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn replay(level: &str, route: &[u8], key: Option<&str>) -> (bool, Value) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sinking-star-challenge"))
        .args(["replay", level])
        .args(key.into_iter().flat_map(|key| ["--key", key]))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(route).unwrap();
    let output = child.wait_with_output().unwrap();
    (
        output.status.success(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}

#[test]
fn final_state_waits_for_moves_and_t_adds_frames_afterward() {
    let key = overworld_key();
    let (ok, map) = replay("overworld", map_through_entry(1), Some(&key));
    assert!(ok);
    let key = map["level_keys"]["heroes1_1"].as_str();

    let (ok, initial) = replay("heroes1_1", b"", key);
    assert!(ok);
    let (ok, moved) = replay("heroes1_1", b"R", key);
    assert!(ok);
    let (ok, waited) = replay("heroes1_1", b"RT25", key);
    assert!(ok);

    let before = &initial["state"];
    let after = &moved["state"];
    assert_eq!(before["frames"], 0);
    assert_eq!(after["input_queue"], serde_json::json!([]));
    let moved_frames = after["frames"].as_u64().unwrap();
    assert!(moved_frames > 1);
    assert_eq!(waited["frames"], moved_frames + 25);
    let player = |state: &Value| {
        state["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["type"] == "Guy")
            .unwrap()
            .clone()
    };
    let start_player = player(before);
    let end_player = player(after);
    assert_eq!(start_player["id"], end_player["id"]);
    assert_ne!(start_player["position"], end_player["position"]);
    assert_eq!(player(&waited["state"])["position"], end_player["position"]);
    assert!(after["entity_defaults"]["Guy"].is_object());

    let terrain = &after["terrain"];
    assert_eq!(terrain["origin"], serde_json::json!([0, 0, -1]));
    assert_eq!(terrain["size"], serde_json::json!([13, 5, 3]));
    // At y=2 the floor is one layer below the player, not level with it.
    let solids = terrain["solid_runs"].as_array().unwrap();
    assert!(solids.contains(&serde_json::json!([-1, 2, 1, 11])));
    assert!(!solids.iter().any(|span| {
        span[0] == 0
            && span[1] == 2
            && span[2].as_i64().unwrap() <= 2
            && span[3].as_i64().unwrap() >= 2
    }));
    assert_eq!(terrain["input_directions"]["R"], serde_json::json!([1, 0]));
    assert_eq!(before["terrain"], after["terrain"]);
}

#[test]
fn intro_state_places_water_in_world_coordinates() {
    let (ok, intro) = replay("intro", b"", None);
    assert!(ok);
    assert!(intro.get("target_level_key").is_none());
    let maps = intro["state"]["terrain"]["ocean_maps"].as_array().unwrap();
    let water = maps
        .iter()
        .find(|map| map["position"] == serde_json::json!([39.0, 60.0, 2.0]))
        .unwrap();
    assert_eq!(water["active"], true);
    assert_eq!(water["kind"], 0);
    assert_eq!(water["local_runs"][0], serde_json::json!([-5, 0, 12]));
    assert_eq!(water["world_runs"][0], serde_json::json!([55, 39, 51]));
}

fn overworld_key() -> String {
    let (ok, intro) = replay("intro", INTRO, None);
    assert!(ok);
    assert_eq!(intro["won"], true);
    assert_eq!(intro["target_level"], "overworld");
    intro["target_level_key"].as_str().unwrap().to_owned()
}

#[test]
fn reference_routes_reach_the_map_and_report_playable_levels() {
    let key = overworld_key();
    let (ok, map) = replay("overworld", OVERWORLD, Some(&key));
    assert!(ok);
    assert_eq!(map["frames"], 12_855);
    assert!(map.get("visited").is_none());
    assert!(map.get("entered_levels").is_none());
    assert!(map.get("unlocked_levels").is_none());
    let expected: Value =
        serde_json::from_str(include_str!("../solutions/overworld.json")).unwrap();
    assert_eq!(map["playable_levels"], expected);
    let keys = map["level_keys"].as_object().unwrap();
    assert_eq!(keys.len(), expected.as_array().unwrap().len());
    for level in expected.as_array().unwrap() {
        assert!(keys[level.as_str().unwrap()].is_string());
    }
}

#[test]
fn replay_requires_the_key_for_the_requested_level() {
    let (ok, error) = replay("overworld", b"", None);
    assert!(!ok);
    assert!(
        error["error"]
            .as_str()
            .unwrap()
            .contains("a key is required")
    );
    let (ok, error) = replay("overworld", b"", Some("wrong"));
    assert!(!ok);
    assert_eq!(error["error"], "invalid key for level \"overworld\"");

    let key = overworld_key();
    let first = map_through_entry(1);
    let (ok, before_first_v) = replay(
        "overworld",
        &with_key(&key, &first[..first.len() - 1]),
        None,
    );
    assert!(ok);
    assert_eq!(before_first_v["standing_level"], "heroes1_1");
    assert_eq!(before_first_v["playable_levels"], serde_json::json!([]));
    assert_eq!(before_first_v["level_keys"], serde_json::json!({}));

    let (ok, map) = replay("overworld", first, Some(&key));
    assert!(ok);
    assert_eq!(map["playable_levels"], serde_json::json!(["heroes1_1"]));
    assert_eq!(map["level_keys"].as_object().unwrap().len(), 1);
    let first_key = map["level_keys"]["heroes1_1"].as_str().unwrap();
    assert!(!replay("heroes1_2_v2", b"", None).0);
    assert!(!replay("heroes1_2_v2", b"", Some(first_key)).0);

    let (ok, map) = replay("overworld", map_through_entry(2), Some(&key));
    assert!(ok);
    assert_eq!(map["level_keys"]["heroes1_1"], first_key);
    let key = map["level_keys"]["heroes1_2_v2"].as_str().unwrap();
    let puzzle = include_bytes!("../solutions/heroes1_2_v2.txt");
    let file_route = with_key(key, puzzle);
    let (ok, error) = replay("heroes1_2_v2", &file_route, Some(first_key));
    assert!(!ok);
    assert_eq!(
        error["error"],
        "--key does not match the route's key header"
    );
    let (ok, result) = replay("heroes1_2_v2", &file_route, None);
    assert!(ok);
    assert_eq!(result["won"], true);
    assert_eq!(result["frames"], 240);
}

#[test]
fn check_scores_intro_and_only_winning_entered_submissions() {
    use serde_json::json;
    let scratch = Scratch::new("score");
    let solutions = &scratch.0;
    for (file, bytes) in [
        ("intro.txt", b"T0".as_slice()),
        ("overworld.txt", map_through_entry(1)),
        (
            "heroes1_1.txt",
            include_bytes!("../solutions/heroes1_1.txt"),
        ),
        (
            "heroes1_2_v2.txt",
            include_bytes!("../solutions/heroes1_2_v2.txt"),
        ),
        ("heroes1_3.txt", b"invalid"),
        ("heroes1_4.txt", b"T0"),
        ("unknown_scene.txt", b"T0"),
        ("notes.json", b"{}"),
    ] {
        fs::write(solutions.join(file), bytes).unwrap();
    }
    let check = || {
        let out = Command::new(env!("CARGO_BIN_EXE_sinking-star-challenge"))
            .arg("check")
            .arg(solutions)
            .output()
            .unwrap();
        assert!(out.status.success(), "{out:?}");
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    let blocked = check();
    assert_eq!(blocked["solved_count"], 0);
    assert_eq!(blocked["total_frames"], 0);
    assert_eq!(
        blocked["levels"]["intro"]["reason"],
        "did_not_reach_overworld"
    );
    assert_eq!(blocked["levels"]["heroes1_1"]["reason"], "intro_required");

    fs::write(solutions.join("intro.txt"), INTRO).unwrap();
    fs::write(
        solutions.join("overworld.txt"),
        with_key("00000000000000000000000000000000", map_through_entry(1)),
    )
    .unwrap();
    fs::write(
        solutions.join("heroes1_1.txt"),
        with_key(
            "00000000000000000000000000000000",
            include_bytes!("../solutions/heroes1_1.txt"),
        ),
    )
    .unwrap();
    assert_eq!(
        check(),
        json!({
            "solved_count": 2,
            "total_frames": 2256,
            "levels": {
                "intro": {"won": true, "frames": 2150},
                "overworld": {"won": false, "reason": "map_only"},
                "heroes1_1": {"won": true, "frames": 106},
                "heroes1_2_v2": {"won": false, "reason": "not_entered"},
                "heroes1_3": {"won": false, "reason": "locked"},
                "heroes1_4": {"won": false, "reason": "locked"},
                "unknown_scene": {"won": false, "reason": "unknown_level"}
            }
        })
    );

    fs::write(solutions.join("overworld.txt"), map_through_entry(3)).unwrap();
    fs::write(solutions.join("heroes1_1.txt"), b"T0").unwrap();
    let score = check();
    assert_eq!(score["solved_count"], 2);
    assert_eq!(score["total_frames"], 2390);
    assert_eq!(score["levels"]["heroes1_1"]["reason"], "not_won");
    assert_eq!(score["levels"]["heroes1_2_v2"]["won"], true);
    assert_eq!(score["levels"]["heroes1_3"]["reason"], "invalid_route");
}
