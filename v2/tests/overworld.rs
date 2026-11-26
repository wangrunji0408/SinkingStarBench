use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

#[test]
fn standing_level_describes_the_endpoint_without_entering_or_solving() {
    // Walk around the wall north of spawn and onto the first heroes entrance.
    let first = format!(
        "S{}{}{}{}",
        "UT12".repeat(5),
        "RT12".repeat(2),
        "UT12".repeat(6),
        "LT12".repeat(2)
    );
    for (suffix, expected) in [("", Some("heroes1_1")), ("DT18", None)] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
            .args(["replay", "overworld"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{first}{suffix}").as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            result.get("standing_level"),
            expected.map(Value::from).as_ref()
        );
        assert_eq!(result["won"], false);
        assert!(result["target_level"].is_null());
        assert!(result.get("visited").is_none());
    }
}

#[test]
fn instant_solve_requires_an_enterable_destination() {
    // North along the neighboring column, then west onto heroes1_2_v2:
    // this reaches its tile without unlocking it at heroes1_1.
    let approach = format!("S{}{}", "UT12".repeat(5), "RT12".repeat(2));
    for (turns, expected) in [
        (format!("{}LT12LT12VT12", "UT12".repeat(7)), vec![]),
        (format!("{}LT12LT12UT12", "UT12".repeat(6)), vec![]),
        (
            format!("{}LT12LT12VVUT12V", "UT12".repeat(6)),
            vec!["heroes1_1", "heroes1_2_v2"],
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
            .args(["replay", "overworld", "--instant-solve"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{approach}{turns}").as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let result: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(result["standing_level"], "heroes1_2_v2");
        assert_eq!(result["visited"], serde_json::json!(expected));
    }
}

#[test]
fn inspection_distinguishes_mirror_placements_at_the_same_player_position() {
    let east = include_str!("../routes/overworld/east.txt");
    let first_four = east.split('V').take(4).collect::<Vec<_>>().join("V") + "V";
    for (moves, mirror_y) in [("URRRUDT86", 71.0), ("URRRUUDDT86", 72.0)] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
            .args([
                "replay",
                "overworld",
                "--instant-solve",
                "--inspect-radius",
                "8",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(format!("{first_four}{moves}").as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let j: Value = serde_json::from_slice(&out.stdout).unwrap();
        let state = &j["map_state"];
        assert_eq!(state["position"], serde_json::json!([100.0, 69.0, 0.0]));
        assert_eq!(j["visited"].as_array().unwrap().len(), 4);
        assert!(
            state["objects"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["kind"] == "Mirror"
                    && o["active"] == true
                    && o["position"] == serde_json::json!([100.0, mirror_y, 0.0]))
        );
    }
}

fn map_replay(route: &str, options: &[&str]) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sinking-star"))
        .args(["replay", "overworld", "--instant-solve"])
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(route.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn mirror_copy_can_enter_an_unlocked_level_without_a_scene_transition() {
    let east = include_str!("../routes/overworld/east.txt");
    let route =
        east.split('V').take(10).collect::<Vec<_>>().join("V") + "VRDDDDDDRRRRRRDDDXRDDDRRT265V";
    let j = map_replay(&route, &["--inspect-radius", "8"]);
    assert_eq!(
        j["map_state"]["position"],
        serde_json::json!([118., 51., 0.])
    );
    assert_eq!(j["standing_level"], "mirror_13");
    assert_eq!(
        j["visited"].as_array().unwrap().last().unwrap(),
        "mirror_13"
    );
    assert!(j["target_level"].is_null());
}

#[test]
fn continuous_movements_settle_before_entering_a_level() {
    let j = map_replay("SDRRRRDRRRRDRRRRRRVRV", &[]);
    assert_eq!(j["visited"], serde_json::json!(["mirror_1", "mirror_2"]));
    assert_eq!(j["standing_level"], "mirror_2");
}

#[test]
fn taking_a_discovered_portal_does_not_enter_another_puzzle() {
    let south = include_str!("../routes/overworld/south.txt");
    let south_three = south.split('V').take(3).collect::<Vec<_>>().join("V") + "V";
    let portal = south_three + "RRVT48";
    let small = map_replay(&portal, &["--inspect-radius", "1"]);
    assert_eq!(
        small["map_state"]["position"],
        serde_json::json!([63., 112., 89.])
    );
    assert!(small.get("standing_level").is_none());
    let back = map_replay(&(portal + "LUUUULVT48"), &["--inspect-radius", "1"]);
    assert_eq!(
        back["map_state"]["position"],
        serde_json::json!([75., 51., 0.])
    );
    assert_eq!(back["visited"], small["visited"]);
    assert_eq!(back["visited"].as_array().unwrap().len(), 3);
}
