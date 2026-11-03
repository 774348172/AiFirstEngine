# 327 Player 静态分层缓存与失效边界收敛 v2 施工文档

状态：已激活。用户授权直接施工 Gate 1-4。

Gate 1 终态：失败并触发停止条件。`cargo metadata --manifest-path rust/Cargo.toml --format-version 1 --locked` 已完成，证据为 `.artifacts/327-cargo-metadata.json`。依赖图显示 `engine_runtime -> project_runtime_abi/project_runtime_sdk`，`runtime_player_winit -> project_game_sdk`，`runtime_cli -> runtime_player_winit`；稳定Engine层反向携带Project SDK，无法在当前静态ABI下独立缓存。Gate 2/3/4未执行，避免引入动态ABI或错误缓存。

## 1. 目标与范围

保留 Cargo 增量与测试编译解耦，验证并实施稳定 Engine 静态层、Project Runtime/Glue 静态层和最终链接层的最小分层。保持 RuntimePackage、公共 Engine Tool、Rust Project Runtime 和最终单一 `Game.exe` 不变。

排除 DLL、动态 ABI、脚本 VM、新工具、RuntimePackage schema、玩法代码、发布和安装替换。

## 2. 必要性与验证经济性

327 A 真实报告显示 Host 编译 42.1 秒、测试编译 22.3 秒。当前 Host 静态链接 Project Module，独立 Host 缓存不能直接复用。先用依赖图证明边界，再改 owner；复用 324 Cargo workspace 证据和 326 build/verify 分离证据。不以小 fixture 冒充完整项目性能。

## 3. Gate 1：依赖图和静态边界

使用 `cargo metadata --manifest-path rust/Cargo.toml --format-version 1 --locked` 与 Host 生成代码，列出 Stable Engine Layer、Project Layer、最终链接直接/间接依赖，确认 Project Runtime 类型不进入稳定层。若需要动态 ABI或出现项目类型泄漏，Gate 失败并停止 B。

## 4. Gate 2：静态层最小实现

仅修改 `project_player_artifact` 及 `incremental` owner。Stable Layer 不含项目源、Glue、AOT digest；Project Layer 只含项目输入；最终 Host 仍静态链接生成单一 Player。运行 owner 测试、`cargo fmt --check` 和 `cargo test -p project_authoring_execution --lib --locked`。出现缓存污染、链接失败或 descriptor 不一致立即回退。

## 5. Gate 3：失效矩阵

分别验证无修改、仅 Project Runtime、仅资源/Scene/AUI、Engine SDK/toolchain/feature 变化。记录两层 cacheStatus、Cargo fresh/rebuilt、artifact hash 和 delivery identity；资源变化不得触发 Rust 编译。

## 6. Gate 4：真实项目终验

对 `samples/complex_shooter_project` 执行冷构建、逻辑修改构建、资源修改构建和显式 `engine_delivery_verify`，保留阶段报告并与 327 A 基线对照。失败时保留首因，不伪称全局性能提升。

## 7. 归档规则

Gate 全部通过后更新 49、54、施工 README 和阶段记录，并将文档移到 `已完成/`。若 Gate 2 触发停止条件，记录 A-only 完成和 B 未完成原因，不归档为全部完成。

## 8. 施工自审

范围与 327 v2 方案一致；无新增工具、ABI 或玩法；每个 Gate 有明确失败合同和最小证据；依赖图先行；复杂验证只在边界证明后执行。
