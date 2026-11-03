# 279 当前可自动化施工文档：Production Editor post-277/278 Tower AUI v2 Consistency Minimal v1

> 状态：已完成，待归档
> 日期：2026-08-09
> 上游：277、278 与 Tower UI-V2 完成基线
> 授权：更新 production Editor、保留二进制和缓存备份、只重建 Tower Preview 缓存、普通 Editor Play smoke，并迁移 Tower generator 到 AUI v2

## 1. 目标

1. Tower generator 与生成产物直接使用 `aui-document.v2`，项目 validator 冻结该合同。
2. 在 repository-external run root 构建 post-277/278 `editor_host` 候选，不直接覆盖 production target。
3. 候选定向验证、headless readiness 与真实窗口 smoke 通过后，才替换 production Editor。
4. 替换前 byte-exact 备份旧 Editor；备份 Tower `scene-main` Preview cache，只重建这一份缓存。
5. 普通 production Editor Play 必须生成成功报告，cooked AUI 必须为 v2，窗口保持响应。

## 2. 已知红灯

```text
status=Failed
cache=Hit
dirtyDomains=[]
failedStage=load_validate_runtime_package
diagnostic=AUI document 'tower-defense-battle-hud' schemaVersion 'aui-document.v1' must be normalized 'aui-document.v2'
```

分类：production activation/version drift。277 已实现 v1 -> v2 cooker migration，但未替换 production Editor；
Tower UI-V2 Gate F 的 source-built composition 成功产物为 v2，普通 production Preview cache 仍保存 v1。

## 3. Scope

Project-owned：

- `samples/tower_defense_project/AUI/generate-p1-0-battle-hud.cjs`
- `samples/tower_defense_project/AUI/battle-hud.aui.json`
- `samples/tower_defense_project/Features/MirroredBoardInterface/tests/validate-p1-0-contracts.cjs`

Engine/activation：不预期修改 engine source；候选构建到外部 root，并可回滚替换
`rust/target/debug/editor_host.exe`。

External state：production Editor binary 与 Tower Preview cache；二者变更前均保留 run-owned 备份。

## 4. 禁止

- 不运行 Local CI、完整 workspace、Tower Gate F/G 或 Windows export。
- 不修改 recent/layout/trust/Codex/MCP 等真实配置。
- 不修改 Tower gameplay、RuntimeModule、Scene、图片或 UI 布局。
- 不替换 MCP、Player 或其它安装态二进制。
- 不 reset/revert/checkout，不覆盖 dirty worktree 既有修改。
- 不删除旧 Preview cache；移动到 run-owned retained backup。

## 5. Gate A：Tower v2 source closure

修改 generator schema literal、机械重建 AUI，并在 MirroredBoard validator 冻结：

```text
aui.schema_version === "aui-document.v2"
```

验证：generator `--check`、MirroredBoard validator、FontBundleV2 consumer。

## 6. Gate B：候选构建与定向验证

使用独立 `CARGO_TARGET_DIR` 运行：

- `cargo test -p editor_core aui_document_ --locked`
- `cargo test -p editor_core preview_package --locked`
- `cargo build -p editor_host --locked --features real-window,real-wgpu-surface`

候选必须包含 v1->v2 migration，headless readiness exit 0，真实窗口可创建且响应。

## 7. Gate C：production replacement

关闭并确认本轮 production Editor PID 退出；保存 before/candidate/after SHA256。候选先复制为 pending 并复核，
再替换 production path。任一步失败，使用 before backup byte-exact 恢复。

## 8. Gate D：cache rebuild 与 ordinary Play smoke

备份并移走唯一 `scene-main` cache；启动更新后的普通 production Editor，打开 Tower 并 Play。要求新 preview
report `status=Success`、无 error diagnostics、cooked AUI 为 v2、GameView 进入 Play。Stop/关闭后 owned process=0。

## 9. 完成条件

Gate A-D 通过；记录 binary/cache 身份与回滚点；原红灯不再复现；同步 49/54/索引、完成记录并归档本文。

## 10. 自审与激活前复核

- 只处理已复现的 schema/version drift 与普通 production Play，没有扩大到其它系统。
- 复用 277/278 能力，不推翻方案，不把 Tower 语义写入引擎。
- 二进制和缓存都有 byte-preserving backup，具备回滚条件。
- 只执行 owner/affected consumer、候选窗口与一次普通 Editor Play smoke。
- 当前无其它可执行施工文档；用户明确授权 production replacement 与 Tower cache mutation；三小时预算充足。

## 11. 完成结果

- Gate A：generator、生成 AUI 与 validator 已直接冻结 `aui-document.v2`；generator `--check`、项目 validator 与 FontBundleV2 consumer 通过。
- Gate B：外部 target 候选通过 AUI migration/Preview owner tests、构建、headless readiness 与真实窗口响应 smoke。
- Gate C：production Editor 已从 `159EECC2...1C8B` 事务替换为 `4B219230...C4E5`；旧二进制 byte-exact backup 保留。
- Gate D：旧 Tower `scene-main` Preview cache 已备份并移出项目 cache；普通 Editor Play 重建成功，runtime load 与 AUI present 成功并正常 Stop。
- Receipt：`C:\AifeProductionRuns\279-post-277-278-20260809-221417\consistency-receipt.json`。
- 未执行：Local CI、Tower Gate F/G、Windows export、其它 production/安装态二进制与真实配置。
