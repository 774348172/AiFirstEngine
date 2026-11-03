# 358-U2 View / Depth / DrawGeometry 实际合同

日期：2026-09-22。依据[358正式方案](358-2D3D-Unified-Render-Contract-and-Asset-Binding-v1方案.md)、[U0合同](358-U0-render-contract-v1合同冻结附录.md)、[U1合同](358-U1-native-render-assets与命名绑定合同附录.md)。源码和指定Windows/Vulkan离屏资格见[完成记录](阶段完成记录/2026-09-22-358-U2-View-Depth-RHI/00-总览.md)。安装状态只见49。

## 1. 实际消费链

```text
U1 PreparedCookedRenderAsset + 既有RenderResourceManager
  -> PreparedGeometry::from_assets(mesh, material, submesh)
Projection输出的world matrix / visibility / named layers / provenance
  + RenderView / 显式CameraPose
  -> GeometryFrame::build
  -> RuntimeRenderer::build_with_geometry
  -> 既有RenderGraph / compile_render_graph_to_rhi_plan
  -> HeadlessRhiBackend / RealWgpuBackend
```

没有第二个AssetResolver、Scene registry或ECS扫描器。新入口消费Projection与Prepare产品；普通场景Camera3D/MeshRenderer3D组件和自动生产这些产品的项目链属于下一步A。旧 `RuntimeRenderer::build` 仍走原输入，并共享同一个graph/report实现。

主要源码：[相机](../../rust/crates/engine_runtime/src/render_contract/camera.rs)、[几何消费](../../rust/crates/engine_runtime/src/render_geometry.rs)、[Graph接入](../../rust/crates/engine_runtime/src/render_geometry/graph.rs)、[实际GPU消费](../../rust/crates/engine_runtime/src/wgpu_backend/real/geometry.rs)。

## 2. 相机与视图

沿用 `render-contract.v1` 和原RenderView wire，不修改RuntimePackage、SDK或ABI版本。新增的ExecutableRenderView是内部派生产物，不要求项目或AI手写矩阵。

- `CameraPose`：显式position和单位四元数xyzw；默认位置0、旋转identity。相机scale不参与镜头；非法/非有限姿态拒绝。
- `RenderView::orthographic` 的size是完整垂直世界范围；水平范围=size×viewport aspect，保留既有2D单位。可用合法World3d约定构造3D正交相机。
- Perspective使用degrees FOV、0<near<far，RH/Y-up/相机负Z；矩阵列主序，clip depth 0..1。near映射0，far映射1。
- LegacyMatrix原样保留；不从矩阵猜FOV，不叠加另一个非默认Pose。旧Sprite consumer及其identity/placeholder判断不切换。
- ExecutableRenderView保留view ID、相机实体、view kind、version、domain、target kind、viewport、target extent、清屏/深度策略、view/projection矩阵和provenance。
- viewport零尺寸、越界、加法溢出及派生非有限矩阵失败。ShadowMap暂不执行typed native路径，返回unsupported；阴影没有借此交付。
- `execution_support()` 对可验证的typed普通视图返回 `NativeGeometry`，表示源码consumer存在，**不是资产就绪、GPU可用或安装完成**。这取代U0当时的ContractOnly结果；U0的legacy无损转换边界继续有效。

## 3. Geometry与材质

`PreparedGeometry::from_assets` 只接受U1的Prepared状态、当前manager中Resident的正确代数句柄、真实Mesh/Material和合法submesh。参数类型、required texture slot、buffer layout/index/bounds继续使用U1校验。额外或丢失的baseColorTexture不能悄悄改变材质。每帧构造GeometryFrame时再次校验绑定，旧句柄或设备失效不会继续成功。

GeometryBuffers是不透明、不可变、Arc共享的真实vertex/index数据；wire反序列化必须通过完整布局、有限数值、索引范围验证。RHI不从count或ref字符串生成假Mesh。保持u16/u32 triangle-list和submesh range；position3必需，normal/uv/color依U1布局可选，unlit忽略normal，缺color取白、缺UV取0。

DrawGeometry包含：

```text
ExecutableRenderView / target / depth_target
geometry buffers + named geometry binding + geometry asset provenance
named material binding + material asset provenance
first_index / index_count / world_transform
baseColor / blend / cull / depth
camera_depth / stable instance_id
instance provenance
```

两个资产provenance分别保留U1 GUID、cooked digest、source path，使用Arc共享，不丢失资源修订与来源。实例和相机身份保持独立。GeometryInstance的稳定instance ID由既有Projection身份提供，本模块不分配第二份实体ID。

材质首批仍为unlit。U1的 `UNLIT_SHADER_SOURCE` 是保留的recipe映射入口，实际中立shader owner为 `wgpu_backend/real/geometry.rs#geometry_pipeline`；执行pipeline identity同时包含recipe、实际shader owner、vertex layout/stride、index format、color/depth attachment、blend/cull/depth、domain及texture presence，不能拿U1 material seed替代。矩阵、颜色、texture身份属于uniform/binding，不制造新的shader变体。

## 4. 逐view可见性与排序

每个view独立计算，键为 `(view_id, instance_id)`：

1. authored/inherited visibility；可见实例的inherited仍NotEvaluated时失败，不假报完成。
2. 坐标域不匹配记DomainExcluded；命名层不相交记LayerExcluded。空view命名集合表示全部；旧mask只在双方均为Legacy时按原表示比较，混合表示拒绝。
3. 用真实local AABB八角经world及view-projection变换做保守齐次六平面剔除，clip z为0..w；不在相机近平面除w，保留相交对象。
4. opaque先画、按camera-space距离前到后；alpha后画、后到前；同距离使用稳定instance ID。旧Sprite的量化z排序保持自己的原路径。

Off只保留绘制必需数据；Summary给每view计数；Trace才生成逐instance ViewVisibility。隐藏对象标记visibility=false/culling=NotEvaluated，不能冒充已做frustum测试。紧凑GeometryBuildError保留原ContractError和失败view/instance ID，调用者可从现有输入provenance定位source；不为默认Off生成逐对象JSON或错误长字符串。

## 5. Graph、Depth与composition

原RenderGraph增加BeginView、DrawGeometry及Depth32Float资源格式；原EngineRhiBackend增加begin_view消费。编译前验证view声明、唯一ID、target、color usage/format、viewport、depth格式/大小/引用及draw对应view一致性；失败计划不产生Draw。

每个native view使用独立depth attachment，以target和view ID命名。Depth32Float清1、LessEqual；材质TestAndWrite/TestOnly/Disabled分别控制写入与比较，Disabled可在有附件的pass中使用Always且不写depth。大小变化重建，离开当前计划的mesh/pipeline/depth缓存退休。native跨view不共用一个深度平面。

所有native view初始化位于全局clear之后、AUI BeforeWorld之前；native world draw位于Particle和ScreenOverlay/Modal之前。多个view的颜色初始化按声明顺序执行，然后执行各view的world阶段；这不是独立camera stack compositor。Clear只影响自己的viewport，Load保留既有color内容；depth仍按view新帧清理。显示content rect存在时按target extent映射viewport/scissor。

AUI不进入世界depth排序，三个composition阶段保持。旧Sprite/DrawMesh/UI继续由原 `compile_render_graph_to_rhi_plan` 分支转换，默认命令不强制改为DrawGeometry；`PreparedGeometry::from_legacy_mesh` 提供已解析GUID与真实prepared数据的显式迁移接缝，字符串缺失或身份不匹配拒绝，不能把旧MeshBasic测试三角形称为真实Mesh。

343粒子保留自己的view/depth合同及simulation owner，与新geometry在同一有序RHI序列组合已验像素。**这里没有宣称native mesh与旧粒子已共享同一3D相机depth attachment**；该消费迁移需后续明确绑定view身份，不能暗中改变旧粒子遮挡行为。

## 6. Backend资格与后续边界

Headless消费完整命令和绑定并产生确定性hash，native geometry不虚增GPU上传数。无设备的WgpuBackend只记录意图/不可用状态。RealWgpuBackend使用真实vertex/index buffer、矩阵uniform、named texture及缓存pipeline执行indexed draw；相同mesh跨实例/帧复用，缓存检测同handle不同实际数据冲突。纹理上传继续走原registry；缺GPU纹理在绘制和粒子simulation前拒绝。

121项相关测试通过，其中6项U2测试必须成功获得设备并做像素断言，另有现有WGPU、粒子、AUI主题/字体consumer回归。实际资格为Windows test profile/Vulkan离屏；不是原生窗口、完整项目、release性能或多平台资格。每draw uniform和pass组织尚未做大场景性能资格，不把接口统一等同于Godot/Unity/UE完整3D能力。

下一步按358原方案A打通generated cube/plane、普通场景typed相机与Mesh、静态glTF/GLB导入、项目运行和observe/check/capture闭环；PBR、阴影、Physics3D、Navigation3D仍在后续。默认安装357未更新，U0/U1/U2源码没有自动进入当前安装宿主。
