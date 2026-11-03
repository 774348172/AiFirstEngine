# 372：二维局部 GPU 几何与增量呈现——方案 B v1

## 0. 决策、权威与状态

- 日期：2026-10-10。用户已明确要求“按照方案B来生成方案文档”。
- 已确认路线：稳定局部几何、实例表现、视图和裁剪分别表达；复杂二维几何驻留 GPU，普通变换由 shader 消费；项目同步调整局部坐标、分块和失效范围。
- 文档状态：`design_documented`。本文件定义后续建设合同，不代表源码、GPU、性能、交付或安装已完成。
- 顶层权威：[产品需求 v1.1](00-AI-First-Game-Engine-权威产品需求-v1.1.md)、[架构设计 v1.1](00-AI-First-Game-Engine-权威架构设计-v1.1.md)。继承 [361](361-AUI表现逻辑分离与常驻节点运行时-v1方案.md)、[363](363-AUI常驻节点与端到端增量更新-v1方案.md)、[365](365-通用二维绘制与AUI表现接口-方案B-v1方案.md)。
- 共同底层语义参照 [371 总方案](371-大型游戏级AI原生3D平台-方案B-v1方案.md) §6.1–6.3 与 [371 基础贯通设计](371-B-基础贯通设计.md) §5–6；二维与三维的实例布局、Projection 和排序语义保持各自边界。
- 配套：[方案自审与决策记录](372-方案自审与决策记录.md)。实际实现／安装状态只见 [49](49-当前状态与下一步入口.md)，唯一施工入口只见 [54](54-当前可自动化施工总入口.md)。

用户本次确认的是方案路线及正式设计生成。本文不是施工文档，不自动激活代码施工、恢复 371、更新安装或改交付程序。新增内部类型名称用于说明职责，不是当前可调用 SDK／ABI 声明。

## 1. 系统作用与首期边界

本系统让稳定的二维复杂图形在平移、调色和镜头滚动时复用局部几何，以紧凑参数表达变化，减少 CPU 顶点加工和大几何上传。对标 Godot Canvas 的局部命令／polygon、Unity UI Toolkit 的变换快通路，以及 Unreal 世界 mesh、Bevy Mesh2D 的几何／实例／视图分离；保持本引擎既有 SDK、AUI、Projection、Renderer 和资源 owner。

画面仍按呈现帧绘制。内容不变意味着不重复执行内容生成、剖分及大 mesh 上传，不意味着完全不做可见性、排序、批次、参数准备或 GPU draw。

### 1.1 本期完整交付范围

1. 稳定 Graphics2D 经现有 AUI Custom／UiProjection 的局部 GPU 几何消费，包括代码构建和已发布图形资源。
2. 普通仿射变换、Tint／Opacity、Canvas／viewport 映射与可精确表达的轴对齐矩形裁剪快通路。
3. 与现有 CPU 精确裁剪、普通 Custom mesh、文字／图片等路径混合时，保持原顺序、颜色、采样、命中和发布语义。
4. 共同渲染 owner 内必要的资源身份、参数内容版本、在途帧保护、设备重建、安全退休和有界预算。
5. 显式诊断下的分类计量，以及像素跳跃冒险的局部坐标／分块对照、真实 SDK／DLL／UI Pass 和普通 Windows 交付验证。

### 1.2 本期不授予的能力

世界矢量 Graphics2D adapter、专用 Tilemap shader、GPU culling／indirect、复杂 GPU stencil／mask、逐节点离屏纹理、group opacity、3D 多视图和完整持久 3D 实例体系不作为首期前提。世界矢量消费未来沿现有 RenderProjection 扩展，需独立确认范围；本期不把 AUI 变成长期世界渲染 API。

原 World Sprite／Animator／Particle 保留各自 owner。小 quad 的轻量流式合批可继续使用，是否迁移实例布局由实际测量选择；本期不以修改所有二维路径才能完成。

## 2. 诊断与成熟引擎源码依据

### 2.1 已有故障证据

[既有诊断](../../.artifacts/pixel-mushroom-fps/诊断结论.md)与[节点核查](../../.artifacts/pixel-mushroom-fps/节点与渲染架构核查.md)针对 `G:/gameEngin-deliveries/pixel-jump-adventure-fps/Game.exe`，Windows dev profile。普通输入回放每组 360 presentation frames，预热 210、采样 150。

| 对照 | 截图 FPS | 平均 CPU ms | 平均 AUI CPU ms | 采样段 UI 上传字节 |
|---|---:|---:|---:|---:|
| 变大后静止 | 58.7 | 9.283 | 2.545 | 3,952,224 |
| 同样变大，镜头滚动 | 25.1 | 37.874 | 17.906 | 31,368,576 |
| 未吃蘑菇，镜头滚动 | 23.4 | 40.595 | 19.177 | 31,966,752 |

FPS 是截图时半秒窗口，CPU 是采样段平均，二者不能直接换算。各回放对象状态不同，只用于识别持续触发链路，不用于推断小性能差异。小角色也出现同类滚动下降，蘑菇变大不是必要条件。

游戏 `field` 一个 Custom 节点包含大量背景和地形；`actors` 一个 Custom 节点包含角色、敌人、金币等。地形 key 包含 camera，绘制把 camera 减入每个图元坐标；镜头变化扩大为内容 Paint 和几何变化。actors 每次呈现更新 revision，失效范围过大。`Painter::flush` 的重叠分组有二次复杂度风险，但尚无函数级证据证明它独自占据主要耗时。

### 2.2 源码比较与采纳

| 参考 | 实际文件／调用链 | 采纳与限制 |
|---|---|---|
| Godot | `scene/main/canvas_item.cpp`：queue_redraw／_redraw_callback；`scene/2d/node_2d.cpp`：set_transform；`scene/2d/camera_2d.cpp`：canvas transform；`renderer_canvas_render_rd.cpp`：request_polygon／PolygonBuffers／实例流；`canvas.glsl`：model 与 canvas matrix | 内容与变换／视图独立，局部 polygon GPU 缓冲复用；有序批次和紧凑实例准备仍存在 |
| Unity UI Toolkit | `UIRRenderEvents.cs`：transformID 更新／GroupTransform／NudgeVerticesToNewSpace；`UIRVEShaderInfoAllocator.cs`：SetTransformValue；`UIRenderDevice.cs`：view／scissor 批次边界 | 明确普通变换快通路和回退；不承诺任何 UI 移动都零顶点加工 |
| Unity UGUI／旧 Sprite | `Graphic.cs`：SetVerticesDirty／Rebuild／SetMesh；`ScrollRect.cs`：content anchoredPosition；旧 `SpriteRenderer.cpp`／`BatchRendering.cpp`：CPU dynamic batch | 分离内容失效与批次失效；小 quad 流式合批可以合理存在 |
| Unreal | ActorComponent 分类 dirty → PrimitiveSceneProxy transform → GPUScene primitive data → SceneView uniform → LocalVertexFactory shader；Slate：FSlateVertex CPU 变换／ElementBatcher／SlateRHIRenderingPolicy | 学习世界稳定资源和分类更新；Slate 仍上传最终 CPU 顶点，不作为全部 UI 零上传证明 |
| Bevy | extract／queue／prepare SpriteInstance → sprite.wgsl；Mesh2D asset allocator → model／view；UI prepare_uinodes | 局部 mesh GPU 常驻和紧凑实例是可行组合；实例提取、排序和参数流可每帧重做 |

地形参考 Godot dirty quadrant、Unity Tilemap Chunk／局部 RefreshTile 与 Bevy TilemapChunk；分块控制失效和可见性，不要求每格一个 Scene 节点。不复制完整 GPUScene、第二 RenderWorld ECS、Canvas registry 或专用地图 shader。

版本、绝对源码路径、行号、官方链接及不可外推范围见 [Godot](../../.artifacts/pixel-mushroom-fps/engine-survey/godot.md)、[Unity](../../.artifacts/pixel-mushroom-fps/engine-survey/unity.md)、[Unreal](../../.artifacts/pixel-mushroom-fps/engine-survey/unreal.md)、[Bevy](../../.artifacts/pixel-mushroom-fps/engine-survey/bevy.md)及[综合研究](../../.artifacts/pixel-mushroom-fps/engine-survey/比较与推荐方案.md)。Godot 4.8-dev、UE 5.8.0／Changelist 0、Bevy 0.20.0-dev 是源码参考，不是同版本性能资格；Unity 旧 native 与现代 C# 的证据分开。

Godot 对照游戏也每帧 queue_redraw 并将 camera 烘焙到图元坐标，不能把它的流畅全部归因于独立地形节点缓存。本轮没有候选实现或跨引擎同负载函数级跑分。

## 3. 既有能力、缺口与唯一 owner

| owner／位置 | 可复用能力 | 本方案增量 |
|---|---|---|
| Project SDK／Generated Glue | Draw2D／Graphics2D、typed 表现属性、图形资源、原子提交 | 默认保留创作入口；身份和缓存由引擎派生，项目不写 GPU 数据 |
| AUI instance／session／geometry | 同值消除、局部失效、最终仿射／颜色、已发布命中 snapshot | 传递相同最终结果，保持回滚、反馈／动画与输入一致 |
| `aui_graphics.rs` | 局部剖分缓存和精度档；64 项／8MiB 的 CPU cache | 稳定不可变局部 mesh 引用和版本贯通，CPU／GPU 预算分开 |
| `aui.rs`／UiProjection | 现有 DrawMesh、保留片段和必要变化闭包 | 对合格 Graphics 消费共享 mesh，减少下游 clone／最终顶点生成 |
| `runtime_renderer.rs` | retained fragment cache、CPU 仿射／精确裁剪／投影 | 按项选择 retained 或 CPU 路径，生成实例／视图／clip 引用 |
| `rhi_command_plan.rs`／既有 packet | UI 最终 UiVertexStream、图／提交计划 | 增加 typed retained 二维 payload，固定 buffer／range／内容版本 |
| `render_resource.rs`／`render_thread`／`wgpu_backend` | 资源 handle／generation、worker、GPU cache、最终顶点增量上传 | 局部 mesh GPU 分配、参数写入、真实提交保护与安全退休 |

`aui.rs` 当前将缓存 mesh clone 到 DrawMesh；`projected_custom_mesh_vertices` 和 `transform_item_polygon` 仍逐顶点／逐三角形处理；UiComposition payload 主要携带最终 UiVertexStream。现有下游缓存和 partial upload 已存在，不能宣称整个 UI 每帧必然全量重传。世界 Sprite 当前也存在 CPU 最终 quad 生成，切到 Sprite 不自动证明实例化已交付。

现有 `render_resource` 的 `safe_frame_delay=2` 与 render worker fence 不构成真实 GPU completion 证明。本方案需要补齐相关语义，而不是将它们包装成已满足的在途保护。

禁止新增平行 Projection Bridge、GPU 全局 registry、独立退休器、第二 UI authority、专用玩法 API 或 Provider 工具。可新增现有 backend 下的二维 pipeline／内部子模块；资源和生命周期沿原共同 owner 负责。

## 4. 内部数据合同

以下是职责和字段类别，具体 Rust 类型／字节布局在实施前冻结，不提前成为公共协议。

| 数据 | 内容 | 身份／版本 | 写入权 |
|---|---|---|---|
| LocalGeometry | 不可变局部顶点／索引、固有颜色／UV／coverage、局部 bounds | 内容版本、算法／格式与精度档；资源或运行时图形身份 | Graphics 准备 owner，backend 管 GPU 分配 |
| Instance2D | geometry 引用、最终仿射矩阵、Tint／Opacity、相应样式／绑定索引 | 实例 incarnation、表现版本、内部 layout key | AUI／UiProjection；backend 仅派生上传 |
| View2D | Canvas 到 target 映射、viewport、目标尺寸／设备相关 epoch | 映射／view 版本 | 原呈现／Projection owner |
| ClipState | 已解析实际交集、快通路资格、target scissor 或精确路径依赖 | 实际 clip 版本 | 原 AUI geometry／clip owner |
| OrderedDraw | 原序位置、pipeline／texture／sampler／clip／geometry 范围与参数引用 | 已提交帧身份和兼容键 | UiProjection／RuntimeRenderer |
| FramePayload | 上述不可变引用、GPU 范围与内容版本、提交使用及资源 lease | packet／device epoch | 原 render packet／backend owner |

### 4.1 内容身份与失效

- 代码构建 Graphics2D 继续使用原 Custom output owner 和 `UiNodeHandle(instance,slot,generation)` 生命周期；图形资产使用已发布 GUID／AssetRef／revision。不强制代码临时图形全部资产化。
- 固有 fill／stroke／顶点颜色／UV 等内容变化产生新内容版本；Tint／Opacity／普通平移不进入几何 key。影响曲线精度和 AA 的有效 scale／DPI 档、算法版本及 vertex layout 进入必要依赖。
- 身份在内容发布／替换时计算并复用；不得每帧深拷贝、序列化、重新 hash 大描述证明未改变。共享内容和实例身份分开，不因每个实例有不同 handle 而复制同一 mesh。
- 缓存命中不代表 GPU allocation 命中，allocation 命中也不代表 upload 为零；计量必须分开。

### 4.2 SDK、ABI 与 RuntimePackage

优先保持 Draw2D／Graphics2D／UiWriteBuffer 的既有公开入口与资源发布路径，本期 retained payload 默认属于引擎内部。二维实例可采用独立 layout key，不照搬 371 的 3D 80-byte 实例结构。

若实际跨 DLL／schema 内容发生变化，须同步 SDK、Generated Glue、Runtime consumer、版本／fingerprint 和旧输入诊断，测试真实接受／拒绝／回滚；若变化仅在内部投影／后端，不无故升级 Package 或 ABI。Renderer 不读取项目源文件、Binding 或玩法状态。

## 5. 分类更新合同

| 变化 | 必要工作 | 不得无故扩大到 |
|---|---|---|
| 普通平移／旋转，精度档不变 | 实例／祖先矩阵、bounds／命中、clip 分类、必要排序 | 原始路径生成和剖分 |
| Tint／Opacity | 实例颜色及相应批次依赖 | 大 mesh 内容替换、文字整形 |
| 公共世界视口平移／镜头滚动 | 公共变换、可见性／clip、必要批次和紧凑参数 | 所有地形内容 key／Paint |
| 一项图形／一个 chunk 内容变化 | 该内容与实际依赖准备、上传、批次 | 不相关图形或全地图 Paint |
| 缩放／DPI／目标映射变化 | 参数、必要精度档与屏幕 AA 更新 | 与精度无关的资源重建 |
| clip 改变 | 实际交集／scissor／路径分类，必要精确最终顶点 | 无条件重新生成原始图形内容 |
| 设备重建／资源淘汰后再次使用 | 受影响 GPU allocation／首次上传 | 伪报为暖资源平移资格 |

父变换影响必要后代的矩阵、宽阶段 bounds／命中和 clip，这是合理工作。目标是免去无必要的大内容及逐顶点 CPU 加工，不承诺所有子树更新 O(1)。原同值消除、局部布局和已发布 snapshot 继续有效。

对合格 retained 项，绘制消费端不得继续生成未使用的 CPU 最终顶点：逐顶点 transform／clip／projection 工作须为 0。必要 bounds／命中、矩形交集、可见性和紧凑参数准备独立计量，不计作这里禁止的大 mesh 绘制加工。

## 6. retained GPU 消费与混合顺序

```text
不可变 Graphics2D／图形资源
  -> 原 Graphics 准备缓存：LocalGeometry
  -> AUI 最终矩阵／颜色／clip／已发布几何
  -> UiProjection：有序 retained item 或精确 CPU item
  -> RuntimeRenderer／原 RHI plan 与 FramePayload
  -> backend 局部 GPU vertex／index + instance／view 参数
  -> 同一 UI Pass 中有序绘制
```

shader 消费等价的 `target_position = ViewMapping × FinalNodeTransform × local_position`。布局、pivot、祖先、反馈和动画已经合成到声明的矩阵，不能重复应用。CPU fallback 提交最终顶点时不再次套 retained 的节点变换。

一个绘制项只有一个实际路径。文字／图片和普通 Custom mesh 可以暂时沿旧最终顶点路径，仍须与 retained 项按原序混合。快通路不得将这些项提前／推后到独立全局阶段导致遮挡改变。

兼容键包含实际 pipeline／layout、texture／sampler、clip、target 及所需 geometry／参数绑定约束。透明／UI 项只在既定顺序中合并相邻兼容项，不做改变重叠关系的全局材质排序。不要求每节点一个 GPU draw，也不以复制整张地图来强行合批。

可见实例流可以按帧整理并上传紧凑参数；静态 mesh 不随其重传。持久参数与稀疏 ranges 是可选深化，不能在尚未测量时以“任何实例上传都错误”扩大建设范围；共同 owner 的变更语义和未来 371 接缝仍保持稳定。

## 7. 裁剪、颜色、精度与交互

### 7.1 路径资格

快通路要求：支持的局部 mesh／内部 layout、有限且可消费的仿射、实际精度档有效、兼容材质，以及能够按现行语义精确表达的最终 clip。首期仅对精确轴对齐矩形交集采用 target scissor。

连续祖先矩形交集在同一 target 空间解析；空交集直接不绘制。非整数边界的采样／coverage 语义必须用 CPU 基准和 GPU 像素对照证明；若 scissor 无法保留现有边界语义，该项走精确路径。坐标换算、取整、目标边界 clamp、resize／DPI 由原映射规则冻结，不能仅凭“矩形”名称判定。

旋转、圆角、复杂祖先交集及其他不合格情况保留 365 精确 CPU 路径，不以 AABB 替代真实裁剪。混合帧按项回退，一个复杂 clip 不使所有合格地形退回最终顶点加工。GPU mask／stencil 后续如有实测需求，再在同一 UI Pass 内设计有界能力。

### 7.2 颜色、AA 与采样

继承 365 实际合同及后续颜色修正：数值 Draw2D／Tint 为线性 RGBA、straight alpha，祖先 modulation 逐项相乘；样式字符串和纹理 sRGB 解码沿既有消费者进行一次，不重复解码，不擅自改预乘格式。它不是重叠子树离屏后整体透明。

局部固有颜色／UV／coverage 与实例颜色组合保持等价；精确路径的裁剪交点保留原插值语义。AA fringe 和曲线精度维持有效屏幕尺度与已声明误差，平移可复用，缩放／DPI 越过必要精度档可以重新准备。镜像、退化／零 scale、像素取整、半像素位置、实际窗口格式和 glyph／图像采样均进入正确性验证。

### 7.3 发布与命中

保留原唯一最终几何 owner。CPU 的命中窄阶段、局部指针转换及 SDK 坐标查询使用与 shader 一致的最终矩阵／clip；AABB 仅作宽阶段。输入读取最后成功发布 snapshot，不读取未接受事务或半构造 GPU payload。

参数／内容候选准备失败时不得发布半份新帧；原错误和回滚／最后成功状态按既有语义处理。GPU 提交与 UI snapshot 发布的先后由现有生产呈现 owner 贯通并验证，不能由 backend 单独修改业务可见状态。

## 8. 驻留、在途内容版本与设备生命周期

### 8.1 几何生命周期

局部 CPU mesh 和 GPU allocation 分开持有。相同内容的活跃实例共享；替换创建新内容 incarnation，旧帧持有旧内容直到对应使用完成。节点释放、内容替换、缓存淘汰和 device epoch 更新都不得让旧引用指向新对象。

GPU 几何容量、参数页／frame slot、staging、retiring、pipeline／bind group cache 分别有预算和高水位。CPU 剖分 64 项／8MiB 只约束原 cache，不代表全部 GPU 内存。

### 8.2 参数快照与提交

FramePayload 固定 buffer、range、layout／device epoch 和区间内容版本，资源 lease 本身不足以保护可写参数。普通矩阵／颜色连续更新，也不能覆盖慢 worker 或在途 GPU 仍需读取的旧帧数据。

首选实现是共同 backend 内有界 frame slot／参数页和使用 lease，支持独占写入及安全复用；也可采用能证明旧读→新写→新读的同 queue 提交顺序。选择须在第一条 GPU 生产链路前冻结并用真实旧帧／新帧像素证明，不只测试数据结构。扩容、compaction 和整页／稀疏上传都有显式版本与计数。

槽位及旧资源回收必须依据实际 GPU 提交完成或能证明等价安全的提交使用顺序。CPU worker 完成、帧编号增加、固定延迟两帧都不能替代这一证明。使用 wgpu 对应 completion／poll 机制或等价 submission owner，异常／设备丢失有可终止的状态处理。

有界容量用尽时采用可观察的背压／确定失败策略，禁止无限增加 retiring 和 frame pages、忙循环等待或覆盖仍使用的数据。临时资源驱逐或兼容回退若影响性能资格，须明确计量，不能隐藏为 cache hit。

### 8.3 设备重建

device epoch 变化后旧 GPU handle／pipeline／bind group 不可进入新提交；可从有效局部 CPU 内容重新准备 GPU 数据。CPU 内容被驱逐则沿原内容 owner 重建，不读取项目源目录或伪造暖资源资格。关闭 session／项目和重开要释放实际 owner 引用，设备释放不成为动画或玩法 owner。

## 9. 与 365／371 的衔接

365 §14.3 的 CPU 仿射／凸裁剪是已交付实际合同，保持其历史事实。372 新增快通路，继承公开创作、精度、颜色、命中和复杂裁剪；实现后新增实际合同及证据，不将旧 365 记录改写为已经采用 GPU 变换。

371 当前按用户请求暂停，恢复点 S1-C。372 不等待未实现的 S1-C 才开始，也不自动恢复 371。它在原共同 owner 内完成首期二维所需的最小资源／packet 增量；3D 常驻实例、multiview、world phase、深度和 GPU-driven 仍属于 371。

371 恢复时必须读取 372 实际交付合同，复核 S1-C／D 的 owner、布局、生命周期和测量，复用已证明的共同部分，避免重复实现；二维通过不能自动授予 371 的 3D／规模／平台资格。UI 保持独立 composition，3D depth 不侵入 AUI。

## 10. 项目整改与对照

像素跳跃冒险使用公开 SDK 和 AUI 文档建立以下坐标／失效关系，不新增项目 GPU API：

```text
固定屏幕层
  固定／独立滚动背景
  固定 viewport clip
    公共世界平移
      稳定地形 chunks
      独立动态砖块／平台
      角色／敌人／金币表现实例
  固定 HUD／菜单／FPS
```

camera 不进入各个图形内容 key，不减进每个图元的局部坐标。现有 field 混入固定 HUD 背景，必须先拆分，不能整项直接平移。viewport clip 固定在父层，世界内容是移动子层。

chunk 是局部内容失效和可见性单位，尺寸按顶点／索引／primitives 上限和实测冻结；不要求每砖独立节点。角色形状可以共享，位置、朝向和颜色是实例表现；形态变化切换相应形状或只替换该内容。地形破坏仅更新相关块，动画砖块可从稳定地形中独立。

该样本继续用 AUI 是现有消费端回归和性能对照，不成为未来所有世界对象的创作模板。玩法、碰撞与 Game／Actor 数据仍在项目，不要求为渲染把每个内部 Body 变成 ECS 节点。

对照至少保存：原故障包、项目局部坐标／分块后仍用 CPU 的版本、retained GPU 版本。比较相同 profile／输入／分辨率／呈现配置／采样段。Release 与 dev 单独给结果；不能混用 profile 把编译优化全部算作 GPU 路径收益。

## 11. 计量、预算与诊断

按 runtime／editor 分域，默认 Off，Summary 仅紧凑计数，Trace 用于显式诊断／测试。不得把 JSON、长字符串、完整候选 trace 或写文件常驻到 runtime 热路径。

必要指标：Paint 调用、剖分 builds／hits、CPU mesh copy 字节、CPU transform／clip／projection、可见性／排序／batch、稳定局部 mesh／CPU fallback 最终顶点／instance／view 各自上传字节、GPU allocation、draw／pass／bind group、新建／复用、各预算 resident／staging／retiring 高水位、总 CPU 平均／P95 和真实 FPS。已有指标复用 owner，新指标按变化范围加入。

预算先区分产品容量与实测负载，不悄悄取消 365 每输出 4096 顶点／12288 索引／4096 primitives 或现有节点／wire 上限。第一实施阶段必须冻结以下参数，未冻结前不得授予 GPU／性能资格：GPU geometry 驻留上限、参数最大在途页和字节、staging／retiring 上限、缓存淘汰／背压规则、几何 workload／chunk 数、设备／profile／分辨率、预热／采样长度、AUI 与总 CPU P95／FPS 预算及允许 draw 上限。冻结须来自现有容量和真实可运行对照，不逐项要求用户重新选择路线。

样本体验目标为恢复流畅镜头滚动，优先按 60Hz 呈现目标验证；实际资格必须绑定上述冻结矩阵。历史 400 节点的 2ms／16.67ms 预算和 dev 故障 FPS 不是新负载已达标证明。不能仅凭剖分为 0、allocation 为 0、平均 CPU 下降或 CPU 耗时倒数宣布稳定 60FPS。

## 12. 可执行需求与验证矩阵

| ID | 需求 | 必须捕获的失败／证据 |
|---|---|---|
| R01 | 稳定局部内容身份贯通 | 清洁／平移不执行大描述 hash、序列化、深拷贝或 Paint；内容替换有独立版本 |
| R02 | 暖几何普通变换快通路 | 快通路合格、可见资源已暖且有效精度档不变时，镜头平移的地形 Paint／剖分／局部 mesh 上传及绘制消费端逐顶点 CPU transform／clip／projection 增量为 0；必要 bounds／命中、可见性、参数／批次分别计量。故意恢复 CPU 最终顶点加工须使机制测试失败 |
| R03 | 局部内容／实例失效 | 一个角色移动和一个砖块改变不更新全场内容；几何／实例流上传分开归因 |
| R04 | 新可见资源和淘汰 | 跨 chunk 冷资源首次上传可发生；暖资源复用与驱逐／重建分别计数 |
| R05 | GPU 与 CPU 混合绘制 | 原序和透明遮挡相同；复杂 clip 只回退相关项，retained 项不整帧退回，不重复套矩阵 |
| R06 | 精确矩形／复杂裁剪 | 非整数边界、祖先交集、旋转／圆角／8层 clip 对照；不能用 AABB 漏裁剪 |
| R07 | 颜色／采样／精度 | Tint／Opacity／祖先调制、直 alpha、glyph／图片混合、镜像／零 scale、缩放／DPI／半像素与 AA 对照 |
| R08 | 输入和发布一致 | 坐标／命中消费同一成功 snapshot；拒绝事务、反馈／动画和重挂载不产生视觉／交互分离 |
| R09 | 在途内容保护 | 慢 worker、延迟提交／GPU、同实例连续变换／颜色和内容替换，旧帧仍画旧值，新帧画新值 |
| R10 | 生命周期有界 | 删除／重用 generation、资源驱逐、容量耗尽、session／项目重开、device epoch 重建；真实完成之前不回收或覆盖 |
| R11 | SDK／DLL／Package 消费 | 普通项目通过现有正式路径；如公共协议变更，接受／拒绝／回滚／旧输入兼容证据齐全 |
| R12 | 真正生产合批与性能 | 后端实际 draw／pass／upload 和预算；同负载 CPU 对照、GPU 版本 FPS／P95，不能仅验高层计划 |

范围相关测试必须能在故意破坏需求时失败。CPU owner／headless 测试用于快速反馈，真实 GPU 像素与在途提交验证补足 GPU 语义，普通 Windows 项目窗口／独立副本验证补足生产链路。只跑受影响测试和必要 consumer，不以文档工作触发全 workspace 编译。

## 13. 建设阶段与完成判定

以下是方案的依赖分解，不是可执行施工命令；正式施工文档须另行定义文件、Gate、测试命令、预算冻结和状态同步。

| 阶段 | 产物 | 不能跳过的条件 |
|---|---|---|
| P0：基线与合同冻结 | 真实源码／消费版本、内部 payload／layout、快通路资格、预算／负载、在途策略、项目 CPU 对照 | 保留原故障证据，确认不依赖未实施 S1-C |
| P1：局部 GPU 几何贯通 | 稳定共享 Graphics mesh → 原 UiProjection／RHI → 真 GPU 绘制 | 几何与实例／view 分离，暖变换计数与像素红绿对照 |
| P2：混合语义与生命周期 | 分路 clip、原序混合、颜色／命中／发布、真实内容版本／退休／设备重建 | R05–R10 与必要 SDK／DLL consumer 通过 |
| P3：普通项目与性能资格 | 项目分层／chunk、CPU／GPU 对照、冻结预算下真实窗口与独立交付 | R01–R12 覆盖实际范围，计数和最终性能都通过 |

只交付 P1、仅剖分 cache hit、仅项目 helper、仅 shader 演示或只跑 headless 不能宣布 372 完成。正式完成须包含首期完整链路、必要共同生命周期、正确性、样本对照及范围匹配的性能／交付证据。安装和默认 Provider 切换独立报告；源码完成不自动改变已安装 DLL 或旧游戏包。

## 14. 已确认决策与后续冻结

已确认：采用 B；首期消费端为稳定 Graphics2D 的 AUI 路径；沿现有 owner；矩形快通路加精确按项回退；CPU／GPU 参数和几何上传分别计量；二维先行补必要共同底座，371 暂停和恢复边界保持。

实施时冻结：内部 vertex／instance／view layout、参数 frame-slot／提交策略、scissor 边界规则、GPU 预算与 chunk 负载、稀疏／整页阈值、production 性能与 draw 预算。改变首期范围、破坏上游语义或降低已冻结资格需回填设计并明确理由；常规内部参数按已确认路线与证据收敛。

## 15. 首期实施引用（2026-10-10）

P0–P3已完成。实际布局／提交策略／scissor／容量／地形AA边界与Windows性能冻结见[372实际合同与验收](372-实际合同与验收.md)，完成状态见[阶段记录](阶段完成记录/2026-10-10-372-Retained-Graphics/00-总览.md)。原设计目标与延期范围保留；首期通过不授安装／3D／全平台资格。
