# FrameLoop / Trace / Snapshot / Hash

## 功能目标

实现 Rust Runtime 第一版单线程帧循环和可验证输出：

```text
FrameLoop
RuntimeTrace
RenderSnapshot
FrameHash
```

## 实现文件

```text
rust/crates/engine_runtime/src/frame_loop.rs
rust/crates/engine_runtime/src/runtime_trace.rs
rust/crates/engine_runtime/src/render_snapshot.rs
rust/crates/engine_runtime/src/frame_hash.rs
```

## 关键实现

FrameLoop 第一版阶段：

```text
FrameBegin
FixedUpdate
Update
LateUpdate
RenderExtract
FrameEnd
```

RenderSnapshot 输出：

```text
sceneId
frame
renderables[]
  entityId
  localPosition
  localRotation
  localScale
  meshRef
  materialRef
  visible
  layer
```

FrameHash：

```text
基于 sceneId / frame / entity id / transform / renderable refs / visible / layer。
使用稳定排序。
不包含时间戳。
不包含随机数。
```

## 最小测试用例

命令：

```powershell
npm.cmd run test:rustruntime
```

覆盖测试：

```text
render_extract_maps_mesh_asset_ref_to_mesh_ref
fixed_tick_produces_stable_frame_hash
runtime_trace_records_frame_phases
```

本轮结果：

```text
RenderSnapshot 能把 mesh.assetRef 映射为 meshRef。
相同输入能产生稳定 FrameHash。
RuntimeTrace 能记录帧阶段。
测试通过。
```

## 边界

```text
不执行项目玩法规则。
不接 IR Interpreter。
不接真实 renderer。
RenderSnapshot 只读 World，不修改 ECS。
```
