# 290 Editor GameView Immediate AUI Sprite2D Binding Minimal Repair v1

## 状态

```text
状态：已完成，待归档
授权：用户已授权按已确认建议修复引擎
边界：源码最小修复；不修改 schema、composition/cache、production Editor 或真实配置；不运行 Local CI
```

## Necessity Audit

```text
confirmed failure:
  EditorRuntimePlayInstance 的即时 AUI 重绘向 render thread 传入 runtime texture bindings，
  但把 Sprite2D texture bindings 传为 None；点击产生的新 AUI presentation 因而把纹理
  Sprite2D 降级为白色 fallback，下一普通帧恢复真实 binding 后形成白闪。

minimum causal fix:
  在 EditorRuntimePlayInstance 启动时从既有 runtime texture bindings 派生并保留
  Sprite2D bindings；即时 AUI 重绘传入同一 bindings。

files that must change:
  rust/crates/editor_core/src/editor_gameview_play.rs

tests that can fail before and pass after the fix:
  editor_gameview_immediate_aui_input_preserves_sprite_texture_binding
  editor_gameview_immediate_aui_input_does_not_tick_runtime

explicitly deferred nearby enhancements:
  renderer API 重构、fallback 策略调整、schema、production replacement、Tower cache、真实视觉矩阵、Local CI
```

## Gate M

1. 增加纹理 Sprite2D 与 AUI PointerDown 共存的 owner 红测试。
2. 只修即时重绘的 Sprite2D binding 传递。
3. 运行新增测试、既有即时 AUI 不推进 runtime 测试和 `cargo fmt --check -p editor_core`。
4. 成功后写一次完成记录并归档本文档。

## 完成结果

```text
红测试：editor_gameview_immediate_aui_input_preserves_sprite_texture_binding
修复前：FAILED，fallback_used=true
修复后：passed，texture handle 存在且 fallback_used=false
direct consumer：editor_gameview_immediate_aui_input_does_not_tick_runtime passed
format：cargo fmt --check -p editor_core passed
```

实现仅在 `EditorRuntimePlayInstance` 启动时从既有 runtime texture bindings 派生并保留
Sprite2D bindings，即时 AUI 重绘复用同一份 bindings。未修改 renderer、fallback、schema、
production Editor、Tower cache 或真实配置，未运行 Local CI。
