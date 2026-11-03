# 275 当前可自动化施工文档：AUI Image Cooked Texture / GPU Present v1

> 状态：Gate A-F 已完成，待归档。
> 用户授权：施工引擎侧 AUI Image cooked texture resolve、GPU upload、textured UI batch、alpha present 与缺失纹理诊断；完成后用新 fresh root 仅重跑 Gate E。
> 禁止：Local CI、production/安装态 binary replacement、真实配置修改、Tower Gate A-D/F/G/H。

## Gate A：Owner Contract 与 red-capable tests

涉及：`engine_runtime/runtime_texture.rs`、`runtime_renderer.rs`、`render_thread.rs`。

任务：定义通用 runtime texture upload/binding context、稳定 handle、UI image batch/report/diagnostic；先补能捕获当前 Image 被跳过的测试。

验证：

```powershell
cd G:\gameEngin\rust
cargo test -p engine_runtime runtime_texture
cargo test -p engine_runtime runtime_renderer
```

## Gate B：Resolve、Batch 与 Alpha Present

涉及：`engine_runtime/runtime_texture.rs`、`runtime_renderer.rs`、`rhi_command_plan.rs`、`wgpu_backend.rs`。

任务：从 RuntimeAssetIndex 加载 cooked RGBA8；按 painter order 生成 solid/image/font 相邻 batch；正确裁剪 UV；上传并用现有 ALPHA_BLENDING pipeline present；缺失纹理结构化诊断。

验证：

```powershell
cd G:\gameEngin\rust
cargo test -p engine_runtime runtime_texture
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime wgpu_backend
```

## Gate C：Editor GameView Consumer

涉及：`editor_core/editor_gameview_play.rs`、`session.rs`、`editor_window_winit/application.rs`、`real_window.rs`。

任务：当前 RuntimePackage texture registry 进入真实共享 GPU present；上传失败与 backend 缺失 binding 回传 GameView diagnostics。

验证：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core editor_gameview_play
cargo test -p editor_window_winit project_editor_composition_production_gate
```

## Gate D：Windowed Player Consumer 与受影响回归

涉及：`runtime_player_winit/src/lib.rs` 及受影响 public contract consumer。

任务：使用同一 registry/handle 规则上传 sprite 与 AUI texture；删除重复的 sprite-only handle 分配逻辑。

验证：

```powershell
cd G:\gameEngin\rust
cargo test -p runtime_player_winit aui
cargo test -p runtime_player_winit texture
cargo test -p engine_runtime
```

上层 suite 已覆盖同配置下的定向测试时记录复用，不机械重复。

## Gate E：Tower Fresh Gate E Only

使用新的 repository-external fresh root。仅运行 Tower active UI-V1 文档 Gate E 的 exact builder、headless qualification、1080x1920 authority、720x1280 authority 与 20 张真实 PNG 人工视觉检查。不得复用旧 root，不得运行 Gate A-D/F/G/H 或 Local CI。

通过条件：AUI Image 背景/格框/卷轴/竹简/印章真实可见，RGBA alpha 正确，authority assertions 全绿，缺失纹理为 0；写入新的 `gate-e-final-report.json`。

### Gate E owner repair

`G:\AifeTowerRuns\ui-v1-gate-e-20260807-011300` 的 1080 authority 已证明 GPU residency 修复后旧 `round-5-settled` 通过，但 `round-8-settled` 在 runtime frame 5275 超时；后期约 4.7 fps。首因是既有 event loop 每次较慢 present 后执行 `next = now + 16ms`，导致 Runtime fixed tick 被 GPU present 帧率限速。

修复限定在 `editor_window_winit/real_window.rs` 的通用 GameView scheduler：16ms deadline 累进、单轮最多 8 tick catch-up、只 present 最新帧、inactive 时重置。先补纯 owner red test，再跑真实 WGPU shared texture consumer；不得增加 scenario timeout。该失败 root 永久保留，修复后必须使用另一个 fresh root。

## Gate F：文档闭环

275 生成阶段完成记录并归档；同步 `49`、`54`、施工 README、阶段完成记录 README。Tower 文档只在其 Gate E 全部通过后按自身规则归档。

## 自审

- 范围未扩大：复用 228/210，未新增项目专用分支或第二套资源系统。
- 验证可红：覆盖当前首因、两纹理分批、同纹理合批、UV clip、alpha readback、缺失 resolve/upload/binding、Rect/Text 不回归。
- 授权一致：不运行 Local CI、不替换 production/安装态二进制、不改真实配置，最终只重跑 Gate E。
