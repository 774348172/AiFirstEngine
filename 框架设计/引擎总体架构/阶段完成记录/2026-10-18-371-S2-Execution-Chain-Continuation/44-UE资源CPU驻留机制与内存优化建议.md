# 371 阶段44：UE资源CPU驻留机制与内存优化建议

2026-10-23。用户先要求同源UE实测与内存机制研究，随后明确「现在还没有UE，那就先不用做UE测试」。因此本轮完成源码研究和优化建议，**未运行UE、没有UE进程内存测量，也不推测UE一定比本引擎低多少**。未实施引擎优化，未生成新的正式架构方案；371和原6GiB压力失败保持原状态。

证据目录：`D:/GameEngine/EngineMigration/EngineMigration/01-Engine/main/.artifacts/371/ue-memory-20261023-123943`。12:39:43 UTC开工，15:24:43停止新增、15:39:43硬截止，实际提前收尾见`05-closeout.json`。当前游戏仍消费V0.0.5，process6GiB、图形常驻4GiB保持。

## 1. 研究对象与证据边界

本机UE源码真实目录为`D:/GameEngine/EngineMigration/EngineMigration/04-Reference/Unreal/uecode/UnrealEngine-release/UnrealEngine-release`，其`Engine/Build/Build.version`标记5.8.0、Changelist 0。这是本地源码快照身份，不是已安装、已编译或已运行的UE版本资格。

源码目录的Win64 Binaries仅有工具子目录与脚本；所查UnrealEditor、UnrealEditor-Cmd、UnrealGame、UBT和AutomationTool二进制均缺。迁入Epic启动器的exe不是UE Editor/Player；旧对比UE目录为空。查过常见安装路径、引擎注册及卸载登记，但未遍历全部硬盘。用户随后明确确认未安装并跳过实测，环境核查即止。原始存在性证据见`01-environment.json`。

16处UE源码引用、函数、行号及SHA见`02-ue-source-catalog.json`，定向片段见`02-ue-source-excerpts.json`。本引擎13处原研究源码身份及2处补充引用见`03-engine-source-catalog.json`；它们在研究期间未修改。第三方UE源未修改。

复用阶段43测量：原1.5倍包含15000个独立mesh、384材质、96张2048² RGBA8纹理、15M三角形；本引擎普通private峰6.424850GiB，Godot两轮峰2.893047/3.090237GiB。43同进程持有/释放实验解释额外CPU载荷量级。本轮没有重跑这些实验，也没有把43旧DLL的测量冒称后来374合入源码的二进制资格。

另取Epic官方Texture Streaming Configuration页面并保存原HTML及URL/SHA；它支持纹理池、临时更新预算等配置解释。CPUAccess API和另一指标页面虽HTTP 200，但只返回页面外壳，**不作为正文证据**。CPUAccess结论直接以本地UE源码为据。所有官方资料身份在`official-docs/sources.json`。

本轮追踪普通静态LOD与Texture2D路径，未验证全部Nanite、虚拟纹理、UMA或其它平台的分配/恢复行为；结论不外推为UE所有资源都采用同一释放时点。

## 2. UE首先把对象寿命与CPU载荷寿命分开

`Engine/Classes/Engine/StaticMesh.h:1418–1423`解释`bAllowCPUAccess`：cooked构建在需要CPU读几何时保留CPU数据；否则上传到GPU后可释放CPU数据。并非所有网格都无条件保留，也不是销毁StaticMesh对象才能回收顶点数组。

`Engine/Private/StaticMesh.cpp:626`的`FStaticMeshLODResources::SerializeBuffers`将是否cooked、是否允许CPU读取合并为`bNeedsCPUAccess`，并传入Position、StaticMeshVertex、Color及Index buffer的反序列化。编辑器/工具与cooked游戏的保留策略不同，故不能用Editor工作集替代打包游戏对照。

`Core/Public/Containers/DynamicRHIResourceArray.h:82`的`TResourceArray::Discard`检查：不需CPU访问、平台需要cooked数据、当前不是commandlet，才清空数组。此策略允许保留资产对象、资源句柄和元数据，同时回收大块CPU数组。特定CPU读取者仍可要求保留；不能将其概括为「UE完全没有CPU几何副本」。

**对我们的直接启示：场景仍引用资产，不应自动等于全部raw、pixels、vertices必须常驻。** 依赖句柄继续保证资产身份与生命周期；大块数据的驻留由实际CPU读者和上传工作决定。

## 3. RHI接管数据后释放原CPU源，不靠逐资源等待GPU

`Core/Public/Containers/ResourceArray.h:25`给出Discard的交接语义：RHI已经复制资源数据、不再需要调用方那份CPU副本。

- `D3D12RHI/Private/D3D12Buffer.cpp:126–164`先向upload allocation复制源数据、提交copy工作，再Discard原resource array；同步点用于跟踪后续GPU工作，并非在Discard前同步等待整段GPU执行结束。新buffer initializer路径在610–613也先WriteData、再Discard。
- `VulkanRHI/Private/VulkanBuffer.cpp:652–663`先把源数据复制到可写目标，随后Discard原array，再按路径Unlock。

因此需要区分三段寿命：原始CPU源、后端上传/staging存储、GPU资源。源数组不再被后端读取时可以释放；staging仍须由后端保活至其安全退休时机；GPU资源按其自己的使用寿命存活。避免把三者全绑到场景退出，也避免通过每资源同步等待GPU来压内存。

本项目`Cargo.lock:4761`实际使用wgpu 26.0.1，已另取该版本官方`wgpu/src/api/queue.rs`并保存URL/SHA。128–129行说明`write_buffer`立即复制到staging，调用返回后调用方可释放源；`write_texture`在217–218行引用相同注意事项。这支持同样的源/上传数据交接原则。

**本轮不据此修改现有完成判定。** 我们的分行纹理上传在最后一行尚未写完前仍需源像素；现有`refresh_geometry_uploads`按GPU完成释放上传队列自己的引用是较保守的策略。提前释放可以后续单独验证；它也无法单独消除loader/prepared/snapshot的其它强引用。当前3GiB主因仍是这些长寿命持有者。

## 4. 纹理像素是临时上传输入，包内来源另行保留

UE纹理初始加载链：

1. `TextureDerivedData.cpp:3544`的`UTexture2D::GetInitialMipData`从平台数据或provider取所需mip。
2. 平台mip加载路径在2620行调用`BulkData.GetCopy(..., true)`，允许丢弃内部副本。
3. `CoreUObject/Private/Serialization/BulkData.cpp:836`的GetCopy可在满足条件时移交已有分配，避免再复制完整一份。`CanDiscardInternalData:1685`要求明确允许discard、SingleUse或可从磁盘重读。
4. `Engine/Private/Rendering/Texture2DResource.cpp:150–162`将mip复制到RHI纹理上传目标；`GetData:350–353`释放原临时MipData并清指针/长度。

流式路径同样有明确临时数据寿命：`Texture2DStreamIn_IO.cpp:96–97`向目标MipData批量读BulkData，并使用不缓存IO标志；`Texture2DStreamIn_IO_AsyncCreate.cpp:33–42`创建后释放新mip临时源，取消路径71–79也释放。它不是一条「不断复制而没有释放」的链。

`Engine/Classes/Engine/Texture.h:1268–1273`还将制作期`FTextureSource`放在editor-only data中。Runtime的来源定位/平台数据与导入源不必同时作为完整CPU像素常驻。

应学的是**保留可验证的来源身份，按需要获取字节，消费结束回收载荷**。不能简单调用RemoveBulkData照抄：本快照`BulkData.cpp:1023`的RemoveBulkData会把数据大小清零并改变可重读信息，而不是通用「保留来源、只卸载内存」操作；UnloadBulkData在此实现又受WITH_EDITOR限制。我们应在原RuntimePackage/loader合同内实现同等结果，而非照搬某个函数名。

## 5. UE预算的作用与适用限制

UE既有纹理池，也有更新纹理所需的临时预算。`TextureStreamingHelpers.cpp:138`定义`r.Streaming.MaxTempMemoryAllowed`；`StreamingManagerTexture.cpp:796–804`在正常有限池路径传入temporary budget、pool及margin，而强制全部加载/处理全部资源会走不同预算路径。

这不是Windows进程PrivateUsage的硬上限，也不是所有CPU解码的全局上限。不能把该设置的源码默认值直接复制成我们全部资源的预算；也不能开启降mip后把更小资源负载冒充同源满分辨率测试。

我们现有2GiB decodedCache测量角色、loader resident字段、上传暂存额度、4GiB图形常驻和6GiB进程门槛也应区别。43已观测decodedCache约3.195GiB，loader字段又不覆盖所有prepared/native CPU源；同步场景load没有与异步request/poll相同的resident准入。必须修实际所有权和预算消费，不能只让报表把资源改个类别。

## 6. 对本引擎的实施建议及优先级

以下是建议，尚未成为已确认的新架构方案，也未实施。细项见`04-optimization-proposal.json`。

| 顺序 | 最小目标 | 主要现有owner | 本负载涉及的有效载荷 |
| --- | --- | --- | ---: |
| 1 | 活跃资产句柄不再无条件钉住原始网格JSON；准备后可回收、需要时从原包重新取得 | RuntimeAssetLoader、cooked producer | 1.194GiB |
| 2 | 已驻留纹理的长期prepared/snapshot事实不再强持有像素；只有真实CPU读者/在途上传持有像素 | runtime_texture、cooked producer、ProductionGeometryCache/snapshot、native streaming | 1.500GiB |
| 3 | GPU resident条目尽量保存已验证身份、布局和范围，按实际CPU需要保留几何源 | GeometryBuffers、production、ResidentGeometry、streaming校验 | 0.436GiB |
| 4 | 按测得的启动峰值补齐原加载准入、在途和暂存控制，随后才处理诊断/上传次数 | 原loader/streaming、上传和诊断owner | 无已证明独立GB收益 |

### 第一块：网格raw驻留

`RuntimeAssetLoader.get_asset_bytes:288`目前只查询缓存；`ensure_decoded:574`只要cache entry存在就早退。单独把`DecodedAsset.bytes`设为None，会留下Ready句柄却让再准备取不到字节。必须一起处理原owner的字节取得/重新取得路径，保持代际、路径安全、hash/size、包缺失/损坏的错误语义。

建议先仅覆盖已确认的静态mesh raw backing，不顺手清全部material、particle或其它typed数据。保留现有包索引、来源定位、资产版本、依赖和句柄，不新增第二资产管理器。准备完成后，没有CPU读者的raw可回收；已借出的Arc继续安全存活至最后消费者释放。重复准备允许按需读取，但暖态命中不得每帧重复读包和解析。

### 第二块：纹理pixels驻留

当前`PreparedTextureAsset`同时持有payload与verified，生产缓存/快照持有该prepared对象；仅清native上传Source的payload不会回收像素。

长期保留的是尺寸、格式、mip/sampler、验证过的内容身份、GPU句柄及原包来源；像素仅在真实准备、CPU读取、上传期间被需要。应在原owner内部调整这些引用，避免全局可变像素池或新资源权威。改动必须保住共享纹理的不同采样器、同ID异内容拒绝、旧帧和旧generation、取消/失败清理及现有重新准备/重建消费者。

我们已有纹理从包加载并校验的路径，可作为重取基础；不需把「避免重读」解释为永远保留全部像素。也不能把一个可伪造的hash字符串当作验证凭据：验证身份只能来自实际校验过的载荷和既有可信构造路径。

### 第三块及峰值处理

`ResidentGeometry.source`当前持有完整GeometryBuffers，streaming在328–335行用内容比较拒绝同句柄异数据。删掉source而不替代这项检查会削弱合同。CPU需要的动态几何继续保留；静态GPU条目可研究保存验证身份及必要布局/范围。紧凑索引/normal和Cooked二进制格式是后续独立选择，不能以较大schema改造阻塞先回收现有raw。

加载顺序方面，`runtime_instance_loader.rs:123`先递归load场景及资源，之后才准备实体；同步loader还会直接解码纹理。因此稳态下降不自动保证启动峰下降。先完成前两项并测完整曲线；若峰仍集中于批量读取/解码/上传交叠，再复用原异步reservation、上传额度和owner做有界流水，限制同时存活的raw、decoded与staging。**不为尚未复现的新峰值先重建整个加载框架。**

## 7. 预期收益与验证出口

前两类有效载荷合计2,892,203,778 bytes，即约2.694GiB；三类共约3.129GiB。这是可研究回收的实际payload量，**不是已测的PrivateBytes节省**。allocator、驱动、暂存、老快照和容器容量会影响提交量及释放时点。

沿用稳定3–4GiB、峰4–5GiB作为同负载工程目标，6GiB硬门槛不放宽。单纯从6.424850机械减去前两类会得到约3.73GiB，只是粗略预算；未实现、未实测。也没有UE基线可用于承诺「达到UE水平」。

后续最小验证顺序：

1. 原loader的确定性寿命/完整性反例：raw已回收但句柄有效；再准备成功；损坏/缺失包拒绝；两个CPU消费者、最后消费者和代际仍正确。
2. 直接prepared/native消费者：部分上传源保活；暖帧不重新解码/读包；同ID异内容仍拒绝；共享采样、旧帧、取消和失败不早释、不滞留。
3. 对应fmt/check及受影响消费者通过后，必要的一次Release消费原1.5倍包，仍为15000mesh/384材质/96张原尺寸无压缩纹理、720帧、相同分辨率、截图与退出核对。统一外部PrivateUsage采样，区分读取、解码、准备、上传、稳态、截图和退出；按实际改动补必要峰值检查。

完整census只作有界诊断，不接回每帧。4GiB图形预算不能替代6GiB进程和2GiB CPU角色预算。不得缩规模、压纹理或降低mip之后沿用原负载资格。没有证据要求先换allocator、重写ECS、移植UE的GC/AssetManager/IoStore或另建全局缓存。

## 8. 本轮收尾

UE实测按用户指示未执行；本轮无UE或本引擎构建、Cook、产品测试、安装或永久源码修改。所有新文件为研究证据及文档；源身份、HEAD/index/空锁、B路登记保留与最终费用见`05-closeout.json`。官方源码/资料获取耗时单列，研究阅读与文档时间不伪装成编译或测试时间。

研究已完成；下一块推荐先实现**静态网格原始载荷按需驻留与安全回收**，再接纹理CPU载荷寿命。当前仍未修复6GiB压力失败，也未完成371整体。
