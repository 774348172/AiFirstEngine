# Project Schema v1

本文档定义当前实现阶段的项目数据真相。

Project Schema v1 的目标不是一次性实现完整 Asset DB、Scene Partition 或 Rust Runtime，而是给 AI Patch、Validation、Editor、Runtime Prototype 和 Build Graph 提供稳定输入。

## 设计目标

优先级：

```text
1. 适配 AI
2. 适配复杂项目
3. 适配后期修改
4. 规则简单，避免过度复杂
```

因此 v1 采用：

```text
Unity-like 稳定 id / ref 思路
Unreal-like registry / build graph 方向
Godot-like 文本可读资源思路
```

但当前实现先保持简单：

```text
Project 内联 Scene 列表
Scene 内联 Entity 树
Project 内联 Asset 列表
Prefab 作为 Asset data 支持
```

后续再升级：

```text
Scene 拆文件
SubScene / Scene Partition
assets.registry.json
per-asset meta
Asset DB / Importer
Bundle / Cook / Hot Update
```

## 当前 Schema 文件

当前第一版 schema 文件位于：

```text
schemas/project.schema.json
schemas/scene.schema.json
schemas/entity.schema.json
schemas/asset.schema.json
schemas/patch-plan.schema.json
schemas/validation-report.schema.json
```

运行时代码 validator 位于：

```text
src/schema/projectSchema.ts
```

当前使用手写 validator，而不是引入 Ajv。

原因：

```text
第一阶段要保持依赖简单
需要同时服务浏览器端和 Node 脚本端
Validation issue 必须输出 kind / path / message
JSON Schema 文件先作为正式契约和后续工具入口
```

## Schema 与 Rust Reflect 的边界

Bevy 的 Reflect 系统适合做运行时类型检查、Inspector 展示、序列化辅助和调试工具。  
但对本项目来说，Reflect 不能成为项目数据真相。

正式规则：

```text
Project Schema / Component Schema 是项目数据真相。
JSON Schema / validator 是 AI Patch、Build Graph、Runtime Package 的契约入口。
Rust Reflect 只能作为运行时辅助工具。
Inspector Registry 可以由 Schema 生成，也可以使用 Reflect 辅助展示。
如果 Schema 与 Reflect 不一致，以 Schema 为准，并由验证层报错。
```

## schemaVersion 规则

v1 支持以下版本：

```text
project.v1
scene.v1
entity.v1
asset.v1
patch-plan.v1
validation-report.v1
```

当前兼容旧项目数据：

```text
如果对象没有 schemaVersion，不直接判错。
如果对象声明了 schemaVersion，则必须等于对应 v1。
```

这是为了让当前 starterProject / shooterProject 可以平滑迁移。

后续进入正式文件格式后：

```text
project.schemaVersion 必须存在
scene.schemaVersion 必须存在
asset.schemaVersion 必须存在
```

## Project v1

Project v1 当前字段：

```text
schemaVersion?: project.v1
name
version
engineMode = 3d
activeSceneId
scenes[]
assets[]
patchHistory?
```

规则：

```text
name 必须非空
version 必须非空
engineMode 必须为 3d
activeSceneId 必须指向已有 scene
scenes 至少一个
assets 必须是数组
```

## Scene v1

Scene v1 当前字段：

```text
schemaVersion?: scene.v1
id
name
gravity
background
skyColor
entities[]
```

规则：

```text
id 必须稳定且非空
同一 Project 内 scene id 不可重复
gravity 必须是有限 number
background / skyColor 必须是 string
entities 必须是数组
```

v1 不做：

```text
SubScene
Scene Partition
World Streaming
Additive Scene 依赖声明
```

## Entity v1

Entity v1 当前字段：

```text
schemaVersion?: entity.v1
id
name
kind
prefabInstance?
transform
mesh?
physics?
camera?
light?
scripts[]
```

规则：

```text
id 必须稳定且非空
同一 Scene 内 entity id 不可重复
name 必须非空
kind 必须属于当前 EntityKind
transform 必须存在
scripts 必须是数组
Component 必须是纯数据
```

v1 暂时保留 `scripts[]` 作为当前 TypeScript runtime prototype 的行为绑定。

长期路线：

```text
scripts[] -> Rule Binding / System Binding
Rule Binding -> Canonical Rule IR
```

## Component v1

当前组件：

```text
Transform
Mesh
Physics
Camera
Light
Scripts
```

规则：

```text
Transform 必备
其他组件可选
组件只保存数据
组件不保存行为逻辑
```

Component Schema 当前仍在：

```text
src/engine/componentSchema.ts
```

后续应将 Component Schema 与 JSON Schema / Inspector Field Registry 统一。

### EntityRef 字段声明规则

Component Schema 必须显式声明哪些字段是 EntityRef。  
Scene / Prefab / Runtime Package 中的 EntityRef 字段保存 AuthoringEntityRef，Runtime 加载后由 SceneInstantiator 修复为 RuntimeEntityHandle。

最小字段：

```text
type:
  EntityRef

required:
  true / false

scope:
  self
  parent
  owner
  scene_local
  prefab_local
  runtime

expected:
  anyOf ComponentTag / ComponentType

allowMissing:
  true / false

display:
  pickerLabel / debugName 可选
```

示例：

```yaml
ProjectileComponent:
  fields:
    owner:
      type: EntityRef
      required: true
      scope: runtime
      expected:
        anyOf: [PlayerTag]

    target:
      type: EntityRef
      required: false
      scope: runtime
      expected:
        anyOf: [Health]

    damage:
      type: number
```

规则：

```text
type=EntityRef 表示该字段需要 Entity Picker / AI 引用校验 / Runtime fixup。
scope 限定引用来源，不做复杂查询。
expected 只做组件存在性校验，不表达业务条件。
required=false 允许空引用，但不等于允许坏引用。
allowMissing=true 允许 Missing Reference 留在数据中，但必须产生 diagnostics。
display 只影响编辑器展示，不影响 Runtime 语义。
```

禁止：

```text
在 Component Schema 中写复杂查询 DSL。
用 expected 表达阵营、距离、血量、AI 状态等业务条件。
让 EntityRef 自动强引用保活 Entity。
让 EntityRef 自动跨 Scene 搜索。
让旧 EntityRef 自动重绑定到新 Entity。
用户 / AI 直接填写 runtimeEntityId 或 RuntimeEntityHandle。
```

职责边界：

```text
Component Schema:
  声明字段类型、引用范围、最小校验。

Inspector:
  根据 EntityRef 字段显示 Entity Picker 和 Missing Reference 状态。

AI Patch:
  只能生成 AuthoringEntityRef，不能生成 runtimeEntityId。

SceneInstantiator:
  根据 Component Schema 批量 fixup EntityRef 字段。

Validation:
  校验 required / scope / expected / allowMissing。

Project Rule / System:
  负责业务条件，例如阵营、距离、是否可攻击。
```

## Asset v1

Asset v1 当前字段：

```text
schemaVersion?: asset.v1
id
name
type
source
ref?
data?
```

规则：

```text
id 必须稳定且非空
name 必须非空
type 必须属于 AssetType
source 必须非空
同一 Project 内 asset id 不可重复
asset.ref 如果存在，必须指向自己
scene asset 必须指向已有 scene
```

v1 不做：

```text
assets.registry.json
per-asset meta
importer state
dependency graph
cooked asset
bundle assignment
hot update package
```

这些放入 Asset DB / Importer MVP 阶段。

## Patch Plan v1

当前 Patch Plan v1 字段：

```text
schemaVersion?: patch-plan.v1
id
title
summary[]
operations[]
sourceMap?
```

当前 operation type：

```text
replaceProject
createEntity
deleteEntity
updateEntity
addScript
removeScript
setActiveScene
```

规则：

```text
AI 只能输出 Patch Plan
Patch Plan 必须可审查
Patch Plan 必须可验证
Patch Plan 必须进入 Patch History
```

v1 暂不实现完整 rollback，但 patch schema 预留 `sourceMap`。

后续 Patch Rollback MVP 会补：

```text
inverse operation
rollback validation
rollback preview
rollback apply
```

## Validation Report v1

Validation Report v1 字段：

```text
schemaVersion?: validation-report.v1
ok
issues[]
```

issue 字段：

```text
kind
path
message
source?
jump?
```

规则：

```text
path 给机器
message 给用户
source 给 AI 修复和 Patch History
jump 给编辑器定位
```

当前代码中已经使用：

```text
kind / path / message
operationIndex / operationType / target
relatedOperationIndex
```

后续应统一到 Validation Report v1。

## 测试要求

Project Schema v1 必须覆盖：

```text
valid starter project passes
valid shooter project passes
missing project.name fails
invalid activeSceneId fails
invalid scene.gravity fails
invalid entity.transform.position.x fails
asset ref mismatch fails
scene asset points to missing scene fails
patch plan unknown operation fails
validation report missing issue path fails
```

最低回归：

```powershell
npm.cmd run build
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
```

## 当前边界

已经完成：

```text
Project / Scene / Entity / Asset / Patch Plan / Validation Report v1 的最小 schema 契约
schema validator 输出 kind / path / message
schemaVersion 可选兼容旧项目
```

暂未完成：

```text
schemaVersion 强制化
Scene 拆文件
Asset Registry
Asset DB
Component Schema 与 JSON Schema 完全统一
Patch Rollback
Validation Report 完整 source / jump 标准化
```

## 下一步

阶段 1 完成后，进入：

```text
阶段 2：编辑器模块化与服务层
```

但如果 Project Schema v1 在实现中暴露字段缺口，应先修正本文档和 schema，再进入阶段 2。

## Current Implementation Addendum: Asset Patch Operations v1

Current Patch Plan supports asset operations:

```text
replaceAsset
deleteAsset
```

replaceAsset fields:

```text
type: replaceAsset
assetId
replacementAssetId
approvedImpact?: boolean
removeOriginal?: boolean
```

deleteAsset fields:

```text
type: deleteAsset
assetId
approvedImpact?: boolean
```

Validation rules:

```text
replaceAsset requires assetId and replacementAssetId
replaceAsset can only replace with the same Asset type
replaceAsset touching existing references requires approvedImpact=true
deleteAsset cannot remove an Asset that is still referenced
deleteAsset does not physically delete source files
```

Regression coverage:

```powershell
npm.cmd run test:patch
npm.cmd run test:schema
```

## Current Implementation Addendum: Patch Rollback Snapshot v1

Patch History entries can now carry rollback metadata:

```text
rollback?: ProjectPatchRollbackSnapshot
status can include rolled_back
```

Rollback snapshot fields:

```text
schemaVersion: patch-rollback-snapshot.v1
beforeProject
beforeHash
afterHash
strategy: project-snapshot
createdAt
```

Rollback rules:

```text
Successful AI patch apply creates a rollback snapshot
Rollback preview checks current project hash against afterHash
Rollback apply restores beforeProject only when the hash guard passes
Rollback apply writes a rolled_back history entry
Patch History normalization preserves valid rollback snapshots
Invalid or missing rollback snapshots are ignored / rejected by rollback preview
Patch History UI can show rollback readiness and execute rollback
```

Regression coverage:

```powershell
npm.cmd run test:rollback
npm.cmd run test:patch
```

## Current Implementation Addendum: Patch History Replay Debug Summary v1

Patch History entry can now carry replay debug summary metadata:

```text
replayDebug?: ProjectPatchReplayDebugSummary
```

Summary fields:

```text
schemaVersion: runtime-replay-debug-summary.v1
packageId
reason
patchId?
frameCount
differences: high / medium / low
errors[]
createdAt
```

Rules:

```text
Patch History stores compact replay summaries, not full replay packages.
Full RuntimeReplayDebugPackage remains a runtime/tooling artifact and can later be saved as JSON.
normalizePatchHistory preserves valid replayDebug summaries and drops invalid ones.
```
