# 358-U1 Native Render Assets 与命名绑定合同附录

日期：2026-09-22。依据：[358方案](358-2D3D-Unified-Render-Contract-and-Asset-Binding-v1方案.md)、[U0合同](358-U0-render-contract-v1合同冻结附录.md)。本文描述U1实际接口与资格边界，执行结果只见49/54和完成记录。

## 1. 输入、Cook与唯一事实链

普通项目资产仍为 `mesh-asset.v1` / `material-asset.v1` 的 `.asset`，新增可选 `renderData` 内容入口。选择此入口时必须明确 `assetId` 和 `assetGuid`，禁止同时提供旧粒子格式 `runtimeData` 或 `sourceImage`。

```json
{
  "schemaVersion": "material-asset.v1",
  "assetId": "material.wall",
  "assetGuid": "guid-material-wall",
  "renderData": {
    "schemaVersion": "render-contract.v1",
    "data": {
      "kind": "material",
      "payload": {
        "preset": "unlit",
        "parameters": {
          "baseColor": {"type": "vector4", "value": [0.2, 0.4, 0.8, 1.0]}
        },
        "textures": {},
        "blend": "opaque",
        "cull": "back",
        "depth": "testAndWrite",
        "shaderSource": "engine_runtime/src/wgpu_backend.rs#real::create_basic_pipeline"
      }
    }
  }
}
```

可执行payload例子：[mesh fixture](../../rust/crates/engine_runtime/tests/fixtures/render_mesh_v1.json)、[material fixture](../../rust/crates/engine_runtime/tests/fixtures/render_material_v1.json)。以上shaderSource是现有内置shader的源码owner映射，不是项目/包内文件；Runtime不会按它读源文件。U1没有新增shader编译器或GPU pipeline执行。

唯一链路为现有 `ProjectRuntimePackageAssembler` 的 `neutral_assembler::append_asset` → `render_asset_cook` → `RuntimePackageBuilder` → `RuntimeAssetIndex` / `RuntimeAssetLoader` → `RuntimeRenderAssetProducer::prepare_cooked_from_package` → 原 `RenderResourceManager`。不新增数据库、资产身份、resolver、Bridge、包schema、SDK/ABI或工具协议。

原生Cook只是明确的校验和规范序列化，不是glTF importer。343粒子 `runtimeData` 保留原来的真实mesh/material格式与消费者。

## 2. Mesh数据

所有字段必需，未知字段/tag/版本拒绝；没有零/空/占位几何隐式补齐。

| 字段 | 类型/语义 | 验证 |
| --- | --- | --- |
| `vertexBytes` / `indexBytes` | 真正的u8数组；little-endian | 长度须等于显式count×stride/index宽度，不从文件长度猜count |
| `vertexStride` | u32字节数 | 非零、4字节对齐，乘法用宽整数校验 |
| `attributes` | semantic/format/offset | position必须float32x3；normal为float32x3、texCoord0为float32x2、color0为float32x4；不重名、不重叠、不越界 |
| `vertexCount` / `indexCount` | u32校验计数 | 大于零；triangle-list索引数为3的倍数；每个索引小于vertexCount |
| `indexFormat` | uint16 / uint32 | 显式；不推断 |
| `topology` | triangleList | 其它primitive尚不支持 |
| `submeshes` | firstIndex/indexCount/materialSlot | 完整覆盖索引、不重叠/留洞、非空、materialSlot必须存在 |
| `materialSlots` | 字符串数组 | 非空、不重名、不含空白名字 |
| `bounds` | min/max、sphereCenter/sphereRadius | 全finite，min≤max、radius≥0；AABB和sphere必须覆盖真实position |
| `coordinateConvention` | U0已有坐标enum | 显式保留；不执行坐标变换 |

浮点顶点属性必须finite。没有切线、骨骼、LOD、形变或拓扑自动修复。CPU保留原字节和ranges，GPU buffer创建和DrawGeometry在U2/A实现。

## 3. Material数据

初始preset仅 `unlit`。`baseColor` 是必需的finite vector4；其它参数、错误类型和其它shaderSource明确拒绝，不静默丢弃。typed参数enum预留Float表示法，但没有宣称任何Float参数已受当前preset支持。

`textures` 是命名AssetRef集合，当前unlit只接受可选 `baseColorTexture`。引用完全复用U0 `RenderAssetReference { expectedKind, reference }` 和原 `id/type/guid/sub_asset` 键；expectedKind必须texture。无此slot表示显式纯色材质，不是隐式纹理丢失fallback；声明过的纹理缺失必须失败。

blend：opaque/alphaBlend；cull：none/back/front；depth：disabled/testAndWrite/testOnly。`pipeline_identity()` 从cook recipe、preset、blend/cull/depth和shader owner映射派生摘要，不受baseColor值/纹理身份改变影响。它是材质侧pipeline recipe identity；U2还须加入view、attachment和vertex layout才能建立具体可执行pipeline key。

Cook依赖解析将已有Compiler snapshot投影到原RuntimeAssetIndex，按同一typed resolver检查GUID/id/type/subasset；重复id/GUID或无法解析的texture拒绝。依赖GUID写入既有RuntimeAssetRecord.dependencies。

## 4. Import/Cook/Runtime identity

Cooked文件复用 `RenderContract<CookedRenderAsset>`：

```text
schemaVersion = render-contract.v1
data.sourceDigest = 原.asset完整bytes的sha256
data.recipe = render-native.v1
data.asset = 已验证的RenderAssetData
```

Cooked bytes SHA写原RuntimeAssetRecord.hash和CookedAssetRecord.hash，实际byte长度写两个表的size；sourceMapDebug继续保存原.asset路径。源digest不冒充cooked digest，cooked digest不写入自身内容。现有包content_hash仍覆盖manifest与实际payload。

现有 `ProjectAssemblyProducerReport` 记录 `render-asset-cook`、recipe、source recipe key、output digest，不新建报告协议/缓存owner。该小型原生cook当前未增加持久缓存；不声称recipe_key是已命中的缓存证明。

Runtime在请求资源之前核对两份既有manifest的digest/size/bundle、实际bytes、payload.kind与引用expectedKind。材质声明的texture依赖必须与record依赖集合一致。读取使用原Loader；复制出owned bytes后平衡load/release及依赖引用，失败分支也不留下该次load引用。

新Geometry/Material resource key使用GUID、record.version和cooked digest，因此record.version仍为旧值也不能遮蔽新内容。Texture准备摘要覆盖真实pixels及sampling metadata，直接流式hash pixels，不为hash把像素数组序列化成长JSON。

## 5. Typed handles、BindingSet与生命周期

`GeometryHandle` / `MaterialHandle` / `TextureHandle` 仅包装原kind/index/generation，内部字段私有。Rust `TryFrom` 和serde decode均检查MeshBuffer/MaterialParams/Texture种类；不新增独立句柄表。句柄受创建它的resource owner约束，不可跨manager解释，也不意味着GPU已经上传。

新BindingSet只存 `slots: [{name, resource:{kind,handle}}]`，不再存第二份resources向量或materialHandle。固定槽名为geometry、materialParams、baseColorTexture、fontAtlas、renderTarget；它们验证对应种类。任意slot不能重名/空名；`texture(name)`按语义名查找，与slot次序无关。fallbackUsed/sampler/debugLabel保留。

旧BindingSet wire仍可读取：resources中第一个符合primary种类的handle映射到primary名字，其余按原顺序保留为legacyResourceN，旧materialHandle保留为legacyMaterial。新旧事实混写拒绝，serialize只输出slots。此兼容入口不把不存在的旧资源变成Ready，也不默默丢多余句柄。

Sprite binding生产、RuntimeRenderer取纹理、Graph/RHI传递、Headless/WGPU计数均消费新结构。U2才更换旧RHI draw command，因此旧DrawSpriteTextured.texture字段仍作为兼容command字段存在，正式生产点从baseColorTexture槽派生。

`validate_residency(manager)`检查该manager中真实state/generation：stale、pendingRelease、released或不存在/旧代数拒绝；requested/failed不能冒充resident。预算不足的同key请求重试复用原pending slot；下一帧预算可用后原句柄进入resident，新版本使旧resident/requested变stale。旧producer也停止无条件Ready及复用时重复统计上传字节。

`PreparedCookedRenderAsset` 保留真实CPU payload、named binding、texture payload、sourcePath/digest和材质recipe identity。`Prepared`只表示CPU校验与逻辑资源准备通过；`Deferred`表示预算待处理。两者都不是WGPU上传/真实3D画面证明。

Texture、FontAtlas和RenderTarget共享typed/named绑定及现有resource manager接缝。`prepare_texture_payload`接受原RuntimeTexturePayload的RGBA8数据（用于普通纹理或RGBA atlas）；既有356真实R8/MSDF字体页、AUI composition、343粒子与纹理registry/backend继续由原owner实际上传。U1没有改写它们，也不把RGBA atlas测试冒称为R8/MSDF后端迁移。对现有外部registry创建的handle，上传资格仍由该registry验证，不能拿到另一manager中判定Ready。

## 6. 失败与按需诊断

复用U0 compact ContractError，扩展invalidAssetPayload、unsupportedMaterialParameter、assetDigestMismatch、missingCookedPayload、resourceTypeMismatch、invalidBinding、staleResourceHandle、resourceNotReady；实际stage区分assetDecode/assetResolution/assetBinding/resourcePreparation。

默认Off仍不执行details closure或生成JSON/长报告；Summary/Trace保持U0有界策略。stale/notReady的retryability为afterResourcePreparation，不要求用户去改正确的源文件。旧Sprite fallback flag/诊断仍保留；原生缺失mesh/material payload不能用默认三角形、计数或空材质替代。

## 7. 明确边界

U1提供可实际Cook/Package/Load/Prepare的原生资源和消费者绑定合同；尚无普通项目MeshRenderer3D执行、相机typed投影、深度附件、per-view剔除、真实DrawGeometry或glTF导入。默认工具链仍以49记录的357安装为准，源码测试不代表已更新安装。
