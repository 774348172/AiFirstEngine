# 249-当前可自动化施工文档：Native Editor Deterministic Project Launcher State Isolation v1

> 状态：已完成施工并归档，已完成激活前复核、自审、全部 Gate、完成记录与入口同步。
> 来源方案：`249-Native-Editor-Deterministic-Project-Launcher-State-Isolation-v1方案.md`。
> 代码基线：`9f5adc73ca02a5e3fed3f38c62bb9a420e6bd5de`。
> 范围说明：用户已明确把 P0-0.5 Run 01 的证据驱动阻断置于 247 P0-2 之前；本施工不修改 240 队列，也不改写 v1/v2 证据。

## 1. 施工目标

完成以下一条可测试链路：

```text
原子启动配置
  -> 显式 project dialog initial directory
  -> Open/Create/RuntimePackage 三个请求一致传递
  -> native backend 先验证再 set_directory
  -> Windows adapter 保留 HRESULT，只有明确取消才返回 Cancelled
  -> Unavailable 进入 Rejected feedback 与可见错误横幅
  -> 普通 recent store 保持
  -> 隔离 recent store 只在 own_run_root
  -> editor_host 单一隔离参数
```

完成后才能生成 P0-0.5 v3 协议并重跑 Run 01。

## 2. 施工范围

代码：

```text
rust/crates/editor_window_winit/src/dialog.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/lib.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/editor_window_winit/src/tests/native_real_window.rs
rust/crates/editor_window_winit/Cargo.toml
rust/crates/editor_host/src/main.rs
rust/crates/editor_host/tests/cli_exit.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/tests.rs
rust/Cargo.lock

Gate 4 阻断恢复仅允许：

rust/crates/architecture_review/src/lib.rs
rust/crates/architecture_review/tests/loopback_provider.rs
rust/crates/quality_gate/src/architecture_review.rs
rust/crates/quality_gate/tests/architecture_gate_matrix.rs（仅在需要真实 artifact matrix 时）
rust/crates/runtime_cli/tests/bounded_child_process.rs（仅允许稳定 Fast Gate 高负载下的既有 timeout 测试预算，不改 production process 实现）
```

必要时只为导出新类型调整相邻 `mod/use`。renderer 变更仅限消费既有 `EditorCommandFeedback` 并绘制最小横幅；不修改 gameplay、ECS、RuntimePackage 内容或 248 widget tree 结构。

文档：

```text
249 正式方案
49 / 54 / 00-文档地图 / 00-阅读顺序
247 临时 blocker 状态
施工文档/README.md
阶段完成记录/README.md
```

## 3. 环境与变更隔离

1. 主工作树已有用户文档修改和未跟踪审查文档，全部保留。
2. `rust/` 在基线检查时无既有修改；本施工只归属上述代码文件。
3. 定向测试使用独立 `CARGO_TARGET_DIR` 时必须记录绝对路径；最终权威回归使用 clean exact candidate。
4. v1/v2 目录只读。禁止删除、覆盖、补写或重新解释 `Invalidated` 终态。
5. 不读取 `I:\AIFirstTest\test2`；测试夹具只使用新建临时目录或本仓库已允许路径。

## 4. Gate 0：基线与红色反馈回路

进入条件：249 已激活，`施工文档/当前/` 中只有本文件。

任务：

1. 记录 `git status`、Rust toolchain、相关 crate baseline。
2. 先写调用链回归测试，让它要求三个请求都携带同一个显式绝对初始目录。
3. 修复前运行该测试并保留失败摘要，证明现有实现不能满足合同。
4. 测试不得真正打开 OS 对话框，也不得依赖机器 MRU。

命令：

```text
cargo test --locked -p editor_window_winit configured_initial_directory_reaches_all_project_folder_requests
```

预计成本：秒级。失败后只修当前测试或确认 seam；不得跳到完整 workspace。

## 5. Gate 1：请求合同与 Native backend

任务：

1. `ProjectFolderDialogRequest` 增加必填 `PathBuf`。
2. `NativeEditorApplication` 增加不破坏现有构造器的显式目录构造入口。
3. Open/Create/OpenRuntimePackage 三个请求全部传递该目录。
4. 抽取纯路径校验函数；空、相对、不存在、文件路径均失败关闭。
5. Windows real-window backend 直接使用 `IFileOpenDialog::SetFolder` 并逐阶段保留 HRESULT；非 Windows adapter 验证成功后调用 `rfd::set_directory`，两者都不提供 fallback。
6. `Cancelled` 与 `Unavailable` 分流；三个调用入口的 `Unavailable` 都产生 Rejected feedback，且 Self UI Renderer 在 launcher/workspace 绘制错误横幅。

命令：

```text
cargo test --locked -p editor_window_winit project_dialog_request_roundtrips_explicit_initial_directory
cargo test --locked -p editor_window_winit configured_initial_directory_reaches_all_project_folder_requests
cargo test --locked -p editor_window_winit --features real-window native_dialog_rejects_invalid_initial_directory_without_fallback
cargo test --locked -p editor_window_winit native_editor_application_distinguishes_cancelled_and_unavailable_project_dialog
cargo test --locked -p editor_ui_renderer renderer_draws_interaction_feedback_in_launcher_and_workspace
cargo test --locked -p editor_window_winit --features real-window dialog::tests::windows_dialog_show_starts_in_exact_initial_directory -- --ignored --exact --nocapture
```

预计成本：秒级到 1 分钟。任一失败先收敛 Gate 1 累计失败集。

## 6. Gate 2：原子启动配置与状态隔离

任务：

1. 新增 `RealNativeEditorLaunchOptions`，默认模式显式包含 picker 起点和默认 recent store。
2. 新增 `isolated_project_launch_root` 构造，派生 `<root>/picker-start` 与 `<root>/state/editor_recent_projects.json`。
3. 隔离构造拒绝相对、不存在、非目录 root 和无效 picker-start。
4. `RealNativeEditorApp` 与顶层 real-window runner 接收 options；旧入口代理默认值。
5. `editor_host --isolated-project-launch-root` 只与 `--real-window` 配合，拒绝缺值、重复值、相对路径和未知冲突参数。
6. 不启动窗口的解析测试证明这是一个原子 profile，且默认 `%APPDATA%` 不在隔离配置中。
7. root 的全部既存祖先组件、root、`picker-start`、`state` 的 junction/symlink/reparse point 和预存在 state 全部失败关闭；配置首次验证发生在创建 EditorSession/model 之前，并在真实 app 构造、recent-store load 前复验 private isolated profile。
8. 进程级 CLI 测试证明非法隔离参数写 stderr 并返回退出码 2。

命令：

```text
cargo test --locked -p editor_window_winit --features real-window isolated_launch_options_use_run_local_dialog_and_recent_state
cargo test --locked -p editor_host --features real-window isolated_project_launch_root
```

预计成本：1 分钟内。失败后不得用两个独立 CLI 参数规避原子性要求。

## 7. Gate 3：受影响域回归

进入条件：Gate 0-2 全绿且累计失败集全绿。

命令：

```text
cargo test --locked -p editor_window_winit --all-features
cargo test --locked -p editor_host --all-features
cargo clippy --locked -p editor_window_winit -p editor_host --all-targets --all-features -- -D warnings
```

同时执行格式检查与 suppression inventory。任何失败先在相同隔离环境精确复现；禁止立即反复跑完整 workspace。

## 8. Gate 4：最终权威回归

进入条件：候选代码和依赖冻结；除完成记录与入口同步外不再修改代码、manifest、lockfile、测试。

要求：

```text
fresh exact-candidate source
独立 CARGO_TARGET_DIR / artifact root
workspace default tests
workspace all-features tests
fmt + clippy -D warnings
高风险测试与累计失败集
source/target/artifact cleanup 证据
```

使用仓库当前权威 Local CI/quality runner；若 runner 合同不覆盖上述矩阵，先补预检而不是把定向测试冒充最终 Gate。最终权威 run 默认只执行一次；若重跑，完成记录必须说明预检遗漏。

### 8.1 Gate 4A：Provider finding evidence contract 阻断恢复

触发证据：首个冻结候选的 streaming Provider 请求成功，但返回 artifact 中三个 finding 的 `rule_ids=[]`、`evidence_digest` 为普通文本，两个 symbol 使用非 canonical 名称；现有 response schema 接受这些值，本地 verifier 必然拒绝。

任务：

1. Provider-facing finding 与最终 `ArchitectureFinding` 分开；Provider 不再生成 evidence digest。
2. 在 `architecture_review` 内建立单一规范化接口，解析 canonical context，验证 subject/path/rule，唯一映射 canonical symbol，并调用共享 digest 接口生成最终 finding。
3. `quality_gate` 提供并复用唯一 finding evidence digest 计算；verifier 必须重算，防止本地 artifact 被改写。
4. `rule_ids=[]` 只表示“未声明 committed dependency rule”，允许语义 finding 继续进入 high/critical disposition 流程；未知或伪造 rule ID 继续失败关闭。
5. 增加回归测试：semantic finding + empty rules 成功、唯一 symbol 规范化成功、未知 rule 拒绝、歧义/缺失 symbol 拒绝、digest 篡改拒绝。
6. 修改后重新运行 architecture review / quality gate 定向与受影响域测试、Fast Gate、fresh exact-commit Local CI；重新冻结后才能准备新的 subject=1 bundle。
7. 再次发送 `aui.rs` 前必须重新取得用户对 scope、最多 100000 total tokens、16384 output tokens 和费用的明确授权；Adapter 仍只能增加 `stream=true`，不得修改 prompt/context/schema/model。
8. 若完整 workspace 揭示既有 bounded child timeout 测试把 spawn 冷启动时间误算成 150 ms 输出预算，只允许提高测试 timeout 与总硬上限；不得借机修改 production process 语义。

命令：

```text
cargo test --locked -p architecture_review
cargo test --locked -p quality_gate architecture_review
cargo test --locked -p quality_gate --test architecture_gate_matrix
cargo clippy --locked -p architecture_review -p quality_gate --all-targets --all-features -- -D warnings
```

进入条件：Gate 3 已通过，首个候选的 Provider artifact 已按原样保留。失败后只收敛该接口和累计失败集，不先重跑完整 workspace。

当前执行证据（2026-07-13）：

```text
恢复候选：a09529898d08f6dfe8961dc35be7c72f4c51ea46
Fast Gate 重跑：12/12 passed，diagnostics=0，duration_ms=414307
exact-commit Local CI：local-a09529898d08-1783957634，12/12 passed
Local CI source/isolated：clean / clean
Local CI cleanup：removed
更新后 strict-schema streaming smoke：HTTP 200，outcome=complete，findings=0
首个正式 canonical streaming 请求：上游 HTTP 504，first_event 缺失，未生成 artifact/expectation
```

该 504 不回退 schema、不改 prompt/context/model、不手工重绑旧 artifact。由于不能确认失败请求的 Provider 计费结果，下一次约 94784-token canonical 重试必须取得新的追加预算授权。

最终执行证据（2026-07-14）：

```text
用户追加授权：最多 400000 total tokens / 96384 output tokens 及对应费用
正式 Provider 重试：第 1 次成功，不需要使用剩余 3 次额度
upstream_status=200
outcome=complete
event_count=1042
first_event_ms=16234
adapter_duration_ms=35891
adapter_semantic_digest_equal=true
findings=7（4 high / 3 medium）
local-maintainer dispositions=4 approved_exception（仅限 249 gate）
EngineStrict=12/12 passed
artifact_status=verified
architecture_diagnostics=0
diagnostics=0
EngineStrict duration_ms=501685
EngineStrict report_digest=sha256:5a3aac51dd1ec1833af0eb16628e13ca2d126bbed2609a743cee104bbe29da89
final_candidate_workspace=clean
```

4 个 high 与 3 个 medium 均为 base/head 未改动的既有 AUI finding。approved exception 不表示修复；完整清单与后续独立整改约束见阶段完成记录。

## 9. 完成与归档

全部 Gate 通过后：

1. 新建 `阶段完成记录/2026-07-13-Native-Editor-Deterministic-Project-Launcher-State-Isolation-v1/00-总览.md`。
2. 记录红色测试的原始失败、正确根因、各层命令/结果/耗时、权威 run ID、candidate commit、cleanup 和残余边界。
3. 将本文件移动到 `施工文档/已完成/`。
4. 同步 `49`、`54`、`247`、两个 README、文档地图与阅读顺序。
5. 下一步写入“冻结 P0-0.5 v3 协议并创建九个全新任务；严格串行启动 v3 Run 01”。

## 10. 自审

- 范围没有扩大到 Windows 安全沙箱或引擎内文件浏览器。
- 方案修正 125 的隐式状态缺口，不重复实现 Project Manager。
- 有正确调用链 seam，先红后绿，不以手工截图代替回归测试。
- 普通用户默认持久化不回归；隔离模式不读取默认用户状态。
- 三个 picker consumer 全覆盖，不只修最先暴露的 Create Project。
- Windows adapter 与 feedback 横幅是本次失败关闭合同的必要收敛，不扩展 248 widget-tree 架构。
- 分层验证、环境敏感点、exact candidate 和 cleanup 已写明。
- Gate 4A 只修复 246 已有质量工具的 Provider/artifact seam，不把 AUI 拆分或新规则系统塞进 249。
- Provider 内容、本地规范化和 verifier 共享同一 evidence digest 真相；不存在手工修 artifact 的旁路。
- v1/v2 证据、240 队列、248 UI 架构均不在修改范围。

自审结论：通过，可施工。
