# 258 Native Editor Floating Workspace + Panel Chrome v1 施工文档

## 0. 状态

```text
状态：W6 / S6 已完成，258 passed
可施工：否（已完成并归档）
正式方案：258-Native-Editor-Floating-Workspace-Panel-Chrome-v1方案.md
方案身份：ee38057
文档编写基线：ee3805705ff789538ebc18b57cd2d90bfc31b7f2
激活基线：12d62775fb17c22a0384356f064f91352c8347ff
当前施工槽：无
当前代码授权：无
W1 代码提交：9cc9e5b1f3012a36ea82ad8c25aec26138acdac3
W2 代码提交：1e3a979153ef0567f41d50236b2aa0d0420d1ddd
W2 evidence：G:\EngineTest\258-W2-20260727-120000\evidence\w2-summary.json
W3 代码提交：89eeb8d5f75de81f11d7e59486c44fd3596b213c
W3 evidence：G:\EngineTest\258-W3-20260727-122000\evidence\w3-summary.json
W4 代码提交：9ccc1e06050c736a03e0c89aff1d40959cd574b9
W4 evidence：G:\EngineTest\258-W4-20260727-124000\evidence\w4-summary.json
W5 代码提交：ca2eca2394c2074a4bd40da1fcd4a52778d86880
W5 evidence：G:\EngineTest\258-W5-20260727-133000\evidence\w5-summary.json
W6 最终提交：b7b5607936e65984e5147200ce1a9ef84031316d
W6 evidence：G:\EngineTest\258-W6-20260727-140000\evidence\w6-summary.json
W7 cleanup：completed；独立 remediation 文档已归档
下一步：回到真实用户使用反馈
```

本文件已归档到 `施工文档/已完成/`。W1-W6 已全部完成；W7 只收口 role-owned cleanup，
未继续追加 258 产品范围。

## 1. 施工目标

在不重新打开 257、不增加公共生命周期菜谱的前提下，完成：

```text
S1 Splitter visual/hit separation；
S2 Panel chrome / Inspector context Lock / Close Tab；
S3 WorkspaceTopology v2 与 EditorWorkspaceLayout.v2；
S4 NativeWorkspaceWindowHost 多窗口 composition；
S5 native drag proxy / floating / re-dock；
S6 真实 Windows 验收、production composition 与归档。
```

用户可见结果：

```text
Splitter 视觉边线约 1 physical px，但仍保留易抓取的透明命中区；
每个可见 Stack header 右上角保留 Lock 和 More；
More 菜单 v1 只有 Close Tab；
Inspector Lock 固定当前 context，不是锁定 layout；
Tab 可拖出成为真实原生窗口，并可拖回主窗口或其它浮动窗口；
主窗口与浮动窗口共享同一个 EditorSession、Gateway 和 project context。
```

## 2. 258 独立性红线

```text
257 保持 completed / archived；
不得修改 257 方案、施工文档或完成结论；
不得重跑 257 D0-D7；
不得复用 257 role root、Local CI run、binary seal 或真实窗口 artifact；
258 使用独立测试 root、提交、production evidence 和完成记录；
258 失败只记录到 258，不得回写为 257 failure；
不得创建 257-R2、257 remediation 或 continuation。
```

258 可以复用当前生产源码 Interface 和现有测试 helper，但复用测试代码不等于复用历史通过证据。

## 3. 明确非目标

```text
项目 AUI / Runtime HUD；
插件二进制 Window ABI；
每个 floating window 独立 EditorSession / Gateway；
业务 panel 自己创建 OS Window；
复制 Unity 全部 generic menu；
Lock 冻结整个 EditorUiModel、项目或 layout；
在线布局同步；
动画 Dock；
macOS / Linux production qualification；
release-wide Local CI 默认资格；
新的长期 construction Runner。
```

发现必须扩大到上述范围时立即停止当前窗口，分类为方案缺口，并先修订 258 正式方案。

## 4. Owner 与消费者

### 4.1 主要产品 owner

```text
rust/crates/editor_ui_renderer/src/workspace_docking.rs
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/panels/mod.rs
rust/crates/editor_ui_renderer/src/draw_list.rs（仅新增 typed hit target 时）
rust/crates/editor_ui_renderer/src/theme.rs（仅 hairline / chrome token）

rust/crates/editor_window_winit/src/application.rs
rust/crates/editor_window_winit/src/real_window.rs
rust/crates/editor_window_winit/src/workspace_persistence.rs
rust/crates/editor_window_winit/src/focus.rs
rust/crates/editor_window_winit/src/bin/editor_ui_authority.rs

rust/crates/editor_ui_model/src/inspector.rs（仅 typed Inspector context anchor / presentation）
rust/crates/editor_core/src/ui_model_composer.rs（仅按 typed anchor 生成 Inspector projection）
rust/crates/editor_core/src/lib.rs（仅必要导出）
```

允许按现有 crate 私有组织新增：

```text
editor_window_winit/src/native_workspace_window_host.rs
editor_window_winit/src/tests/workspace_panel_chrome.rs
editor_window_winit/src/tests/workspace_multi_window.rs
editor_window_winit/src/tests/workspace_native_drag.rs
```

`NativeWorkspaceWindowHost` 必须是 winit 内部 Adapter，不得导出成第二个公共生命周期 Module。

### 4.2 直接消费者

```text
editor_window_winit::NativeEditorApplication；
editor_window_winit real-window event loop；
editor_wgpu_renderer 每窗口 surface / renderer；
editor_host real-window production composition；
WorkspaceLayoutStore；
现有 workspace splitter / tab drag / persistence / DPI tests。
```

### 4.3 默认不受影响

```text
Runtime Player；
项目 AUI；
RuntimePackage；
Build / Export；
Asset DB；
AI Tool Gateway typed tools；
项目 mutation / digest；
复杂打飞机 gameplay。
```

若实际 diff 触及默认不受影响域，先更新本施工文档的消费者闭包和验证范围。

## 5. 施工窗口与三小时合同

258 使用六个连续授权窗口：

```text
W1 -> S1；
W2 -> S2；
W3 -> S3；
W4 -> S4；
W5 -> S5；
W6 -> S6。
```

规则：

```text
每个窗口必须由用户单独授权；
每个窗口从首次施工动作起最长三小时；
同一窗口内允许多次运行 Local development tests，不要求重复授权；
不得把 Local CI 当修复循环；
2h45 后不启动新实现或验证；
3h 时必须停止并记录 stopped_by_time_limit；
当前窗口失败时不得越过到下一窗口；
失败先保留首因证据并分类为 design / construction-doc / implementation / environment / baseline；
修复只续跑被该修改失效的证据。
```

每个窗口使用独立外部 root：

```text
G:\EngineTest\258-<Wn>-<yyyyMMdd-HHmmss>\
  cargo-target\
  temp\
  evidence\
  state\
```

必须显式设置该窗口独占的 `CARGO_TARGET_DIR`、`TEMP`、`TMP`。禁止复用 workspace 默认 target
作为并行或权威测试真相，禁止多个窗口共享 mutable state。

## 6. 激活前复核

本步骤已于 2026-07-27 按用户要求完成。

复核结果：

```text
[x] 258 正式方案仍为当前有效方案；
[x] 方案身份 ee38057 之后没有推翻性修订；
[x] 激活前 当前/ 没有另一份 active 施工文档；
[x] 激活前本文件位于 待执行/；
[x] Git 基线 12d62775fb17c22a0384356f064f91352c8347ff，工作树 clean；
[x] owner 文件、crate 与 real-window / real-wgpu-surface feature 已重新读取并存在；
[x] production Editor PID 12368，路径 G:\gameEngin\rust\target\debug\editor_host.exe，
    参数 --real-window；只记录，不控制；
[x] W1 可在三小时内完成；
[x] 测试命令与实际 crate / feature 名称一致；
[x] 不存在必须先处理的 overlapping user change。
```

激活动作：

```text
[x] 已记录 activation baseline；
[x] 已把本文件移动并重命名为：
  施工文档/当前/258-当前可自动化施工文档-Native-Editor-Floating-Workspace-Panel-Chrome-v1.md
[x] 已同步入口；
[x] W1 已获得单独授权并完成。
```

复核已经通过；若 W1 授权前出现方案、代码或外部状态漂移，必须在 W1 开始前重新核对受影响项。

## 7. W1 / S1 - Splitter visual/hit separation

### 7.1 红测试

在 owner test 中先证明当前失败：

```text
Horizontal / Vertical splitter hit rect 仍约 7 logical px；
visual rect 约 1 physical px 或 DPI 等价 hairline；
两者中心线一致；
hit rect 透明，不生成宽色带；
visual rect 不承担 pointer hit；
现有 hover cursor、active drag、capture、cancel 和 commit 不退化。
```

建议 owner：

```text
editor_ui_renderer::workspace_docking tests；
editor_window_winit::tests::workspace_splitter。
```

### 7.2 实现

```text
WorkspaceSplitter 明确提供 hit_rect / visual_rect；
retained Splitter widget 使用 hit_rect；
视觉 child / paint 使用 visual_rect；
hairline 经既有 DPI / pixel snapping Adapter 解析；
不得通过缩小 hit rect 伪装修复视觉。
```

### 7.3 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_docking -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_splitter -- --nocapture
cargo fmt --all -- --check
```

同一测试从 red 到 green 是 development feedback，不是重复资格。W1 不运行真实窗口、全 crate、
workspace 或 Local CI。

### 7.4 W1 完成条件

```text
red evidence 有效；
owner tests green；
原 Splitter resize / cursor tests green；
diff 只触及 S1 owner；
形成独立代码提交；
记录 W1 elapsed / target / tests；
停止，不自动进入 W2。
```

### 7.5 W1 实际结果

```text
状态：passed
窗口：2026-07-27 11:23-11:40 +08:00，约 17 分钟
代码提交：9cc9e5b1f3012a36ea82ad8c25aec26138acdac3
role root：G:\EngineTest\258-W1-20260727-112300
evidence：G:\EngineTest\258-W1-20260727-112300\evidence\w1-summary.json
red：WorkspaceSplitter 缺少 hit_rect / visual_rect，证据有效
实现：7 logical px 透明 hit_rect + 1 logical px visual_rect，共用中心线
交互：视觉层位于透明 pick target 下方，不拦截 hover / drag / capture
owner：workspace_docking 13 passed；renderer hairline 1 passed
consumer：workspace_splitter 5 passed；workspace_dpi 2 passed
format：cargo fmt --all -- --check passed
production Editor：PID 12368 未控制；binary 未替换
cleanup：cargo-target / temp / state removed；evidence retained
W1 结束时后继阶段：W2（需单独授权）
```

## 8. W2 / S2 - Panel chrome、Close Tab 与 Inspector Lock

### 8.1 红测试

先新增并失败：

```text
每个可见 Stack header 预留稳定 Tab viewport / Lock / More geometry；
Tab 数量增加时不能覆盖右侧 chrome；
Lock / More 是 WidgetTree 中稳定、可命中的真实 widget；
More 打开 anchored retained popup；
popup v1 只有 Close Tab；
closable panel 可执行，non-closable panel 显示 disabled；
外部点击、Escape、focus lost、Tab move/close 会关闭 popup；
Inspector Lock 固定 typed context anchor；
global selection 改变时 locked Inspector 不跟随；
unlock 后重新跟随 live selection；
target 消失时显示 typed unavailable，不偷偷切换 selection；
不支持 context lock 的 panel 显示 disabled Lock 与 tooltip。
```

### 8.2 实现边界

```text
Chrome 属于 Stack host，不进入业务 panel renderer；
Dock Module 只拥有 capability / context_locked presentation state；
Inspector/application 只保存 typed context anchor，不缓存整个 EditorUiModel；
EditorSession / composer 按 anchor 产生 Inspector projection；
Lock 不阻止 Tab drag、Close Tab、项目 mutation 或 selection 本身变化；
Close Tab 复用现有 ClosePanel transaction；
图标复用 Editor icon 资源；不得手写临时 SVG。
```

若现有 Inspector composer 无法按 typed anchor 生成 projection，可以新增窄 Interface；不得增加
通用 `Any`、任意 PanelModel cache 或跨 panel context router。

### 8.3 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_panel_chrome -- --nocapture
cargo test -p editor_core --lib --locked inspector_context_lock -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_panel_chrome -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_panel_visibility -- --nocapture
cargo fmt --all -- --check
```

施工时测试名称必须按新增 owner test 保持这些稳定 filter；不得用完整 crate 代替缺失的红测试。

### 8.4 W2 完成条件

```text
Chrome、popup、Close Tab 和 Inspector Lock owner tests green；
现有 panel visibility / tab behavior 无回归；
没有通用业务 model cache；
没有第二个公共 Chrome Module；
形成独立代码提交；
停止，不自动进入 W3。
```

### 8.5 W2 完成记录

```text
代码提交：1e3a979153ef0567f41d50236b2aa0d0420d1ddd
evidence：G:\EngineTest\258-W2-20260727-120000\evidence\w2-summary.json
chrome：每个 Stack 固定预留 Tab viewport / Lock / More；业务 panel renderer 未持有 chrome
popup：retained anchored popup v1 仅含 Close Tab；disabled closability fail-closed
lock：typed entity context anchor；selection 改变不跟随；目标消失显示 locked unavailable
owner：renderer 1 passed；editor_core 1 passed；window chrome 2 passed
consumer：workspace_panel_visibility 1 passed
format：cargo fmt --all -- --check passed
production Editor：PID 12368 未控制；binary 未替换
cleanup：cargo-target / temp / state removed；evidence retained
未执行：W3、真实窗口、production replacement、Local CI
下一步：停止，等待 W3 单独授权
```

## 9. W3 / S3 - WorkspaceTopology v2 与持久化迁移

### 9.1 红测试

```text
main root + zero/many floating roots；
一个 PanelId 在全 topology 最多出现一次；
floating root 至少一个有效 Stack；
空 Stack / 退化 Split / 空 floating root 在 transaction 内清理；
main root 固定 Shell 不被移出；
创建 floating、跨 root move、close floating、rehome non-closable 原子完成；
stale target 保持旧 topology；
v1 layout 迁移到 v2.main_root；
v2 roundtrip；
单个损坏 floating root 局部丢弃并诊断；
显示器缺失或窗口不可见 placement clamp；
原子保存失败不覆盖 last good layout。
```

### 9.2 实现

```text
WorkspaceTopology / WorkspaceWindowId / WorkspaceWindowRoot；
EditorWorkspaceLayout.v2；
v1 -> v2 显式 migration；
WorkspaceWindowPlan 作为目标级 projection；
winit WindowId / native handle / surface 不进入持久化；
Panel context anchor 默认不持久化；
现有 single-root 调用方通过 main root compatibility 收敛，不保留第二份 layout truth。
```

允许施工中拆分 `workspace_docking.rs` 私有实现以保持 locality，但外部状态 owner 仍只有
`EditorWorkspaceDockingModule`。

### 9.3 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_topology -- --nocapture
cargo test -p editor_ui_renderer --lib --locked workspace_layout_v2 -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_persistence -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_tab_drag -- --nocapture
cargo fmt --all -- --check
```

### 9.4 W3 完成条件

```text
topology 与 migration 红测试有效并 green；
v1 fixture 真实迁移；
single-root 现有 consumer 已收敛到 main root，不维护镜像；
持久化无项目路径、native handle 或业务 Model；
形成独立代码提交；
停止，不自动进入 W4。
```

### 9.5 W3 完成记录

```text
状态：passed
role root：G:\EngineTest\258-W3-20260727-122000
代码提交：89eeb8d5f75de81f11d7e59486c44fd3596b213c
evidence：G:\EngineTest\258-W3-20260727-122000\evidence\w3-summary.json
实现：WorkspaceTopology v2、main/floating roots、全局 PanelId 唯一性、v1 -> v2 migration、
      invalid floating root 局部丢弃、display/placement clamp、原子 save 与 last-good restore
验证：workspace_topology 2 passed；workspace_layout_v2 1 passed；
      workspace_persistence 5 passed；workspace_tab_drag 2 passed；cargo fmt passed
后继：W4 条件授权在 W3 独立通过、前置未漂移且时间充足后激活
```

## 10. W4 / S4 - NativeWorkspaceWindowHost 多窗口 composition

### 10.1 红测试与 seam

先验证目标级 reconcile：

```text
一个 WorkspaceWindowPlan 产生一个 main + N floating native window state；
重复 reconcile 幂等；
plan 删除 floating root 后只销毁对应 owned window；
每个 window 独占 renderer / surface / viewport / scale factor；
WindowId 正确映射回 WorkspaceWindowId；
CloseRequested / Resized / ScaleFactorChanged / Focused 路由到正确 root；
关闭 main 走 Editor shutdown；
关闭 floating 只执行 topology close / rehome；
任一 create/surface 失败时未提交的 root 不进入 live mapping；
所有窗口仍共享同一 NativeEditorApplication / EditorSession / Gateway。
```

测试可以使用内部 fake window factory 作为 Module 的 internal seam；不得把 factory 或逐步生命周期
提升为公共 Interface。

### 10.2 实现

```text
RealNativeEditorApp 的单一 window / renderer 状态收敛为按 WorkspaceWindowId 管理的 map；
main 与 floating 共享 application；
每窗口 input、cursor、redraw、capture、surface error 独立；
WindowPlan reconcile 在 ActiveEventLoop owner 线程执行；
真实创建成功且 surface ready 后才确认 floating live；
失败返回 typed diagnostic 并保持 source topology。
```

### 10.3 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_window_winit --lib --locked native_workspace_window_host -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_multi_window -- --nocapture
cargo check -p editor_window_winit --locked --features real-window,real-wgpu-surface
cargo check -p editor_host --locked --features real-window,real-wgpu-surface
cargo fmt --all -- --check
```

W4 只做真实 feature composition 编译与 deterministic host tests，不做用户桌面 authority。

### 10.4 W4 完成条件

```text
host reconcile 与 per-window event routing green；
real-window / real-wgpu-surface composition 编译通过；
内部 Adapter 未变成公共 Module；
没有创建第二个 Session / Gateway；
形成独立代码提交；
停止，不自动进入 W5。
```

### 10.5 W4 完成记录

```text
状态：passed
role root：G:\EngineTest\258-W4-20260727-124000
代码提交：9ccc1e06050c736a03e0c89aff1d40959cd574b9
evidence：G:\EngineTest\258-W4-20260727-124000\evidence\w4-summary.json
实现：deterministic NativeWorkspaceWindowHost reconcile / routing；
      RealNativeEditorApp main + N floating window map；
      per-window Window / renderer / surface / viewport / DPI / cursor / focus ownership；
      root-aware snapshot、frame 与 input preparation；floating close 只关闭/rehome 对应 topology；
      一个共享 NativeEditorApplication / EditorSession / Gateway
验证：native_workspace_window_host 2 passed；workspace_multi_window 1 passed；
      editor_window_winit real-window + real-wgpu-surface check passed；
      editor_host real-window + real-wgpu-surface check passed；cargo fmt passed
外部状态：未启动真实测试窗口；production Editor PID 12368 未停止、未替换、未控制
cleanup：W3/W4 的 cargo-target/temp/state 已通过 strict-child 与 reparse 安全校验，但宿主在
         执行前拒绝 literal PowerShell 递归删除，记录 retained_by_host_policy；evidence 保留
后继：停止，等待 W5 单独授权
```

## 11. W5 / S5 - Native drag proxy、floating 与 re-dock

### 11.1 红测试

```text
Docked drag 超过阈值后进入 DraggingDocked；
离开所有有效 target 后请求唯一 native proxy；
proxy 不进入 topology / persistence / business Model；
screen-space pointer 在 main / floating roots 间解析唯一 target；
preview 与 commit 消费同一 resolved target token；
无 target commit 创建 floating root；
floating -> main、main -> floating、floating -> floating 可原子提交；
cancel / focus lost / capture lost 销毁 proxy 并保持 source；
stale target 不丢 panel；
最后 Tab 移出后销毁空 floating window；
proxy create/move/destroy 失败能 fail closed。
```

### 11.2 实现

```text
复用现有 Begin / Update / Commit / Cancel panel drag intent；
只增加跨 window facts、resolved target token 与 CommitFloating 语义；
native proxy 是 NativeWorkspaceWindowHost 的瞬时内部状态；
不得新增 AI-facing create_window / attach_surface / move_tab 顺序；
屏幕坐标和每窗口逻辑坐标只通过既有 DPI Adapter 转换；
drop transaction 仍由 Dock Module 原子拥有。
```

### 11.3 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_ui_renderer --lib --locked workspace_cross_window_drag -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_native_drag -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_tab_drag -- --nocapture
cargo test -p editor_window_winit --lib --locked workspace_dpi -- --nocapture
cargo check -p editor_window_winit --locked --features real-window,real-wgpu-surface
cargo fmt --all -- --check
```

### 11.4 W5 完成条件

```text
全部方向 drag 与 failure tests green；
proxy 生命周期没有泄漏；
topology / persistence 不含 proxy；
单窗口 Tab drag 行为无回归；
形成独立代码提交；
停止，不自动进入 W6。
```

## 12. W6 / S6 - 受影响域、真实 Windows 与 production 验收

W6 是 release/activation 级窗口，必须单独授权。开始前必须确认能够在剩余三小时内完成真实
evidence、必要 production replacement 和 cleanup；否则不得开始。

### 12.1 受影响闭包

默认：

```text
editor_ui_model（仅实际修改时）；
editor_core（仅实际修改时）；
editor_ui_renderer；
editor_window_winit；
editor_wgpu_renderer（仅实际修改时）；
editor_host production composition。
```

不得因为多窗口 Editor 改动运行 Runtime/AUI/Build/Export/AI Gateway 全量验收。

### 12.2 代码质量与 crate 回归

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all -- --check
cargo test -p editor_ui_renderer --locked
cargo test -p editor_window_winit --locked
cargo test -p editor_core --locked inspector_context_lock -- --nocapture
cargo check -p editor_window_winit --locked --features real-window,real-wgpu-surface
cargo check -p editor_host --locked --features real-window,real-wgpu-surface
cargo clippy -p editor_ui_renderer -p editor_window_winit --all-targets --all-features `
  --no-deps --locked -- -D warnings
```

若 `editor_ui_model`、`editor_core`、`editor_wgpu_renderer` 有实际产品 diff，分别加入对应 crate 的
定向测试和 `--no-deps -D warnings`，不机械运行无改动 crate。

同配置下，完整 crate suite 已包含的 S1-S5 filtered tests 不再重复单独运行。

### 12.3 真实 Windows authority

窄扩展现有：

```text
editor_window_winit::editor_ui_authority
```

禁止新建 258 长期 Runner。authority 必须使用 test-owned project fixture、layout store、TEMP/TMP、
evidence root、进程和窗口，不得控制用户当前打开的 Editor。

至少产生三个独立 scenario：

```text
258-splitter-panel-chrome：
  hairline actual RGBA；
  resize cursor；
  Lock / More / Close Tab；

258-main-to-floating：
  真实 PointerDown / Move / Up；
  native proxy observed；
  floating native window created；
  source panel 唯一；
  per-window surface / DPI / screenshot；

258-floating-redock-close：
  floating -> main re-dock；
  empty floating cleanup；
  floating close / non-closable rehome；
  v2 persistence save / reopen；
  topology invariant。
```

authority 输出必须扩展为结构化 258 evidence，至少包含：

```text
source commit；
binary hash；
scenario id；
main/floating window count；
WorkspaceWindowId <-> native WindowId lineage；
surface / scale factor / screen rect；
drag proxy created/destroyed；
resolved target token；
layout revision before/after；
panel uniqueness；
actual RGBA path / sha256；
diagnostics；
owned process cleanup。
```

激活 W6 后，必须按最终 authority Interface 把下面骨架替换为无占位符的精确命令，再执行：

```powershell
Set-Location G:\gameEngin\rust
cargo run -p editor_window_winit --bin editor_ui_authority `
  --features real-window,real-wgpu-surface --locked -- `
  <explicit test-owned 258 scenario arguments>
```

只要仍含 `<...>` 就禁止执行 W6。真实桌面/GPU不可用时记录 `environment_blocked`，不得用 headless
证据冒充通过，也不得擅自改变系统 DPI、显示器或外部配置。

### 12.4 Production binary

只有用户在 W6 明确授权 production replacement 时：

```text
精确读取当前 Editor PID / executable path / command line；
只允许停止已由用户授权且身份完全匹配的 Editor PID；
先停止旧进程，再覆盖对应 EXE；
build -p editor_host --locked --features real-window,real-wgpu-surface；
记录 commit / EXE sha256 / LastWriteTime / feature composition；
以 --real-window 启动新进程；
确认新 PID、路径、参数和持续存活；
用户实际验证细 Splitter、Chrome、浮动与 re-dock。
```

若没有 production replacement 授权，只能形成代码/测试结论，258 不得标记最终完成。

### 12.5 Local CI

258 不默认要求 Local CI。只有以下任一条件成立且用户明确授权时才运行：

```text
实际修改 release-wide 共享质量合同；
跨出声明的受影响闭包；
用户要求 exact-commit Local CI；
最终 completion claim 明确绑定 Local CI。
```

一次明确 Local CI 授权覆盖同一连续 W6 内的初次 run 和修复后的 fresh-root rerun；Local CI
不能作为调试循环，且总时间仍计入三小时。

### 12.6 W6 完成条件

```text
受影响 crate 回归与 scoped Clippy 通过；
真实 Windows 三个 scenario 通过；
所有 test-owned window/process/state cleanup 完成；
production binary 已在明确授权下替换、seal、reopen；
用户可见行为完成验收；
工作树 clean；
生成独立 258 完成记录；
同步入口；
施工文档移动到 已完成/；
257 完成态未变化。
```

## 13. Failure 收敛

任何窗口失败后：

```text
停止后续 Gate；
保留首个 failing assertion / stderr / structured report / environment facts；
记录 product source、test harness、fixture、binary、external state 的独立 identity；
分类首因；
先建立最窄 owner reproduction；
修复后只更新被修改失效的证据；
外部 one-shot 行为不得隐式重试；
不得通过增加永久公共流程掩盖 owner bug。
```

方案问题：

```text
停止施工；
修订 258 正式方案；
修订本施工文档并重新自审；
获得后续窗口授权。
```

施工文档问题：

```text
修正文档命令、owner、范围或完成条件；
不改产品代码来迎合错误 Gate。
```

既有 baseline lint 或无关失败：

```text
记录具体文件、首个 diagnostic 和不相关理由；
使用 scoped --no-deps 证明当前 owner；
不得顺手清理无关 lint；
若它实际阻塞生产 composition，再单独讨论 remediation。
```

## 14. 外部状态与 cleanup

```text
所有测试窗口、进程、state、TEMP/TMP 和 evidence root 必须有 owner；
真实测试不得触碰用户窗口；
超时或异常必须终止 owned descendants；
先保存必要 evidence，再 cleanup；
role-owned cargo target / temp 可以按公共施工 skill 的 strict-child + reparse + process proof 递归删除；
不得删除 workspace target、用户布局目录、home、drive root 或共享 artifact root；
cleanup 失败记录 retained_by_host_policy。
```

每个窗口结束时必须报告：

```text
elapsed；
source commit；
修改文件；
测试与结果；
失败和 rerun；
evidence root；
owned processes；
cleanup result；
剩余风险；
下一授权窗口。
```

## 15. 文档同步与归档

只有 W6 全部完成后：

```text
生成：
  阶段完成记录/2026-<date>-Native-Editor-Floating-Workspace-Panel-Chrome-v1/00-总览.md

同步：
  README.md
  00-文档地图.md
  00-阅读顺序.md
  49-当前状态与下一步入口.md
  54-当前可自动化施工总入口.md
  施工文档/README.md
  阶段完成记录/README.md

移动：
  施工文档/当前/258-当前可自动化施工文档-Native-Editor-Floating-Workspace-Panel-Chrome-v1.md
  -> 施工文档/已完成/
```

未通过真实 Windows 或未完成明确授权下的 production replacement 时，不得生成“258 已完成”结论。

## 16. 自审

### 16.1 正式方案一致性

```text
[x] 读取并以 ee38057 的 258 正式方案为唯一设计真相
[x] 方案 B：深化现有 EditorWorkspaceDockingModule
[x] NativeWorkspaceWindowHost 保持内部 Adapter
[x] 包含 Splitter、Panel chrome、Inspector Lock、floating/re-dock、v2 persistence
[x] 不修改项目 AUI、Runtime、Build 或 AI Tool Interface
[x] 257 保持完成态，不复用施工 lineage
[x] 当前无需修改 258 正式方案
```

### 16.2 深 Module 与工具外自由

```text
[x] 一个公共状态 owner
[x] 调用方消费目标级 WorkspaceWindowPlan
[x] 未公开 create/attach/detach/destroy 生命周期顺序
[x] 业务 panel 不实现 Dock 或 OS Window
[x] AI 不需要执行固定窗口菜谱
[x] S1-S6 只是施工内部切片
```

### 16.3 测试纪律

```text
[x] 每个需求有 red-capable owner test
[x] 前五个窗口不使用真实 production / Local CI 作调试循环
[x] W6 才运行真实 Windows 和 production composition
[x] 完整 crate suite 后不重复相同 filtered tests
[x] Local CI 条件化，不是默认完成门
[x] failure evidence 要求首因与独立 identity
```

### 16.4 时间与授权

```text
[x] 六个窗口分别授权
[x] 每个窗口最长三小时
[x] 2h45 停止启动新工作
[x] W1-W5 完成后立即停止，不自动越窗
[x] W6 production replacement 需要明确外部状态授权
[x] 激活前复核已通过，当前文档位于 当前/
[x] W1 已单独授权、完成并结束授权窗口
[x] W2 已单独授权、完成并结束授权窗口
[x] W3 已单独授权、完成并结束授权窗口
[x] W4 已由 W3 条件授权激活、完成并结束授权窗口
[x] W5 已单独授权、完成并独立冻结
[x] W6 已连续授权、完成真实 Windows authority 与 production replacement
```

### 16.5 自审结论

```text
结论：通过
正式方案：无需修改
施工范围：未扩大
公共 Module：未增加
测试：按 claim 分层，无默认全 workspace / Local CI 重复
施工风险：多窗口 host 与 native drag 风险最高，已分置 W4 / W5
当前状态：W6 passed，258 完成并归档
```
