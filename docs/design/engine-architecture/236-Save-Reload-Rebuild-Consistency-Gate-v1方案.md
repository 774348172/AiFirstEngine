# 236-Save / Reload / Rebuild Consistency Gate v1 方案

> 状态：已施工完成（Gate A-F 与整体回归通过，2026-07-10）。  
> 方案日期：2026-07-10。  
> 自审修订：2026-07-10；已吸收 digest 分层、进程隔离、路径与发布安全、正式 runtime loader、mutation completeness 等修正。  
> 采用方案：`B-min+: Canonical Multi-Checkpoint Consistency Gate`。  
> 路线优先级：`227` 的 `P2-1`。  
> 前置系统：`189`、`217`、`225`、`226`、`228`、`229`、`230`、`231`、`232`、`233`、`234`、`235`。  
> 目标：证明复杂项目经过编辑、保存、关闭重开、清除派生产物和重新构建后，authoring 真相、RuntimePackage 内容与运行时可读取语义保持一致。

## 1. 这个系统是干什么的

直白地说：

```text
用户修改复杂项目
  -> 保存
  -> 终止原编辑器进程并由新进程重新打开项目
  -> 删除预览缓存和旧派生产物
  -> 重新构建 RuntimePackage
  -> Scene / Prefab / Rule / AUI / Input / AssetRef 不能丢失或悄悄变化
```

这个系统不是新的保存按钮，也不是新的项目数据层。

它是一条自动化证据链，用来回答：

```text
编辑器内看见的修改是否真的保存成功？
重新打开项目后是否仍能读到同一语义？
不依赖旧缓存重新构建时，是否得到同一运行包？
RuntimePackage Loader 最终读到的是否仍是同一内容？
如果不一致，具体是哪个 domain、哪个对象、哪个字段发生了变化？
```

在其它成熟引擎中的大致对标：

```text
Unity：SaveScene / SaveAssets + AssetDatabase Refresh / dependency hash + BuildPipeline。
Unreal：SaveDirtyPackages + ReloadPackages + Cook determinism / DiffCook。
Godot：ResourceSaver -> ResourceLoader round-trip + export project files。
```

在本引擎中的作用：

```text
Project Authoring Assets
  -> existing domain save commands
  -> process-isolated reopen / typed loaders
  -> ProjectRuntimePackageAssembler
  -> RuntimePackageBuilder
  -> RuntimePackage Loader
  -> canonical checkpoint comparison
  -> SaveReloadRebuildConsistencyReport
```

## 2. 为什么现在必须做

复杂打飞机主线已经完成：

```text
真实纹理解码与 Sprite textured present。
真实 Project Rule runtime execution。
真实 ProjectUiStateSnapshot / AUI HUD。
导出 Windows Game.exe 的可玩 golden gate。
Editor Build & Run。
Rule Cards、Input Mapping、Asset Browser 原生产品面。
```

但这些完成记录主要证明各系统在当前进程、当前磁盘状态或当前构建产物下能够工作。

当前仍缺少统一证据证明：

```text
编辑结果不是只存在于 EditorSession 内存。
保存后重新加载不会迁移、丢失或改写语义。
RuntimePackage 不依赖旧缓存、旧文件或上一次构建残留。
manifest.contentHash 对所有运行时有效内容都敏感。
同一项目从干净环境重建仍得到等价结果。
```

如果没有这一 Gate，后续 bug 会表现为：

```text
编辑器里正确，重开后丢失。
Play 正确，Build & Run 错误。
增量构建正确，干净机器构建错误。
删除资产后旧 cooked 文件仍残留。
Rule/Input 已变化，但 RuntimePackage contentHash 没变化。
AI 只得到“hash mismatch”，无法知道应修哪个字段。
```

## 3. 本项目当前真实基线与缺口

### 3.1 已有可复用能力

```text
EditorSession 和结构化 UiCommand / ProjectPatch。
SceneSavePipeline。
PrefabWorkflowService / Prefab Stage。
RuleAuthoringService。
AuiAuthoringService。
InputMappingEditorState + source hash guarded transactional save。
ProjectRuntimePackageAssembler。
RuntimePackageBuilder / RuntimePackage Loader。
EditorPreviewPackageFingerprint / dirty domain detection。
Report Panel / project_e2e_gate。
复杂打飞机真实样例项目与确定性 runtime 输入。
```

本系统必须复用这些入口，不给每个 domain 新造导出桥、保存器或测试专用 parser。

### 3.2 当前发现的具体问题

#### 问题 A：RuntimePackage content hash 覆盖不完整

当前 `runtime_package_builder.rs::stable_package_hash`：

```text
Scene 只计算 scene id 和 entity count。
没有完整计算 Scene entity / component / transform / AssetRef。
没有计算 InputMapping document。
没有计算 Rule manifest 的完整内容。
没有计算 component schema 的完整内容。
FontAtlas 主要计算长度和 glyph count，没有覆盖完整 bitmap 内容。
部分 Asset 在缺少 source hash 时只剩 asset id。
```

因此现有 `manifest.contentHash` 不能作为一致性 Gate 的最终证据。

#### 问题 B：RuntimePackage 直接覆盖旧输出目录

当前 `RuntimePackageBuilder` 在目标目录中创建和覆盖文件，但不会先形成完整 staging 产物再发布。

风险：

```text
删除 Prefab / AUI / Input / cooked asset 后，旧文件可能继续残留。
构建中途失败时，目标目录可能同时包含新旧文件。
比较整个目录时，reports 中的 duration/path/request 等易变字段会制造误报。
```

#### 问题 C：保存语义不统一

当前状态：

```text
Scene：内存 working document，显式 Save。
Prefab：Prefab Stage working copy，显式 Save。
Input：editor working copy，source hash guarded Save。
Rule：结构化编辑命令当前直接保存源资产。
AUI：结构化编辑命令当前直接保存源资产。
```

这不是必须统一成一种用户模型，但 Gate 必须知道每个 domain 的真实持久化边界，不能假设所有 domain 都有同一种 dirty/save 生命周期。

#### 问题 D：Prefab 可能在写盘失败前清除 dirty

当前 `PrefabWorkflowService::save_stage` 先把 `stage.dirty` 设为 `false`，随后才执行磁盘写入。

如果写入失败：

```text
文件没有更新。
Prefab Stage 却可能显示为 clean。
后续 Build 从磁盘读取旧 Prefab。
```

本系统必须修正为“持久化成功后才清 dirty”。

#### 问题 E：Play autosave 只覆盖 active Scene

当前 PlayService 会在 Play 前保存 dirty Scene，但不会自动保存 active Prefab Stage 或 Input working copy。

P2-1 不把 Play 改造成全局 Save All；但一致性 Gate 必须：

```text
显式列出所有 dirty working copy。
通过既有 domain save command 保存，或拒绝继续。
禁止静默忽略未保存草稿。
```

#### 问题 F：已有测试是局部测试

当前已有 Scene reload、Input save、AssetRef save/reopen/build 等局部测试，但没有一条统一 Gate 同时覆盖：

```text
Scene
Prefab
Rule
AUI
Input
Asset / AssetRef
BuildProfile / Project manifest
RuntimePackage clean rebuild
RuntimePackage load
```

#### 问题 G：Build recipe 与 Runtime content hash 容易混为一谈

当前 `BuildProfile` 独立存在于 assembly result，且包含：

```text
target / runtime_package_mode
frame_limit
headless_surface_gate
real_window_smoke
```

其中部分字段只控制构建或验证流程，并不进入 RuntimePackage 运行内容；`RuntimePackageBuildInput` 本身也不包含完整 BuildProfile。

因此本方案必须区分：

```text
build_recipe_digest：构建选择、配置、schema/cooker 版本。
assembly_input_digest：ProjectRuntimePackageAssembler 产生的完整 typed build input。
runtime_content_hash：Runtime 真正发布和消费的内容身份。
```

禁止把验证参数机械塞进 `manifest.contentHash`。

#### 问题 H：Runtime payload path 与发布过程缺少完整安全合同

当前 Builder/Loader 会把 manifest、AUI、FontAtlas、Texture 中的相对路径与 package root 直接拼接。

如果不先验证路径和发布所有权，clean staging/publish 可能遇到：

```text
absolute path / `..` 逃逸 staging root。
Windows 大小写折叠后发生文件冲突。
两个 Build/Play 同时发布同一个 final output。
final -> backup -> staging 的中途失败留下不明确状态。
```

236 必须先定义 package-relative path、单写者发布和最小 rollback/recovery 规则，再允许删除或替换旧输出。

#### 问题 I：`load_runtime_package` 不能单独证明所有 payload 已被 Runtime 正式读取

正式 `load_runtime_package` 会加载 active Scene、Asset manifest/index、Rule、AUI、FontAtlas 和 Input，但不会单独物化全部 Prefab 正文或读取 Texture RGBA payload。

因此 `LoadedRuntimePackage` checkpoint 必须复用现有正式 loader 链：

```text
load_runtime_package
RuntimeAssetLoader
RuntimeInstanceLoader / Prefab loader
Runtime texture loader
```

禁止用测试专用 `fs::read` 代替 Runtime loader 后宣称运行时闭环通过。

## 4. 其它引擎源码参考

### 4.1 Unity

本地源码：

```text
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\EditorSceneManager.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Modules\AssetDatabase\Editor\ScriptBindings\AssetDatabase.bindings.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\AssetModificationProcessor.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\DataBuildDirtyTracker.cs
<UNITY_CS_REFERENCE>\UnityCsReference-master\UnityCsReference-master\Editor\Mono\BuildPipeline\BuildPipeline.bindings.cs
<UNITY_LEGACY_SOURCE>\Editor\Src\AssetPipeline\AssetInterface.cpp
```

关键实现：

```text
EditorSceneManager.SaveScene -> SaveSceneInternal。
AssetDatabase.SaveAssets 保存 dirty serialized assets。
AssetModificationProcessor.OnWillSaveAssets 在保存前处理可写性和版本控制。
AssetDatabase.Refresh 检查磁盘变化并触发 import。
AssetInterface::RefreshAndSaveAssets 执行 Refresh -> SaveAssets -> Refresh。
DataBuildDirtyTracker 使用 GetAssetDependencyHash、scene list、build options、module list 和 Unity version 判断构建输入是否变化。
BuildPipeline.BuildPlayer 校验场景和构建参数后进入 BuildPlayerInternal，并返回 BuildReport。
```

可学习：

```text
保存、重导入、构建是三个明确阶段。
构建 dirty 判断使用依赖内容 hash，不依赖单一时间戳。
构建参数和引擎版本也属于结果一致性的输入。
```

不照搬：

```text
不复制 Unity 全局 AssetDatabase 隐式状态。
不依赖 Library 缓存作为项目真相。
不要求普通用户手动 ForceReserializeAssets 才能通过 Gate。
不把 Domain Reload 或全局 Object instance state 引入本系统。
```

在线参考：

```text
https://docs.unity3d.com/6000.0/Documentation/ScriptReference/AssetDatabase.GetAssetDependencyHash.html
https://docs.unity3d.com/6000.0/Documentation/ScriptReference/AssetDatabase.Refresh.html
https://docs.unity3d.com/6000.0/Documentation/ScriptReference/AssetDatabase.ForceReserializeAssets.html
```

### 4.2 Unreal Engine

本地源码：

```text
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\FileHelpers.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\PackageTools.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Runtime\CoreUObject\Private\UObject\PackageReload.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\Cooker\DiffPackageWriter.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\Cooker\CookDeterminismManager.cpp
<UNREAL_ENGINE_SOURCE>\UnrealEngine-release\UnrealEngine-release\Engine\Source\Editor\UnrealEd\Private\Commandlets\DiffCookCommandlet.cpp
```

关键实现：

```text
FEditorFileUtils::SaveDirtyPackages 收集 dirty package，再进入 InternalSavePackages。
UPackageTools::ReloadPackages 按依赖排序 package，卸载/重载并修复对象引用与编辑器状态。
FDiffPackageWriter 比较当前 cook 和 previous cooked bytes。
发现差异后 IsAnotherSaveNeeded 触发第二次 save，收集 Serialize 调用栈、对象、属性和差异统计。
FDeterminismManager 保存 old/new diagnostics，解释 package/export 为什么变化。
UDiffCookCommandlet 按 package 输出 Added / Removed / Modified，并报告首个字节差异位置。
```

可学习：

```text
authoring package 与 cooked output 必须分别验证。
只给一个总 hash 不够，必须有 package/domain/path 级诊断。
确定性 Gate 应支持旧结果与新结果比较，并能解释差异来源。
```

不照搬：

```text
不复制 UObject package reload、对象重定向和完整 Cooker/DDC。
不把二进制 byte diff 作为唯一正确性标准。
不在本轮引入大型 content-addressed cook graph。
```

在线参考：

```text
https://dev.epicgames.com/documentation/en-us/unreal-engine/cooking-content-in-unreal-engine
https://dev.epicgames.com/documentation/en-us/unreal-engine/derived-data-cache
```

### 4.3 Godot

本地源码：

```text
<GODOT_SOURCE>\godot\editor\editor_node.cpp
<GODOT_SOURCE>\godot\core\io\resource_saver.cpp
<GODOT_SOURCE>\godot\editor\export\editor_export_platform.cpp
<GODOT_SOURCE>\godot\tests\core\io\test_resource.cpp
```

关键实现：

```text
EditorNode::_save_scene 把编辑场景打包后交给 ResourceSaver::save。
EditorNode::reload_scene 从磁盘重新加载场景。
EditorExportPlatform::export_project_files / save_pack 读取项目资源生成发布包。
test_resource.cpp 对 text/binary resource 执行 save -> load round-trip 并比较数据。
```

可学习：

```text
第一版一致性验证可以保持简单：保存、重新加载、比较 typed semantic value。
round-trip 测试应直接覆盖真实 loader/saver，而不是复制解析逻辑。
```

不照搬：

```text
不把 path-only resource reference 作为长期身份。
不让 export 直接读取 editor-only memory object。
```

在线参考：

```text
https://docs.godotengine.org/en/stable/classes/class_resourcesaver.html
```

## 5. 候选方案与正式选择

### 方案 A：File Hash Round-trip Gate

```text
保存后比较源文件字节。
重建后比较输出目录字节。
```

优点：施工最小。  
缺点：JSON 格式、报告耗时、绝对路径会误报；当前弱 content hash 会漏报；AI 无法定位语义字段。  
结论：不采用。

### 方案 B-min+：Canonical Multi-Checkpoint Consistency Gate

```text
使用现有 typed loader 和 builder。
为 authoring、build recipe、assembly input、runtime payload、loaded package 建立分层规范摘要。
保存、进程隔离重开、干净重建分别形成 checkpoint。
按 domain/path 输出结构化 mismatch。
只修复 Gate 揭露的保存和构建正确性缺陷。
```

优点：

```text
AI 可以定位具体 domain/path。
避免原始 JSON 格式误报。
覆盖复杂项目长期维护所需的跨会话和干净构建。
不新增运行时架构层。
后续可以追加双 Runtime 回放，不推翻 checkpoint/report schema。
```

缺点：需要补完整 canonical digest、进程隔离 checkpoint、safe staging publish 和少量保存缺陷。  
结论：正式采用。

### 方案 C：Content-addressed Reproducible Build System

```text
完整 CAS/DDC。
每个 domain 独立 artifact key。
增量构建与 clean build 等价证明。
双 Runtime 固定输入回放。
跨机器/跨平台构建矩阵。
```

优点：长期上限最高。  
缺点：会把本系统扩张成完整 Build Graph / DDC / reproducible build 工程。  
结论：长期方向，本轮只预留兼容字段，不施工。

## 6. 正式架构边界

### 6.1 不新增持久化真相层

唯一真相保持不变：

```text
编辑真相：Project typed authoring assets。
构建输入真相：ProjectRuntimePackageAssembler 产生的 RuntimePackageBuildInput。
运行真相：RuntimePackage。
```

本轮新增的 checkpoint / digest / report 都是：

```text
editor/test evidence
可丢弃
可重新生成
不被 Runtime 消费
不被用户作为项目资产编辑
```

禁止新增：

```text
ConsistencyAsset
ProjectSnapshotAsset
第二套项目 manifest
第二套 RuntimePackage assembler
Runtime 常驻 consistency service
每个 domain 独立 consistency manager
```

### 6.2 单一 Gate 编排

正式 Gate：

```text
SaveReloadRebuildConsistencyGate
```

职责：

```text
在复杂打飞机临时副本执行结构化编辑。
调用现有 domain save command。
由进程 A 生成 SavedAuthoring checkpoint artifact 后正常退出。
由独立进程 B 重新 OpenProject 并生成 ReopenedAuthoring checkpoint artifact。
分别计算 build recipe、assembly input、runtime content 和 loaded semantics 摘要。
执行独立 clean build。
从已发布 final output 加载两个 RuntimePackage，并调用正式 RuntimeAsset/Prefab/Texture loader。
比较规范语义并写报告。
```

不负责：

```text
直接修改 JSON。
实现新的 domain save 规则。
替代 ProjectRuntimePackageAssembler。
替代 RuntimePackageBuilder。
启动常驻 runtime telemetry。
```

## 7. Checkpoint 合同

正式 checkpoint 顺序：

```text
A. SavedAuthoring
B. ReopenedAuthoring
C. FirstRuntimeBuild
D. CleanRebuild
E. LoadedRuntimePackage
```

其中 `B. ReopenedAuthoring` 的正式 E2E 证据必须是 `process_isolated`。同进程 `drop EditorSession -> new EditorSession` 只保留为快速单元/集成测试，不能单独让完整 Gate 通过。

### 7.1 Checkpoint A：SavedAuthoring

在复杂打飞机临时副本中通过真实命令修改：

```text
Scene：transform/component/AssetRef 中至少一个真实字段。
Prefab：Prefab Stage 中至少一个实体字段。
Rule：至少一个 Rule Card / IR 对应字段。
AUI：至少一个 node field、binding 或 image AssetRef。
Input：至少一个 stable binding_id 对应字段。
Asset：至少一个 typed asset descriptor 或真实 PNG reference。
```

保存规则：

```text
Scene / active Prefab Stage / Input working copy 必须通过现有 Save command。
Rule / AUI 记录其结构化编辑命令已经持久化的 source hash。
保存失败立即终止 Gate。
保存成功后仍有 dirty working copy 时 Gate 失败。
禁止测试直接 fs::write 模拟用户保存。
```

Checkpoint A 记录：

```text
project-relative source paths
domain semantic digests
stable object ids
expected witness values
source file digests
dirty state after save
save transaction ids / status
producer_process_id（只作 Trace 证据，不进入 digest）
producer_invocation_id（只作进程隔离证据，不进入 semantic digest）
reopen_handoff_artifact
```

Checkpoint artifact 只写入 Gate 临时 workspace，属于可丢弃测试证据，不成为项目资产或新的持久化真相。

### 7.2 Checkpoint B：ReopenedAuthoring

正式 E2E 必须先结束进程 A，再由独立进程 B 创建全新 `EditorSession`：

```text
process A writes SavedAuthoring checkpoint -> exit success
process B starts with the same temp project path
  -> new EditorSession
  -> OpenProject
  -> 通过现有 typed loader 打开相关 Scene / Prefab / Rule / AUI / Input
  -> 构造 ReopenedAuthoring checkpoint
  -> report reopen_mode=process_isolated
```

进程 B 不得接收进程 A 的内存对象、typed document 或 assembler result；进程间只允许传递临时项目路径、场景/资产 stable id 和 checkpoint artifact 路径。

比较：

```text
SavedAuthoring.domain_semantic_digest
  == ReopenedAuthoring.domain_semantic_digest
```

此外必须检查代表性 witness：

```text
Scene entity/component/AssetRef。
Prefab entity field。
Rule IR hash / operation value。
AUI node/binding/action/image ref。
Input action/binding_id/device path。
Asset GUID/id/source hash。
```

不能只比较总 hash。

### 7.3 Checkpoint C：FirstRuntimeBuild

从 ReopenedAuthoring 已确认的磁盘状态执行：

```text
project manifest + selected BuildProfile + build request semantics
  -> BuildRecipeDigest A
ProjectRuntimePackageAssembler
  -> RuntimePackageBuildInput A
  -> AssemblyInputDigest A
  -> RuntimePackageBuilder
  -> safe staging/publish
  -> published RuntimePackage A final output
  -> RuntimeContentHash A
  -> runtime payload tree digest A
```

`BuildRecipeDigest` 至少覆盖：

```text
selected project manifest semantics
selected BuildProfile semantics
active scene selection
RuntimePackageBuildRequest 中影响构建选择的字段
engine/package/schema/cooker version inputs
```

`previous_package_manifest`、output path、report level、request id、duration 和只影响验证流程的运行参数可以进入 Trace，但不得进入 `RuntimeContentHash`。

### 7.4 Checkpoint D：CleanRebuild

必须证明不依赖旧缓存：

```text
关闭旧 session。
删除 Gate 临时目录中的 preview cache / prior derived outputs。
从项目源文件重新运行 ProjectRuntimePackageAssembler。
重新生成 BuildRecipeDigest B / AssemblyInputDigest B。
构建到另一组新的空 staging/final 目录。
得到已发布 RuntimePackage B。
```

比较：

```text
build_recipe_digest A == build_recipe_digest B
assembly_input_digest A == assembly_input_digest B
runtime_payload_tree_digest A == runtime_payload_tree_digest B
manifest.contentHash A == manifest.contentHash B
runtime-consumed relative file set A == B
```

### 7.5 Checkpoint E：LoadedRuntimePackage

分别从已发布的 final output 加载 A/B：

```text
published RuntimePackage A
  -> load_runtime_package
  -> RuntimeAssetLoader all inventory-declared typed assets
  -> RuntimeInstanceLoader representative Prefab
  -> runtime texture loader all packaged cooked textures
  -> LoadedSemanticDigest A

published RuntimePackage B
  -> same formal loader chain
  -> LoadedSemanticDigest B
```

比较：

```text
active scene full semantic value
prefab document loaded through RuntimeAssetLoader and representative instance materialization
rule manifest / modules
input manifest / mappings
AUI manifest / documents
font atlas metadata + bitmap digest
cooked texture metadata + RGBA payload loaded through the formal texture path
AssetRef resolve result
```

注意：

```text
Authoring digest 不直接要求等于 Runtime digest。
因为 Prefab bake、Rule manifest、texture cook、font atlas cook 会改变表示形式。
二者通过明确 witness / source mapping 验证，而不是错误地要求字节相等。
```

每个 required witness 必须有机器可读映射：

```text
SourceRuntimeWitness
  witness_id
  domain
  source_path
  source_object_id
  source_field_path
  build_input_path
  runtime_path
  runtime_object_id
  expected_semantic_value_digest
  actual_semantic_value_digest
  status
```

映射由正式 assembler/builder/loader 结果生成或验证；禁止只在复杂打飞机测试中手写一份永远会通过的 expected 表。

## 8. Canonical Semantic Digest 规则

### 8.1 Digest 结构

每个 digest 必须自描述：

```text
ConsistencyDigest
  schema_version
  kind
  algorithm
  canonical_encoding
  value
```

v1 正式值：

```text
schema_version = consistency-digest.v1
kind = authoring_domain | build_recipe | assembly_input | runtime_content | runtime_payload_tree | loaded_semantics
algorithm = sha256
canonical_encoding = aife-canonical-framed.v1
value = lowercase hex
```

`kind` 必须进入 hash preimage，形成 domain separation；禁止使用未标注算法/编码的裸字符串作为长期合同。

现有 `RuntimePackageManifest.contentHash` 在 `runtime-package.v1` 中仍是 string。为保持 schema 兼容，Builder 正式写入：

```text
sha256:<64 lowercase hex>
```

报告/checkpoint 中仍使用完整 `ConsistencyDigest` 对象。Loader 可以继续读取历史 opaque/legacy string；但 236 Gate 只把符合 `sha256:<hex>` 合同并可用同一 canonical preimage 重算的值视为新一致性证据。不得为了结构化 hash 单独升级 RuntimePackage schema 或新增第二 manifest。

正式 preimage 使用长度分帧，不能用 `parts.join("|")`：

```text
magic = "AIFE-CONSISTENCY\0"
frame(schema_version)
frame(kind)
frame(payload_schema_version)
frame(canonical_payload_bytes)
```

`frame(x)` 必须包含确定宽度的 byte length 和原始 bytes，避免不同字段组合产生相同拼接结果。大尺寸 FontAtlas/Texture payload 必须流式送入 SHA-256，不要求先复制成巨型 JSON/string。

### 8.2 规范化原则

```text
先用正式 typed loader 解析，再序列化规范语义。
对象字段按 UTF-8 key bytes 稳定排序。
路径统一为 project-relative `/` 形式。
无序集合按 stable id 排序。
有语义顺序的数组必须保留顺序。
JSON 空格、缩进、对象 key 原始顺序不影响 semantic digest。
enum/union 必须写入显式 type tag。
字符串按正式 typed value 的 UTF-8 bytes 编码，不擅自做 Unicode normalization 或大小写折叠。
整数使用规范十进制；有限浮点使用稳定 shortest-round-trip 表示，`-0` 规范为 `0`。
NaN / Infinity 或无法规范编码的数值直接产生 canonical_encode_failed。
```

missing / null / default 的语义由正式 schema 与 typed loader 决定：loader 规范化为同一 typed value 时 digest 相同；仍是不同 enum/option/value 时编码器不得自行合并。

必须保留顺序的示例：

```text
Scene sibling order。
Prefab hierarchy / sibling order。
Rule statement / operation execution order。
AUI child/render order。
Input context priority 和 resolver 使用的 binding order。
```

禁止为了“哈希稳定”把这些数组盲目排序，否则会掩盖真实行为变化。

禁止为每个 domain 手写 `parts.push(...)` 式字段清单。canonical contribution 必须来自完整 typed value 或 Builder 的单一内部 write plan；新增 typed 字段默认进入摘要，只有显式标注为非语义字段才能排除，并必须有测试说明。

### 8.3 排除的易变字段

不得进入 semantic digest：

```text
absolute project/output path
duration_ms
timestamp / last_success_at
request_id / transaction_id
report file path
editor selection / hover / scroll
dirty flag / in-memory revision
process id
diagnostic display ordering中不影响语义的部分
```

排除规则必须按 digest kind 生效，不能用一张全局忽略表掩盖真实输入。例如 BuildProfile 的 `frame_limit` 可以进入 `build_recipe_digest`，但在它不改变 RuntimePackage payload 时不得机械进入 `runtime_content_hash`。

### 8.4 Digest 分层与作用域

#### BuildRecipeDigest

```text
project manifest typed semantics
selected BuildProfile typed semantics
active scene selection
build target / mode / output-affecting options
engine/package/schema/cooker versions
```

它回答“使用了什么构建配方”，不直接写入 `manifest.contentHash`。

#### AssemblyInputDigest

覆盖 `ProjectRuntimePackageAssembler` 输出的完整 `RuntimePackageBuildInput` typed semantics，包括有语义顺序和 stable identity。它不包含 assembly duration、diagnostic 顺序或绝对项目路径。

#### RuntimeContentHash / `manifest.contentHash`

它回答“这个 RuntimePackage 发布并由 Runtime 消费的内容是什么”。必须覆盖：

```text
package schema/mode and emitted project info
active scene id
complete RuntimeScene values
complete RuntimePrefab documents
component schema
asset identities / GUID / type / dependencies / runtime URI / source content hash
complete Rule manifest / module references / IR hash
complete InputMapping documents
complete AUI manifest / documents
font atlas metadata + atlas bitmap content digest
texture metadata + RGBA payload content digest
```

明确排除：

```text
manifest.contentHash 自身
BuildProfile 中只控制验证/启动但不改变 RuntimePackage 的字段
previous_package_manifest
absolute output/project path
request/transaction id
reports/** 和 duration/timestamp
```

target、mode、cooker version 等构建配方只有在它们作为 Runtime 消费字段被实际写入 package，或改变了 emitted payload 时，才通过对应 emitted semantic value 影响 `RuntimeContentHash`。

现有 `stable_package_hash` 必须由同一 canonical digest 实现替代或升级，禁止 Gate 和 Builder 各自维护一套不同 hash 算法。

Builder 应在内部先形成单一 runtime write plan/inventory，再由同一份 plan：

```text
计算 RuntimeContentHash
写入 runtime payload
生成 runtime-consumed file inventory
执行 post-write verification
```

这是 `RuntimePackageBuilder` 的内部实现深化，不是新的持久化资产、公共 Router 或架构层。

### 8.5 Runtime payload inventory 与 path 合同

每个准备写入或加载的 RuntimePackage path 必须先通过：

```text
非空 package-relative path
统一 `/`
禁止 absolute/root/drive prefix
禁止 `.` / `..` component
join 后仍位于 canonical staging/package root 内
禁止穿过逃逸 root 的 symlink/reparse point
Windows collision key（case-fold、尾随点/空格、保留设备名）后仍全局唯一
文件名来源的 scene/prefab/asset/document id 必须是安全单段 path segment
```

同一个规范路径被两个 payload claim，或两个路径在 Windows 大小写折叠后冲突，必须在写盘前失败。

Runtime payload inventory 由 Builder write plan 和 manifest/runtime URI 引用共同验证；不能只靠 Gate 维护一张容易漏更新的目录列表。新增 payload 类型若没有进入 inventory，构建必须失败而不是静默跳过。

### 8.6 Runtime payload tree digest

除 semantic content hash 外，Gate 还计算运行时实际消费文件树：

```text
relative_path + file_content_digest
```

包括：

```text
manifest.json
scenes/**
prefabs/**
assets/**
input/**
rules/**
aui/**
fonts/**
cooked/**
schema/**
```

排除：

```text
reports/**
editor preview report
build duration / process report
```

原因：报告本身包含易变诊断和耗时，不是 Runtime 消费内容。

## 9. Save 正确性规则

### 9.1 成功后才能清 dirty

统一合同：

```text
validate
-> serialize
-> write/replace succeeds
-> refresh source hash / path
-> clear dirty
-> committed report
```

任何失败：

```text
保留 dirty。
保留原文件或恢复 backup。
返回 Failed。
报告 source path / domain / error / next action。
```

Prefab 必须修正当前“save_stage 先清 dirty”的顺序。

### 9.2 原子文件替换

本轮允许提取一个项目无关的文件写入 helper：

```text
validate target ownership/path
-> create unique temp with create-new semantics in the same directory
-> write all bytes
-> flush + file sync where the platform supports it
-> backup existing target when platform requires
-> rename temp to target
-> restore backup on failure
-> remove backup on success
```

它是 IO helper，不是新的 Save Manager 或架构层。

v1 保证写入错误、rename 错误和进程内提交失败时不把未完成文件当成功；不宣称已经实现跨断电 durable transaction 或通用 crash journal。父目录 sync 能力按平台封装并通过 best-effort/unsupported 状态诚实报告。

helper 的复杂失败分支必须通过内部 fault-injection seam 测试，业务 saver 只依赖一个小接口，不能让每个 domain 自己复制 temp/backup/rollback 流程。

优先让 Scene / Prefab / Rule / AUI 复用；Input 已有 transactional write，施工时应收敛到同一正确性合同，避免长期出现多份不同实现。

### 9.3 本轮不新增全局 Save All 产品层

Gate 可以按已知 dirty working copy 调用现有 save command，但不新增：

```text
GlobalSaveManager
SaveOwnershipRouter
第二套 document registry
```

未来如果用户需要完整 Save All UI，再独立讨论产品入口；不能借一致性 Gate 增加新架构层。

## 10. RuntimePackage clean staging / publish

为避免旧文件残留和半成品覆盖，RuntimePackage build 应遵守：

```text
canonical owned final_output parent
  -> acquire single-writer publish guard for final_output
  -> recover package-local orphan backup/staging state
  -> create unique staging sibling on the same volume
  -> write all runtime payloads
  -> write manifest
  -> load_runtime_package(staging)
  -> formal inventory-wide asset/prefab/texture load validation
  -> validation success
  -> final_output -> unique backup when final exists
  -> staging -> final_output
  -> load/verify final_output again
  -> remove backup and release publish guard
```

规则：

```text
staging 必须是空目录。
staging/backup 必须是 final 的 sibling，并位于 Builder 明确拥有的 output root 内。
所有 payload path 必须先通过 8.5 的 containment/collision 校验。
成功发布后 final output 不能包含不在本次 payload file set 中的旧 runtime 文件。
同一 final output 同时只允许一个 publisher；冲突立即返回 output_publish_busy，不隐式互相覆盖。
publish 前失败时保留 last-good final output，不把 staging 当成功产物。
final replace 或 post-publish load 失败时恢复 backup；恢复失败必须保留证据并返回 output_publish_rollback_failed。
启动发布时若发现 final 缺失而 backup 存在，先恢复 backup；若 final 与 backup 同时存在，则先正式 load/verify final，成功后保留 final 并清理 backup，失败时恢复 backup。
Gate 的 A/B 两次 build 必须使用独立空目录。
Gate 所有 publish/cleanup 只操作临时 workspace 下经过 containment 验证的目录，不触碰仓库样例或用户真实 preview/export output。
```

stale artifact 不能只靠“两个空目录构建结果相等”来证明。Gate C 必须至少执行：

```text
在临时 final output 中放入旧 runtime sentinel 或先构建包含额外 payload 的旧版本
-> 通过正式 staging/publish 构建新版本到同一 final output
-> 证明旧 payload/sentinel 不存在
-> 对新 final 运行 inventory + formal loader verification
```

failed-build-preserves-last-good 必须使用 fault injection 或确定性失败输入验证，不能依赖权限错误等平台偶发现象。

本轮不实现内容寻址缓存、增量 artifact graph 或远程 DDC。

## 11. 比较矩阵

| 比较 | 必须相等 | 不直接比较 |
|---|---|---|
| SavedAuthoring vs ReopenedAuthoring | 各 domain semantic digest、stable ids、witness values | editor selection、dirty revision |
| BuildRecipe A vs BuildRecipe B | selected project/profile/request/schema/cooker canonical digest | output path、request id、duration |
| AssemblyInput A vs AssemblyInput B | complete canonical `RuntimePackageBuildInput` digest | assemble duration、diagnostic 展示顺序 |
| RuntimePackage A vs B | contentHash、runtime payload file set/tree digest | reports/**、absolute output path |
| Loaded Package A vs B | Scene/Prefab/Rule/Input/AUI/Asset/Font/Texture formal-loader semantic digest | loader timing、process id |
| Authoring vs Runtime | 明确的 source mapping 与 witness | authoring/runtime 原始字节总 hash |

## 12. Mutation Sensitivity Gate

一致性 hash 不能只证明“同样输入得到同样值”，还必须证明“有效变化会改变正确的 digest”。

mutation coverage 分为两层：

```text
unit/schema layer：验证每个 canonical top-level typed contribution 和关键有序字段。
product-path layer：通过真实 command/save -> process-isolated reopen -> assembler -> builder -> loader 验证每个项目 domain 至少一个代表性变化。
```

正式最小矩阵：

```text
Scene transform/component value 改变 -> Scene + AssemblyInput + RuntimeContentHash 改变。
Prefab entity/component value 改变 -> Prefab + AssemblyInput + RuntimeContentHash 改变。
Rule IR hash/operation 改变 -> Rule + AssemblyInput + RuntimeContentHash 改变。
AUI node/binding/image ref 改变 -> AUI + AssemblyInput + RuntimeContentHash 改变。
Input binding device/trigger 改变 -> Input + AssemblyInput + RuntimeContentHash 改变。
ProjectManifest project/default scene 语义改变 -> Authoring/BuildRecipe；影响 emitted manifest/scene selection 时 RuntimeContentHash 改变。
BuildProfile 验证参数改变 -> BuildRecipeDigest 改变；未改变 emitted payload 时 RuntimeContentHash 保持不变。
active scene selection 改变 -> BuildRecipe + emitted manifest/RuntimeContentHash 改变。
component schema 字段改变 -> AssemblyInput + RuntimeContentHash 改变。
FontAtlas metadata 或 atlas bitmap byte 改变 -> AssemblyInput + RuntimeContentHash 改变。
Texture decoded RGBA/metadata 改变 -> Asset/Texture + RuntimeContentHash 改变。
AssetRef id/guid/type/dependency/runtime URI 改变 -> source domain + RuntimeAssetIndex / RuntimeContentHash 改变。
删除 Prefab/AUI/Input/Texture payload -> inventory/file set/RuntimeContentHash 改变，旧 final 中对应文件消失。
Scene sibling、Prefab hierarchy、Rule operation、AUI child/render、Input resolver priority 顺序改变 -> 对应 semantic digest 改变。
```

同时验证无语义变化：

```text
JSON 空格/缩进变化 -> semantic digest 不变。
JSON object key 顺序变化 -> semantic digest 不变。
reports duration/path 变化 -> runtime payload semantic digest 不变。
BuildProfile 中只影响 headless 验证的字段变化 -> RuntimeContentHash 不变。
同一 typed default 的缺省写法与显式默认写法 -> loader 规范化后 semantic digest 相同。
```

如果有效字段变化后目标 digest 未变化，Gate 必须以 `digest_insensitive` 失败；如果只影响 BuildRecipe 的字段错误地改变 RuntimeContentHash，则以 `digest_scope_violation` 失败。

新增 runtime payload type 或 canonical typed field 时，必须同时满足：

```text
进入 Builder 单一 write plan/inventory。
进入对应完整 typed canonical payload。
至少一条 sensitivity 或 scope test。
没有 owner/inventory/digest contribution 时 fail closed。
```

禁止只增加一条手工 `parts.push` 后宣称 schema coverage 已完成。

## 13. Report 合同

正式报告：

```text
save-reload-rebuild-consistency-report.v1
```

建议结构：

```text
SaveReloadRebuildConsistencyReport
  schema_version
  scenario_id
  status
  report_level
  project_id
  project_root
  reopen_evidence
  checkpoints[]
  comparisons[]
  domain_results[]
  source_runtime_witnesses[]
  mutation_coverage[]
  artifacts[]
  diagnostics[]
  next_actions[]
```

Checkpoint：

```text
checkpoint_id
kind
status
producer_process_id?
producer_invocation_id?
reopen_mode?
domain_digests
build_recipe_digest?
assembly_input_digest?
runtime_content_hash?
runtime_payload_tree_digest?
loaded_package_digest?
source_paths
```

Mismatch：

```text
code
domain
stage
path
object_id?
expected_digest/value?
actual_digest/value?
human_explanation
suggested_fix
```

正式 diagnostic code 至少包括：

```text
consistency.save_failed
consistency.dirty_after_save
consistency.process_reopen_not_isolated
consistency.authoring_reload_mismatch
consistency.build_recipe_mismatch
consistency.assembly_input_mismatch
consistency.runtime_content_hash_mismatch
consistency.unsupported_content_hash_algorithm
consistency.runtime_payload_mismatch
consistency.loaded_package_mismatch
consistency.asset_ref_unresolved
consistency.digest_insensitive
consistency.digest_scope_violation
consistency.canonical_encode_failed
consistency.unsafe_runtime_path
consistency.runtime_path_collision
consistency.stale_runtime_artifact
consistency.output_publish_busy
consistency.output_publish_failed
consistency.output_publish_rollback_failed
```

## 14. Report 分档与性能

### Editor Off

```text
不自动运行一致性 Gate。
不扫描项目，不构建包，不生成 JSON report。
```

### Editor Summary

```text
只展示最新 Gate 状态。
checkpoint pass/fail。
domain mismatch counts。
top diagnostics / next actions。
```

### Editor Trace / Test

```text
完整 domain digest。
完整 source/runtime mapping。
完整 mismatch path。
runtime payload file set。
mutation sensitivity evidence。
process-isolated reopen evidence。
publish/rollback/stale-artifact evidence。
```

### Runtime

```text
正式 Runtime 默认 Off。
Runtime 不加载 ConsistencyGate 或其 report model。
manifest.contentHash 是功能性 package metadata，不等同于开启 report。
```

一致性 Gate 是 CI、施工 Gate 或用户显式验证命令，不在每次 Play、每帧或普通编辑操作中自动运行。

## 15. Report Panel 与 AI 使用规则

Report Panel 只注册一个 provider：

```text
validation.save_reload_rebuild
```

它读取当前 active project 对应的最新 report artifact，不在 UI compose 时执行 Gate。

作用域规则：

```text
provider key = active project stable id + provider id
当前项目没有 report -> NotRun
切换项目后禁止继续展示上一项目的 Passed/Failed
Summary 不暴露 temp workspace 绝对路径
Trace 才展示 process/path/publish 细节
```

AI 默认使用：

```text
status
failed checkpoint
domain
source path
object id / field path
expected / actual
suggested fix
```

AI 修复规则：

```text
只修改 authoring source 或对应 saver/builder。
禁止修改 RuntimePackage cooked 输出作为长期修复。
禁止只更新 expected hash 掩盖 mismatch。
修复后必须重新完整执行 Gate。
```

## 16. 复杂打飞机验收场景

所有破坏性验证在 `samples/complex_shooter_project` 的临时副本中执行，禁止直接修改仓库样例源目录。

最小链路：

```text
复制 complex shooter project 到 temp workspace
-> parent Gate process creates process-isolated handoff paths
-> child process A: OpenProject
-> child process A: 修改 Scene 中一个真实 entity/component/AssetRef
-> child process A: 修改一个 Prefab Stage 字段并 Save
-> child process A: 修改一个 Rule Card 字段
-> child process A: 修改一个 AUI node/binding/image field
-> child process A: 修改一个 Input binding_id 对应字段并 Save
-> child process A: 通过 Asset Browser/正式 domain command 修改一个真实 AssetRef/asset
-> child process A: 保存 Scene，写 SavedAuthoring checkpoint artifact，正常退出
-> child process B: 独立启动，new EditorSession + OpenProject
-> child process B: typed load，写 ReopenedAuthoring checkpoint artifact，正常退出
-> parent verifies two distinct OS child invocations, invocation tokens and exit statuses, then compares A/B
-> seed owned temp final A with stale runtime sentinel
-> build recipe A + assemble input A + safe publish RuntimePackage A
-> verify stale sentinel removed and load published final A
-> 清除 temp derived/cache
-> fresh build recipe B + assemble input B + safe publish RuntimePackage B to independent final
-> compare recipe/input/contentHash/inventory/tree digest A/B
-> load published final A/B through RuntimePackage + Asset + Prefab + Texture formal loaders
-> verify source/runtime witness mapping and runtime AssetRef / Rule / Input / AUI / Prefab / texture/font evidence
-> write report
```

必须覆盖的真实样例域：

```text
project.aife.json
BuildProfiles/windows.dev.json
Scenes/Main.scene.json
Prefabs/*.prefab.json
Rules/*.rule.json + rule-manifest.json
AUI/hud.aui.json
Input/input.default.json
Assets/*.asset + Assets/Images/*.png
RuntimeAssetIndex
```

`component_schema` 即使当前复杂打飞机使用 Builder 默认值，也必须进入 RuntimeContentHash，并由 Builder unit sensitivity 覆盖；项目侧出现正式 component schema source 后，自动升级为 product-path witness，不能等待另一个一致性系统。

Passed 条件：

```text
所有 save transaction 成功。
所有显式 working copy 保存后 clean。
ReopenedAuthoring 由独立 OS child invocation 产生，两个 child process 均正常退出；不能只靠可能复用的 PID 判断。
SavedAuthoring == ReopenedAuthoring。
BuildRecipe A == B。
AssemblyInput A == B。
Runtime payload A == B。
Loaded RuntimePackage A == B。
所有代表性 witness 保留。
所有 AssetRef 可 resolve。
inventory 声明的 typed assets/Prefab documents/Textures 全部通过正式 runtime loader；至少一个 Prefab 完成代表性实例化。
所有 mutation sensitivity test 通过。
所有 digest scope test 通过。
没有 stale runtime artifact。
所有 runtime path 均通过 containment/collision 校验。
发布失败注入证明 last-good 能恢复。
```

## 17. 与现有系统的关系

### 与 189 ProjectRuntimePackageAssembler

```text
189 仍是唯一项目目录 -> RuntimePackageBuildInput 入口。
236 只比较它在两个独立 checkpoint 产生的结果。
```

### 与 217 Editor Preview Package

```text
217 的 fingerprint 用于高频 Play 缓存判断。
236 的 canonical digest 用于低频严格一致性证明。
二者可以共享规范 hash helper，但职责不同。
236 不把严格 clean rebuild 塞进每次 Play。
```

### 与 225/226 Authoring Asset / Prefab Bake

```text
225/226 定义 authoring asset 完整性和 Prefab bake。
236 验证这些结果保存、重开、干净 bake 后一致。
```

### 与 228-230 Runtime 内容系统

```text
228 提供真实 texture cook/present。
229 提供 Rule runtime execution。
230 提供 ProjectUiStateSnapshot / HUD state。
236 只验证其输入在重建前后没有变化，不重做各自功能实现。
```

### 与 231 Exported Windows Golden Gate

```text
231 证明一个导出的 Game.exe 能运行并满足玩法/HUD/贴图证据。
236 证明同一 authoring 项目在重开和干净重建后产生等价 RuntimePackage。
236 v1 不重复启动两次 exported process；双 Runtime 回放属于方案 C 后续增强。
```

### 与 232 Build & Run

```text
232 是用户产品入口。
236 是低频验证 Gate。
未来 Build 面板可以显示 latest consistency status，但普通 Build & Run 不强制每次执行完整 Gate。
```

### 与 233/234/235

```text
233 Rule Cards、234 Input Mapping、235 Asset Browser 提供真实结构化编辑入口。
236 必须通过这些 domain command 或它们复用的 service 修改临时项目。
禁止为测试直接改 JSON 绕过产品链路。
```

## 18. 本轮明确不做

```text
完整 CAS / DDC / remote cache。
完整增量 Build Graph。
跨机器、跨操作系统 byte-identical build matrix。
Windows exe 本体 byte-for-byte reproducibility。
代码签名、installer、store package。
双 Runtime 固定输入回放。
完整 runtime frame/world/UI/render semantic replay diff。
全局 Save All 产品面和 document ownership framework。
文件 watcher / external change merge UI。
通用 crash-recovery journal。
source control checkout 产品化。
自动修复 mismatch。
```

这些能力不能回塞进 236 B-min+；成为真实阻塞后再独立讨论。

## 19. 预期涉及模块

优先新增或扩展：

```text
rust/crates/engine_runtime/src/runtime_package_builder.rs
  canonical RuntimeContentHash + internal write plan/inventory
  manifest.contentHash
  runtime path validation
  single-writer clean staging/publish/rollback

rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/runtime_asset_loader.rs
rust/crates/engine_runtime/src/runtime_instance_loader.rs
  loaded package/asset/prefab/texture semantic verification helper（如需要）

rust/crates/editor_core/src/project_runtime_package_assembler.rs
  BuildRecipeDigest / AssemblyInputDigest
  canonical source mapping / stable ordering corrections（只在必要时）

rust/crates/editor_core/src/scene_editing.rs
rust/crates/editor_core/src/prefab_workflow.rs
rust/crates/editor_core/src/aui_authoring.rs
rust/crates/editor_core/src/rule_authoring.rs
rust/crates/editor_core/src/input_mapping_authoring.rs
  save-after-success / shared atomic write contract

rust/crates/editor_core/src/report_panel.rs
  validation.save_reload_rebuild provider

rust/crates/project_e2e_gate/src/save_reload_rebuild_consistency.rs
  parent orchestration/report + process-isolated checkpoint protocol

rust/crates/project_e2e_gate test helper executable（仅当现有 executable 无法承载 checkpoint child mode）
  child A author/save/checkpoint
  child B reopen/load/checkpoint

rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs
```

实际施工文件以施工文档和代码复核为准，不因本节列表机械扩大范围。

## 20. 推荐施工 Gate

### Gate A：Canonical Digest Contract

```text
ConsistencyDigest schema。
domain-separated + length-framed canonical encoding。
BuildRecipeDigest / AssemblyInputDigest / RuntimeContentHash scope。
manifest.contentHash 使用 Builder 单一 write plan 实现并排除自身。
ordered/unordered collection canonical rules。
runtime payload inventory ownership。
mutation sensitivity + digest scope unit tests。
```

建议测试：

```powershell
cargo test -p engine_runtime runtime_package_digest
cargo test -p engine_runtime runtime_package_builder
```

### Gate B：Save Correctness

```text
shared atomic replace helper。
Prefab write success 后才清 dirty。
Scene/Prefab/Rule/AUI/Input save failure preserves source/dirty。
```

建议测试：

```powershell
cargo test -p editor_core scene_save
cargo test -p editor_core prefab_authoring
cargo test -p editor_core aui_authoring
cargo test -p editor_core rule_authoring
cargo test -p editor_core input_mapping_authoring
```

### Gate C：Clean RuntimePackage Publish

```text
runtime path containment / Windows case-fold collision validation。
single-writer publish guard。
empty staging build。
formal load validation before and after publish。
stale runtime file removal。
failed build preserves last-good output。
rollback/recovery fault-injection tests。
runtime payload tree digest excludes reports。
```

建议测试：

```powershell
cargo test -p engine_runtime runtime_package_staging
cargo test -p editor_core editor_preview_package
cargo test -p editor_core desktop_export
```

### Gate D：Authoring Save / Reopen Checkpoints

```text
temporary complex shooter copy。
real domain commands。
process A save/checkpoint/exit -> process B reopen/checkpoint/exit。
SavedAuthoring / process-isolated ReopenedAuthoring comparison。
domain/path mismatch diagnostics。
```

建议测试：

```powershell
cargo test -p project_e2e_gate save_reload_authoring
cargo test -p editor_core project_consistency
```

### Gate E：Rebuild / Load / Report Panel

```text
FirstRuntimeBuild / CleanRebuild / LoadedRuntimePackage。
RuntimeAssetIndex resolve。
Rule/Input/AUI/Prefab/texture/font formal-loader witness；inventory-declared typed payload 全加载。
source/runtime witness mapping completeness。
Report Panel provider。
```

建议测试：

```powershell
cargo test -p project_e2e_gate save_reload_rebuild_consistency
cargo test -p editor_core report_panel
```

### Gate F：整体回归与归档

```powershell
cargo fmt --check
cargo test -p engine_input
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_input
cargo test -p editor_ui_renderer
cargo test -p editor_wgpu_renderer
cargo test -p editor_window_winit
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate
```

## 21. 风险与控制

### 风险 1：把 raw file hash 当语义一致

控制：typed load + canonical semantic digest；raw source hash 只作辅助证据。

### 风险 2：错误排序掩盖行为变化

控制：只有语义无序集合按 stable id 排序；Rule/AUI/Input/Scene 的语义顺序必须保留并测试。

### 风险 3：一个永远不变的弱 hash 造成假通过

控制：每个 domain 都有 mutation sensitivity test；有效变化不影响 digest 时 Gate 失败。

### 风险 4：Gate 自己修改仓库样例

控制：所有编辑和构建只发生在 temp copy；报告记录 source sample 与 temp workspace。

### 风险 5：范围膨胀成完整构建系统

控制：只做 complete digest、clean staging/publish、checkpoint/report；CAS/DDC/incremental graph 全部 deferred。

### 风险 6：报告成为 Runtime 热路径负担

控制：Runtime 不加载 Gate；Editor 默认 Off/Summary；Trace 只用于测试和显式验证。

### 风险 7：修正 saver 时引入跨域 Save Manager

控制：只共享项目无关 atomic IO helper 和成功后清 dirty 合同，不新增 ownership/router 层。

### 风险 8：把 BuildProfile/验证参数错误塞进 RuntimeContentHash

控制：BuildRecipeDigest、AssemblyInputDigest、RuntimeContentHash 分层；增加 scope test，验证 recipe-only 变化不会污染 runtime content identity。

### 风险 9：同进程新建 Session 造成“伪重开通过”

控制：正式 E2E 的 SavedAuthoring/ReopenedAuthoring 必须来自不同 child process；同进程测试只作快速反馈。

### 风险 10：payload path 逃逸或并发 publish 破坏输出

控制：写盘前执行 containment/case-fold collision 校验；final output 使用单写者 guard、same-parent staging/backup 和确定性 rollback/recovery。

### 风险 11：只运行 `load_runtime_package` 却宣称 Prefab/Texture 已读取

控制：LoadedRuntimePackage checkpoint 明确调用 RuntimeAssetLoader、RuntimeInstanceLoader/Prefab loader 和正式 texture loader；加载 inventory 声明的全部 typed payload，并对代表性 Prefab 做实例化 witness。

## 22. 方案自审（修订后）

### 22.1 审查问题吸收

修订后通过。

```text
Build recipe 与 Runtime content hash 已拆分，并明确 contentHash 排除自身。
正式 ReopenedAuthoring 已改为 process-isolated。
runtime path containment、Windows collision、single-writer publish、rollback/recovery 已进入合同。
LoadedRuntimePackage 已补正式 Asset/Prefab/Texture loader。
mutation matrix 已覆盖 Project/BuildProfile/active scene/component schema/font/删除与顺序语义。
canonical encoding 已改为 domain-separated、length-framed、fail-closed 合同。
runtime-package.v1 的 string contentHash 已固定为 `sha256:<hex>`，无需新增 manifest/schema 层。
```

### 22.2 与用户选择一致

通过。

```text
采用 B-min+。
不采用 raw file hash A。
不直接施工完整 CAS/DDC C。
```

### 22.3 AI 适配性

通过。

```text
报告包含 checkpoint/domain/path/object/source-runtime witness/expected/actual/next action。
AI 不需要从构建日志猜失败位置。
mutation sensitivity 防止弱 hash 假通过。
```

### 22.4 复杂项目适配

通过。

```text
覆盖 Project/BuildProfile/Scene/Prefab/Rule/AUI/Input/Asset/component schema/RuntimePackage。
保留 stable identity 和语义顺序。
不把复杂打飞机玩法写进 engine core；玩法名称只存在于 project_e2e fixture/witness。
```

### 22.5 长期可维护

通过。

```text
Builder 与 Gate 共用同一 canonical encoder；RuntimeContentHash 只来自 Builder 单一 write plan。
新 domain/payload 没有 inventory owner、完整 typed contribution 和 sensitivity/scope test 时 fail closed。
后续 C 方案可以追加 artifact/replay checkpoint，不推翻 v1 report。
```

### 22.6 结构复杂度

通过。

```text
没有新增持久化资产或 Runtime 层。
只有一个 Gate、一套分层 digest contract 和一份 report。
shared atomic write 是 IO helper，不是业务架构层。
Builder write plan 是内部实现，不是新的用户心智或持久化层。
process child helper 只服务 E2E checkpoint，不进入 Runtime。
```

### 22.7 效率

通过。

```text
严格 Gate 不在每次 Play 自动执行。
正常 Runtime 只读取 manifest.contentHash，不生成 report。
完整双 build 只用于 CI/施工/显式验证。
process-isolated reopen 只用于完整 E2E；普通 saver/digest unit test 仍可快速运行。
```

### 22.8 是否重复 231 Golden Gate

不重复。

```text
231 证明一个导出包能玩。
236 证明保存、重开、清缓存重建后仍产生等价运行内容。
```

### 22.9 是否错误要求 authoring/runtime 字节相等

没有。

```text
authoring 与 runtime 通过 source mapping/witness 比较。
只有同类 checkpoint 才要求 semantic digest 相等。
```

### 22.10 是否增加新的架构层来解决审查问题

没有。

```text
BuildRecipeDigest、inventory、witness 和 publish evidence 都是 editor/test/build 内部值。
它们不成为项目资产，不被普通用户编辑，不被 Runtime 常驻消费。
没有新增 Save Manager、Consistency Manager、Router、第二 assembler 或第二 manifest。
```

## 23. 结论

正式采用：

```text
B-min+: Canonical Multi-Checkpoint Consistency Gate
```

完成后，本引擎将从：

```text
“各个编辑和构建功能分别有测试”
```

推进到：

```text
“复杂项目经过真实编辑、保存、跨会话重开、清缓存重建后，
  authoring 与 RuntimePackage 的关键语义仍可被结构化证明一致。”
```

`236` 已按施工文档完成 Gate A-F：canonical digest、共享原子保存、RuntimePackage safe staging/publish、进程隔离 save/reopen、双 clean rebuild、正式 loader、mutation matrix、Report Panel cache/provider 与整体回归均已通过。实际实现与测试边界见 `阶段完成记录/2026-07-10-Save-Reload-Rebuild-Consistency-Gate-v1/00-总览.md`；下一步按 `227` 进入 `P2-2 Release Package Polish / Metadata / Icon / Layout v1` 讨论。

## 24. 参考

本项目：

```text
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
189-Project-RuntimePackage-Assembly-Completeness-v1方案.md
191-Authoring-Walkthrough-Missing-Operations-Convergence-v1方案.md
217-Editor-Play-RuntimePackage-Preview-Productization-v1方案.md
225-Project-Authoring-Asset-Completeness-Prefab-Rule-Assetization-Gate-v1方案.md
226-PrefabInstance-RuntimePackage-Bake-Authoring-Prefab-Instance-Expansion-v1方案.md
228-Real-Texture-Decode-GPU-Texture-Upload-Sprite-Textured-Present-v1方案.md
229-Complex-Shooter-Gameplay-Rule-Runtime-Execution-v1方案.md
230-Project-Rule-Driven-UiStateSnapshot-Producer-v1方案.md
231-Exported-Windows-Playable-Golden-Gate-v1方案.md
232-Editor-Build-And-Run-Productization-v1方案.md
233-Rule-Graph-Card-Authoring-Productization-v1方案.md
234-Input-Mapping-Visual-Authoring-Panel-v1方案.md
235-Asset-Browser-Native-Productization-v1方案.md
227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
4.8AI审查目录/02-目标-复杂打飞机可自由编辑并Windows打包运行-优先级路线图.md
```

外部官方资料：

```text
Unity AssetDatabase.GetAssetDependencyHash
https://docs.unity3d.com/6000.0/Documentation/ScriptReference/AssetDatabase.GetAssetDependencyHash.html

Unity AssetDatabase.Refresh
https://docs.unity3d.com/6000.0/Documentation/ScriptReference/AssetDatabase.Refresh.html

Unity AssetDatabase.ForceReserializeAssets
https://docs.unity3d.com/6000.0/Documentation/ScriptReference/AssetDatabase.ForceReserializeAssets.html

Unreal Engine Cooking Content
https://dev.epicgames.com/documentation/en-us/unreal-engine/cooking-content-in-unreal-engine

Unreal Engine Derived Data Cache
https://dev.epicgames.com/documentation/en-us/unreal-engine/derived-data-cache

Godot ResourceSaver
https://docs.godotengine.org/en/stable/classes/class_resourcesaver.html
```
