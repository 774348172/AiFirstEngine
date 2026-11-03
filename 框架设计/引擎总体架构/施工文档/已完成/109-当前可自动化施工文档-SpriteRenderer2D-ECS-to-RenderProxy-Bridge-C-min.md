# 109-当前可自动化施工文档-SpriteRenderer2D ECS-to-RenderProxy Bridge C-min

## 1. 施工目标

补齐 `SpriteRenderer2D` 从 ECS `World` 到 `RenderProxyPayload::Sprite` 的正式桥，让复杂打飞机验证不再依赖 `Renderable quad` 代替 Sprite。

## 2. 施工原则

```text
只补引擎底座能力。
不加入 enemy / bullet / health / score 等项目概念。
不让项目逻辑直接写 RenderProxy。
复用 DirtyRecord / RenderExtract / RenderCommand / RenderSceneState。
保留 Mesh / Renderable 现有行为。
```

## 3. 代码施工项

### A1. ECS typed component

文件：

```text
rust/crates/engine_runtime/src/archetype.rs
rust/crates/engine_runtime/src/world.rs
```

新增：

```text
ComponentValue::SpriteRenderer2D
ComponentColumn::SpriteRenderer2D
ArchetypeTable::sprite_renderer2d(row)
World::insert_sprite_renderer2d
World::sprite_renderer2d
World::remove_sprite_renderer2d
```

规则：

```text
engine.sprite_renderer2d -> DirtyType::RenderState
```

### A2. RenderProxy / RenderCommand 泛化

文件：

```text
rust/crates/engine_runtime/src/render_state.rs
rust/crates/engine_runtime/src/render_command.rs
```

要求：

```text
AddProxy 可承载 SpritePayload。
UpdateRenderState 可更新 SpritePayload。
Renderable / Mesh 行为不变。
```

### A3. RenderExtract 支持 SpriteRenderer2D

文件：

```text
rust/crates/engine_runtime/src/render_extract.rs
```

规则：

```text
如果实体有 SpriteRenderer2D，RenderExtract 生成 Sprite proxy。
如果实体有 Renderable，继续生成 Mesh proxy。
第一版不允许一个实体同时作为 Mesh 和 Sprite 的复杂混合渲染对象；优先保持简单，后续需要多渲染组件再讨论。
```

### A4. 测试

新增测试：

```text
World 写入 SpriteRenderer2D 标记 RenderState dirty。
RenderExtract 从 SpriteRenderer2D 生成 AddProxy。
apply_batch 后 RenderSceneState payload 为 Sprite。
RendererFeatureBuilder 输出 sprite draw item。
RuntimeRenderer 输出 sprite draw pass。
Mesh / Renderable 旧测试继续通过。
```

## 4. 测试命令

```text
cargo fmt --check
cargo test -p engine_runtime sprite
cargo test -p engine_runtime render_extract
cargo test -p engine_runtime render_command
cargo test -p engine_runtime
cargo test --workspace --no-run
```

## 5. 完成标准

```text
SpriteRenderer2D ECS -> RenderProxyPayload::Sprite 闭环可测。
现有 Mesh 渲染闭环不回退。
复杂项目验证中 sprite_renderer2d_not_extracted_from_ecs 缺口后续可移除或更新。
```

