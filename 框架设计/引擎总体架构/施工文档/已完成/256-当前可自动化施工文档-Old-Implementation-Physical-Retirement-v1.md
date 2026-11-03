# 256 当前施工文档：Old Implementation Physical Retirement v1

## 0. 文档状态

```text
正式方案：256-Project-Create-Tool-v1方案.md
文档状态：已自审、已完成、已归档
施工状态：R0-R3 completed
授权来源：2026-07-24 用户明确要求先物理删除旧 256 源码
施工范围：仅旧实现 retirement
新 project.create：本施工明确不实现
时间预算：最多 3 小时
```

旧三工具方案、旧 P8 lineage 和旧 256 专用实现已经由正式方案判定为 deprecated。本施工把该判定落实为 active source、Cargo target、Tool Registry、Catalog、Gateway route、Native Editor wiring 和发布入口的物理退役。

## 1. 目标

1. 删除旧 `project.create`、`project.open`、`session.request_mutation_access` 的全部 256 专用 contract、route、probe、receipt 和测试。
2. 删除旧 launcher bootstrap/rebind/access implementation。
3. 删除整个 `gate_256_p8` construction Runner、Windows helper、contract tests、CLI 和依赖。
4. 删除仅为旧 P8 activation 增加的 config helper 扩展，恢复 pre-256 通用 config helper 边界。
5. 保留早于 256 已存在的 Native Editor CreateProject、OpenProject、SafeProjectPath、Gateway Core、Catalog framework 和 mutation authority owner。
6. 保留不属于旧 Interface/Runner 的通用缺陷修复。
7. 形成没有旧 256 active symbol、没有旧 Cargo target、可以编译并通过受影响域回归的干净基线。

## 2. 明确不施工

```text
不实现新的 project.create
不保留旧 project.create 作为过渡实现
不增加 deprecated shim 或兼容 Tool
不修改 256 新正式方案
不运行旧 P8/P8Q/P8F/P8A/P9/P10
不运行 production candidate、freeze、activation 或真实 Codex conformance
不为本次 retirement 新增 Runner、artifact schema 或 one-shot Gate
不回退与旧 256 Interface 无关的通用修复
```

## 3. 基线与分类

旧 256 激活前源码基线：

```text
commit: e7478ab96033cd50b580744d37b27154c0ffdf26
tree: ba6623dec839791b57b67660ac46387e40406731
```

旧产品实现提交：

```text
f78e3c5 red contracts
774cfdc launcher gateway binding
667b81d project bootstrap access
58bfb4e publication binding
2d20c07 typed mutation access
0b787fd capability-aware access surface
023a4bf affected-domain regression
```

旧 construction-only 实现提交：

```text
7a29c87 gate_256_p8 Runner/config hardening
717427d lifecycle stress harness
33dd6d6/7e91de5/e785e2f Runner fixes and split
```

### 3.1 必须物理删除或恢复 pre-256 内容

完整删除：

```text
rust/crates/ai_tool_gateway/src/project_access.rs
rust/crates/quality_gate/src/gate_256_p8.rs
rust/crates/quality_gate/src/gate_256_p8_windows.rs
rust/crates/quality_gate/tests/gate_256_p8_contract.rs
rust/crates/quality_gate/build.rs
rust/crates/ai_tool_gateway/tests/codex_config_process.rs
```

恢复到 pre-256 通用边界：

```text
rust/crates/ai_tool_gateway/Cargo.toml
rust/crates/ai_tool_gateway/src/bin/ai_engine_gateway_codex_config.rs
rust/crates/ai_tool_gateway/src/codex_config.rs
rust/crates/ai_tool_gateway/src/core.rs
rust/crates/ai_tool_gateway/src/discovery.rs
rust/crates/ai_tool_gateway/src/editor_host.rs
rust/crates/ai_tool_gateway/src/lib.rs
rust/crates/ai_tool_gateway/src/mcp_stdio.rs
rust/crates/ai_tool_gateway/src/named_pipe.rs
rust/crates/ai_tool_gateway/src/protocol.rs
rust/crates/ai_tool_gateway/src/remote_adapter.rs
rust/crates/ai_tool_gateway/tests/core_scope_contract.rs
rust/crates/ai_tool_gateway/tests/explicit_reconnect.rs
rust/crates/ai_tool_gateway/tests/mcp_process_smoke.rs
rust/crates/editor_core/src/ai_capability_tool_kernel.rs
rust/crates/editor_core/src/ai_tool_catalog.rs
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests/ai_capability_tool_kernel_tests.rs
rust/crates/editor_core/src/tests/ai_tool_catalog_tests.rs
rust/crates/editor_host/tests/gateway_process.rs
rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/tests/gateway_reconnect.rs
rust/crates/editor_window_winit/src/tests/native_app.rs
rust/crates/project_e2e_gate/src/tests.rs
rust/crates/quality_gate/Cargo.toml
rust/crates/quality_gate/src/lib.rs
```

`Cargo.lock` 不手工恢复；源码和 manifest 退役后使用 locked dependency reconciliation 生成并审查最小变化。

### 3.2 明确保留的通用修复

```text
rust/crates/editor_window_winit/src/dialog.rs
rust/crates/editor_core/src/desktop_export.rs
rust/crates/editor_window_winit/src/tests/reachability_gate.rs
rust/crates/editor_window_winit/src/tests/support.rs
rust/crates/project_e2e_gate/src/llm_worker_lifecycle.rs
rust/quality/lint-debt-ledger.v1.json（由当前源码重新对账，不机械回退）
```

保留原因：这些修复分别属于 Windows path adapter、sample fixture ownership 和 LLM lifecycle owner，不是旧 256 Tool/Runner 的 active Interface。

## 4. R0：Retirement 红测试与 inventory

先确认旧 active symbol 当前存在：

```text
TOOL_ID_PROJECT_CREATE
TOOL_ID_PROJECT_OPEN
TOOL_ID_SESSION_REQUEST_MUTATION_ACCESS
GatewayRequestPayload::ProjectCreate
GatewayRequestPayload::ProjectOpen
GatewayRequestPayload::RequestMutationAccess
mod project_access
mod gate_256_p8
gate-256-p8-qualify
gate-256-p8-freeze
gate-256-p8-config-activate
```

记录 pre-256 baseline 与当前文件分类。R0 不运行完整测试。

退出条件：删除目标和保留目标不存在交叉 owner。

## 5. R1：物理退役

1. 删除第 3.1 节完整删除文件。
2. 对第 3.1 节共享文件只移除旧 256 增量，恢复 pre-256 通用内容。
3. 删除 Tool export、Registry descriptor、Catalog probe、Gateway protocol variant 和 Native Editor bootstrap wiring。
4. 删除 quality_gate CLI command、module、Windows dependency 和 build script。
5. 对 Cargo manifests/lockfile 做最小依赖收敛。
6. 全仓搜索旧 symbol，active Rust/Cargo 命中必须为 0。

退出条件：

```text
旧三 Tool active symbol = 0
gate_256_p8 active symbol = 0
旧 P8 CLI = 0
旧专用文件 = absent
保留通用修复 = present
```

## 6. R2：最小编译与定向验证

顺序执行：

```text
cargo fmt --check
cargo check --workspace --all-targets --locked
cargo test -p editor_core --lib --locked
cargo test -p ai_tool_gateway --locked
cargo test -p editor_window_winit --lib --locked
cargo test -p editor_host --locked
cargo test -p quality_gate --locked
```

规则：

- 不在完整 crate 命令前重复执行已被覆盖的定向 filter。
- 首个编译或测试失败先修责任 owner，不继续更昂贵命令。
- 不执行 all-features；新的 `project.create` 最终施工会承担一次去重权威验证。
- 不运行被删除的旧 Runner 或旧 contract test。

退出条件：以上命令全部通过，旧 symbol 仍为 0。

## 7. R3：Ledger、状态与完成

1. 运行 canonical lint ledger producer。
2. 只接受由源码删除产生且经审查的 ledger 收敛；禁止手写隐藏漂移。
3. 再次执行 `git diff --check` 和旧 symbol 零命中检查。
4. 更新本施工文档完成状态以及 49/54/施工 README。
5. 将本文移入 `施工文档/已完成/`，生成简短完成记录。

本阶段不创建新的 `project.create` 施工文档；retirement 完成后再以干净源码基线单独编写。

## 8. 失败收敛

- 发现目标文件包含后续共享 owner：停止恢复整文件，改为符号级删除并回填本文。
- 删除后出现旧 256 之外的 consumer：先判断 consumer 是否应退役；不得恢复兼容 shim。
- 通用修复被意外回退：立即恢复通用修复并补定向测试。
- 任一命令超时或超过总预算：停止、清理子进程并记录 `stopped_by_time_limit`。
- 本施工失败不授权恢复旧 P8，也不授权开始新的 `project.create`。

## 9. 施工文档自审

### 9.1 实际完成结果

```text
pre-256 source baseline commit: e7478ab96033cd50b580744d37b27154c0ffdf26
pre-256 source baseline tree:   ba6623dec839791b57b67660ac46387e40406731
旧三 Tool active symbol: 0
旧 gate_256_p8 active symbol/CLI: 0
旧专用源码与测试文件: absent
canonical lint ledger: previous_readable=true；167 entries；369 warning occurrences；11 suppressions；五类 diff 均为 0
authoritative Gate run count: 0（本施工明确不运行旧 P8 或新 Tool Gate）
```

通过的验证：

```text
cargo fmt --check
cargo metadata --locked
cargo check --workspace --all-targets --locked
cargo test -p editor_core --lib --locked
cargo test -p ai_tool_gateway --locked
cargo test -p editor_window_winit --lib --locked
cargo test -p editor_host --locked
cargo test -p quality_gate --locked
cargo run -p quality_gate --locked -- propose-ledger
git diff --check
```

`editor_core` 首轮出现一次未修改模块 `ai_image_generation_loop` 的并行波动；exact test 通过，完整 `editor_core --lib` 再跑为 549 passed / 0 failed / 2 ignored。未为该无关波动扩大 retirement 范围。

### 9.2 是否符合正式方案

符合。只落实旧实现 physical retirement，不实现新 Tool。

### 9.3 是否把删除与新实现重新绑定

没有。retirement 完成是独立终态；新的 `project.create` 使用之后的干净 baseline。

### 9.4 是否可能误删通用能力

已通过 pre-256 baseline 和提交分类列出恢复/删除/保留三类边界。Native Editor CreateProject、SafeProjectPath、Gateway Core、Catalog framework 和 mutation owner 不删除。

### 9.5 测试是否过多

没有。只有一次 workspace all-targets 编译和五个受影响 crate 默认测试；不重复定向 filter、不运行 all-features、不运行旧 P8。

### 9.6 是否超过三小时

没有。本次 retirement 在三小时绝对施工上限内完成；未启动任何旧 P8 或其它长 Runner。

## 10. 当前授权边界

用户已经授权本 retirement 文档从 R0 连续执行到 R3。授权不包含：

```text
新的 project.create 实现
旧 Gate 重跑
production config 修改
真实 MCP/Editor 外部验收
其它系统重构
```
