# 281 Editor GameView Sprite2D Cooked Texture Binding Minimal Repair v1 方案

## 1. 问题

普通 Editor GameView 已能加载 RuntimePackage、运行项目规则并生成 Sprite2D RenderProxy，但
`EditorRuntimePlayInstance` 只收集 AUI Image 的纹理引用。`EngineHostLoop` 的常规 tick 提交又把
`sprite_texture_bindings` 固定为 `None`。结果是 Scene `SpriteRenderer2D` 可以进入 draw plan，却只能
使用 fallback binding；Tower 的世界背景与移动怪物均不可见。

## 2. 目标

复用 275 已完成的 `RuntimeTextureUploadRegistry` 与 cooked RGBA8 资源，不新增第二套纹理系统：

1. Editor Play 收集 AUI Image 与 active RuntimeScene SpriteRenderer2D 的纹理并集。
2. 同一个 `RuntimeTextureBindingContext` 同时供 AUI 与 Sprite2D consumer 使用。
3. `EngineHostLoop` 把 runtime texture bindings 转换为现有 `Sprite2DTextureBindingContext`，进入
   `RenderFramePacket`。
4. Editor 把 Sprite2D cooked texture resolve 失败报告为独立的
   `sprite2d.texture_not_resolved`，保留 AUI 既有诊断。
5. 不修改 Tower gameplay、动画规则、AUI 布局或真实项目配置。

## 3. 验收

- owner 红测能在修复前观察到 Scene Sprite texture 未上传或 `DrawSpriteTextured.texture=None`。
- 修复后 Scene-only Sprite texture 进入 upload registry，Sprite draw command 使用非空 texture handle，
  缺失 Sprite texture 输出 owner-specific diagnostic。
- 更新普通 production Editor 前保留旧二进制备份；只备份并重建 Tower `scene-main` Preview cache。
- 不运行 Local CI，不替换 Player/MCP/其它安装态二进制，不修改真实配置。

