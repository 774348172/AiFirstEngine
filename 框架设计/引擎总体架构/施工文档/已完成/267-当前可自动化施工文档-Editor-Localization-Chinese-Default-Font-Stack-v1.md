# 267 Editor Localization / Chinese Default / Font Stack v1 施工文档

```text
状态：Gate A-F 已完成，待归档
方案：267-Editor-Localization-Chinese-Default-Font-Stack-v1方案.md（B+）
日期：2026-08-03
施工证据：dirty-worktree development/integration evidence
```

## 1. 授权与禁止范围

用户已授权生成施工文档并直接开始施工。当前施工槽没有活动施工 `.md`，待执行队列为空；
本文档激活后是唯一当前施工文档。

允许：

- 修改通用 Native Editor 的 localization、preferences、UI presentation 和字体渲染代码。
- 增加 `zh-CN` / `en-US` Catalog、Editor CJK 字体、许可证和校验 fixture。
- 运行 owner、affected consumer 和受影响 integration 测试。
- 使用 run-owned state root 验证偏好持久化和 Editor 组合。

禁止：

- 修改 Project Runtime/AUI localization、261 Project FontBundle 或塔防 gameplay。
- 修改真实用户配置。
- 替换 production/安装态 Editor 或稳定 MCP。
- 运行 Local CI、exact-commit qualification 或发布激活。
- 把当前 dirty worktree 测试冒充 clean release evidence。

## 2. 基线与施工约束

当前工作树包含用户/历史 260-266 和塔防成果，施工必须增量保留。不得重置、回滚、全仓机械
格式化或改写无关文件。267 只触碰以下 owner/consumer：

```text
rust/crates/editor_ui_model
rust/crates/editor_ui_renderer
rust/crates/editor_wgpu_renderer
rust/crates/editor_window_winit
rust/crates/editor_core（只在共享 UI model contract 必需时）
rust/resources/editor（受信 Catalog/font/license）
框架设计/引擎总体架构（267 文档闭环）
```

现有 `samples/tower_defense_project` 字体只可作为已取得的二进制/许可证来源证据；Editor 必须复制到
engine-owned resource path 并从该路径或 embedded engine resource 装载，运行时不得依赖 sample path。

## 3. Gate A - Localization 合同与红测试

目标：建立独立、可测试的 schema-first localization owner。

任务：

- 在 `editor_ui_model` 新增 `EditorLocaleId`、`EditorMessageKey`、typed arguments、
  `EditorTextRef`、Catalog schema、validator、snapshot 和 diagnostics。
- 默认 `zh-CN`，fallback `en-US`；严格 key/argument parity。
- 增加 missing key、argument mismatch、unsupported locale 和 revision 测试。
- Catalog 解析只发生于构建/switch，不进入 draw 热路径。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_model localization -- --nocapture
```

## 4. Gate B - EditorPreferenceStore 与切换事务

目标：locale 由 application-owned store 持久化，测试 root 可注入。

任务：

- 在 `editor_window_winit` 增加版本化 `editor-preferences.v1` store。
- 缺文件默认中文；malformed/unsupported 使用中文并诊断。
- 原子写入；写盘失败不推进 locale/revision。
- 增加 `ChangeEditorLocale` typed command 和 application transaction。
- 不复用 workspace/recent-project/project settings。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit editor_preferences -- --nocapture
cargo test -p editor_window_winit editor_locale -- --nocapture
```

## 5. Gate C - Trusted Catalog 与 Editor 文本消费

目标：production reachable Editor 文本通过稳定 key/explicit invariant 进入 presentation。

任务：

- 增加 trusted `zh-CN` / `en-US` Catalog 和 resource locator/embedded package seam。
- command ID 与 display message identity 分离。
- Launcher、shell/menu/toolbar、workspace/panel、Trust、Build/Export、AI/Project Browser、modal、
  settings 文本迁移或进入显式 inventory。
- renderer 消费同一 immutable snapshot，不读磁盘/全局 locale。
- dynamic 文本使用 named args；项目名、路径、ID 和原始错误保持 invariant。
- 增加 Catalog completeness 和 production reachable literal inventory gate。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_model localization_catalog -- --nocapture
cargo test -p editor_ui_renderer localization -- --nocapture
cargo test -p editor_ui_renderer
```

## 6. Gate D - EditorFontStack 与 CJK glyph

目标：默认中文使用 engine-owned 可再分发字体真实绘制。

任务：

- 锁定 Noto Sans SC TTF、OFL 许可证、来源和 SHA-256。
- 用当前 `ab_glyph` qualification 确认中文 glyph/metrics。
- `EditorFontSystem` 从单 face 改为有序 font stack 和 per-glyph selection。
- cache identity 包含 `font_face_id`；layout/raster metrics 来自同一 face。
- 缺 glyph 不再静默报告成功；atlas capacity 有 typed/统计证据。
- Editor 不读取 project FontBundle 或 sample path。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_wgpu_renderer font_stack -- --nocapture
cargo test -p editor_wgpu_renderer chinese -- --nocapture
cargo test -p editor_wgpu_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu
```

## 7. Gate E - Language Settings 与组合验证

目标：用户可以从 Native Editor 设置入口即时切换中文/英文。

任务：

- 增加 application-level Preferences/Language 入口。
- 显示 `简体中文（zh-CN）` 与 `English (en-US)` 自称。
- 成功切换下一 UI publication 全量一致；失败保留 last-good。
- 普通 Editor 和 generated specialized Editor 使用同一 contract。
- 在 run-owned state root 验证 launcher/workspace/trust/build-export 的中英文 draw text。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit localization -- --nocapture
cargo test -p editor_window_winit --no-run
```

真实 Windows 多 DPI/截图只在不修改 production/真实配置、且剩余施工窗口足够时作为 run-owned
integration evidence 执行；否则明确保留为后续 release qualification，不阻塞源码 Gate 的真实声明。

## 8. Gate F - 受影响回归与文档闭环

在 Gate A-E owner 测试通过后运行不重复的受影响 closure：

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --check
cargo test -p editor_ui_model
cargo test -p editor_ui_renderer
cargo test -p editor_wgpu_renderer --features real-wgpu
cargo test -p editor_window_winit
```

若共享 `EditorUiModel` contract 改动影响 `editor_core`，补充：

```powershell
cargo test -p editor_core --no-run
```

全部 required Gate 通过后：

- 生成 `阶段完成记录/2026-08-03-Editor-Localization-Chinese-Default-Font-Stack-v1/00-总览.md`。
- 更新 `00-文档地图.md`、`00-阅读顺序.md`、`49`、`54`、施工文档 README、阶段记录 README。
- 把本文档从 `当前/` 移到 `已完成/`。
- 明确未运行 Local CI、未替换 production/安装态 binary、未修改真实用户配置。

## 9. 失败处理

- 先保留首个 owner failure 的 assertion/stdout/stderr，再修 owner，不用全仓测试调试。
- 若发现方案合同错误，先回填 267 方案/本文档，再继续。
- 若字体二进制与 parser 不兼容，停止字体 Gate，选择可再分发 compatible TTF；不得退回 OS-only。
- 若现有用户改动与 267 同一行冲突，保留用户语义并做最小融合，不还原文件。
- 任何真实用户配置、production binary 或 Local CI 需求都必须另行授权。

## 10. 施工文档自审

- [x] 已读取 267 正式方案，无外部审查要求修改方案。
- [x] 没有扩大到 Project Runtime/AUI、塔防或 261 Project FontBundle。
- [x] 每个可执行合同都有 owner/consumer 验证或明确 release-level 豁免。
- [x] 没有机械重复同配置测试；Gate F 上层 suite 替代同输入定向测试作为最终 closure。
- [x] preferences 和视觉 state 使用 run-owned root，不污染真实用户状态。
- [x] production replacement、Local CI、exact-commit 和发布激活明确不在授权中。
- [x] 当前源码很脏，因此只声明 development/integration evidence。
- [x] 施工槽入口声明无活动施工，本文档可作为唯一 active `.md` 激活。

自审结论：施工文档与 267 B+ 一致，范围、Gate、测试、失败恢复和外部状态边界完整，可以施工。

## 11. 完成状态

- [x] Gate A：Localization 合同与双 Catalog。
- [x] Gate B：EditorPreferenceStore 与中文默认。
- [x] Gate C：production text inventory、静态消费与 typed named arguments。
- [x] Gate D：engine-owned Noto Sans SC、OFL、per-glyph font stack/cache identity。
- [x] Gate E：Launcher/workspace Language UI、持久化后即时 publication。
- [x] Gate F：受影响回归、完成记录与入口闭环。

完成证据见
`阶段完成记录/2026-08-03-Editor-Localization-Chinese-Default-Font-Stack-v1/00-总览.md`。
