# 91-当前可自动化施工文档：AI图片生成到项目资源库闭环 C-min

## 目标

按照 `91-AI图片生成到项目资源库闭环-C-min方案.md`，实现最小 AI 图片生成闭环：

```text
AiImageGenerationRequest
  -> MockImageGenerationProvider
  -> GeneratedImageSource (.png + .ai.json)
  -> Importer / Asset DB
  -> ProjectDock / AssetPipelineState
  -> PlaceAssetIntoScene
  -> headless report
```

本轮重点不是接真实 AI 模型，而是把流程跑顺。

用户理解模型必须保持简单：

```text
我说一句话
  -> 出一张图
  -> 图出现在项目资源里
  -> 可以放进场景
```

## 已确认规则

```text
1. AI 第一版只生成图片源文件。
2. 图片必须写入当前项目目录。
3. 图片默认写入 Project Library / Generated。
4. 图片写入后必须走现有 Importer / Asset DB。
5. ProjectDock 只展示 Importer 成功后的资源。
6. 放入 Scene 继续走 90 已完成的 PlaceAssetIntoScene。
7. 不做复杂质量评分。
8. 不做复杂多模型路由。
9. 不做复杂任务系统。
10. 失败必须返回清楚 diagnostic。
```

## 明确不做

本轮不做：

```text
真实 AI 图片模型接入
3D model generation
Audio generation
Animation generation
Material Graph generation
multi-step autonomous editing
semantic quality scoring
cloud queue
multi-user generation queue
commercial license auto judgment
复杂 Provider Router
复杂 Generation Graph / Revision 系统
真实 UI 弹窗
真实拖拽
```

## 必须复用

优先复用已有系统：

```text
rust/crates/editor_core
rust/crates/editor_ui_model
rust/crates/editor_input
rust/crates/editor_host
rust/crates/engine_runtime

Asset Pipeline / Asset DB / Importer 相关现有模块
ProjectDock / AssetPipelineState 相关现有模块
90 PlaceAssetIntoScene 已完成链路
```

先搜索现有 asset pipeline 代码，不新造第二套 Asset DB。

## A1：新增 AI 图片生成核心数据结构

候选位置：

```text
rust/crates/editor_core/src/ai_image_generation.rs
```

新增：

```text
AI_IMAGE_GENERATION_REQUEST_SCHEMA_VERSION
GENERATED_IMAGE_SOURCE_SCHEMA_VERSION
AI_IMAGE_GENERATION_RESULT_SCHEMA_VERSION

AiImageGenerationRequest
GeneratedImageSource
AiImageGenerationResult
AiImageGenerationDiagnostic
AiImageGenerationStatus
ImageKind
```

字段按方案文档：

```text
AiImageGenerationRequest:
  schema_version
  request_id
  prompt
  reference_image_paths
  target_folder
  asset_name
  image_kind
  width
  height
  transparent_background

GeneratedImageSource:
  schema_version
  source_id
  request_id
  path
  image_kind
  content_hash
  metadata_path

AiImageGenerationResult:
  schema_version
  request_id
  status
  generated_images
  imported_assets
  diagnostics
```

规则：

```text
prompt 必填。
target_folder 必须在当前项目目录内。
asset_name 必须可转成安全文件名。
image_kind 第一版只允许 texture / sprite / uiImage / referenceImage。
reference_image_paths 可为空。
```

测试：

```text
cargo test -p editor_core ai_image_generation_data
```

最低断言：

```text
request / source / result 可序列化。
空 prompt 被拒绝。
非法 image_kind 不可构造或反序列化失败。
target_folder 越界被拒绝。
```

## A2：新增 MockImageGenerationProvider

同文件或新增：

```text
rust/crates/editor_core/src/ai_image_generation.rs
```

新增简单接口：

```text
ImageGenerationProvider.generate_image(project_root, request) -> AiImageGenerationResult
```

第一版实现：

```text
MockImageGenerationProvider
```

规则：

```text
Mock Provider 必须 deterministic。
Mock Provider 生成真实 .png 文件。
Mock Provider 生成 .ai.json metadata 文件。
Provider 不写 Asset DB。
Provider 不写 Scene。
Provider 不创建 Entity。
```

PNG 可极简：

```text
1x1 或 16x16 固定 RGBA PNG。
透明背景按 transparent_background 决定 alpha。
```

metadata 最小字段：

```text
schemaVersion: generated-image-metadata.v1
requestId
prompt
referenceImagePaths
imageKind
width
height
transparentBackground
providerId
createdAt
```

测试：

```text
cargo test -p editor_core mock_image_generation_provider
```

最低断言：

```text
生成 .png 文件。
生成 .ai.json 文件。
content_hash 稳定。
同 request 生成同路径 / 同 hash。
Provider 不产生 Asset DB record。
```

## A3：接入现有 Importer / Asset DB

目标：

```text
GeneratedImageSource
  -> existing importer / asset db registration
  -> imported asset record
```

施工前必须先搜索：

```text
rg -n "AssetPipeline|AssetDb|AssetDatabase|Importer|ProjectDock|AssetPipelineState|asset manifest" rust/crates -S
```

规则：

```text
不要新造第二套 Asset DB。
如果现有 importer 只支持最小测试格式，就按现有能力接入。
如果缺少图片 importer，先做最小 image importer adapter，但必须挂到现有 Asset Pipeline 入口。
```

测试：

```text
cargo test -p editor_core ai_image_generation_imports_generated_image
```

最低断言：

```text
GeneratedImageSource 经过 importer 后出现 Asset DB record。
sprite image_kind -> asset_type sprite。
texture image_kind -> asset_type texture。
失败时 result.diagnostics 有明确 code。
```

## A4：ProjectDock / AssetPipelineState 可见

目标：

```text
生成并导入后的图片资源，可以被 ProjectDock / AssetPipelineState 查询到。
```

规则：

```text
ProjectDock 不读取 AI 临时结果。
ProjectDock 只读 Asset DB / AssetPipelineState。
AI 生成成功但导入失败时，ProjectDock 不显示为可用资源。
```

测试：

```text
cargo test -p editor_core ai_image_generation_project_dock_visible
```

最低断言：

```text
生成 sprite 后，AssetPipelineState 或对应 UI model 中能看到 sprite asset。
asset id / asset type / source path 稳定。
```

## A5：接入 PlaceAssetIntoScene

目标：

```text
AI 生成 sprite
  -> 导入成 Project Asset
  -> UiCommandPayload::PlaceAssetIntoScene
  -> Scene Entity
```

规则：

```text
AI 生成系统不直接写 Scene。
必须复用 90 的 PlaceAssetIntoScene。
```

测试：

```text
cargo test -p editor_core ai_image_generation_to_scene_authoring_loop
```

最低断言：

```text
打开默认 Scene。
生成 sprite 图片。
导入到 Asset DB。
调用 PlaceAssetIntoScene。
Hierarchy 出现新 Entity。
Inspector 能看到 mesh / assetRef 或当前 sprite 映射字段。
Scene dirty == true。
Save 成功。
```

## A6：新增 headless 闭环报告

新增：

```text
AiImageGenerationLoopReport
run_ai_image_generation_loop_headless()
```

报告字段：

```text
schema_version
request_id
prompt
generated_image_path
metadata_path
imported_asset_id
imported_asset_type
placed_entity_id
dirty_before_save
dirty_after_save
play_finished
diagnostics
```

测试：

```text
cargo test -p editor_core ai_image_generation_loop
```

最低断言：

```text
report 可序列化。
生成 -> 导入 -> 入场 -> 保存 -> Play 全链路通过。
失败 diagnostic 可读。
```

## A7：可选 editor_input 最小命令入口

如果现有 `editor_input` 命令模式适合扩展，新增：

```text
project_ai_generate_image_command(...)
```

但不强行做真实 UI。

测试：

```text
cargo test -p editor_input ai_generate_image
```

如果本轮只做 editor_core headless，允许跳过 editor_input，但最终报告必须说明。

## 验收命令

必须执行：

```text
cargo fmt
cargo fmt --check
cargo test -p editor_core ai_image_generation_data
cargo test -p editor_core mock_image_generation_provider
cargo test -p editor_core ai_image_generation_imports_generated_image
cargo test -p editor_core ai_image_generation_project_dock_visible
cargo test -p editor_core ai_image_generation_to_scene_authoring_loop
cargo test -p editor_core ai_image_generation_loop
cargo test -p editor_core
cargo test --workspace
```

如果某个 filter 没有匹配测试，必须记录：

```text
该 filter 当前无匹配测试，不视为失败。
```

## 完成后文档收尾

完成后：

```text
施工文档/当前/91-当前可自动化施工文档-AI图片生成到项目资源库闭环-C-min.md
  -> 施工文档/已完成/91-当前可自动化施工文档-AI图片生成到项目资源库闭环-C-min.md
```

新增阶段记录：

```text
阶段完成记录/2026-06-28-AI图片生成到项目资源库闭环-C-min/00-总览.md
```

更新：

```text
49-当前状态与下一步入口.md
54-当前可自动化施工总入口.md
87-当前实现状态总览-2026-06-28.md
00-文档地图.md
README.md
施工文档/README.md
阶段完成记录/README.md
```

## 当前结论

91 C-min 只做图片生成闭环，不做复杂 AI 资源平台。它的价值是把“AI 生成图片”接入现有项目资源系统，让图片能被 Importer、Asset DB、ProjectDock、Scene Authoring、Build 后续流程理解。
