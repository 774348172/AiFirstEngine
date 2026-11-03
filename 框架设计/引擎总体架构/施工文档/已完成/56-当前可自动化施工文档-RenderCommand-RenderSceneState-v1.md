# 当前可自动化施工文档：RenderCommand / RenderSceneState v1

> 状态提示：RenderCommand / RenderSceneState v1 CPU-side sync 已完成。本文档保留为已完成施工计划和复盘依据，不再作为当前开工入口；当前入口以 `49-当前状态与下一步入口.md` 和 `54-当前可自动化施工总入口.md` 为准。

目标不是做真实 renderer。  
目标是先完成 Game / ECS 到 Render 侧长期状态的 CPU 数据闭环。

依据：

```text
17-Runtime-FrameLoop.md
39-Rust-Runtime-ECS路线.md
49-当前状态与下一步入口.md
50-RenderCommand-RenderSceneState方案.md
51-RenderDirtyTracker-RenderExtract-RenderCommand闭环方案.md
52-ECS-Storage-v1方案.md
55-RenderSceneState-RenderProxy-v1方案.md
```

## 1. 第一版施工边界

本轮只做：

```text
RenderProxy
RenderSceneState
RenderViewState
RenderFrameViewData
RenderCommand
RenderCommandQueue
RenderCommandDiagnostic
RenderFrameReport
RenderExtract from DirtyRecord
CPU-side tests
```

本轮不做：

```text
真实 Render Thread
真实 RHI / RDG
真实 GPU resource
真实 draw command
真实 culling
真实 Material / Shader / Pipeline
真实多 View 渲染
完整 Job System
```

一句话边界：

```text
DirtyRecord -> RenderCommand -> RenderSceneState -> RenderFrameReport
```

## 2. 核心结构

第一版结构：

```text
RenderSceneState
  proxies: Map<RenderProxyId, RenderProxy>
  entity_to_proxy: Map<RuntimeEntityId, RenderProxyId>
  source_to_proxy: Map<SourceEntityId, RenderProxyId>
  view_registry: Map<RenderViewId, RenderViewState>
```

```text
RenderProxy
  common: RenderProxyCommon
  payload: RenderProxyPayload
```

```text
RenderProxyPayload
  Mesh
  Sprite
  SkinnedMesh
  Light
  Camera
  Particle
  Instance
```

```text
RenderViewState
  view_id
  source_entity_id optional
  view_kind: Game / SceneView / Preview / Shadow / Reflection
  viewport
  target
  view_matrix
  projection_matrix
  clear_color
  layer_mask optional
  version
```

```text
RenderFrameViewData
  view_id
  visible_proxy_ids
  culling_result_summary
  render_phase_summary
  diagnostics
```

## 3. RenderCommand v1

第一版只保留 6 类命令：

```text
AddProxy
RemoveProxy
UpdateRenderState
UpdateTransform
UpdateDynamicData
UpdateInstanceData
```

RenderCommand 外层固定：

```text
RenderCommand
  command_id
  frame_index
  source_entity_id
  runtime_entity_id
  proxy_id optional
  command_type
  payload_kind
  payload
  sort_key
  source_dirty_type
  trace_id optional
```

运行时禁止用 JSON / Map 作为正式 payload。  
Debug / Evidence 可以导出 JSON 视图，但它只是报告格式。

## 4. Queue / Normalize / Merge

第一版流程：

```text
RenderExtract workers
  -> ThreadLocalCommandBuffer
  -> RenderCommandQueue.collect()
  -> stable_sort()
  -> normalize()
  -> merge()
  -> RenderSceneState.apply_batch(commands)
  -> diagnostics
  -> RenderFrameReport summary
```

第一版先允许单线程调用，但结构必须按多生产者单消费者设计：

```text
worker_count=1 只是测试配置。
不能绕过 ThreadLocalCommandBuffer / RenderCommandQueue。
```

ObjectCommandSlot：

```text
ObjectCommandSlot
  runtime_entity_id
  proxy_id optional
  existed_at_frame_start
  lifecycle: None / Add / Remove / Recreate / NoOp
  render_state_payload optional
  transform_payload optional
  dynamic_data_payload optional
  instance_data_payload optional
  diagnostics[]
```

合并规则：

```text
同一 proxy/entity 的命令进入同一个 ObjectCommandSlot。
生命周期 AddProxy / RemoveProxy 优先级最高。
普通 Update 只在最终对象存在时生效。
同类 payload last value wins。
不同类 payload 可以并存。
Update missing proxy 不自动创建，记录 diagnostic。
Remove missing proxy 不崩溃，记录 diagnostic。
Add existing proxy 同 payload kind 可降级为 UpdateRenderState，不同 kind 记录 conflict diagnostic。
```

## 5. 多 View 最小边界

第一版必须实现数据边界，不做真实多 View 渲染：

```text
RenderSceneState = 场景里有什么。
RenderViewState = 从哪里看、怎么看、渲染到哪里。
RenderFrameViewData = 这一帧这个视图看到了什么。
```

规则：

```text
RenderSceneState 保存最小 view_registry。
visible_proxy_ids / culling result / render phase 属于 RenderFrameViewData。
RenderFrameViewData 每帧生成，可复用内存，但不能作为项目逻辑输入。
Editor Scene View 不能直接修改 Game Camera 的 RenderViewState。
Preview / Shadow / Reflection 都是 View，不是独立 Scene。
```

第一版测试只需要证明：

```text
Game View 和 Scene View 可以同时存在。
它们共享同一个 RenderProxy 集合。
它们拥有独立 RenderViewState。
FrameViewData 可以按 view_id 分开记录。
```

## 6. Diagnostics / RenderFrameReport

RenderCommandDiagnostic 最小字段：

```text
diagnostic_id
frame_index
severity: Info / Warning / Error
code
stage: Collect / Sort / Normalize / Merge / Apply
runtime_entity_id optional
source_entity_id optional
proxy_id optional
command_id optional
command_type optional
payload_kind optional
result: Applied / Merged / Covered / Skipped / Failed
reason_code
trace_id optional
```

RenderFrameReport 最小字段：

```text
frame_index
report_level
counters
changed_entities
render_events
trace_refs
```

第一版 reason_code：

```text
missing_proxy
missing_resource
payload_kind_conflict
update_after_remove
add_existing_proxy
remove_missing_proxy
covered_by_remove
covered_by_noop
merged_last_value_wins
invalid_payload
apply_failed
fallback_used
```

Release 默认不生成完整 RenderFrameReport。  
Evidence 模式才允许展开 payload、from/to、source map、ObjectCommandSlot。

## 7. 自动化施工步骤

### A0：回归门禁

目标：

```text
确认当前 Rust Runtime 基线通过。
```

命令：

```powershell
cd G:\gameEngin\rust
cargo test -p engine_runtime
cargo test
```

通过标准：

```text
engine_runtime 测试通过。
workspace 测试通过。
```

### A1：新增 render_state 模块骨架

目标：

```text
新增 engine_runtime::render_state。
定义 RenderProxyId / RenderViewId / RenderProxy / RenderSceneState / RenderViewState / RenderFrameViewData。
不接入 World。
```

最小测试：

```text
create_render_scene_state_with_two_views
add_proxy_and_find_by_source_entity
```

### A2：新增 render_command 模块骨架

目标：

```text
定义 RenderCommand / RenderCommandType / RenderCommandPayload / RenderCommandQueue。
定义 sort_key。
实现 collect + stable_sort。
```

最小测试：

```text
sort_commands_deterministically_by_sort_key
thread_local_buffers_collect_in_stable_order
```

### A3：实现 apply_batch 基础命令

目标：

```text
RenderSceneState.apply_batch 支持 AddProxy / RemoveProxy / UpdateTransform / UpdateRenderState。
暂不做 DynamicData / InstanceData 的真实复杂字段，只保留结构入口。
```

最小测试：

```text
add_proxy_creates_mappings
remove_proxy_removes_mappings
update_transform_changes_common_transform_only
update_render_state_changes_visibility_only
```

### A4：实现 normalize / merge

目标：

```text
实现 ObjectCommandSlot。
同 proxy/entity 命令聚合。
同类 update last value wins。
生命周期命令按规则生成 Add / Remove / Recreate / NoOp。
```

最小测试：

```text
multiple_transform_updates_keep_last_value
add_then_update_merges_into_add
add_then_remove_becomes_noop
update_missing_proxy_reports_diagnostic
remove_then_add_recreates_proxy
```

### A5：实现 diagnostics / RenderFrameReport

目标：

```text
RenderCommandDiagnostic 记录命令级最小证据。
RenderFrameReport 汇总 counters / changed_entities / render_events。
```

最小测试：

```text
missing_proxy_generates_warning_event
merged_commands_increment_covered_or_merged_counter
release_report_level_does_not_store_payload_dump
```

### A6：实现 RenderExtract from DirtyRecord

目标：

```text
从 World.take_dirty_records() 读取 dirty。
读取 Transform / Renderable。
生成 AddProxy / UpdateTransform / UpdateRenderState / RemoveProxy 的最小命令。
```

最小规则：

```text
实体有 Transform + Renderable，且没有 proxy -> AddProxy。
Transform dirty 且已有 proxy -> UpdateTransform。
RenderState dirty 且已有 Renderable -> UpdateRenderState。
RenderState dirty 且 Renderable 缺失 -> RemoveProxy。
```

最小测试：

```text
spawn_renderable_entity_extracts_add_proxy
transform_write_extracts_update_transform
remove_renderable_extracts_remove_proxy
dirty_records_are_consumed_once
```

### A7：多 View 数据边界测试

目标：

```text
RenderSceneState 注册 Game View 和 Scene View。
RenderFrameViewData 按 view_id 生成。
两个 view 共享 proxies，但 view state 相互独立。
```

最小测试：

```text
game_and_scene_view_share_proxies_but_not_view_state
frame_view_data_is_per_view_and_not_stored_as_scene_truth
```

### A8：兼容旧 RenderSnapshot

目标：

```text
旧 render_snapshot compatibility output 继续可用。
新 RenderSceneState 不依赖旧 RenderSnapshot。
```

最小测试：

```text
legacy_render_snapshot_still_outputs_renderables
render_scene_state_pipeline_does_not_require_snapshot
```

### A9：Golden Scenario 回归

目标：

```text
已有 Golden Scenario 不退化。
新增一个 RenderCommandSceneState Golden Scenario。
```

最小测试：

```text
golden_render_command_scene_state_add_update_remove
```

## 8. 完成标准

本轮完成必须满足：

```text
cargo test -p engine_runtime 通过。
cargo test 通过。
RenderCommand / RenderSceneState v1 有独立单元测试。
DirtyRecord -> RenderCommand -> RenderSceneState -> RenderFrameReport 跑通。
旧 Runtime Package / Scene Loader / RenderSnapshot compatibility 不退化。
多 View 边界有最小测试，但没有伪装成真实多 View renderer。
```

## 9. 禁止项

禁止：

```text
为了跑通而直接从 World 生成 RenderSceneState，绕过 RenderCommandQueue。
让项目逻辑或 AI 直接写 RenderProxy。
把 RenderSnapshot 重新作为正式同步模型。
在 v1 引入真实 RHI / RDG。
在 v1 引入完整 RenderWorld ECS。
在 release 报告里默认保存完整 payload dump。
```

## 10. 下一阶段

本轮完成后再讨论：

```text
Minimal Job System 是否进入施工。
真实 Render Thread 消费 RenderCommandQueue 的边界。
RendererFeatureBuilder 如何从 RenderSceneState 构建 RDG 输入。
Native Editor Host Viewport 如何读取 RenderFrameReport / RenderSceneState 摘要。
```
