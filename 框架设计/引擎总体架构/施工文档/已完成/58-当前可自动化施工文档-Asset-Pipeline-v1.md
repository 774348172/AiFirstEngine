# 当前可自动化施工文档：Asset Pipeline v1

本文档是 `Asset Pipeline v1` 的自动化施工入口。

它不是新的架构设计，长期规则以 `框架设计/引擎总体架构/25-Asset-DB-Importer-MVP.md` 为准。  
本轮施工目标是把已确认的 Asset Pipeline v1 总方案落成最小可测试闭环。

## 0. 施工原则

```text
长期路线优先：按 Unity Artifact / UE DDC / Bevy processed asset 的长期结构建模。
第一版实现收窄：只做本地 D-min，不做远程缓存、团队共享缓存和复杂资源处理。
每个阶段必须有最小测试。
Importer 不直接写 Asset DB。
Asset DB / Meta / ImportResult / Artifact Cache / Dependency Graph 必须职责分离。
失败只能生成 report，不自动修复资源、不自动改引用、不自动删文件。
```

## 1. 市场方案对照

| 引擎 | 对应系统 | 本轮学习点 | 本轮不照搬 |
|---|---|---|---|
| Unity | AssetDatabase / .meta / Importer / Artifact / dependency hash | guid 稳定身份、artifact 导入产物、依赖 hash | 不照搬黑盒 AssetDatabase 行为，必须输出 AI 可读报告 |
| UE | AssetRegistry / UFactory / Reimport / Derived Data Cache / CookDependency | 大项目资源索引、DDC 长期路线、构建依赖 | 不实现完整 DDC、CookDependency 函数依赖、复杂编辑器 UI |
| Bevy | AssetServer / AssetProcessor / processed asset / full_hash / dependents | Rust 化、hash 驱动、依赖变化触发重新处理 | 不把运行时 Handle 写进项目源数据 |
| Godot | .import / imported cache | 简洁导入边界 | 不采用弱诊断、弱依赖图方案 |

## 2. 本轮目标

实现一个最小 Asset Pipeline v1：

```text
外部资源文件 / AI 生成文件 / 手动导入请求
  -> ImportPlan
  -> ImportTransaction
  -> ImporterContract
  -> ImportResult
  -> DependencyRecord
  -> ArtifactRecord
  -> Asset DB atomic commit
  -> ImportReport / CacheReport / DependencyReport
```

本轮完成后，至少可以用测试证明：

```text
资源有稳定 guid / assetId / type。
Importer 输出 ImportResult，而不是直接写 Asset DB。
Artifact Cache 可以命中、失效、清空后重建。
Dependency Graph 能记录直接依赖和反向依赖。
Reimport 能保持 main asset guid，并按 subAsset.kind + subAsset.id 做 diff。
导入失败不会污染旧 Asset DB。
AI / Console 可以读取结构化报告。
```

## 3. 本轮不做

```text
远程缓存。
团队共享缓存。
复杂 LRU。
完整模型骨骼 / 动画 retarget。
完整 shader 编译管线。
资源高级预览缓存。
运行时动态依赖追踪。
依赖图高级可视化。
自动修复资源引用。
真实 GPU 资源上传。
真实 Bundle 构建。
真实热更 mount。
```

## 4. 施工阶段

### A0：施工前基线检查

目标：

```text
确认当前测试可运行。
确认现有 Asset DB / scanner / import lock 测试状态。
记录任何施工前已经存在的失败。
```

建议检查：

```powershell
npm.cmd run test:assetdb
npm.cmd run test:assetscan
npm.cmd run test:assetlock
npm.cmd run test:assetimportrepair
```

验收：

```text
测试通过，或明确记录已有失败不是本轮引入。
```

### A1：整理 Asset Pipeline v1 类型边界

目标：

```text
新增或整理 AssetPipeline v1 的核心类型。
```

建议类型：

```text
AssetMeta
AssetRecord
AssetRef
ImporterContract
ImportSettings
PlatformImportSettings
ImportPlan
ImportTransaction
ImportResult
ImportedAsset
SubAsset
ImportReport
```

规则：

```text
类型定义必须可序列化。
不得把 runtime handle / GPU id / bundle path 写进 AssetRef。
SubAsset 不生成独立 guid。
```

最小测试：

```text
asset pipeline types serialize stable json
asset ref rejects runtime-only fields
subasset identity equals parent guid + kind + id
```

### A2：ImporterContract Registry v1

目标：

```text
建立 ImporterContract Registry。
为第一版资源类型注册最小 contract。
```

第一版资源类型：

```text
Texture
Audio
Model
Material
Scene
Prefab
DSL-IR
```

每个 contract 至少声明：

```text
id
version
assetType
source extensions
settingsSchema
platformSettingsSchema
outputSchema
dependencyPolicy
cachePolicy
reimportPolicy
diagnosticsSchema
```

最小测试：

```text
registry resolves importer by extension
registry rejects unknown extension
all v1 asset types have importer contract
cache policy includes dependencyHash and engineImportVersion
```

### A3：ImportResult / SubAsset v1

目标：

```text
Importer 输出 ImportResult。
ImportResult 显式记录 mainAsset、subAssets、dependencies、diagnostics。
```

规则：

```text
ImportResult 必须有 mainAsset。
SubAsset identity = parent guid + kind + id。
SubAsset id 必须稳定。
不提供 SubAsset 提取为独立资源。
```

最小测试：

```text
model import result contains stable subassets
subasset id cannot be random
subasset ref serializes as object not string path
```

### A4：Artifact Cache D-min

目标：

```text
实现本地 Artifact Cache。
```

第一版只实现：

```text
ArtifactKey
ArtifactRecord
ArtifactStore
ArtifactResolver
CacheNamespace = project-local
CacheBackend = LocalFileCacheBackend
CacheReport
```

规则：

```text
Artifact Cache 不是真相源。
删除 Artifact Cache 只导致重新导入。
ArtifactKey 是缓存命中的唯一依据。
CacheReport 必须解释 hit / miss / invalidated 的原因。
```

最小测试：

```text
artifact cache hit when key unchanged
artifact cache miss when sourceHash changes
artifact cache invalidates when dependencyHash changes
artifact cache can be cleared and rebuilt
cache report explains changed input
```

### A5：Dependency Graph D-min

目标：

```text
实现 DependencyRecord / DependencyGraph / ReverseDependencyIndex。
```

第一版 dependency kind：

```text
sourceFile
assetRef
artifact
importRule
buildRule
```

第一版 usage：

```text
runtime
import
build
editor
```

规则：

```text
只保存直接依赖。
递归依赖通过图查询。
Artifact Cache dependencyHash 来自规范化后的直接依赖。
import / build 循环依赖是 error。
runtime 循环依赖是 warning。
```

最小测试：

```text
dependency graph records direct dependency
reverse dependency index finds affected assets
dependency hash changes when dependency hash changes
runtime dependency cycle reports warning
import dependency cycle reports error
```

### A6：ImportTransaction 原子提交

目标：

```text
所有导入变更通过 ImportTransaction 提交。
失败不污染 Asset DB。
```

规则：

```text
Importer 不能直接写 Asset DB。
ImportTransaction 先写临时 workspace。
只有 ImportResult / ArtifactRecord / DependencyRecord 都有效才 commit。
commit 失败保留旧 Asset DB 状态。
```

最小测试：

```text
successful transaction commits asset record and artifact record
failed import keeps old asset database unchanged
failed commit emits import report
importer cannot mutate asset database directly
```

### A7：Reimport C 规则

目标：

```text
实现 main asset guid 保持和 SubAsset diff。
```

规则：

```text
Reimport 保持 main asset guid。
SubAsset 只按 kind + id 匹配。
updated / added / removed / renameCandidate / conflict 必须进入 ReimportReport。
删除、改名、冲突不自动迁移引用。
```

最小测试：

```text
reimport preserves main asset guid
same subasset kind and id remains referenced
removed subasset reports referencedBy
rename candidate requires report and user action
```

### A8：最小 Importer 实现

目标：

```text
实现可以支撑测试的最小 Importer，不追求真实美术管线完整性。
```

第一版建议先做：

```text
TextureImporter：读取文件 hash，输出 Texture main asset。
MaterialImporter：读取结构化 JSON，提取 texture AssetRef。
ModelImporter：读取 model-like fixture，输出 Mesh / Material / Animation subAssets。
SceneImporter：读取 scene JSON，SchemaWalker 提取 AssetRef。
PrefabImporter：读取 prefab JSON，SchemaWalker 提取 AssetRef。
LogicImporter：读取 DSL / IR JSON，提取 declared AssetRef。
AudioImporter：读取文件 hash，输出 Audio main asset。
```

最小测试：

```text
texture importer creates import result
material importer extracts texture dependency
model importer creates stable subassets
scene importer extracts asset refs through schema walker
prefab importer extracts asset refs through schema walker
logic importer extracts declared asset refs
audio importer creates import result
```

### A9：报告与 AI Debug 数据

目标：

```text
统一输出结构化报告，供 Console / AI Debug View 读取。
```

报告：

```text
ImportReport
ReimportReport
CacheReport
DependencyReport
```

规则：

```text
报告只描述事实和建议，不自动修复。
AI Repair Candidate 必须 reportOnly / requiresUserApproval / canAutoApply=false。
```

最小测试：

```text
failed import creates import report
cache invalidation creates cache report
dependency cycle creates dependency report
ai repair candidate is report-only
```

### A10：编辑器导入锁接入

目标：

```text
导入期间禁止用户和 AI 修改 Project Model。
```

规则：

```text
导入锁阻止普通 commitProject / updateEntityLive / undo / redo。
导入任务自身可以通过受控 bypass 写入导入结果。
锁不阻止视图切换、选择、日志查看。
```

最小测试：

```text
import lock blocks project mutation
import transaction bypass can commit import result
console can read import task while locked
```

### A11：端到端 Golden Scenario

目标：

```text
验证 Asset Pipeline v1 最小闭环。
```

测试场景：

```text
导入 enemy_albedo.png
导入 enemy.material 引用 enemy_albedo
导入 enemy.glb fixture 生成 mesh_body / material_body / animation_idle
导入 enemy.prefab 引用 model 和 material
修改 enemy_albedo.png
Artifact Cache miss
Dependency Graph 找到受影响 material / prefab
Reimport enemy.glb 保持 main guid
删除 animation_idle 后生成 ReimportReport，不自动迁移引用
```

验收：

```text
Asset DB 有稳定资源记录。
Artifact Cache 有 artifact record。
Dependency Graph 有正向和反向依赖。
报告可被 AI Debug 读取。
失败路径不污染旧 Asset DB。
```

## 5. 最小测试命令

本轮建议新增或扩展：

```powershell
npm.cmd run test:assetpipeline
npm.cmd run test:assetdb
npm.cmd run test:assetscan
npm.cmd run test:assetlock
npm.cmd run test:assetimportrepair
```

如果新增 Rust 侧 Asset Pipeline crate，则同步新增：

```powershell
cargo test -p engine_asset_pipeline
```

## 6. 验收标准

```text
A0-A11 每个阶段都有最小测试。
Asset Pipeline v1 端到端 Golden Scenario 通过。
原有 Asset DB / scanner / import lock / repair plan 测试不回退。
文档中确认的长期边界没有被临时实现破坏。
施工完成后必须新增阶段完成记录。
施工完成后必须把本文档移动到 施工文档/已完成。
施工完成后必须更新 49 / 54 / 18 / 阶段完成记录 README。
```

## 7. 风险与暂停条件

遇到以下情况必须暂停实现并回到架构讨论：

```text
需要改变 AssetRef 长期结构。
需要让 SubAsset 拥有独立 guid。
需要让 Importer 直接写 Asset DB。
需要把 Artifact Cache 当作项目真相源。
需要自动修改用户引用或自动删除资源。
需要引入远程缓存 / 团队共享缓存作为第一版依赖。
```
