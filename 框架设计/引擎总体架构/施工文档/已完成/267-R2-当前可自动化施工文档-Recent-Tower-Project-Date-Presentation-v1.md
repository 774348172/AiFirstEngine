# 267-R2 Recent Tower Project / Date Presentation v1 施工文档

```text
状态：Gate A-C 已完成并归档
父方案：267-Editor-Localization-Chinese-Default-Font-Stack-v1方案.md（B+）
日期：2026-08-03
施工证据：dirty-worktree development/integration evidence + authorized user-state repair
```

## 1. 施工目标

恢复普通 Native Editor Launcher 中缺失的塔防最近项目，并将修改时间由 Unix epoch 秒显示为
无前导零的 `年.月.日`（例如 `1977.2.3`）。项目清单和 recent-project 持久化仍保存稳定原始
epoch；日期转换只属于 Editor presentation。

## 2. 授权与边界

用户明确要求恢复塔防项目并修改日期显示。允许：

- 修改 Native Editor Launcher 日期 presentation 与测试。
- 以结构化、保留现有条目的方式将 `G:\gameEngin\samples\tower_defense_project` 合并进真实
  recent-project store；修改前保存 byte-exact backup，修改后验证。
- 重建并重启源码 debug Editor。

禁止：修改塔防项目玩法/资产、清空或覆盖其他最近项目、修改 project manifest 时间合同、运行
Local CI、替换 production/安装态二进制、启动 Tower Gate G/Gate H。

## 3. 涉及文件

```text
rust/crates/editor_ui_renderer/src/panels/launcher.rs
rust/crates/editor_ui_renderer/src/tests.rs
C:\Users\zenghaoran\AppData\Roaming\AI First Engine\editor_recent_projects.json（授权状态修复）
框架设计/引擎总体架构（267-R2 文档闭环）
```

## 4. Gate A - 红测试与根因

- 断言真实 recent store 中存在 canonical Tower Defense path；先确认失败。
- 构造 `last_modified_at=223776000`，断言 Launcher 绘制 `1977.2.3` 且不绘制 epoch；先确认失败。
- 保留 invalid/non-numeric legacy display 值，避免破坏兼容。

## 5. Gate B - 日期 presentation

- 在 Launcher presentation owner 增加确定性 Unix seconds -> Gregorian date formatter。
- 使用 UTC calendar date，避免测试/机器时区导致日期漂移；不显示时分秒。
- 数字越界或非数字值保持原样；`None` 保持 `-`。

验证：

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer recent_project -- --nocapture
cargo test -p editor_ui_renderer
cargo fmt --check
```

## 6. Gate C - recent store 修复与激活

- 读取并验证 `editor-recent-projects.v1`。
- 保存原文件 byte-exact backup；prepend Tower Defense entry，按 canonical path 去重，保留其他条目。
- Tower entry 的 modification epoch 来自 `project.aife.json` 文件 metadata，engine version/name 来自 manifest。
- 重建并重启源码 debug Editor，确认窗口存活；不替换 production。

验证：

```powershell
cargo test -p editor_window_winit --no-run
cargo build -p editor_host --features real-window,real-wgpu-surface
```

## 7. 施工自审

- [x] 塔防项目实体已确认完整，不修改项目内容。
- [x] 缺失原因是普通/隔离 recent store 分离，不是项目删除或校验过滤。
- [x] 日期变化只在 presentation 层，不迁移持久化 schema。
- [x] 真实配置修改由本次用户请求授权，且要求 backup、保留、去重和验证。
- [x] 当前施工槽原为空，本文档是唯一 active 施工文档。
- [x] 不运行 Local CI、不替换 production、不执行 Tower Gate。

自审结论：范围、验证与外部状态恢复边界完整，可以施工。

## 8. 完成状态

- [x] Gate A：真实 store 中 Tower 为 0、日期绘制 epoch 的两条红信号已确认。
- [x] Gate B：`1977.2.3` 日期测试及 renderer 63 项通过，legacy 非数字值保持原样。
- [x] Gate C：真实 store 已备份并保留原三项，Tower 唯一 prepend；debug Editor 已重建启动。
- [x] 真实窗口截图确认 Tower 第一行及全部日期格式正确。

完成证据见
`阶段完成记录/2026-08-03-Recent-Tower-Project-Date-Presentation-v1/00-总览.md`。
