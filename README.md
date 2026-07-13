# 《沉星之序》CLI

构建：

```sh
cargo build --release
```

`levels/` 中每关一个地图文件。构建时会自动扫描这些 `.txt` 文件并嵌入可执行文件，成品运行时不需要携带关卡文件。

命令：

```sh
# 交互游玩（不写关卡名时会先选择关卡）
cargo run -- play
cargo run -- play 1-1
cargo run -- play 1-1 --save solutions/1-1.txt

# 一次性执行动作，坐标均为从 0 开始的 (x, y)
cargo run -- run 1-1 'WASDD'
cargo run -- run 1-1 'WASDD' --json

# 展示、列举关卡
cargo run -- show 2-3
cargo run -- list
cargo run -- list --json
```

输入支持 `WASD` 或 `↑↓←→`，`Z` 撤销，`R` 重置，`X` 触发机关（当前无效果），`C` 切换角色。一次性输入中的空白和逗号会被忽略。

游戏规则按 `levels/` 中的地图实现，空地用空格表示：战士推动相连的一串物体；盗贼移动时只拉动身后紧邻的一个物体；巫师与移动方向上、墙或关闭的门之前的第一个物体隔空交换位置。能力可作用于石头或其他角色。地图外视为墙；至少有一个开关且全部被石头或角色压住时门开启；关门时，门格上的石头被碾碎，角色则被卡住、无法主动移动。角色位置集合与目标集合相同时通关。

`show`、`play` 和普通 `run` 会显示地图外隐含的一圈墙；`run --json` 保留原始地图尺寸，使 `grid` 索引与从 0 开始的状态坐标一致。
