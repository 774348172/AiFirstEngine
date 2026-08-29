# AI Asset Generation MVP

本文记录 Phase 14 AI 资源生成 MVP 的第一版实现。

## 定位

AI 资源生成不是直接让模型吐出一个文件。

第一版实现的正式链路是：

```text
Asset Spec
  -> Asset Generation Plan
  -> Generation Graph
  -> Candidate Variants
  -> Quality Gate
  -> Asset Revision
```

这条链路优先满足：

```text
AI 可生成
AI 可审查
质量可验证
失败可解释
版本可追踪
Provider 可替换
不直接写项目文件
不绕过 Asset DB / Importer
```

## 当前实现

新增代码：

```text
src/asset-generation/assetGeneration.ts
src/asset-generation/assetEditPlan.ts
src/asset-generation/mockAssetGenerationProvider.ts
src/asset-generation/mockAssetEditProvider.ts
scripts/test-asset-generation-mvp.cjs
scripts/test-asset-edit-plan.cjs
```

新增命令：

```powershell
npm.cmd run test:assetgeneration
npm.cmd run test:assetedit
```

## Asset Spec v1

结构：

```text
schemaVersion: asset-spec.v1
id
title
prompt
assetKind
usageSlot
targetPlatforms[]
styleTags[]
acceptance
references[]
createdAt
```

当前 acceptance 支持：

```text
dimensions
transparentBackground
maxSizeKb
requiredTags[]
```

规则：

```text
Asset Spec 是资源需求真相。
AI 后续修改资源时，应先修改或派生 Asset Spec / Asset Edit Plan，而不是直接替换文件。
usageSlot 是项目系统连接资源的稳定入口。
```

## Asset Generation Plan v1

结构：

```text
schemaVersion: asset-generation-plan.v1
id
specId
provider
steps[]
candidateCount
createdAt
```

当前标准步骤：

```text
prompt-normalize
provider-generate
postprocess
quality-gate
revision
```

规则：

```text
Plan 描述如何生产候选资源。
Provider 可以替换。
Plan / Spec / Quality Gate 不能被具体模型替换。
```

## Generation Graph v1

结构：

```text
schemaVersion: asset-generation-graph.v1
id
specId
planId
nodes[]
```

当前节点：

```text
reference
prompt-normalize
provider-generate
postprocess
quality-gate
revision
```

规则：

```text
Generation Graph 记录资源生成证据链。
引用图、文本说明、生成步骤都必须可追踪。
后续真实模型接入时，prompt / reference / seed / provider / postprocess 都应进入 Graph。
```

## Candidate / Quality Gate / Revision

Candidate v1：

```text
schemaVersion: asset-candidate.v1
id
specId
provider
assetKind
uri
metadata
source
```

Quality Report v1：

```text
schemaVersion: asset-quality-report.v1
ok
candidateId
issues[]
errors[]
warnings[]
```

Revision v1：

```text
schemaVersion: asset-revision.v1
id
specId
candidateId
status
quality
lineage.planId
lineage.graphId
lineage.provider
lineage.previousRevisionId?
lineage.previousCandidateId?
lineage.editPlanId?
createdAt
```

规则：

```text
Quality Gate 的 error 会拒绝 Revision。
warning 不阻止 Revision accepted，但必须保留给 AI 和用户审查。
Revision 必须记录 plan / graph / provider lineage。
编辑产生的 Revision 必须记录 previousRevisionId / previousCandidateId / editPlanId。
```

## Asset Edit Plan v1

结构：

```text
schemaVersion: asset-edit-plan.v1
id
specId
sourceCandidateId
sourceRevisionId
provider
mode
promptDelta
styleTagDelta
metadataDelta
preserve
impact
createdAt
```

当前 mode：

```text
parameter-edit
regenerate-variant
local-edit
provider-edit
```

当前 preserve 规则：

```text
usageSlot
dimensions
transparentBackground
targetPlatforms
references
requiredTags
```

规则：

```text
AI 修改资源时不能直接替换文件。
AI 必须先生成 Asset Edit Plan，说明要改什么、保留什么、影响哪些 usageSlot / asset / system。
Edit Plan 必须先 preflight validation，再调用 provider。
编辑后的 Candidate 必须重新跑 Quality Gate。
编辑后的 Candidate 必须检查 preserve rules。
违反 preserve rules 的编辑必须产生 rejected Revision，而不是静默覆盖旧资源。
```

## Mock Provider

当前实现：

```text
MockAssetGenerationProvider
MockAssetEditProvider
createMockAssetGenerationProvider
createMockAssetEditProvider
```

规则：

```text
Mock Provider 只用于测试和流程验证。
它必须 deterministic。
它不代表真实模型能力。
真实 Image / 3D / Audio / Material Provider 后续必须实现同一 AssetGenerationProvider 接口。
```

## 当前边界

已完成补充：

```text
Asset DB 注册前的可审查数据层 v1。
Asset Candidate -> AssetRegistrationPlan。
AssetRegistrationPlan -> asset.meta proposal。
AssetRegistrationPlan -> register-generated-asset patch candidate。
```

当前仍暂不做：

```text
真实图片 / 模型 / 音频生成
真实文件写入
自动 Asset DB 注册
Asset Import Lock 接入
编辑器 UI
AssetSlot 自动绑定
Prefab / Scene / DSL 自动接入
版权 / license gate
```

这些属于后续 Phase 14 子阶段。

第一版已建立 AI 资源生成与资源编辑的可测数据闭环。

## 当前测试覆盖

当前测试覆盖：

```text
natural language style prompt creates Asset Spec
reference image attaches to Asset Spec
Asset Generation Plan has standard steps
Generation Graph records reference and quality gate nodes
Mock Provider creates deterministic candidates
Quality Gate accepts valid candidates
Quality Gate rejects dimension mismatch
Quality Gate warns on missing style tag
Asset Revision records accepted / rejected status
Asset Revision records plan / graph / provider lineage
Asset Edit Plan preserves usage slot / dimensions / transparency / references / required tags
Asset Edit Plan records impact references
Asset Edit Provider produces deterministic edited candidate
edited candidate reruns quality gate
edited revision records previous revision / candidate / edit plan lineage
illegal edit that changes protected dimensions is rejected
illegal edit that changes protected reference lineage is rejected
empty edit plan fails preflight validation
valid candidate creates AssetRegistrationPlan
registration plan is reportOnly
asset meta proposal preserves lineage
registration patch candidate requires user approval
registration plan does not mutate Asset DB
rejected candidate blocks registration
```

回归命令：

```powershell
npm.cmd run test:assetgeneration
npm.cmd run test:assetedit
npm.cmd run test:assetregistration
npm.cmd run build
```
