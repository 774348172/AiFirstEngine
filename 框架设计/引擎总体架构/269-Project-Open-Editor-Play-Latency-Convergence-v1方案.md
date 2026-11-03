# 269 Project Open + Editor Play Latency Convergence v1 方案

状态：已确认，施工中。

## 1. 问题与证据

真实 Tower Defense 捕获报告证明两条独立慢路径：

- Project Open `315077ms`，其中 `build_composition_release=311405ms`；
- Editor Play `220418ms`，其中 `fingerprint_sources=217450ms`。

Project Open 的主因是 ProjectRust specialized Editor artifact 以完整 artifact seal 查缓存，miss 后在
run-owned staging `target` 从零执行 release build。Editor Play 的主因是 preview fingerprint 自行递归
扫描 `RuntimeModule`，把 Cargo `target` 当源码并使用整文件 `fs::read`；cache lookup 位于该扫描之后。
Editor Play 随后还无条件准备独立 Project Player，尽管 in-process GameView 已使用 linked RuntimeModule。

## 2. 选定方案

按用户确认顺序实施：

1. Preview fingerprint 复用 268 的统一 project source policy、Cargo tree `target` 排除和 64 KiB
   流式摘要；缓存命中前不得扫描 generated/build output。
2. Native Editor 将 Editor Play preparation 放入唯一 owned worker；UI thread 只 pump progress/result，
   重复 Play exactly-once 拒绝，关闭时 cancel + join。prepared result commit 前复核 project identity。
3. `EditorGameView` in-process Play 不构建独立 Project Player artifact。Player artifact 只属于独立
   Player / Export consumer；RuntimePackage assembly/load 和 specialized composition 校验保持不变。
4. specialized Editor composition 保持 exact artifact seal，但新增内部 compilation compatibility identity；
   Cargo target 使用 application-owned、toolchain/target/profile/dependency-compatible 的共享编译缓存。
   Cargo 自身 fingerprint 决定增量重编译，最终 executable 仍按 exact identity 查询 descriptor、哈希并封存。

## 3. 边界

- 不修改 Tower Defense 玩法、资产、RuntimeModule 或 manifest；
- 不引入 Rust dylib、任意 native module loading 或降低 trust；
- 不让 preview/source digest 接受 build output；
- 不替换 production/安装态二进制，不修改真实用户配置，不运行 Local CI；
- 本轮验证以 owner/affected consumer、run-owned fixture 和真实 Tower read-only timing 为准。

## 4. 验收

- 含多 GiB `RuntimeModule/target` 的 fixture 中，fingerprint 与 target 内容无关且不读取 target；
- Play preparation 不阻塞 UI，重复点击只保留一个 operation，shutdown 无遗留 worker；
- in-process GameView report 明确 `player_artifact_status=not_required_in_process`，Export 路径仍准备 Player；
- composition exact seal 不放宽，连续 exact-identity build 命中 artifact cache；seal 变化但 compatibility
  identity 相同时复用同一受控 Cargo target root；
- `editor_core`、`editor_ui_model`、`editor_ui_renderer`、`editor_window_winit` 受影响回归通过。
