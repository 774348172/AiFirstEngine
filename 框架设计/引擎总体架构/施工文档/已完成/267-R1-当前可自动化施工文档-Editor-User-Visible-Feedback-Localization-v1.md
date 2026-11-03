# 267-R1 Editor User-Visible Feedback Localization v1 施工文档

```text
状态：Gate A-C 已完成并归档
父方案：267-Editor-Localization-Chinese-Default-Font-Stack-v1方案.md（B+）
日期：2026-08-03
施工证据：dirty-worktree development/integration evidence
```

## 1. 施工目标

修复 Native Editor 红色 interaction feedback banner 中用户可见错误仍显示英文的问题。默认
`zh-CN` 时，Catalog 已登记的静态用户消息必须显示中文；`en-US` 保持英文。稳定 diagnostic
code、command ID、路径、模块 ID、request ID 和用户/项目内容保持 invariant。

## 2. 授权与禁止范围

用户已要求把这类红色报错改成中文。允许修改 Editor localization Catalog、反馈 presentation
消费和相应测试，并重建、重新启动源码 debug Editor 进行验证。

禁止修改 Project Runtime/AUI localization、塔防 gameplay、真实用户配置、production/安装态
二进制；不运行 Local CI，不把 dirty-worktree 结果声明为 release evidence。

## 3. 涉及文件

```text
rust/crates/editor_ui_model/resources/localization/zh-CN.editor.json
rust/crates/editor_ui_model/resources/localization/en-US.editor.json
rust/crates/editor_ui_model/src/localization.rs（仅测试/合同确有需要时）
rust/crates/editor_ui_renderer/src/tests.rs
rust/crates/editor_ui_renderer/src/localization.rs（仅消费边界确有缺口时）
rust/crates/editor_window_winit（仅动态反馈需要 typed message 时）
框架设计/引擎总体架构（267-R1 文档闭环）
```

## 4. 分阶段任务与测试

### Gate A - 红测试与反馈 inventory

- 构造截图中的原始英文消息，经过默认中文 DrawList publication。
- 断言红条不再包含英文且包含预期中文；先运行确认测试为红。
- 盘点进入 `interaction_feedback` 的 production 用户消息，区分可翻译静态文案与 invariant。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer feedback_banner_localizes_cataloged_project_runtime_error -- --nocapture
```

### Gate B - Catalog 修复

- 保留 engine core 英文 fallback，不把稳定诊断合同硬编码成中文。
- 为静态用户错误在 `zh-CN` / `en-US` Catalog 增加同 key、同 arguments 合同。
- 动态消息如需迁移，必须使用 typed named arguments，不做前缀字符串解析。

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_model localization -- --nocapture
cargo test -p editor_ui_renderer feedback_banner_localizes_cataloged_project_runtime_error -- --nocapture
```

### Gate C - 受影响回归、debug Editor 与文档闭环

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer
cargo test -p editor_window_winit --no-run
cargo fmt --check
cargo build -p editor_host --features real-window,real-wgpu-surface
```

通过后关闭旧的源码 debug Editor，启动新构建；不替换 production/安装态二进制。生成 267-R1
完成记录，更新 54、施工 README 和阶段记录 README，并将本文档移到 `已完成/`。

## 5. 失败处理

- 保留首个失败 assertion/stdout/stderr，先修 owner，不用 Local CI 调试。
- 若 Catalog 收录后仍不生效，检查 DrawList localization publication 边界。
- 若消息包含动态 invariant，升级为 typed named arguments；不得机械翻译整个诊断字符串。
- 保留当前 dirty worktree 中所有用户和 260-267 既有改动。

## 6. 施工自审

- [x] 这是已确认 267 B+ 的用户可见反馈补漏，不重开架构方案。
- [x] 红测试直接覆盖用户截图中的英文消息和默认中文显示结果。
- [x] 核心英文 fallback 与稳定技术标识保持不变。
- [x] 测试与构建限于受影响 closure；不运行 Local CI。
- [x] debug Editor 重启不会替换 production 或修改真实用户配置。
- [x] 当前施工槽原为空，本文档可作为唯一 active 施工文档。

自审结论：范围、测试和外部状态边界完整，可以施工。

## 7. 完成状态

- [x] Gate A：截图英文已由精确红测试复现，根因确认为 Catalog 缺项。
- [x] Gate B：具体错误使用双 Catalog 中文；未迁移失败使用中文摘要与稳定诊断码。
- [x] Gate C：Catalog、renderer、window consumer、格式和真实窗口 debug build 通过。
- [x] 新源码 debug Editor 已启动，PID `21608`；未替换 production/安装态二进制。

完成证据见
`阶段完成记录/2026-08-03-Editor-User-Visible-Feedback-Localization-v1/00-总览.md`。
