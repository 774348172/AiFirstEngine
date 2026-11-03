# 358-U0 render-contract.v1 合同冻结附录

日期：2026-09-22。依据：[358 正式方案](358-2D3D-Unified-Render-Contract-and-Asset-Binding-v1方案.md)。本文记录 U0 实际源码合同；不升级 RuntimePackage、Project SDK、ABI 或九工具协议。施工/完成状态由49/54及 U0 完成记录维护。

## 1. Owner 与唯一事实

- 新合同 owner：[render_contract.rs](../../rust/crates/engine_runtime/src/render_contract.rs)，内部按 asset / view / diagnostic 拆文件。
- Sprite Adapter：[render_projection_adapter.rs](../../rust/crates/engine_runtime/src/render_projection_adapter.rs)，被既有 [render_extract.rs](../../rust/crates/engine_runtime/src/render_extract.rs) 的 Sprite 分支真实调用。
- 可执行合同：[unified_render_contract_u0.rs](../../rust/crates/engine_runtime/tests/unified_render_contract_u0.rs)，分 contract / adapter / consumer，包含固定旧行为预期及失败输入。
- `RuntimeAssetRef`、`RuntimeAssetIndex`、World dirty、RenderCommand、proxy identity 与 Renderer/Graph/RHI 仍归原 owner；没有第二资产身份或第二 RenderScene。

## 2. Envelope 与通用规则

```json
{
  "schemaVersion": "render-contract.v1",
  "data": {
    "expectedKind": "texture",
    "reference": {
      "id": "texture-a",
      "type": "texture",
      "guid": "guid-a",
      "sub_asset": "region-a"
    }
  }
}
```

`RenderContract<T>::decode` 是新合同的严格入口，先做 serde 结构校验，再调用该 payload 的 `ValidateRenderContract`；未知版本、tag、字段和类型拒绝。Rust 直接构造值时须显式调用 `validate`。`new` 只是 envelope 包装，不代替验证。

新字段采用 camelCase；tagged union 用 `kind`，`Evaluation<T>` 用 `state/value`。所有非 Option 字段必需，没有隐式零/true/identity 默认；Option 缺失或 null 为 None。构造 helper 的默认值见下表，和 decoder 缺字段默认是不同概念。

新输入校验不能反向修改旧 decoder。旧 `RuntimeAssetRef` 的 `id/type/guid/sub_asset` 原键保留；private serde remote 定义仅为新入口拒绝拼错字段，不是第二份身份存储。旧 decoder 原先忽略的额外字段仍按旧规则处理。

## 3. 资产、变换与可见性

| 对象/字段 | 类型、来源与默认 | 校验/兼容 |
| --- | --- | --- |
| `RenderAssetReference.expectedKind` | texture/material/mesh/font；消费方指定，必需 | 必须与 reference.type 一致 |
| `reference` | 原 `RuntimeAssetRef`；id/type 必需，guid/sub_asset 可缺省 | id 与 guid 不能同时为空白；有 index 时调用原 resolve，并拒绝 id/guid 冲突、错误类型和不存在的 subasset |
| `RenderAssetResolution` | 没有 index 返回 Unresolved；命中时返回借用的原 record | Resolved 不是 GPU Ready；asset version/hash/source 读取原 record，不另存独立可写副本 |
| `TransformMetadata.convention` | legacy2d 或 world3dRightHandedYUpMetersCameraNegativeZ，必需 | 新 3D 是右手/Y-up/米/camera -Z；旧2D原数值单位不迁移 |
| `position/rotationDegrees/scale` | 各为3个 f32；legacy helper 原样复制 local Transform | 新输入必须 finite；旧 Sprite 继续 half-extents、Z degrees，保留 x/y 值而不改变旧消费方式 |
| `worldMatrix/previousWorldMatrix` | Evaluation<[f32;16]>；legacy helper 均为 notEvaluated | known 时数值须 finite；U0 不计算 hierarchy/world matrix/quaternion |
| `BoundsMetadata.center/extents` | 各3个 f32，来自旧 Bounds | 新输入须 finite，不归一化负 scale/extent |
| `quality/space` | legacyApproximation / legacyTransform | 不声称真实 Mesh bounds 或世界 AABB |
| `Visibility.authored/inherited` | bool / Evaluation<bool>；Sprite helper 复制 visible、inherited=notEvaluated | unknown 与 false 分开；无 inherited 传播算法 |
| `ViewVisibility.viewId/instanceId` | u64；结果以二者组成的 pair 为键 | 不把某个 view 结果写回实例共享 bool |
| `visibility/culling` | Evaluation<bool>；notEvaluated/included/layerExcluded/frustumExcluded | 表示派生结果合同；U0 不执行 culling |
| `LayerSelection` | legacy { mask: Option<String> } 或 named { names: String[] } | named 拒绝空白名和重复名；空集合本身合法，U0 不执行新 named-mask 语义；禁止降级时取第一个名字 |

`Evaluation<T>` 的 wire 例：`{"state":"notEvaluated"}` 或 `{"state":"known","value":true}`。旧数值的直接兼容转换不运行新入口的 finite 校验，避免 strict schema 意外改变既有 runtime 输入；非有限值不会被新合同输入接受。

## 4. View 与策略

| 字段 | 类型/来源/默认 | 校验及旧 consumer 边界 |
| --- | --- | --- |
| `viewId/sourceCameraEntityId/version` | u64 / Option<String> / u64；legacy helper 原样复制 | 不丢 source camera，真实 camera 的 identity matrix 不当作 placeholder |
| `viewKind` | game/sceneView/preview/shadow/reflection；原 enum 的 wire 投影 | 全部可还原；不修改旧 enum |
| `domain` | world2d/world3d/ui | World View 接受相容的 world2d/world3d；ui 由既有 UiProjection/composition 执行，不创建新 World UI view |
| `projection` | legacyMatrix {view,projection} / orthographic {size,near,far} / perspective {fovYDegrees,near,far} | raw 矩阵不反推相机参数；typed 只验证，U0 不生成投影矩阵 |
| typed projection 数值 | finite f32，无 decoder 缺省 | Orthographic：size>0、near<far；Perspective：0<fov<180、0<near<far |
| `viewport` | [x,y,width,height]，u32像素；legacy 原样复制 | typed view width/height>0，x+width/y+height不溢出；legacy零尺寸照旧保留 |
| `target` | window/viewportTexture/renderTexture/shadowMap/previewTexture | 只复用目标类别，U0 不分配新 GPU target |
| `clearPolicy` | color {color:[f32;4]} / load | 色值须 finite；旧转换只接受 color，不吞 load 意图 |
| `depthPolicy` | disabled/testAndWrite | 旧转换只接受 disabled；U0 不实现深度测试/附件 |
| `layers/coordinateConvention/provenance` | 上文类型 | 新3D须新3D坐标约定且不能用 legacyMatrix；Perspective 不能冒充 world2d |

`RenderView::perspective` 是 contract-only helper：Game/World3D、指定参数/像素尺寸、Window、黑色 clear、TestAndWrite、named 空集合、version=0、来源未提供。这些是该 helper 的显式默认，不表示 GPU 资源或 camera 实体已存在。

`execution_support` wire 值为 `legacyCompatible` / `contractOnly`，只说明当前能否无损送入旧 consumer，不是 Ready 或渲染成功证明。

`RenderPolicy` 为 legacySprite {sortingLayer:i16, orderInLayer:i32, sortZ:f32} / opaque / transparent / uiComposition。Sprite 只接受 legacySprite；新3D/UI策略只冻结意图，不让它们执行旧 Sprite 路线。legacySprite 保留 sortZ=0 的 transform.z 回退、千分量化及 proxy ID tie-break。

## 5. Sprite Adapter 与无损界面

`SpriteProjection` 字段：domain、geometry、legacyMaterialRef、visibility、layers、policy、provenance。

- geometry 为 `legacyGeneratedQuad { spriteRef: Option<String>, color:[f32;4], flipX:bool, flipY:bool }`；显式标为 unresolved 旧 key，不能伪造 GUID 或 typed 已解析资产。
- legacyMaterialRef 同样保留 None/空字符串/原 key；严格的 typed AssetRef 是上文另一个入口，不把旧错误输入截断在 Adapter。
- Adapter 从真实 Sprite component 直接生成中立 projection，再 lowering 到旧 descriptor；并非先做旧 descriptor 后反复来回转换。
- Sprite helper 固定 World2D、layer=sprite2d、inherited=notEvaluated、legacySprite policy，复制其余 component 值。新入口的 strict validate 不插入旧提取链。
- `to_legacy` 拒绝新 domain、named layers、已计算 inherited、新 policy。`from_legacy` 拒绝非 Sprite payload。
- `LegacyConversion { payload, provenance }` 将来源作为显式附件返回；旧结构没有的新来源不会被宣称已经持久化进旧格式。Transform 的已计算 world/previous 或新3D convention 也不能静默降级。

旧空 Sprite key 继续沿原 consumer 得到 `sprite_missing_ref`；不转换为 Mesh test geometry。proxy/source/runtime identity、generation、dirty merge 和 despawn 都由既有 owner 管理。

## 6. Provenance 与诊断

Provenance：`sourceEntityId/sourceComponent/sourcePath/projectRevision/operationId/assetId/contentDigest` 为可缺省字符串；`producerStage` 与 `availability` 必需。前者为 contractValidation/assetResolution/renderProjection；后者区分 legacyContextOnly、inputContextNotSupplied、suppliedContext。

Sprite helper 只知道 source entity/component 和 RenderProjection stage；没有的路径/revision/digest/operation 仍为空。typed 相机 helper 使用 inputContextNotSupplied，不能误标 legacy 或伪造来源。

`ContractError` 为 compact code、field 和实际失败 stage。producer provenance 与 failure stage 分开；例如原数据来自 RenderProjection，资源查找失败仍记 assetResolution。

新入口结构错误保留 serde 的具体字段/tag/line/column；`ContractDecodeError::compact` 归为 invalidSchema，按需将 Display 文本放入 Diagnostic.actual。有效结构但非法语义继续保留具体错误码和 field。

Diagnostic 输出包含 schemaVersion、code、field、stage、domain、provenance、expected、actual、dependencyChain、fallback、retryability、nextAction。没有新 registry。U0 实际错误只有 schema、asset missing/type/subasset/identity、nonfinite、projection/viewport/layers/domain 和 unsupported legacy conversion。

- 默认 Off：不调用 details closure、不 clone provenance、不生成条目/JSON/文件，原 compact Result 仍返回。
- Summary：最多16条、每条最多8层 dependency；actual及每个dependency文本最多256字符。
- Trace：显式请求时展开 details；不作为默认逐帧输出。
- 拒绝值的 fallback=unsupported；无损转换错误 requiresConsumer，其余 afterSourceChange。Accepted/degraded 是预留的状态词，不表示 U0 已实现资源 fallback 生产链。

## 7. 执行与资格边界

真实接入限 Sprite dirty projection 到旧 descriptor。typed View/Transform/Asset 合同和诊断可被测试、内部 consumer 使用；工具界面没有新增或发布这些字段。

U1 仍需真实 cooked geometry/material payload、named binding、typed handles 和资源 Ready；U2 仍需 typed camera 执行、depth、per-view culling、DrawGeometry/backend 接入。3D 方案 A 的 cube/plane/glTF 没有在 U0 实现。
