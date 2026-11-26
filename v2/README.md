# sinking-star

在 macOS 上回放 Steam 原版关卡、导出视频和截图。构建产物是一个可执行文件；Apple Silicon 上需要 Rosetta 2。普通回放和 JSON 状态流不需要安装游戏；导出画面时需要匹配的游戏安装及 D3DMetal。

## 构建

需要 Rust、Xcode 命令行工具，以及 Windows Steam 版游戏 r68307（安装目录中含 `sinking_star.exe`）。在本目录运行：

```sh
rustup target add x86_64-apple-darwin
export GAME_ROOT="/path/to/steamapps/common/Order of the Sinking Star"
cargo build --locked --release
./target/x86_64-apple-darwin/release/sinking-star --help
```

标准 Steam 安装和默认的 Steam CrossOver bottle 可自动探测，其他安装位置用 `GAME_ROOT` 指定。命令行格式为 `sinking-star <command> ...`。

## 挑战版

```sh
cargo build --locked --profile challenge --bin sinking-star-challenge
cp target/x86_64-apple-darwin/challenge/sinking-star-challenge .
```

挑战版是独立的二进制目标，构建会剥离调试符号。给挑战者一份二进制和空工作目录即可，运行时不需要原版游戏目录。

挑战版从 `intro` 开始，无需密钥。成功进入大地图时返回 `won: true`、`target_level: "overworld"` 和 `target_level_key`。回放 `overworld` 时提供该密钥。地图回放会把在已解锁入口按 `V` 视为立即通关，以便继续探索；`playable_levels` 按首次进入的顺序列出本次路线中按 `V` 进入过的关卡，`level_keys` 给出这些关卡各自的密钥。单关回放只需提供目标关卡密钥，同一份二进制的密钥可重复使用。

```sh
./sinking-star-challenge replay intro -i solutions/intro.txt
./sinking-star-challenge replay overworld --key "$OVERWORLD_KEY" -i solutions/overworld.txt
./sinking-star-challenge replay heroes1_1 --key "$HEROES1_1_KEY" -i solutions/heroes1_1.txt
```

也可以把密钥写在路线文件首行 `# 密钥`，下一行写按键串（末尾不加换行），回放时即可省略 `--key`。两处都提供时密钥必须一致；`intro` 不需要密钥行。

用 `./sinking-star-challenge check solutions` 对目录计分，输出一行 JSON，包含 `solved_count`、`total_frames` 与按关卡 ID 列出的 `levels`（每项有 `won`、通关帧数 `frames` 或未计分原因 `reason`）。`intro` 进入大地图计一关；其他关卡必须在地图路线中进入且本关解法获胜才计分；`overworld.txt` 本身不计分（`map_only`）。

## 回放

```sh
sinking-star list-levels  # 每行一个关卡名
sinking-star replay heroes1_1 -i solutions/heroes1_1.txt --trace trace.bin
echo -n 'RT30X' | sinking-star replay heroes1_1
```

回放和单关渲染都用 `-i ROUTE`（或 `--input ROUTE`）指定路线文件，省略时从标准输入读到 EOF。路线是连续的 ASCII 按键串，不含空白或结尾换行；方向键 `U/D/L/R` 按关卡初始镜头解释，`X` 执行动作。连续按键可排队，读到 `Tn` 或路线末尾时先等队列清空、角色移动完成，`Tn` 随后再推进 n 帧（60 帧/秒），`T0` 只等待操作完成。

输出一行 JSON：`frames` 是实际推进的帧数，`won` 表示是否获胜。触发具名场景切换时 `won` 为 `false`，`target_level` 是下一个场景（如 `intro` 出口的 `overworld`），其余为 `null`。`--trace PATH` 可保存逐操作状态，同时生成 `PATH.json` 说明格式。

挑战版的 `replay <LEVEL>` 默认输出 `state`；普通版可加 `--state`。`state` 包含末帧的场景、家族、待执行输入、进行中的移动与当前实体的可读字段；实体字段由 `state.entity_defaults[type]` 与实体 `fields` 合并得到，后者优先。空路线可查看初始状态。快照用于观察模拟状态，不含进程内存或渲染资源，不能用来恢复运行。

挑战版的 `state.terrain` 还包含末帧地形网格：

- `origin`、`size` 按 `[x, y, z]` 排列；`input_directions` 给出各方向键的 `[dx, dy]`。
- `solid_runs` 每条 `[z, y, x0, x1]` 是一段连续实心格（含两端）；地面是角色脚下的实心格。`enabled_column_runs` 以 `[y, x0, x1]` 表示引擎启用的列，仅有启用列不足以判定可走。
- `ocean_maps` 记录水域的位置、朝向、类型与启用状态；普通水面看 `kind: 0` 且 `active: true`，水面高度取 `position[2]`。

判断能否走到某格还要结合高度、门、栅栏、石头与角色能力；没有实心格也可能落水或落空。

### 大地图路线

普通 `replay overworld` 在终点位于入口时输出 `standing_level`。它与 `target_level` 独立：站在入口上不等于触发了场景切换。

`--instant-solve` 模式会在 `V` 前等待已排队的操作与角色移动完成；站在可进入入口上按 `V` 才计数，只路过或在锁定入口按键都不算。随后按入口的 AND/OR 前置条件更新后续入口与冰障，`visited` 按经过顺序记录首次通关。

```sh
sinking-star replay overworld --instant-solve -i solutions/overworld.txt
sinking-star render overworld --instant-solve -i solutions/overworld.txt -o overworld.mp4
```

普通版可加 `--inspect-radius 8` 查看附近机关的实时状态，[路线说明](routes/overworld/README.md) 给出了利用这些状态筛选、扩展候选输入的示例。

仓库自带的[路线](solutions/overworld.txt) 经过 [95 个不同入口](solutions/overworld.json)，包含三次往返小世界已发现传送点；[支线说明](routes/overworld/README.md) 给出睡莲、移动光束、双镜分身与传送点的例子。

## 导出视频和截图

程序会尝试探测常见的 Steam 安装、`/Applications` 下的 CrossOver 与系统目录中的 D3DMetal；未探测到或需指定其他安装时，用 `GAME_ROOT` 和 `D3DMETAL_PATH`。

```sh
sinking-star render heroes1_1 -i solutions/heroes1_1.txt --resolution 1600x900 --fps 60 -o heroes1_1.mp4
sinking-star render heroes1_1 -i solutions/heroes1_1.txt --resolution 960x540 -o final.png
sinking-star render --inputs solutions/heroes1_1.txt solutions/heroes2_2b.txt --output-dir videos --jobs 2 --resolution 960x540 --fps 20
```

导出视频或截图必须用 `-o PATH` 指定输出。视频默认 1280×720、30 fps 的 HEVC（VideoToolbox 恒定质量，`--quality 0..100`）；`--fps` 只用于视频，必须是 60 的正因数，不改变游戏 60 帧/秒的步进；`--resolution` 宽高须在 16 到 8192 之间，HEVC 还要求偶数。输出名用 `.png`/`.jpg` 时只导出最后一帧。

首次渲染会初始化 GPU 并准备场景资源，之后复用与版本匹配的 GPU 辅助程序以复用 Metal 编译结果。普通版设置 `SINKING_FRAME_PROFILE=1` 可在输出 JSON 的 `profile` 中看到各阶段耗时。

批量导出用 `--inputs FILE...` 与 `--output-dir DIR`，文件名去掉扩展名后必须是关卡 ID（`heroes1_1.txt` → `heroes1_1.mp4`）。可通过 `VERIFIED_JSON` 指定判胜帧索引。

## 验证解法

```sh
cargo test --locked --release --test solutions
cargo test --locked --release --test solutions -- --exact heroes1_1
```

批测默认 4 个并发进程，可用 `STAR_CORPUS_WORKERS` 设为 1–16；报告默认写到 `target/original-rust-key-verification.json`，可用 `STAR_CORPUS_REPORT` 指定路径。
