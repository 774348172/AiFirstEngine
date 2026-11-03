# 110-当前可自动化施工文档-World Projection Adapter 统一跨域同步收敛

## 1. 施工目标

把当前已经出现的多个 `Bridge / Extract / Hydration` 统一归拢到 `World Projection / Projection Adapter` 体系下。

本施工不是为了新增一套复杂框架，而是为了减少后续复杂度：

```text
不再为每个类型新增一座桥。
统一跨域同步心智。
旧桥全部归拢为 Projection Adapter 的历史命名。
SpriteRenderer2D 作为第一批落地 case。
```

## 2. 对应正式规则

```text
110-World-Projection-Adapter统一跨域同步规则.md
108-RuntimeScene-World-Hydration-C-min方案.md
109-SpriteRenderer2D-ECS-to-RenderProxy-Bridge-C-min方案.md
95-Physics2D-Foundation-C-min方案.md
96-Sprite2D-Rendering-C-min方案.md
102-AUI-Render-Extract-RuntimeRenderer接入方案.md
17-Runtime-FrameLoop.md
```

## 3. 施工原则

```text
先归拢文档和概念，再做最小代码收敛。
不大规模重命名已稳定通过测试的模块。
新增系统必须使用 Projection / Adapter 命名。
旧 Bridge 类型可以暂时保留，但必须有文档说明它的 Projection 归属。
每完成一个子阶段必须跑最小测试。
```

## 4. 子阶段 A：文档归拢

### A1. 更新当前入口文档

文件：

```text
49-当前状态与下一步入口.md
00-文档地图.md
施工文档/README.md
```

动作：

```text
新增 110 规则入口。
声明旧 Bridge 名称全部归拢为 Projection Adapter 历史命名。
下一步施工入口改为本施工文档。
```

验收：

```text
rg -n "World Projection|Projection Adapter|110-" 框架设计/引擎总体架构
```

### A2. 给旧系统补归属说明

需要补充的文档：

```text
108-RuntimeScene-World-Hydration-C-min方案.md
109-SpriteRenderer2D-ECS-to-RenderProxy-Bridge-C-min方案.md
95-Physics2D-Foundation-C-min方案.md
96-Sprite2D-Rendering-C-min方案.md
102-AUI-Render-Extract-RuntimeRenderer接入方案.md
06-资源系统架构.md
17-Runtime-FrameLoop.md
```

补充规则：

```text
Hydration = HydrationProjection
RenderExtract = RenderProjection
Physics2DBridge = Physics2DProjection 的历史命名
RenderAssetBridge = AssetProjection 的历史命名
AuiRenderExtract / AuiRendererBridge = UiProjection 的历史命名
SpriteRenderer2D ECS-to-RenderProxy Bridge = RenderProjectionAdapter<SpriteRenderer2D> 的历史命名
```

验收：

```text
rg -n "历史命名|Projection" 对应文档
```

## 5. 子阶段 B：代码层最小统一模型

### B1. 新增 projection 基础类型

建议文件：

```text
rust/crates/engine_runtime/src/projection.rs
```

第一版只放通用 report / enum，不强行抽象 trait：

```rust
pub enum ProjectionDomain {
    RuntimePackage,
    World,
    Render,
    Physics2D,
    AssetRuntime,
    Ui,
}

pub enum ProjectionKind {
    Hydration,
    Render,
    Physics2D,
    Asset,
    Ui,
}

pub struct ProjectionDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub adapter: Option<String>,
}

pub struct ProjectionReport {
    pub kind: ProjectionKind,
    pub source_domain: ProjectionDomain,
    pub target_domain: ProjectionDomain,
    pub adapter_name: String,
    pub projected_count: usize,
    pub skipped_count: usize,
    pub error_count: usize,
    pub diagnostics: Vec<ProjectionDiagnostic>,
}
```

验收：

```text
cargo fmt --check
cargo test -p engine_runtime projection
```

### B2. 给现有 report 增加 Projection 归属

不破坏现有 report 字段，只新增可选或辅助转换：

```text
RuntimeSceneHydrationReport -> ProjectionKind::Hydration
RenderExtractReport -> ProjectionKind::Render
Physics2DSyncReport / Physics2DTrace -> ProjectionKind::Physics2D
RenderAssetBridgeReport -> ProjectionKind::Asset
AUI render report -> ProjectionKind::Ui
```

第一版可以提供：

```rust
impl From<&RuntimeSceneHydrationReport> for ProjectionReport
```

或：

```rust
RuntimeSceneHydrationReport::projection_summary()
```

验收：

```text
每类 report 至少有一个 projection_summary 单元测试。
```

## 6. 子阶段 C：SpriteRenderer2D 第一落地

### C1. RuntimePackage 增加 typed SpriteRenderer2D 表达

文件：

```text
rust/crates/engine_runtime/src/runtime_package.rs
```

新增：

```rust
pub struct RuntimeSpriteRenderer2D {
    pub sprite_ref: Option<RuntimeAssetRef>,
    pub material_ref: Option<RuntimeAssetRef>,
    pub color: Option<[f32; 4]>,
    pub flip_x: Option<bool>,
    pub flip_y: Option<bool>,
    pub sorting_layer: Option<i16>,
    pub order_in_layer: Option<i32>,
    pub sort_z: Option<f32>,
    pub visible: Option<bool>,
}
```

`RuntimeEntity` 增加：

```rust
pub sprite_renderer2d: Option<RuntimeSpriteRenderer2D>
```

规则：

```text
RuntimeScene 保存 SpriteRenderer2D 数据。
不在 RuntimeScene 中保存 RenderProxyPayload::Sprite。
```

### C2. HydrationProjection 接入 SpriteRenderer2D

文件：

```text
rust/crates/engine_runtime/src/runtime_instance_loader.rs
```

动作：

```text
collect_entity_asset_refs 读取 sprite_ref / material_ref。
attach_components 把 RuntimeSpriteRenderer2D 转成 ECS SpriteRenderer2D。
通过 World::insert_sprite_renderer2d 写入，保持 dirty 机制。
```

规则：

```text
这个逻辑归属 HydrationProjectionAdapter<RuntimeSpriteRenderer2D>。
不要叫新的 Sprite Hydration Bridge。
```

### C3. RenderProjection 使用现有 Sprite adapter

文件：

```text
rust/crates/engine_runtime/src/render_extract.rs
```

动作：

```text
确认现有 SpriteRenderer2D -> RenderProxyPayload::Sprite 归属 RenderProjectionAdapter<SpriteRenderer2D>。
如需要，只补注释 / report adapter_name。
```

不做：

```text
不新增第二套 Sprite bridge。
不让 Hydration 直接写 RenderProxy。
```

## 7. 子阶段 D：旧 Bridge 命名收敛

### D1. 文档收敛

把旧表述：

```text
唯一同步桥
xxxBridge
xxx桥
```

收敛为：

```text
Projection Boundary
Projection Adapter
历史命名：xxxBridge
```

### D2. 代码命名策略

第一版不强制立即重命名所有公开类型，避免影响已通过测试。

允许保留：

```text
RenderAssetBridge
Physics2DBridge
```

但新增代码中必须补充：

```rust
// Historical name: this is an AssetProjection adapter.
```

长期可以逐步迁移为：

```text
AssetProjection
Physics2DProjection
RenderProjection
UiProjection
```

## 8. 测试计划

### T1. Projection 基础类型测试

目标：

```text
ProjectionReport 可以表达 Hydration / Render / Physics2D / Asset / Ui。
```

### T2. SpriteRenderer2D RuntimePackage 反序列化测试

目标：

```text
RuntimeEntity.spriteRenderer2D 能从 JSON 读取。
默认字段能转成 ECS SpriteRenderer2D 默认值。
```

### T3. Hydration 写入 SpriteRenderer2D 测试

目标：

```text
RuntimePackage -> HydrationProjection -> World.sprite_renderer2d
```

### T4. Asset dependency 缺失测试

目标：

```text
spriteRef 缺失时 HydrationProjection report 有明确 diagnostics。
```

### T5. 端到端 Sprite RenderProjection 测试

目标：

```text
RuntimePackage
  -> HydrationProjection
  -> World.SpriteRenderer2D
  -> RenderProjection
  -> RenderProxyPayload::Sprite
```

### T6. 旧系统不回退测试

目标：

```text
Physics2D 原测试通过。
RenderExtract 原测试通过。
AUI render 原测试通过。
RuntimeScene Hydration 原测试通过。
```

命令：

```text
cargo fmt --check
cargo test -p engine_runtime projection
cargo test -p engine_runtime runtime_scene_hydration
cargo test -p engine_runtime render_extract
cargo test -p engine_runtime sprite
cargo test -p engine_runtime physics
cargo test -p engine_runtime
cargo test --workspace --no-run
```

## 9. 完成标准

```text
110 正式规则文档存在。
当前施工文档存在。
旧 Bridge 系统在文档中归拢到 Projection Adapter。
新增 projection 基础 report 类型。
SpriteRenderer2D 能从 RuntimePackage 经 HydrationProjection 进入 ECS World。
RenderProjection 能继续把 SpriteRenderer2D 生成 Sprite RenderProxy。
所有相关最小测试通过。
```

## 10. 不做范围

本施工不做：

```text
完整重命名所有 Bridge 代码文件。
引入复杂 trait registry。
引入脚本插件式 adapter 注册系统。
支持 SpriteAtlas / 动画 / 九宫格。
支持完整 Rigidbody2D solver。
把项目玩法概念加入 Projection。
```

