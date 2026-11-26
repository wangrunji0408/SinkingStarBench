这四条输入都从新开的大地图出发，无需存档、预先解锁或移动角色。每次进入都显式按 `V`。例如：

```sh
sinking-star replay overworld --instant-solve -i replay/routes/overworld/south.txt
```

| 方向 | 独立回放首次进入的关卡数 | 终点 |
| --- | ---: | --- |
| [南](south.txt) | 4 | `skipping_shores_v3` |
| [西](west.txt) | 6 | `torcoban_2` |
| [东](east.txt) | 15 | `mirror_17` |
| [北](north.txt) | 70 | `heroes3_8_v3_alt3` |

[东](east.moves)、[西](west.moves)的搜索输入没有 `T`。 [南](south.moves)只保留两个与睡莲相位有关的时间点。在仓库根目录运行：

```sh
sinking-star replay overworld --instant-solve -i replay/routes/overworld/east.moves
```

连续方向键送进原版队列。回放在 `V`、`T` 或路线末尾等待队列清空和角色移动完成，replay 和 render 可直接使用同一串按键。

- 西：推动已通电的橙光发射器到 (40,74)，从 (40,66) 跨墙进入第二关。解冻后把发射器继续向西移到 (38,74)，从 (38,65) 向西跨墙，打开通向 `cyan_overlap_intro_longer` 的走廊。
- 南：从北向南推 (82,30) 的跳石，踏着沉石上东岸；沿岸走到 `skipping_shores_v3`。回程在 (79,10) 按 `V`，从小世界返回出生点，无需再等船。
- 东：从 (118,57) 使用两面相向的魔镜，两边的角色分别落到 (115,54) 和 (121,54)。右边的分身进入 `mirror_13`，左边再进入刚解锁的 `mirror_14`，回到投射点后两人合并。

传送点上的 `V` 是原版交互。可在 `map_state` 查看附近传送点的 `discovered` 和 `target_id`；小世界中的人物也需要实际行走。`render --render-keys` 的 `visit_events` 只记录首次通关，含模拟帧和从零开始的操作索引，可与 `visited` 按顺序对应。

使用 `replay overworld --instant-solve --inspect-radius 8` 可以在输入结束时查看人物位置、技能、输入队列和附近机关。比如东线两条路径都能走到 (100,69)，只有镜子位于 (100,72) 且角度为 90° 的候选能通过下一次投射上岛。

从仓库根目录复现这一段搜索：

```sh
python3 tools/search_overworld_checkpoints.py replay/routes/overworld/east-mirror-checkpoints.json --output /tmp/east-six.txt
sinking-star replay overworld --instant-solve -i /tmp/east-six.txt
```

也可运行[南线睡莲](south-lily-checkpoints.json)、[西线光束](west-beam-checkpoints.json)或[北线压力板](north-switch-checkpoints.json)计划。北线已验证德鲁伊变树为石，再用吟游诗人将石头移上压力板，门实际打开；离开后能否保持开门仍待解决。南线登船使用 `explore`：从候选动作逐步展开，按人物与睡莲距目标的距离保留分支，再用两者的实际位置确认登船。西线的[旧机关检查点](west-frontier.txt)停在第二个宝石插槽；新的完整西线还会移动发射器。

带有 `choices` 的检查点枚举 `choices` 中各组输入的组合，交给 replay 执行，再按 `goal` 筛选。多个候选都满足条件时，优先保留模拟帧更少、同帧数时等待指令更少的序列；`--beam-width` 控制保留数。加上 `--on-stall /tmp/ow-stall`，检查点失败时会保存最接近目标的候选输入和完整状态（`route.txt` 和 `state.json`），便于检查哪个机关仍未满足，再调整动作。这是有界搜索，不保证全局最短。
