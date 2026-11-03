# 261 Window H production 视觉导出阻塞记录

日期：2026-07-31

授权范围：执行 261 Window H 真实 production 视觉矩阵和独立 Windows Player 导出；
允许建立冻结候选；禁止替换 production/安装态二进制；禁止 Local CI。

授权开始：2026-07-31 09:50 +08:00。阻塞报告时授权结束；三小时最晚截止
2026-07-31 12:50 +08:00，12:35 后不得开启新阶段。

## 证据身份

本轮使用独立证据根：

```text
rust/target/261-window-h-visual-20260731-0950/
```

未提交 dirty worktree，因此冻结候选使用：

```text
HEAD commit
tracked product diff SHA-256
product source file manifest SHA-256
font SHA-256
Project RuntimeModule source digest
Engine SDK digest
```

最终到达阻塞的候选为 `261-h-r3`：

```text
HEAD=acaffa8cb95bbb54d093bbf45835a59315c82da5
trackedDiffSha256=8b41983beb312482ff634a95ca198b1d3efc354cd42bc324d59813512966e92d
productSourceManifestSha256=c5d81726082346f9464d9c55e5de41307ad0ce7da9493022db4167a762076c12
fontSha256=f70ecb32e5b312ba7bc724977352139a3f691566dc2491377be3828631c9fab2
```

这仍是 frozen dirty-worktree candidate，不是 clean exact-commit 或发布资格。

## 使用的 production owner

隔离 harness 仅调用现有公开产品 API：

```text
DesktopExportPipeline
DesktopExportRequest::windows_dev
ExplicitExportOutput
ProjectRuntimePlayerArtifactBuildRequest
ProjectPlayerArtifact::build_project_rust
```

导出输出、Player artifact build root、harness Cargo target 和 evidence 均位于独立
`rust/target/261-window-h-visual-20260731-0950/`，没有写入项目 `Build/`、production
目录或安装态。harness 不进入产品源码或公共引擎接口。

## 红测序列

### Candidate 1：隐式 source kind

首个正式导出在 Player 选择阶段失败：

```text
project_runtime.player_artifact_build_failed
package ID specification `tower_defense_player` did not match any packages
```

原因是塔防 `project.aife.json` 未显式声明 `runtimeModule.sourceKind=projectRust`，导出器走了
旧预编译 workspace Player 路径。项目 manifest 增加公开字段，并更新项目自测断言。

### Candidate 2：测试依赖污染 production RuntimeModule

进入 ProjectRust builder 后失败：

```text
controlled_source_patch.cargo_policy_rejected
Cargo manifest field/table 'dev-dependencies' is forbidden.
```

原 FontBundle v2 项目集成测试把 `editor_core` 作为 RuntimeModule dev dependency。已将测试
迁移到项目自有独立 crate `Tests/FontBundleV2`；RuntimeModule 恢复纯 production 依赖。

### Candidate 3：冗余 lib path

继续进入受控 Cargo 合同后失败：

```text
controlled_source_patch.cargo_policy_rejected
Cargo lib.path is forbidden.
```

`src/lib.rs` 是 Cargo 默认路径，已移除冗余 `[lib] path`。

### Candidate 4 前置检查揭示通用引擎缺口

在开启下一次高成本导出前读取 owner policy，确认它还要求：

```text
dependencies 只能包含 engine_runtime 和可选 engine_input
engine 依赖必须写成精确版本字符串，例如 =0.1.0
serde / serde_json 等项目依赖全部拒绝
```

塔防 RuntimeModule 对 `serde/serde_json` 有 170 处实际使用，承担数据合同、规则状态、
snapshot、事件和 deterministic fixture 的正式序列化。删除依赖或把复杂规则移出 Rust
会推翻已确认的塔防项目方案。Complex Shooter / Switch Puzzle 的项目模块也仍使用本地
Engine SDK path 依赖，说明该限制不是塔防专用问题。

`ProjectRuntimePlayerArtifact` 当前直接复用 SourcePatch v1 的窄 Cargo policy 作为普通
ProjectRust production export policy。普通项目开发合同与 production staging 合同没有
分开，导致合法项目模块无法进入独立 Player 构建。

因此没有开启 Candidate 4，没有运行 GPU/窗口/截图，也没有使用手工 host、测试旁路或
workspace 专用 Player 伪造 production 证据。

## 已完成的项目侧验证

```text
tower RuntimeModule fmt
passed

tower RuntimeModule full suite
60 passed; 0 failed

project-owned Tests/FontBundleV2 fmt
passed

project-owned FontBundle v2 integration
1 passed; 0 failed
```

既有 warning 未变化：`engine_runtime` 三项 dead-code、`project_font_bundle.rs` 未使用
`BTreeSet`。没有新增测试失败。

## 外部状态

- production/安装态二进制：未读取替换入口，未修改；
- Local CI：未运行；
- real Windows/GPU one-shot：未消费；
- Cargo/Rustc 子进程：全部结束；
- 失败 export、artifact root 和报告：保留在独立 evidence root，用于后续诊断；
- 项目 `Build/`：未写入。

## 阻塞结论

Window H production visual/export 当前被通用引擎能力缺口阻塞：

```text
ProjectRuntimePlayerArtifact production staging
错误复用 SourcePatch v1 最小依赖政策
无法构建合法、带受控第三方依赖的普通 ProjectRust RuntimeModule
```

修复需要明确授权修改引擎 owner。未获授权前不得继续 Candidate 4、真实 GPU screenshot、
exported Player 验收或 Gate H 归档。Gate H 保持 `incomplete`。
