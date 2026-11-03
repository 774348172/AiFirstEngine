# 250-A 当前可自动化施工文档：ProjectProduction Empty Project Readiness v1

> 状态：当前唯一施工文档，已激活  
> 对应方案：`250-AI-Primary-ProjectProduction-Dual-Path-v1方案.md`  
> 施工范围：仅 Phase 1  
> 激活日期：2026-07-14

## 1. 目标

修正 BuiltInEmpty 与 ProjectRust 的 runtime source 语义，建立结构化 `ProjectReadiness` Interface，使新建空项目在进入 Editor 时是真实 authoring-ready，而不是依赖不存在的项目源码路径。

## 2. 涉及文件

预期修改：

```text
rust/crates/editor_core/src/project_launcher.rs
rust/crates/editor_core/src/project_readiness.rs（如拆分后更深）
rust/crates/editor_core/src/lib.rs
rust/crates/editor_core/src/tests 或同 module tests
必要的 project launcher / RuntimePackage / export consumer tests
框架入口、完成记录和本文归档
```

不得修改：

```text
engine gameplay Core API
samples/complex_shooter_project 的玩法实现
RuntimePackage 派生产物
用户仓库外 C-01 项目（完成 Gate 前不迁移）
```

## 3. Gate A：Baseline 与合同测试

目标：冻结当前旧 manifest、BuiltInEmpty 特例、ProjectRust 缺文件和新建项目行为。

定向测试：

```powershell
cargo test -p editor_core project_launcher
cargo test -p editor_core project_runtime_package_assembler
```

进入条件：当前代码可编译；记录已有失败。  
失败处理：先证明失败来自本施工范围，不修改无关脏工作树文件。

## 4. Gate B：Runtime Source Kind

目标：

```text
manifest 可明确表达 BuiltInEmpty / ProjectRust。
旧 aife-project.v2 的 engine.empty.runtime 可兼容读取为 BuiltInEmpty。
BuiltInEmpty 不要求 RuntimeModule/Cargo.toml 存在。
ProjectRust 必须拥有有效 project-relative manifest 和 source root。
非法组合 fail closed 并返回诊断 code。
```

定向测试：

```powershell
cargo test -p editor_core project_runtime_source
cargo test -p editor_core project_launcher
```

## 5. Gate C：ProjectReadiness 深 Module

目标：实现小 Interface 返回结构化报告，覆盖 manifest、Scene、Input、目录与 runtime source 检查。

否定矩阵至少包括：

```text
missing manifest
invalid manifest
unsupported engine version
missing/default invalid scene
missing/invalid input
BuiltInEmpty 非法字段组合
ProjectRust missing Cargo.toml
ProjectRust missing src/lib.rs
path escape
```

定向测试：

```powershell
cargo test -p editor_core project_readiness
```

## 6. Gate D：Launcher 与 Consumer 接入

目标：

```text
create_project 生成真实一致的 BuiltInEmpty 模板。
open/refresh 使用 ProjectReadiness authoring level。
现有 ProjectRuntimePackage assembler 继续接受 BuiltInEmpty digest。
已有样例 ProjectRust 不退化。
```

测试：

```powershell
cargo test -p editor_core project_launcher
cargo test -p editor_core project_runtime_package_assembler
cargo test -p editor_core play_session
cargo test -p editor_core desktop_export
```

## 7. Gate E：受影响域回归

```powershell
cargo test -p editor_core
cargo test -p editor_window_winit project
cargo test -p project_e2e_gate second_project
```

如果 `editor_window_winit` 因用户现有未提交改动失败，必须定位并报告；不得回滚用户改动。

## 8. Gate F：最终预检与权威回归

预检：

```powershell
cargo fmt --check
cargo test --workspace
cargo test --workspace --all-features
```

最终权威回归只在候选冻结、预检全绿后运行一次。若本轮无法建立 clean exact-source 隔离环境，必须明确记录为残余风险，不能用定向测试冒充。

## 9. 完成条件

```text
新建空项目不再宣称拥有不存在的项目 Rust source。
ProjectReadiness 报告可解释 Ready/Incomplete/Invalid/Unsupported。
BuiltInEmpty 与 ProjectRust 否定矩阵通过。
RuntimePackage/Play/Export consumers 无回归。
入口文档、250 方案、240/247 状态、完成记录和施工归档同步。
```

## 10. 自审

```text
只执行 250 Phase 1：通过。
没有提前实现 SourcePatch/Candidate/Dev Host：通过。
没有玩法专用 API：通过。
测试分为定向、受影响域、最终权威回归：通过。
外部审查 45/46/51 的 Phase 1 约束已写入：通过。
当前目录原为空，本文是唯一非 README 施工文档：通过。
```
