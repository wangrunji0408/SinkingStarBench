# 适配新的 Steam 游戏版本

先用新安装目录构建。只需要本目录和 Windows 游戏文件：

```sh
GAME_ROOT="/path/to/steamapps/common/Order of the Sinking Star" cargo build --locked --release
```

构建脚本读取 AMD64 PE 的映像基址、节表、导入表和异常表。它在 Cargo 的构建输出目录写入 `abi-report.json`；未知映像会在链接前停止，并打印该报告的完整路径。对照两版报告的 `mapped_sha256`、`anchors` 和 `unresolved`。后者列出无匹配、匹配不唯一或目标越界的具体名字；即使有失败，其余匹配仍会保留。文件 SHA-256 单独记录，已构建二进制可以用 `sinking-star --version` 查询。

| 更新涉及的内容 | 定位方法与复核位置 |
| --- | --- |
| 游戏函数、分段调用的结束位置 | [routines.txt](build_support/routines.txt)、[stage_ends.txt](build_support/stage_ends.txt)。每行是符号名和唯一的指令片段；`?` 忽略链接时会变化的操作数。`@function` 表示通过 PE 异常表从片段定位函数入口。结束位置所覆盖的指令由 [stages.rs](build_support/stages.rs) 重定位。 |
| 时钟、输入、目录和阶段变量 | [anchors.rs](build_support/anchors.rs)、[globals.txt](build_support/globals.txt)。从 RIP 相对指令算出目标并校验所在节；报告中地址为 RVA。 |
| 实体、网格、关卡状态和资源成员 | [reflection.rs](build_support/reflection.rs) 的 `HOST_FIELDS` 按类型名、成员路径和宽度解析，生成 `layout.rs`。如果成员改名或宽度改变，先复核读取语义。 |
| 原版未反射的部分状态 | [offsets.txt](build_support/offsets.txt) 从指令操作数提取。它们在报告中的值是**结构内偏移**，不是 RVA。构建时用所属类型的反射大小检查边界。 |
| 场景、图和网格绑定 | 从包目录枚举资源，角色图清单由场景里的角色网格和怪物类型生成。网格字段编号从序列化反射表推导；启动时的网格绑定从原版 mesh retain 调用、全局写入和字符串实参提取。不用维护资源总数。 |
| 仍需人工确认的行为 | [profile.rs](build_support/profile.rs) 的宿主服务替换、[platform.s](platform.s) 的调用约定，以及 [init.rs](src/init.rs) 和 [main.rs](src/main.rs) 中未暴露的私有布局、阶段条件及输入队列协议。指令位置相同不保证行为相同。 |

若符号失配，应在新版 EXE 中确认同一段逻辑，选择能区分该位置的指令上下文；遇到多个匹配不能取第一个。复核剩余私有接口并登记新映像后，用新安装文件重新构建，执行 `cargo test --locked --release --test solutions`。在完整求解仓库中，还可用 `cargo test --locked --release --test reference` 比较胜利帧与逐操作状态摘要，再用 `STAR_CORPUS_REPORT=REPORT.json STAR_CORPUS_WORKERS=8 cargo test --locked --release --test corpus -- --ignored --nocapture` 对比每关结果。游戏更新确实改变规则时，应记录差异原因，再决定是否更新基线。

当前已经验证 r68307。如果只改变 EXE 映射节之外的签名或附加数据，不必新增游戏版本；代码或映射数据改变时仍需复核。
