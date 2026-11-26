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

标准 Steam 安装和默认的 Steam CrossOver bottle 可自动探测，其他安装位置用 `GAME_ROOT` 指定。可将生成的 `sinking-star` 复制到其他目录运行。以下命令假定它已加入 `PATH`；也可以直接使用它的路径。

命令行格式：`sinking-star <command> ...`。

## 挑战版

在本目录运行：

```sh
cargo build --locked --profile challenge --bin sinking-star-challenge
cp target/x86_64-apple-darwin/challenge/sinking-star-challenge .
```

挑战版是独立的二进制目标。以上构建会剥离调试符号。挑战规则和开始步骤见顶层 `--help`，按键、密钥和计分细则见 `replay --help` 和 `check --help`。给挑战者一份二进制和空工作目录即可；运行时不需要原版游戏目录。运行平台为 macOS，Apple Silicon 需要 Rosetta 2。

挑战版从 `intro` 开始，无需密钥。成功进入大地图时返回 `won: true`、`target_level: "overworld"` 和 `target_level_key`。回放 `overworld` 时提供该密钥。地图回放会把在已解锁入口按 `V` 视为立即通关，以便继续探索；`playable_levels` 按首次进入的顺序列出本次路线中按 `V` 进入过的关卡，`level_keys` 给出这些关卡各自的密钥。单关回放只需提供目标关卡密钥。同一份二进制的密钥可重复使用。

```sh
./sinking-star-challenge replay intro -i solutions/intro.txt
./sinking-star-challenge replay overworld --key "$OVERWORLD_KEY" -i solutions/overworld.txt
./sinking-star-challenge replay heroes1_1 --key "$HEROES1_1_KEY" -i solutions/heroes1_1.txt
```

也可以把 `# 密钥` 写在路线文件首行，下一行写原来的按键串（末尾仍不加换行），这样回放时可以省略 `--key`。两处都提供时，密钥必须一致。Intro 的路线文件不需要密钥行。

用 `./sinking-star-challenge check solutions` 对目录计分。计分会跳过可选的密钥行，直接验证提交的路线。输出一行 JSON，包含 `solved_count`、`total_frames` 和按关卡 ID 列出的 `levels`。每个提交的 `.txt` 都有 `won`，并附通关帧数 `frames` 或未计分原因 `reason`。Intro 成功进入大地图计一关；其他关卡必须在地图路线中进入且本关解法获胜才计分。`overworld.txt` 本身不计分（`map_only`）。

## 回放

```sh
sinking-star list-levels  # 每行一个关卡名
sinking-star --version
sinking-star replay heroes1_1 -i replay/solutions/heroes1_1.txt --trace trace.bin
echo -n 'RT30X' | sinking-star replay heroes1_1
```

从仓库根目录运行以上示例。回放和单关渲染都用 `-i ROUTE`（或 `--input ROUTE`）指定路线文件。省略时，程序从标准输入读到 EOF。路线是连续的 ASCII 按键串，不含空白或结尾换行。方向键 `U/D/L/R` 按关卡初始镜头解释；`X` 执行动作。连续按键可排队，读到 `Tn` 或路线末尾时，回放会先等队列清空、角色移动完成。`Tn` 随后再推进 n 帧（60 帧/秒），`T0` 只等待操作完成。

回放输出一行 JSON：`frames` 是实际推进的帧数，包含自动收尾和显式等待，`won` 表示是否赢得谜题。触发具名场景切换时，`won` 为 `false`，`target_level` 是游戏请求进入的场景（如 `intro` 出口的 `overworld`）；其余情况为 `null`。回放在切换请求处停止，不加载下一个场景。`replay_ns` 是回放耗时。可用 `--trace PATH` 保存逐操作状态，同时生成 `PATH.json` 说明文件格式。完整按键表见 `sinking-star replay --help`。

挑战版的 `replay <LEVEL> -i <ROUTE>` 默认输出 `state`；普通版可加 `--state` 输出快照。`state` 包含末帧的场景、家族、待执行输入、进行中的移动、当前实体及其可读字段。每个实体的完整字段由 `state.entity_defaults[实体.type]` 与 `实体.fields` 合并得到，后者优先。空路线可查看初始状态；触发切换时记录离开前的关卡，并在 `state.target_level` 指出目标。快照用于观察模拟状态，不包含进程内存或渲染资源，不能用来恢复运行。

挑战版的 `state.terrain` 还包含末帧的地形网格：

- `origin` 和 `size` 按 `[x, y, z]` 排列，使用与实体位置相同的世界坐标。 `input_directions` 给出各方向键的 `[dx, dy]`。
- `solid_runs` 的每条 `[z, y, x0, x1]` 表示该层一段连续的实心格（含两端）。地面是角色脚下的实心格。 `enabled_column_runs` 以 `[y, x0, x1]` 表示引擎启用的列；只有启用列也不能判定可走。
- `ocean_maps` 记录每块水域的位置、朝向、类型和启用状态。 `local_runs` 是原版水域标记位的连续格；能直接对齐到世界网格时也输出 `world_runs`。两者都用 `[y, x0, x1]`。普通水面看 `kind: 0` 且 `active: true` 的记录，水面高度取 `position[2]`。

判断能否走到某格还要结合高度、门、栅栏、石头和角色能力；没有实心格也可能是水或落空。

### 大地图路线

普通 `replay overworld` 在终点位于入口时输出 `standing_level`（关卡名），不在入口上时省略该字段。它与 `target_level` 独立：站在入口上并不等于触发了场景切换。

普通版的 `--instant-solve` 模式会在 `V` 前等待已排队的操作和角色移动完成；角色站在可进入的入口上时将该关立即记为通关。只路过、站着不按键或在锁定入口按键均不计数。随后按入口的 AND/OR 前置条件更新后续入口和冰障；只记录首次通关，列表按经过顺序输出到 `visited`。这个模式不加载子关卡、不修改游戏存档，也不替代塔、剧情 rank 或金库的推进机制。

从仓库根目录回放并导出同一路线：

```sh
sinking-star replay overworld --instant-solve -i replay/solutions/overworld.txt
sinking-star render overworld --instant-solve -i replay/solutions/overworld.txt -o overworld.mp4
```

普通版可在上述 replay 命令加上 `--inspect-radius 8`，查看附近机关的实时状态。[四方路线说明](routes/overworld/README.md)提供了利用这些状态筛选、扩展候选输入的示例。

当前[路线](solutions/overworld.txt)从 `heroes1_1` 到 `mirror_17`，经过 [95 个不同入口](solutions/overworld.json)，其中三次往返经过小世界的已发现传送点。[支线说明](routes/overworld/README.md)给出了睡莲、移动光束、双镜分身和传送点的例子。

## 导出视频和截图

程序会尝试探测常见的 Steam 安装，以及 `/Applications/CrossOver.app`、`~/Applications/CrossOver.app` 和 Apple 系统目录中的 D3DMetal。若未探测到，或需要指定其他安装，可设置 `GAME_ROOT` 和 `D3DMETAL_PATH`。在仓库根目录运行：

```sh
export D3DMETAL_PATH="/path/to/D3DMetal.framework"
sinking-star render heroes1_1 -i replay/solutions/heroes1_1.txt --resolution 1600x900 --fps 60 -o heroes1_1.mp4
sinking-star render heroes1_1 -i replay/solutions/heroes1_1.txt --resolution 960x540 -o final.png
sinking-star render --inputs replay/solutions/heroes1_1.txt replay/solutions/heroes2_2b.txt --output-dir videos --jobs 2 --resolution 960x540 --fps 20
sinking-star render heroes1_1 -i replay/solutions/heroes1_1.txt --render-stream
```

单关导出视频或截图必须用 `-o PATH`（或 `--output PATH`）指定输出文件。视频默认使用 1280×720、30 fps 的 HEVC，采用 VideoToolbox 恒定质量 92（`--quality 0..100`，越大越清晰），由编码器按画面复杂度分配码率。不需要按分辨率或帧率估算。该尺度不是 x265 CRF。视频附带同名 JSON。`--fps` 只用于视频，必须是 60 的正因数（1、2、3、4、5、6、10、12、15、20、30、60），不改变游戏的 60 帧/秒步进。`--resolution` 用于视频和截图，宽高均须在 16 到 8192 之间，HEVC 视频还要求宽高都是偶数，系统编码器可能有更小的上限。

首次渲染需要初始化 GPU 并准备场景资源。程序会在当前用户的临时目录复用与此版本匹配的 GPU 辅助程序，使后续命令也能复用 Metal 编译结果；场景和截图仍逐次生成。普通版设置 `SINKING_FRAME_PROFILE=1` 后，截图和视频的输出 JSON 会在 `profile.seconds` 中列出主进程各阶段耗时，在 `profile.worker_mean_ms` 中列出渲染子进程每帧的平均耗时。子进程耗时与主进程等待时间有重叠，不要直接相加。见 [overworld 渲染实测](../docs/overworld-render-performance.md)。

大地图截图保留完整场景和原版相机。GPU 在首次提交使用某个图形管线的绘制命令时才编译它。普通网格材质的纹理只在绘制实际请求时读取和上传；贴花、天空、粒子及其共用纹理提前加载，以保留阴影、反射和其他间接采样。

通关后，渲染继续更新场景，直到没有阻塞下一条输入的动作，或达到 180 个游戏帧的上限。输出名使用 `.png`、`.jpg` 或 `.jpeg` 时，只导出最后一帧。结果中的 `ticks` 是判胜或路线结束的游戏帧，`render_ticks` 是实际渲染到的游戏帧；`settled` 表示通关后是否等到动作完成，未通关时为 `null`。`--render-stream` 和 `--render-keys` 输出 JSON，无需图形环境。

批量导出用 `--inputs FILE...` 指定一个或多个解法文件，并用 `--output-dir DIR` 指定输出目录。文件名去掉扩展名后必须是关卡 ID，例如 `heroes1_1.txt` 会生成 `heroes1_1.mp4`。可通过环境变量 `VERIFIED_JSON` 指定判胜帧索引。更多选项见 `sinking-star render --help`。

## 检查动画位移

回放坐标决定角色在地面的行走位置。渲染器在混合动画前逐帧清除专用轨迹根骨骼 `rig_TrajectorySHJnt` 的水平平移，包括整段恒定的非零偏移；高度、转身和身体动作保留。更新动画或修改姿态解码后，在本目录运行：

```sh
cargo test --locked --release --bin sinking-star audit_replay_root_motion -- --nocapture
```

该测试需要游戏资源，会遍历动画映射中的所有候选片段，核对每帧每个关节。若新骨架的其他根关节出现至少 1 厘米的水平偏移，测试会报告片段和关节名，供判断它表示角色行进还是身体动作。另可在仓库根目录运行 `PYTHONPATH=renderer:renderer/crossover:tools python3 renderer/test-castaway-motion.py`，检查女王和漂流者切入跑步动画时的实际骨盆位置。

## 验证解法

在本目录运行 Cargo 测试；`--exact` 可只跑一关：

```sh
cargo test --locked --release --test solutions
cargo test --locked --release --test solutions -- --exact heroes1_1
```

在完整仓库中，从本目录运行以下测试来核对参考轨迹或批量生成报告：

```sh
cargo test --locked --release --test reference
cargo test --locked --release --test corpus -- --ignored --nocapture
```

批测默认使用 4 个并发进程，可用 `STAR_CORPUS_WORKERS` 设置为 1 到 16。报告默认写到 `target/original-rust-key-verification.json` 及同名 `.jsonl`，可用 `STAR_CORPUS_REPORT` 指定 JSON 路径。测试成功表示报告已生成，逐关结果见报告。
