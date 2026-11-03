# 371 阶段43：与 Godot 内存差额的因果研究

2026-10-23；用户要求全面研究内存为何高于Godot。本轮研究完成，未实施永久引擎优化；371/S1/S2仍partial，现行进程6GiB门槛保持。12:09:28 UTC开工，14:54:28停止新增、15:09:28硬截止；实际提前收尾见证据closeout。证据根`.artifacts/371/memory-research-20261023-120928`。

## 1. 核心结论与可信程度

当前差距的主因是**渲染资源上传后，CPU仍长期保留序列化网格、纹理像素和解码几何数据**。本轮对原包逐项计量，并在同一Godot Release进程做持有/释放差分，得到3.145GiB的额外进程私有提交量，量级相当于阶段42两侧差额的89%–94%。这不是仅凭clone/Arc名称作出的推测。

该比例表示“额外保留这几类数据足以解释大部分量级”，不是精确的跨引擎分配账本，也不是已经在本引擎中测得同等节省。两边allocator、驱动、CPU组件/材质/渲染图有差异。剩余差距没有全部定位；没有证明Rust固有开销或某个驱动泄漏导致全部差距。

## 2. 基线和观测口径

复用阶段42：同源15000独立mesh/instance、384材质、96张2048² RGBA8无压缩/无mip纹理、15M三角形，RTX4090/Vulkan，1765×993，720帧。普通本引擎private峰6.424850GiB，Godot4.7.1 Release两轮3.090237/2.893047GiB。旧证据根`budget-godot-20261023-111515`，不重Cook/export或重复旧功能测试。

本轮引擎观测继续使用阶段42已测DLL `94ca6123b942770ce6984454bda592559dd0b745bc0be0b3f4f1259ea3fe96a0`及原包，未把374后来合入的当前源码冒称此DLL的构建输入。源码追踪用当前文件，相关函数/行号/SHA见11-engine-source-evidence.json；B已合入源和旧实测二进制分身份。

统一外部采样仍为Windows `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`，0.1s间隔。Private提交、working set、独立VRAM是不同口径，不能作为互斥项目直接相加。内部CPU backing观察按同时存活地址去重，coverage为partial，不包含全部allocator/driver/系统容器节点。

## 3. 本引擎真实保留的数据

| 数据 | 本负载实际有效载荷 | 为什么仍在CPU上 |
| --- | ---: | --- |
| 96张解码RGBA8纹理 | 1,610,612,736 bytes，**1.500GiB** | loader的VerifiedTexturePayload、PreparedTextureAsset及ProductionGeometryCache/snapshot共同保留像素Arc |
| 15000份网格序列化JSON | 1,281,591,042 bytes，**1.194GiB** | RuntimeAssetLoader的DecodedAsset.bytes保存原始mesh bytes；场景及实体依赖句柄仍活跃 |
| 解码几何vertex/index | 468,000,000 bytes，**0.436GiB** | GeometryBuffers/PreparedGeometry/快照以及native Mesh.source保留CPU源缓冲，供上传、内容一致性和后续重建路径消费 |
| 以上三类有效载荷合计 | 3,360,203,778 bytes，**3.129GiB** | 这是三类独立CPU数据；不含GPU副本，不再把Arc别名相加 |

包中顶点/索引实际二进制只有468MB，但CookedRenderAsset仍以JSON数字数组保存u8，序列化网格体积约为有效几何字节的2.74倍。JSON文本膨胀并且长驻，构成了独立于解码几何的一份占用。

01-single-census仅运行121帧、120预热+1次扫描，未请求截图：

- 去重已观察CPU backing合计3,673,622,989 bytes（3.421GiB）；assets为3,437,002,706 bytes（3.201GiB）。其余已观察owner合计约225.7MiB：instrumentation68.9MiB、snapshot元数据85.9MiB、queues25.8MiB、renderer24.3MiB、World20.7MiB。
- `cpu.decodedCache`角色3,430,951,243 bytes（约3.195GiB）覆盖原始cache及prepared几何等共享backing；它不是仅loader.streaming_snapshot的cache字段。其余asset metadata/容量差额不能全部叫“另一个完整mesh副本”。
- `cpu.snapshot`约2.031GiB与decodedCache角色重叠，**不能再相加**。相同像素/几何被多个消费者观察时按地址去重。
- `streamingResources.cpuResidentBytes=2,909,468,145`只统计loader的bytes_len及外部reservation，未覆盖全部renderer/prepared源缓冲；不能拿它代表进程总量。
- 扫描前逐帧private样本约**6.392502GiB**，说明未截图时本身已经超过6GiB，截图不是主要的3GiB差额来源。

扫描本身产生6,363,550条观察记录，捕获约1.600s、后续归并约1.430s，外部峰升至9.708GiB。这个峰包含诊断分配，**不能替代普通6.425GiB基线**。本轮只扫描一帧，不把昂贵全量census接回每帧路径；观测自扰也是单独的工具问题。

## 4. 生命周期调用链

`RuntimeInstanceLoader.load_scene`调用asset_loader.load(scene)，递归加载依赖，并收集实体资源；RuntimeSceneInstance.owned_asset_handles继续持有句柄。RuntimeAssetLoader.load_internal为每个handle及其依赖保留引用，ensure_decoded对mesh/material保留原始bytes。场景仍活跃时，这些cache不会因GPU已上传就释放。

`RuntimeRenderAssetProducer.prepare_cooked_from_package`读取原bytes，校验digest/size，解析CookedRenderAsset；prepare_mesh_renderer再通过GeometryBuffers::from_cooked生成CPU几何源。临时producer handle已正确release，但场景/实体仍有自己的handle，因此不会移除原cache。前阶段41修好了**最后引用释放后的退场**；这不等于活跃场景内原始载荷会自动回收，因而普通峰值不降与该修复不矛盾。

纹理方面，rgba8是Arc<Vec<u8>>，材质采样变体可克隆小metadata但共享像素。并非15000个物体各存一张16MiB纹理；**共享已经存在，共享那一份仍长期驻留**。ProductionGeometryCache及ProductionGeometrySnapshot持有PreparedTextureAsset，后者payload和verified仍连到CPU像素。native streaming.complete_progress已把自己Source::Texture.payload置None；此处没有证据支持“上传队列从不释放像素”的指控，其它Arc owner才使backing继续存活。

GeometryBuffers也通过Arc共享，native Mesh.source同样持有它用于内容比较和上传。当前路径不能直接把CPU bytes设空：再准备、同ID异内容验证、设备重建、热更和取消/旧代生命周期都仍需保全。

## 5. Godot源码对照

本机既有源码目录version.py为4.8-dev，不能直接当4.7.1实测源码。本轮从官方`4.7.1-stable`标签读取对应文件，原文、URL及SHA保存在`godot-official-4.7.1/sources.json`，以下结论基于这些同版本文件及实际实验：

- `scene/resources/image_texture.cpp`：ImageTexture::create_from_image→set_image→RenderingServer.texture_2d_create。ImageTexture保留尺寸、format、RID等；`image_stored`是状态，不是持有完整Image对象。get_image转到RenderingServer读图，不能由这个bool推断始终保留CPU像素。
- `renderer_rd/storage_rd/texture_storage.cpp`：texture_2d_initialize用局部Image和data_slices创建RD texture；常规创建不填充完整image_cache_2d。显式texture_2d_get读回时才可能缓存Image。本次基线脚本没有调用这些纹理的get_image；局部PNG解码Image在循环/函数退出后不再由项目持有。
- `scene/resources/mesh.h/.cpp`：ArrayMesh::Surface保留format、count、AABB、material等metadata；add_surface_from_arrays打包后交给RenderingServer，Surface没有原始位置/法线/UV/索引四组CPU数组字段。surface_get_arrays需要向RenderingServer取得数据；基线只检查len，没有全量读回。
- `renderer_rd/storage_rd/mesh_storage.cpp`：mesh_add_surface将打包数据用于创建GPU vertex/index buffers；后续取surface数据会通过buffer_get_data读回。不能由此宣称驱动零CPU开销，但它避免了与本引擎相同的长期资源源数据持有策略。
- RenderingDevice有独立staging池和复用；Vulkan后端资源使用VMA并偏好device memory。没有将这些内部实现差别直接换算成未测出的GB节省，也未据此建议更换本引擎allocator。

## 6. 同一Godot进程的因果实验

先按原负载正常运行720帧；然后**保持GPU资源、对象和绘制不变**，逐步把真实CPU源数据保存在脚本数组里：96张原PNG重新解码的Image、15000份原包mesh JSON字节、同源转换文件中的468MB解码几何字节。各有效阶段持有约2秒，统一采样；最后逐项释放，仍保持原场景绘制，之后才截图和卸载。

08-godot-retention报告确认真实Release（editorFeature/debugFeature均false）、原数量、retentionExperimentPassed=true、截图OK、exit0。各持有阶段GPU video/buffer/texture计数始终为2,038,410,752 / 368,455,260 / 1,648,524,800 bytes，15000 objects/draws、15M primitives不变。

| 阶段 | 阶段末次PrivateUsage |
| --- | ---: |
| 原始Godot场景 | **2.922367GiB** |
| 加保留CPU纹理像素 | **4.425995GiB** |
| 再保留原mesh JSON | **5.654922GiB** |
| 再保留解码CPU几何 | **6.066994GiB** |
| 全部三类CPU数据释放，GPU场景仍在 | **2.929291GiB** |

实际额外提交**3.144627GiB**；释放后距原值约**7.09MiB**。这与三类实际载荷3.129GiB相符，且把Godot进程推到了接近本引擎的量级。图见10-cpu-retention-cause.png/PDF，数据见09-findings.json。单独释放geometry和raw时，进程提交减少不严格等于那一类payload大小，因为allocator可延后回收或合并空闲区域；不能把每一步的提交差都当作独立精确归属。

05第一次实验因run-owned清单使用相对路径，在Godot证据目录下找不到原网格，被显式size/count检查拒绝、exit2，未完成后两类持有；该轮不作为完整因果结果。首份脚本、输入、日志及06-review保留；修成已验证的绝对路径，并明确传播内层失败后，07独立导出、08完成。没有放宽断言或修改原包。

## 7. 次要原因、预算与未知项

**GPU布局确有差异，但量级小得多。**本引擎GPU mesh为468MB。Godot4.7.1同源600顶点/mesh使用16-bit索引：45M indices从180MB降为90MB；正常normal占4bytes，并补4bytes tangent，相对本源12byte normal净少4bytes/vertex（36MB）。同源mesh打包预测为342MB，与Godot运行buffer总量减卸载后基础buffer后342,049,152 bytes接近；总mesh差约126MB（120.2MiB）。这不是减面、LOD或纹理压缩；GPU内部normal表示有量化，故不能称两边GPU字节完全相同。

两侧GPU账本总差约91.8MiB，包含其它资源，口径也不完全相同；更不能把它直接加到CPU私有提交差。该项可以后续优化，却解释不了原3.3–3.5GiB差距。

**预算角色与加载准入并未完全覆盖同一存储。**当前cpu.decodedCache门槛2GiB，本轮census角色约3.195GiB，实际已超约1.195GiB。同步RuntimeAssetLoader.load→load_internal→ensure_decoded路径没有与async request/poll相同的resident准入；2GiB测量预算不意味着启动会被硬拦在2GiB。且只盯loader.streaming_snapshot会漏prepared/renderers持有的CPU源。应作为后续预算/所有权修复的明确范围，不把本轮可运行当成缓存预算通过。

**诊断和上传形态是后续项。**loader diagnostics仍append-only，当前已观察instrumentation约68.9MiB，值得单独按report策略处理；它不是数GiB主因，也不证明静态运行一直泄漏。纹理上传逐行调用write_texture，96×2048=196608次，与计数吻合；显式staging峰约32MiB，稳态pending和staging不支持“显式队列持续堆了1.5GiB”的说法。WGPU隐式staging、allocator高水位、驱动提交的精确占比仍未完整归属，不能虚构数值。

剩余约数百MiB差额还含World/queue/snapshot metadata、CPU容器capacity和驱动/allocator差异。本轮没有每笔系统分配的完整分类，也没有长窗泄漏资格；保留这些边界，不将“约90%量级已解释”写成100%账平。

## 8. 建议的实施优先级（尚未实施）

1. **先研究原始mesh JSON backing的可回收时机。**保留资产句柄、版本、digest/size、依赖和包内来源身份，准备/验证完成后不再无条件钉住全部原始文本。当前get_asset_bytes及重新prepare仍依赖raw，必须在原owner明确可再读/重新准备合同，不能只清空字段或放松hash校验。该类有效载荷上限约1.194GiB，不能预先承诺同量PrivateBytes下降。
2. **然后处理静态纹理CPU像素驻留。**GPU实际完成且无CPU读者后回收可再生payload；必须同时检查loader、prepared、snapshot等所有Arc持有，清一个上传队列无效。设备重建/新代上传从原RuntimePackage恢复，流式取消/在途提交和旧帧仍安全；不引入第二资源权威或全局cache。当前该类载荷1.5GiB，是第二个主要收益来源。
3. **再处理CPU几何源和GPU布局。**CPU几何不能在仍承担校验/再上传时删除；可比较保留冷CPU源与按需重建两种成本。16-bit索引/紧凑normal及Cooked二进制产物属于后续选择，前者GPU收益约120MiB，优先级低于前两项。改变Cooked合同/设备恢复语义时先讨论具体方案，研究记录不等于已批准新schema。
4. 最后按实际收益处理diagnostics有界化、上传分块及容器/allocator高水位。没有证据要求先更换allocator、重写ECS、重建RenderGraph或调整6GiB硬门槛。

可继续用稳定3–4GiB、峰4–5GiB作为原负载工程目标。仅按前两类载荷机械相减会得到约3.73GiB，但那是粗略预算推算，**不是已测优化结果**；必须做原规模实跑后确认。已批准GPU resident4GiB不变，process仍6GiB。

## 9. 后续实现如何验证

先在原owner构造能观测backing存活的确定性反例：准备结束且无CPU需求时raw/pixels可释放；两个共享消费者、最后消费者、失败/取消、旧generation/epoch均保留；原hash/size错误仍拒绝。若新语义涉及可再读，则覆盖重复prepare、同ID异内容及缺包/损坏恢复拒绝。真实GPU验证上传完成前源/lease保活，完成后回收不改变画面；设备重建可恢复上传，不能靠关掉截图或缩规模通过。

源码修复后一次必要Release消费同一1.5×K0资源，复核720帧/截图/资源数量、统一private采样和实际CPU backing；不把本轮Godot增量直接当引擎节省。再按原固定退出条件补持续/规模资格，复用身份未变证据，不重跑整个历史workspace。此处为后续测试建议，本轮未新增产品测试或执行优化。

## 10. 费用与收尾

本轮无引擎构建或Cook，无永久Rust/SDK/schema/安装/预算改动。现有DLL单帧census21.469s；Godot有效因果运行33.860s，首次失败22.641s单列；import/export、图生成及官方源码获取费用见12-cost-ledger.json。研究入口/阶段记录已同步，374 B路当前状态保留，原HEAD/index/空锁保持。源码观察身份、截图与owned进程最终核对见13-closeout.json。本研究完成不关闭371整体或原6GiB内存失败。
