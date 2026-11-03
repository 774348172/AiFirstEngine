# 116-当前可自动化施工文档-真实 WgpuBackend 完整 v1

## 目标

把 Runtime WGPU 后端从 unavailable skeleton 升级成完整 v1 backend 形态：

```text
RenderGraph
  -> RhiCommandPlan v2
  -> EngineRhiBackend
  -> WgpuBackend
```

## 施工步骤

### A1：RhiCommandPlan v2

```text
新增 RhiDrawPayload：
  TestGeometry
  MeshBasic
  SpriteBasic
  UiOverlay

Draw command 保留外层统一 schema：
  target
  draw_kind
  vertex_count
  payload
```

验收：

```text
MeshBasic payload 保留 mesh_ref / material_ref。
SpriteBasic payload 保留 sprite_ref / material_ref / sort_key。
UiOverlay payload 保留 item_count / text_count / image_count。
```

### A2：EngineRhiBackend trait 承载 payload

```text
EngineRhiDrawCall 新增 payload。
execute_plan 默认编排继续存在。
HeadlessRhiBackend 用 payload 参与 deterministic hash。
```

验收：

```text
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime rhi_command_plan
```

### A3：WgpuBackend 完整 v1 默认结构

```text
默认无 real-wgpu feature：
  WgpuBackend 保留同一 trait
  report 清晰标记 backend_unavailable

real-wgpu feature：
  device / queue / target / pipeline cache / upload counters
  offscreen texture target smoke
  clear / mesh / sprite / ui draw command execution path
```

验收：

```text
cargo test -p engine_runtime
cargo check -p engine_runtime --features real-wgpu
```

### A4：RuntimeRenderer 接入

```text
RuntimeRenderer 不直接接触 wgpu。
Sprite / Mesh / UI 只通过 RenderGraph -> RhiCommandPlan -> EngineRhiBackend。
```

验收：

```text
Sprite / Mesh / UI payload 不丢失。
Headless backend draw_count 正确。
RhiBackendReport 可序列化。
```

### A5：回归

```powershell
cargo test -p engine_runtime
cargo test -p runtime_cli
cargo check -p editor_host --features real-window
cargo check -p engine_runtime --features real-wgpu
```

## 完成记录

施工完成后移动到：

```text
施工文档/已完成/116-当前可自动化施工文档-真实WgpuBackend完整v1.md
阶段完成记录/2026-06-30-真实WgpuBackend完整v1/00-总览.md
```
