# 91-AI图片生成到项目资源库闭环 C-min 方案

## 1. 设计问题

90 已经完成：

```text
Project asset
  -> PlaceAssetIntoScene
  -> Scene Entity
```

91 只解决一个更小的问题：

```text
用户用自然语言让 AI 生成图片，图片如何顺滑进入项目资源库，并能继续被项目使用。
```

本系统不做复杂 AI 资源生产平台。第一版只做图片：

```text
用户描述想要什么图片
  -> 调用 AI 图片模型生成源图片文件
  -> 源图片写入项目目录
  -> 走 Importer / Asset DB
  -> ProjectDock 可见
  -> 可继续 PlaceAssetIntoScene
```

## 2. 其它引擎参考

### Unity

Unity 的成熟资源路线是：

```text
Assets/source file
  -> .meta GUID
  -> AssetImporter
  -> AssetDatabase
  -> Project Window
```

Unity Muse / Sprite 生成更像是在编辑器里生成候选图片，然后用户把结果加入项目或拖入 Scene。

对我们的启发：

```text
AI 生成图片本质上还是一张 source image。
进入项目后必须走 AssetDatabase / Importer。
AI 不应该绕过资源数据库。
```

### Unreal Engine

UE 的成熟资源路线是：

```text
source file
  -> Factory / Interchange
  -> AssetRegistry
  -> Content Browser
```

本地 UE 源码可见相关系统：

```text
UFactory
AssetTools
AssetRegistry
ContentBrowser
UInterchangeManager
UInterchangeSourceData
```

对我们的启发：

```text
长期可以参考 UE Interchange 做复杂导入 pipeline。
但第一版图片生成不需要 UE 式 Pipeline Stack。
```

### Godot

Godot 的成熟资源路线是：

```text
project file
  -> EditorFileSystem scan_changes
  -> ResourceImporter
  -> .import metadata
  -> FileSystemDock
```

本地 Godot 源码可见相关系统：

```text
EditorFileSystem
ResourceImporter
ResourceUID
FileSystemDock
.import
reimport_files
```

对我们的启发：

```text
AI 生成图片应该像用户拷贝图片到项目目录一样，进入扫描和导入流程。
Importer 产物不是编辑真相，源图片和导入设置才是维护入口。
```

### Bevy

Bevy 更偏 Runtime：

```text
AssetServer
  -> AssetLoader
  -> Handle<T>
```

对我们的启发：

```text
Runtime 引用要简单。
但编辑器图片生产闭环不能只靠 AssetServer。
```

## 3. 方案收缩

不采用复杂通用结构：

```text
AiGenerationRequest
AiGenerationJob
GeneratedSourceFile
AiGenerationReport
AiProviderAdapter
Generation Graph
Quality Gate
Revision
```

C-min 只保留图片生成最小结构：

```text
AiImageGenerationRequest
GeneratedImageSource
AiImageGenerationResult
```

其中：

```text
Request = 用户想生成什么图片。
GeneratedImageSource = 模型生成出来的源图片文件。
Result = 生成和导入结果，给 UI / Console / AI 看。
```

## 4. 正式规则

第一版规则：

```text
1. AI 只生成图片源文件。
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

用户理解模型必须保持简单：

```text
我说一句话
  -> 出一张图
  -> 图出现在项目资源里
  -> 可以放进场景
```

## 5. C-min 支持范围

支持：

```text
imageKind:
  texture
  sprite
  uiImage
  referenceImage
```

输入：

```text
prompt
referenceImagePaths[]
targetFolder
assetName
imageKind
width
height
transparentBackground
```

输出：

```text
generated .png source file
generated .ai.json metadata file
Importer request / result
Asset DB record
AiImageGenerationResult
```

第一版不做：

```text
3D model
Audio
Animation
Material Graph
multi-step autonomous editing
semantic quality scoring
cloud queue
multi-user generation queue
commercial license auto judgment
```

## 6. 数据结构

### AiImageGenerationRequest

```json
{
  "schemaVersion": "ai-image-generation-request.v1",
  "requestId": "req-enemy-sprite-001",
  "prompt": "红色阵营敌机，俯视角，透明背景，适合打飞机游戏",
  "referenceImagePaths": [
    "Assets/References/enemy_style.png"
  ],
  "targetFolder": "Assets/Generated/Sprites",
  "assetName": "enemy_fighter_red",
  "imageKind": "sprite",
  "width": 512,
  "height": 512,
  "transparentBackground": true
}
```

字段规则：

```text
prompt 必填。
targetFolder 必须在当前项目目录内。
assetName 必须可转成安全文件名。
imageKind 第一版只允许 texture / sprite / uiImage / referenceImage。
referenceImagePaths 可为空。
```

### GeneratedImageSource

```json
{
  "schemaVersion": "generated-image-source.v1",
  "sourceId": "gen-img-enemy-fighter-red-001",
  "requestId": "req-enemy-sprite-001",
  "path": "Assets/Generated/Sprites/enemy_fighter_red.png",
  "imageKind": "sprite",
  "contentHash": "sha256:...",
  "metadataPath": "Assets/Generated/Sprites/enemy_fighter_red.ai.json"
}
```

字段规则：

```text
GeneratedImageSource 不是 Asset DB 资源。
它只是交给 Importer 的源图片文件。
path 必须是项目内路径。
metadataPath 记录 prompt / reference / model / seed / createdAt 等最小来源信息。
```

### AiImageGenerationResult

```json
{
  "schemaVersion": "ai-image-generation-result.v1",
  "requestId": "req-enemy-sprite-001",
  "status": "succeeded",
  "generatedImages": [
    {
      "path": "Assets/Generated/Sprites/enemy_fighter_red.png",
      "assetType": "sprite"
    }
  ],
  "importedAssets": [
    {
      "assetId": "sprite-enemy-fighter-red",
      "assetType": "sprite"
    }
  ],
  "diagnostics": []
}
```

状态：

```text
succeeded
failed
cancelled
```

Result 必须回答：

```text
图片生成成功了吗？
源图片写到哪里？
Importer 成功了吗？
Asset DB 里生成了什么资源？
失败原因是什么？
```

## 7. Provider 规则

第一版不要设计复杂 Provider Router。

只需要一个简单接口：

```text
ImageGenerationProvider.generate_image(request) -> GeneratedImageSource
```

C-min 可以先实现：

```text
MockImageGenerationProvider
```

真实模型后续再接：

```text
ExternalImageGenerationProvider
LocalImageGenerationProvider
```

Provider 只负责生成图片文件，不负责：

```text
写 Asset DB
写 Scene
创建 Entity
修改 ProjectDock
```

## 8. 与既有文档关系

```text
13-AI资源生产管线.md：
  长期 AI 资源生产总目标。

28-AI-Asset-Generation-MVP.md：
  长期 Spec / Plan / Graph / Candidate / Revision 数据闭环。

25-Asset-DB-Importer-MVP.md：
  Asset DB / Importer 规则。

90-ProjectAsset-to-SceneEntity-Authoring-C-min方案.md：
  Project Asset 进入 Scene。

91：
  只做图片生成进入 Project Asset，再复用 90 进入 Scene。
```

## 9. 下一步

生成施工文档，第一版施工目标：

```text
AiImageGenerationRequest
GeneratedImageSource
AiImageGenerationResult
MockImageGenerationProvider
生成 .png 源文件
生成 .ai.json metadata
调用现有 Importer / Asset DB 最小入口
ProjectDock / AssetPipelineState 可看到生成资源
可继续 PlaceAssetIntoScene
headless 测试闭环
```
