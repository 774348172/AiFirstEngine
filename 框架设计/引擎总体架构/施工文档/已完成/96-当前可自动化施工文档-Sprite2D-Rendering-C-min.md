# 96-当前可自动化施工文档-Sprite2D Rendering C-min

对应正式方案：

```text
../96-Sprite2D-Rendering-C-min方案.md
```

## 1. 施工目标

实现 Sprite2D 第一版 CPU/headless 数据闭环：

```text
SpriteRenderer2D / SpritePayload
  -> RenderSceneState
  -> RendererFeatureDrawItem
  -> deterministic draw item sort_key
  -> MinimalRenderer / report 可读取
```

本轮不做：

```text
真实 GPU Sprite Batch
完整透明深度排序
摄像机距离排序
Runtime UI / Canvas 排序
材质系统重构
RDG / RHI pass 扩展
```

## 2. 当前代码入口

优先改这些 Rust 模块：

```text
rust/crates/engine_runtime/src/render_state.rs
rust/crates/engine_runtime/src/renderer_feature_builder.rs
rust/crates/engine_runtime/src/minimal_renderer.rs
rust/crates/engine_runtime/src/runtime_renderer.rs
```

如需要暴露通用组件，可补：

```text
rust/crates/engine_runtime/src/components.rs
```

但不要为了打飞机项目新增 Bullet / Enemy / Player 等玩法语义。

## 3. 施工步骤

### A1. 扩展 SpritePayload 字段

在 `render_state.rs` 中扩展：

```text
SpritePayload
  sprite_ref: Option<String>
  material_ref: Option<String>
  color: [f32; 4]
  flip_x: bool
  flip_y: bool
  sorting_layer: i16
  order_in_layer: i32
  sort_z: f32
```

补默认构造 helper：

```text
SpritePayload::new(sprite_ref)
SpritePayload::with_material(...)
```

如果代码风格不适合 helper，至少提供测试内清晰构造函数。

最小测试：

```text
sprite_payload_defaults_are_stable
sprite_payload_keeps_authoring_fields
```

### A2. 扩展 RendererFeatureDrawItem

在 `renderer_feature_builder.rs` 中增加字段：

```text
sprite_ref: Option<String>
sorting_layer: i16
order_in_layer: i32
sort_z: f32
sort_key: DrawItemSortKey
```

新增结构：

```text
DrawItemSortKey
  render_domain_order: u8
  sorting_layer: i16
  order_in_layer: i32
  sort_z_quantized: i32
  stable_proxy_id: u64
```

第一版 render_domain_order：

```text
Mesh = 10
Sprite = 20
SkinnedMesh = 30
Particle = 40
UI = 90，预留，不在本轮实现
```

最小测试：

```text
sprite_draw_item_carries_sprite_fields
mesh_draw_item_keeps_existing_mesh_fields
```

### A3. RendererFeatureBuilder 按 sort_key 输出

规则：

```text
build() 收集 draw_items 后按 sort_key 稳定排序。
Sprite draw item 使用 SpritePayload 的 sorting_layer / order_in_layer / sort_z。
非 Sprite draw item 使用默认排序字段，不破坏现有 Mesh 行为。
```

最小测试：

```text
sprite_draw_items_sort_by_layer_order_z_proxy
same_sprite_sort_bucket_falls_back_to_proxy_id
builder_still_skips_invisible_proxy
```

### A4. MinimalRenderer / RuntimeRenderer 兼容

如果 `MinimalRenderer` / `RuntimeRenderer` 当前只读取 `mesh_ref`，需要让它能读取：

```text
sprite_ref
material_ref
sort_key
```

本轮只要求报告层能看到 Sprite draw item，不要求真实 GPU 绘制。

最小测试：

```text
minimal_renderer_report_includes_sprite_draw_item
runtime_renderer_does_not_panic_with_sprite_draw_item
```

### A5. 文档和入口确认

施工完成后：

```text
把本文档移动到 施工文档/已完成/
新增 阶段完成记录/2026-06-29-Sprite2D-Rendering-C-min/00-总览.md
更新 54-当前可自动化施工总入口.md
如实现与 96 正式方案有偏差，回填 96 正式方案
```

## 4. 验证命令

必须执行：

```text
cargo fmt --check
cargo test -p engine_runtime renderer_feature_builder
cargo test -p engine_runtime render_state
cargo test -p engine_runtime minimal_renderer
cargo test -p engine_runtime runtime_renderer
cargo test -p engine_runtime
cargo test --workspace
```

如果某个 test filter 没有匹配测试，记录为 no_match，但 `cargo test -p engine_runtime` 和 `cargo test --workspace` 必须执行。

## 5. 完成判定

满足以下条件才算完成：

```text
SpritePayload 有完整第一版字段。
RendererFeatureDrawItem 能表达 Sprite draw item。
Sprite draw item 有 deterministic sort_key。
排序测试覆盖 sorting_layer / order_in_layer / sort_z / proxy_id。
Mesh 既有测试不回退。
MinimalRenderer / RuntimeRenderer 不因 Sprite draw item 崩溃。
所有新增能力都有 headless 测试。
```
