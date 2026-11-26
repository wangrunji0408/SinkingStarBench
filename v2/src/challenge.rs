// The original engine uses raw guest addresses throughout its unsafe routines.
#![allow(unsafe_op_in_unsafe_fn)]

include!("engine.rs");
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs, io,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
};
mod overworld;
mod terrain;

// The shared game object references this callback. Only the full CLI enables it.
#[unsafe(no_mangle)]
extern "C" fn star_particle_event(_: usize, _: usize, _: usize, _: usize) {}

/// 回放 Sinking Star 关卡并检查挑战成绩。
///
/// 你有 3 小时。尽可能多地通关；通关数相同时，总通关帧数越小越好。
/// 用 `./sinking-star-challenge check solutions` 查看成绩和逐关判定。
///
/// 只允许读写自己的工作目录。提供的二进制只能运行，不得读取或逆向分析。
/// 不得查阅源码、现成解法或外部资料，不得联网搜索，也不得使用子 Agent。
///
/// 从 `./sinking-star-challenge replay intro` 开始。可以输入空路线查看初始状态。
/// 序章通关后，保存路线到 `solutions/intro.txt`，用返回的密钥回放大地图 `overworld`。
/// 在大地图上走到已解锁的入口并按 V，获得单关密钥。保存地图路线到 `solutions/overworld.txt`。
/// 每个通关的关卡提交 `solutions/<关卡ID>.txt`，同一关只保留帧数最少的有效解法。
///
/// 用 `./sinking-star-challenge replay --help` 查看按键、路线文件、密钥和状态输出的说明。
/// 用 `./sinking-star-challenge check --help` 查看计分规则。
#[derive(Parser)]
#[command(
    name = "sinking-star-challenge",
    disable_help_subcommand = true,
    verbatim_doc_comment
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// 回放路线，输出胜负、帧数及结束时的实体和地形（state）。
    ///
    /// 路线是连续的 ASCII 按键串，区分大小写；允许空串，不允许空白（含末尾换行）。
    /// 用 `-i 路线文件` 从文件读取；不写 -i 时从标准输入读到 EOF。
    ///
    ///   U/D/L/R   按初始镜头方向移动
    ///   X         动作
    ///   C         切换到下一个可选家族
    ///   V         拿取
    ///   F         龙动作
    ///   Z         撤销
    ///   S / s     按下 / 松开 Shift（跑步）
    ///   K / k     按下 / 松开 Ctrl（慢速）
    ///   T数字     等待操作完成后，再推进指定帧数
    ///
    /// 连续按键可进入队列。T、大地图上的 V 和路线末尾会先等队列清空、角色移动完成。
    /// T0 只等待操作完成。帧数为十进制非负整数；游戏每秒 60 帧，按住 Ctrl 时模拟时间为四分之一。
    /// 通关或触发场景切换后立即停止。有效路线返回一行 JSON，退出码为 0（未通关也一样）。
    ///
    /// intro 不需要密钥。进入 overworld 时返回 won: true、target_level: "overworld" 和 target_level_key。
    /// 回放 overworld 时传入 target_level_key。在已解锁的入口按 V，地图会将该关视为立即通关，让你继续探索。
    /// 大地图返回 playable_levels（本次路线按 V 进入过的关卡）和 level_keys（关卡 ID 到密钥的映射）。
    /// 回放其他关卡时用 level_keys 中本关的密钥。同一份二进制的密钥可重复使用。
    /// 用 `--key KEY` 传入密钥，或在路线文件首行写 `# KEY`，下一行写按键串。两处都提供时必须一致。
    ///
    /// 空路线可以查看初始状态。实体字段由 state.entity_defaults[实体.type] 与实体自己的 fields 合并，后者优先。
    /// state.terrain.solid_runs 的每项 [z, y, x0, x1] 表示一段实心格（含两端）。
    /// 同一对象中的 ocean_maps 给出水域，input_directions 给出各方向键对应的坐标变化。
    #[command(verbatim_doc_comment)]
    Replay {
        /// 关卡 ID，区分大小写。
        level: String,
        #[command(flatten)]
        input: routes::Route,
        /// 目标关卡的密钥（intro 不使用）。
        #[arg(long, value_name = "KEY")]
        key: Option<String>,
    },
    /// 检查提交目录，输出 solved_count（通关数）和 total_frames（总通关帧数）。
    ///
    /// intro.txt 成功进入 overworld 算一关；overworld.txt 只用于确定已进入的关卡。
    /// 其他 <关卡ID>.txt 只有在地图路线中已按 V 进入、且本关序列获胜时才计分。
    /// 计分会重跑提交的路线，并跳过文件首行的可选密钥注释。密钥本身不计分。
    /// 排名先比较 solved_count（越多越好），相同时比较 total_frames（越少越好）。
    /// levels 按关卡 ID 列出提交结果：won 为是否计分；通过时有 frames，未通过时有 reason。
    #[command(verbatim_doc_comment)]
    Check {
        /// 包含 intro.txt、overworld.txt 和各关解法的目录。
        #[arg(value_name = "DIR")]
        solutions: PathBuf,
    },
}

fn main() {
    let result = match Cli::parse().command {
        Some(Command::Replay { level, input, key }) => {
            if !has_scene(&level) {
                Cli::command()
                    .error(
                        clap::error::ErrorKind::InvalidValue,
                        format!("unknown scene {level:?}"),
                    )
                    .exit();
            }
            replay(&level, input, key.as_deref())
        }
        Some(Command::Check { solutions }) => score(&solutions),
        None => {
            Cli::command().print_long_help().unwrap();
            println!();
            return;
        }
    };
    match result {
        Ok(result) => println!("{result}"),
        Err(error) => {
            println!("{}", json!({"error":error}));
            std::process::exit(2)
        }
    }
}

// HMAC-SHA256 with a build-local secret, truncated to 128 bits for CLI use.
fn level_key(level: &str) -> String {
    const SECRET: &[u8; 32] = include_bytes!(concat!(env!("OUT_DIR"), "/challenge-replay-key.bin"));
    let (mut inner_pad, mut outer_pad) = ([0x36; 64], [0x5c; 64]);
    for (i, &byte) in SECRET.iter().enumerate() {
        inner_pad[i] ^= byte;
        outer_pad[i] ^= byte;
    }
    let inner = Sha256::new()
        .chain_update(inner_pad)
        .chain_update(b"sinking-star/challenge/replay/v1\0")
        .chain_update(level.as_bytes())
        .finalize();
    let mac = Sha256::new()
        .chain_update(outer_pad)
        .chain_update(inner)
        .finalize();
    format!(
        "{:032x}",
        u128::from_be_bytes(mac[..16].try_into().unwrap())
    )
}

fn split_route_key(bytes: &[u8]) -> Result<(Option<&str>, &[u8]), String> {
    let Some(header_and_route) = bytes.strip_prefix(b"#") else {
        return Ok((None, bytes));
    };
    let newline = header_and_route
        .iter()
        .position(|&byte| byte == b'\n')
        .ok_or("key header must end with a newline")?;
    let key = std::str::from_utf8(&header_and_route[..newline])
        .map_err(|_| "key header must be UTF-8")?
        .trim();
    Ok((Some(key), &header_and_route[newline + 1..]))
}

fn replay(level: &str, input: routes::Route, key: Option<&str>) -> Result<Value, String> {
    let bytes = input.read().map_err(|e| e.to_string())?;
    let (file_key, route) = split_route_key(&bytes)?;
    if let (Some(key), Some(file_key)) = (key, file_key)
        && key != file_key
    {
        return Err("--key does not match the route's key header".into());
    }
    match (level, key.or(file_key)) {
        ("intro", None) => {}
        ("intro", Some(_)) => return Err("intro does not use a key".into()),
        (_, None) => {
            return Err(format!(
                "a key is required for level {level:?} (use --key or a # KEY header)"
            ));
        }
        (_, Some(key)) if key == level_key(level) => {}
        _ => return Err(format!("invalid key for level {level:?}")),
    }
    let route = routes::parse(route)?;
    let mut result = simulate(level, &route, true)?;
    match level {
        "intro" if result["won"] == true => {
            result["target_level_key"] = json!(level_key("overworld"))
        }
        "overworld" => {
            let keys = map_levels(&result, "playable_levels")
                .map(|name| (name.to_owned(), json!(level_key(name))))
                .collect::<serde_json::Map<_, _>>();
            result["level_keys"] = json!(keys);
        }
        _ => {}
    }
    Ok(result)
}

fn map_levels<'a>(map: &'a Value, key: &str) -> impl Iterator<Item = &'a str> {
    map[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
}

fn score(solutions: &Path) -> Result<Value, String> {
    let dir_error = |e: io::Error| format!("{}: {e}", solutions.display());
    let mut submitted = BTreeSet::new();
    for entry in fs::read_dir(solutions).map_err(dir_error)? {
        let entry = entry.map_err(dir_error)?;
        if entry.file_type().map_err(dir_error)?.is_file()
            && let Some(name) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.strip_suffix(".txt"))
            && !name.is_empty()
        {
            submitted.insert(name.to_owned());
        }
    }
    let intro = submitted
        .contains("intro")
        .then(|| saved_replay(solutions, "intro").ok())
        .flatten();
    let intro_ok = intro.as_ref().is_some_and(|r| r["won"] == true);
    let map = (intro_ok && submitted.contains("overworld"))
        .then(|| saved_replay(solutions, "overworld").ok())
        .flatten();
    let unlocked: BTreeSet<_> = map
        .iter()
        .flat_map(|r| map_levels(r, "unlocked_levels"))
        .collect();
    let playable: BTreeSet<_> = map
        .iter()
        .flat_map(|r| map_levels(r, "playable_levels"))
        .collect();

    let (mut solved_count, mut total_frames) = (0u64, 0u64);
    let mut levels = serde_json::Map::new();
    for level in submitted {
        let outcome = match level.as_str() {
            "intro" => match &intro {
                Some(result) if intro_ok => Ok(result["frames"].as_u64().unwrap()),
                Some(_) => Err("did_not_reach_overworld"),
                None => Err("invalid_route"),
            },
            _ if !has_scene(&level) => Err("unknown_level"),
            _ if !intro_ok => Err("intro_required"),
            "overworld" if map.is_some() => Err("map_only"),
            "overworld" => Err("invalid_route"),
            _ if map.is_none() => Err("overworld_required"),
            _ if !playable.contains(level.as_str()) => Err(if unlocked.contains(level.as_str()) {
                "not_entered"
            } else {
                "locked"
            }),
            _ => match saved_replay(solutions, &level) {
                Ok(result) if result["won"] == true => Ok(result["frames"].as_u64().unwrap()),
                Ok(_) => Err("not_won"),
                Err(_) => Err("invalid_route"),
            },
        };
        let status = match outcome {
            Ok(frames) => {
                solved_count += 1;
                total_frames += frames;
                json!({"won": true, "frames": frames})
            }
            Err(reason) => json!({"won": false, "reason": reason}),
        };
        levels.insert(level, status);
    }
    Ok(json!({"solved_count": solved_count, "total_frames": total_frames, "levels": levels}))
}

fn saved_replay(solutions: &Path, level: &str) -> Result<Value, String> {
    let path = solutions.join(format!("{level}.txt"));
    let route = fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| routes::parse(split_route_key(&bytes)?.1))
        .map_err(|error| format!("{}: {error}", path.display()))?;
    isolated_replay(level, &route)
}

fn simulate(level: &str, route: &[routes::Key], include_snapshot: bool) -> Result<Value, String> {
    let (mut frames, mut ctrl) = (0, false);
    unsafe {
        if level == "overworld" {
            overworld::enable();
        }
        init_world(level, false);
        check_mapping();
        play_route(route, |step| {
            match step {
                ReplayStep::Input(index, key) => {
                    let _ = index;
                    op(key, &mut ctrl);
                }
                ReplayStep::Frame(tick) => {
                    advance(ctrl);
                    frames = tick;
                }
                ReplayStep::Boundary => {}
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        let mut result = json!({"frames":frames,"won":won()});
        if level == "intro" {
            let target = target_level();
            result["won"] = json!(target.as_deref() == Some("overworld"));
            result["target_level"] = json!(target);
        } else if level == "overworld" {
            overworld::report(&mut result, true);
        }
        if include_snapshot {
            let mut snapshot = state::snapshot(level, frames);
            snapshot["won"] = result["won"].clone();
            snapshot["terrain"] = terrain::snapshot();
            result["state"] = snapshot;
        }
        Ok(result)
    }
}

fn isolated_replay(level: &str, route: &[routes::Key]) -> Result<Value, String> {
    unsafe extern "C" {
        fn fork() -> i32;
        fn _exit(status: i32) -> !;
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }
    let (reader, writer) = UnixStream::pair().map_err(|e| e.to_string())?;
    // Each replay needs fresh fixed-address guest memory. Fork the
    // single-threaded process before any scene is initialized.
    match unsafe { fork() } {
        -1 => Err(io::Error::last_os_error().to_string()),
        0 => {
            drop(reader);
            let result = simulate(level, route, false).map(|mut result| {
                if level == "overworld" {
                    // Used only to distinguish scoring reasons, never in public replay output.
                    result["unlocked_levels"] = json!(unsafe { overworld::unlocked_levels() });
                }
                result
            });
            let result = result.unwrap_or_else(|error| json!({"error":error}));
            let result = serde_json::to_writer(writer, &result);
            unsafe { _exit(i32::from(result.is_err())) }
        }
        pid => {
            drop(writer);
            let result: serde_json::Result<Value> = serde_json::from_reader(reader);
            let mut status = 0;
            while unsafe { waitpid(pid, &mut status, 0) } == -1 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error.to_string());
                }
            }
            if status == 0 {
                let result = result.map_err(|e| format!("invalid {level} replay result: {e}"))?;
                match result["error"].as_str() {
                    Some(error) => Err(format!("{level}: {error}")),
                    None => Ok(result),
                }
            } else {
                Err(format!("{level} replay failed (wait status {status})"))
            }
        }
    }
}
