# 327 Player 静态分层缓存与失效边界收敛 v2

状态：正式方案草案，承接 327 A 阶段分解和用户确认的 A+B 组合；尚未生成施工文档。

## 1. 问题与目标

当前 Project Player 将生成 Host、Project Runtime Module、Runtime Glue、runtime_cli、runtime_player_winit 和 Engine Runtime 放入同一 Cargo 静态链接构建链。真实报告显示 Host 构建为主要耗时；项目逻辑变化也会重新触碰最终 Host 链。

目标是在保持最终单一静态 `Game.exe`、Rust Project Runtime、RuntimePackage、结构化 artifact/delivery identity 和显式 verify 合同的前提下，把稳定引擎部分与项目变化部分分层缓存，缩小重编译边界。

## 2. 方案定义：A+B 组合

### A：保留并深化 Cargo 增量

- 继续使用兼容键隔离的 `CompileWorkspace`。
- Project、SDK、manifest/lock、toolchain、target、feature 和环境分别参与正确的失效判断。
- 普通开发 build 跳过测试目标编译；显式测试/verify 继续编译并报告测试诊断。
- 最终 artifact identity 仍包含完整 Project Runtime、Runtime Glue、SDK 和 ABI 输入。

### B：静态库分层

将当前单一 Host Cargo 目标拆成两个内部静态层，并在最后生成同一个 Player：

```text
Stable Engine Layer
  = engine_runtime + engine_input + runtime_player_winit + runtime_cli 的稳定部分

Project Layer
  = Project Runtime Module + Generated Runtime Glue + 项目绑定入口

Stable Engine Layer + Project Layer
  → 最终单一 ai_project_runtime_player / Game.exe
```

这里的“层”是 Compiler 内部的 Cargo/静态产物缓存边界，不是新的 Runtime Module、Provider、Engine Tool 或用户可见 ABI。运行时仍加载一个最终 Player，仍由 RuntimePackage 提供运行输入。

## 3. 关键约束

- 不引入 DLL、WASM、脚本 VM、动态注册表或第二个 Player。
- 不把具体项目玩法放入 Engine Core。
- 不让稳定层缓存 Project Runtime 类型、AOT digest 或项目资源。
- 任何改变 Engine Runtime API、Cargo feature、toolchain、target、依赖 lock 或 ABI 的输入，必须使稳定层失效。
- 任何改变 Project Module、Runtime Glue、项目 manifest 或 Project Game SDK 绑定的输入，只失效 Project Layer 和最终链接产物。
- 资源、Scene、Prefab、AUI 变化如果不改变 Project Runtime 编译输入，不得触发 Rust Project Layer 编译。
- 最终 `Game.exe` 必须重新生成并重新做 descriptor、hash、delivery identity 和显式 verify；不能直接复用旧最终 Player。

## 4. 失效矩阵

| 输入变化 | Stable Engine Layer | Project Layer | 最终链接 | artifact identity |
| --- | --- | --- | --- | --- |
| 资源/Scene/Prefab/AUI，仅 RuntimePackage 输入变化 | 复用 | 复用 | 视交付内容更新 | 变化 |
| Project Runtime `.rs` | 复用 | 重编 | 重链 | 变化 |
| Generated Runtime Glue | 复用 | 重编 | 重链 | 变化 |
| Project `Cargo.toml`/lock/feature | 复用或失效，按依赖判断 | 重编 | 重链 | 变化 |
| Engine Runtime/SDK 源码或 API | 失效 | 重编 | 重链 | 变化 |
| toolchain、target、real-window feature、链接环境 | 失效 | 重编 | 重链 | 变化 |
| 仅测试源 | 复用 | 不影响普通 build；显式测试单独编译 | 不变 | 不变 |

## 5. 实现边界

第一阶段只允许改 `project_player_artifact` owner、其 incremental cache helper、生成 Host manifest/源码和报告字段。不得修改公共 Engine Tool surface、RuntimePackage schema 或项目游戏逻辑。

首先用 Cargo metadata/依赖图验证 `runtime_cli` 与 `runtime_player_winit` 中哪些类型被 Project Runtime 直接耦合。若无法在不改变公开 ABI 的情况下形成静态层，B 必须停止并保留 A-only 结果。

## 6. 验证合同

必须分别验证：

1. 冷构建仍能生成单一正确 Player，并通过 descriptor/显式 verify。
2. 无修改重复构建复用两层，Cargo fresh/rebuilt 为零或只包含必要的最终链接。
3. 只改 Project Runtime 时 Stable Engine Layer 不重建。
4. 只改资源/Scene/AUI 时两层均复用，仅 RuntimePackage/交付更新。
5. Engine SDK、toolchain、target、feature 或 ABI 变化会使 Stable Engine Layer 失效。
6. 普通 build 不编译测试目标，显式测试/verify 仍保留测试诊断。
7. complex shooter 的 build、delivery verify、playtest/observe 仍通过，artifact、revision、delivery identity 正确变化。

## 7. 失败与回退

如果静态层需要引入动态 ABI、项目类型泄漏到稳定层、链接产物无法复用，或最终 Player 行为/identity 不稳定，停止 B，不引入替代架构；保留 A 的测试编译解耦和 Cargo workspace 增量结果，并将未完成原因写入阶段记录。

## 8. 方案自审

- 必要性：由 42 秒 Host 编译和 22 秒测试编译报告证明；不是为小游戏引入新运行时。
- 最小改动：先复用既有 Cargo/静态链接 owner，仅改变内部编译缓存边界。
- 明确排除：DLL、脚本 VM、新工具、RuntimePackage schema、玩法迁移、完整发布矩阵。
- 证据经济性：先依赖图和 owner 报告，再四类变更矩阵，最后一次完整 complex shooter 组合；不重复运行相同输入。
- 风险：Cargo 静态链接可能无法形成独立稳定层；已定义 fail-closed 终止条件。
