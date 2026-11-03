# 289 当前可自动化施工文档：Editor GameView Hover Three-Frame White-Flash Minimal Repair v1

> 状态：已完成，归档至 `施工文档/已完成/`
> 日期：2026-08-14
> 上游：265 Stable GameView Publication、284 Normal-Frame Immediate AUI Input、286-R1 Texture Last-Good、288 Tower Button No-Brightness-Pulse
> 授权：引擎侧普通 Editor GameView hover 三帧捕获、最小诊断与修复；允许修改引擎源码和定向测试

## 目标

在不替换 production Editor 的前提下，用普通 Editor GameView 的真实 WGPU 链路捕获
`hover 前 -> hover 命中 -> hover 后稳定` 三帧，定位短暂白帧的唯一 owner，并实施最小修复。

## 必要性审计

```text
confirmed failure:
- Tower 已消除 Button hover/pressed/activated brightness，cooked AUI 与 Preview cache 也已确认为新产物；
- 用户在普通 production Editor Play 中仍观察到鼠标划过时短暂闪白。

minimum causal fix:
- 先捕获三帧并比较 AUI presentation identity、publication receipt/index、viewport texture
  generation/residency 与 RGBA 亮度；
- 证据命中 AUI-only publication、viewport texture binding/residency 或 Editor surface sample 中的
  一个 owner 后，只修该 owner。

files that may change:
- rust/crates/editor_window_winit/src/real_window.rs
- rust/crates/editor_window_winit/src/editor_frame_publication.rs
- rust/crates/editor_window_winit/src/tests/* 中与捕获/回归直接相关的文件
- 若三帧证据直接命中采样 owner，才允许修改
  rust/crates/editor_wgpu_renderer 中唯一对应文件

tests that can fail before and pass after:
- 真实 WGPU 三帧定向捕获：hover 帧不得出现白色 fallback 或显著全局亮度跃变；
- 命中 owner 的最小 deterministic regression test。

explicitly deferred nearby enhancements:
- 不新增通用 schema、migration、report family 或 capture service；
- 不重构 AUI renderer、publication 和 Editor UI renderer 整条链路；
- 不修改 Tower 项目源码或真实配置；
- 不替换 production/installed Editor、Player 或 MCP；
- 不重建或删除 Tower 正式 Preview cache；
- 不运行 Local CI 或完整视觉矩阵。
```

## Gate A：Hover 三帧红测试与定位

1. 在 run-owned 根目录运行普通 Editor 真实 WGPU 路径，不消费真实配置。
2. 捕获 hover 前、PointerMove 命中后首帧、后续稳定帧。
3. 记录每帧 RGBA SHA-256/亮度、AUI identity、Runtime frame identity、publication
   receipt/status/index、viewport texture generation/residency 和诊断。
4. 保留可捕获用户症状的失败断言，将首因归类到唯一 owner。

## Gate B：单 owner 最小修复与回归

1. 只修改 Gate A 证据命中的 owner。
2. 先运行三帧红测试证明由红转绿，再运行该 owner 和直接 consumer 的定向测试。
3. 清理临时诊断代码；保留最小 regression test。
4. 写完成记录、同步入口并归档本文档。

## 验证边界

```powershell
cd G:\gameEngin\rust
cargo fmt --check -p editor_window_winit -p editor_wgpu_renderer
cargo test -p editor_window_winit <targeted-filter>
cargo test -p editor_wgpu_renderer <targeted-filter> # 仅在命中该 owner 时
```

不运行 Local CI；不替换 production Editor；不修改真实配置；不重建 Tower 正式 Preview cache。

## 激活前复核与自审

- `54` 显示当前无施工、待执行队列为空；289 不与其它施工冲突。
- 用户已明确授权引擎源码和定向测试，同时明确禁止 Local CI、production 替换和真实配置修改。
- 修复以真实三帧证据为前置；不先验假设，不新增公共架构。
- 当前 worktree 存在大量既有用户修改；本轮只在局部 owner 上叠加，不回退任何既有更改。

自审结论：两个 Gate 与已确认故障直接对应，没有过量施工；允许激活并执行。

## 施工过程复盘与 Skill 回填

- 本次真实 burst 已足以确认用户症状；根因随后由 owner/consumer deterministic test 收敛到首次
  PointerMove 错误保留 keyboard focus-visible，而不是 renderer、纹理 residency 或 publication owner。
- 基础 `editor_window_winit` test binary 不含 Tower RuntimeModule composition。发现这一事实后，临时
  project-linked source candidate runner 已不再具备最小诊断价值，不应继续扩展到 candidate、qualification、
  seal、promotion 或 production replacement。
- 冷编译耗时与测试执行耗时必须分开记录；编译等待或工具 timeout 在没有 test verdict 时不算产品失败。
- 该经验已回填到 `.agents/skills/game-engine-construction/SKILL.md` 的
  `Keep Small Diagnostics Small`，作为后续小型诊断的默认止损规则。

## Gate 完成结果

- Gate A：production Editor burst 30 帧中 13 帧命中白块，对照 burst 40/40 帧正常；
  异常只在 GameView 内 PointerMove 后出现。
- 唯一根因：首次 PointerMove 先应用 default focus，却保留 `Keyboard` focus reason，
  使 `focus_visible` 引入额外 brightness override 和 AUI-only publication。
- Gate B：hover-capable PointerMove 现在保留 logical focus，但将表现模态切换为
  `Pointer`；键盘/手柄 submit、PointerDown、同帧 AUI feedback/action 保持不变。
- owner 与直接 consumer 回归、受影响 crate 格式检查全部通过；289 临时诊断标签零命中。
- 未运行 Local CI，未替换 production Editor，未修改真实配置或 Tower Preview cache。
