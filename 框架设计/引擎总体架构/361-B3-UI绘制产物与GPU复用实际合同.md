# 361 B3 UI绘制产物与GPU复用实际合同

日期：2026-09-25。范围：现有Presenter的绘制产物 → 共用RuntimeRenderer → RenderGraph/RHI → 两条真实WGPU绘制路径。不是全部B、项目窗口或FPS验收。

## 1. 共享产物及修改安全

`AuiCompositionStageFrame.draw_items`改为`Arc<Vec<AuiOverlayDrawItem>>`；Overlay/Composition共享`Arc<AuiTextGlyphPlan>`；UI专用RenderPassCommand/RhiDrawPayload的vertices共享`Arc<Vec<RenderDrawVertex>>`。Serde数组/对象形状不变，未更改项目SDK、ABI 1.3、RuntimePackage版本或安装。

Rust内部生产者使用`.into()`发布，修改已发布内容使用`Arc::make_mut`。缓存持有强引用，修改者不能原地改掉仍被缓存引用的数据；写时分离产生新身份。不会仅相信可伪造的公开revision，也不靠裸地址而放任旧地址被重用。

控件反馈仍通过原theme/visual override/paragraph路径计算。当新stage内容或glyph plan与上一份相等时保留上一份Arc；按下、过渡和松开恢复的实际差异继续发布。这里的内容相等检查及反馈重建仍为全量，不能称为反馈本身零扫描。

## 2. Renderer复用与失效

原RuntimeRenderer持有一个线程安全缓存，至多保留BeforeWorld/ScreenOverlay/Modal三个stage。Player与Editor的RenderThread继续使用此owner，不各自实现另一套投影算法。

命中条件包含draw_items和glyph plan的共享身份、实际canvas→target映射及目标尺寸、纹理映射。RuntimeTextureBindingContext内部使用Arc映射，受控insert写时分离；共享身份可快速判同，新建但内容相同的资源上下文以映射相等兼容。这里可能比较纹理表，不宣称任意资源producer均O(1)。presentation revision/target标签不同但实际坐标映射相同，不强制重投影。

未提供共享presentation时，原备用解析会扫描draw items收集canvas；本轮也缓存该解析，按draw_items身份、canvas references、extent、scale policy失效，避免在投影命中之前偷偷全扫。提供共享presentation时直接消费既有映射。正常清洁帧不会再次索引glyph、解析颜色、裁剪/细分几何或深拷贝顶点。

变化帧目前仍重新投影整个stage，然后与相同顺序位置的旧batch比较，保留未改变batch的顶点Arc，供后端免上传。只合并相邻同材质/纹理batch，不跨中间透明内容重排。节点精细依赖索引和局部几何重建不在本块完成声明中。

CPU投影缓存的顶点分配capacity合计上限32 MiB；超额不缓存该产物，必要时淘汰旧stage，但当帧仍绘制。该上限仅指投影顶点，不包括共享源文档/绘制项/字形产物及图命令元数据。备用映射缓存每stage一份，无历史revision队列。UI卸载/空stage在下一次build清除缓存，renderer销毁随owner释放。

`ui_projection_stats()`按需读取累计stage命中/重建、备用映射扫描、投影item、字形索引、生成顶点、batch复用计数；不每帧输出JSON或日志。图命令与batch列表仍会组装，不能把“不扫节点/字形/顶点”写成整帧完全零工作。

## 3. RHI及真实GPU

RHI编译clone顶点Arc，不复制完整顶点数组。RealWgpuBackend的普通UI/Sprite路径和Geometry/Particle/UI有序混排路径共用准备函数。

UI顶点按共享分配身份查找驻留buffer，命中时不再将OrderedF32逐顶点转换为RuntimeVertex，也不create_buffer_init或上传。GPU仍每帧按原顺序执行可见draw；不会复用旧帧画面。非UI绘制继续沿用原来的顶点准备语义。

每个backend最多1024个驻留UI buffer，合计32 MiB上传字节；超额绘制使用当帧临时buffer，不丢draw。每次有效计划准备仅保留该计划引用的UI源；无UI的有效计划清空，backend/device生命周期结束释放。错误计划提前拒绝时可以保留上一有效计划的有界缓存。缓存强持有源Arc，保证地址身份有效；新源即使名字、帧号、节点ID相同也不能错误复用旧buffer。

`ui_vertex_upload_stats()`提供累计上传次数/字节、命中数和当前驻留数量/字节。源变更可以新建buffer；本块是未改变batch的上传复用，没有宣称已实现同一大buffer内的任意局部区间写入。

## 4. 验收与剩余工作

生产owner测试覆盖连续8帧零新增扫描/几何/上传、写时分离、clip/viewport/canvas reference/纹理与字体generation失效、透明顺序、反馈稳定及按下恢复、RHI共享、容量和卸载。真实offscreen GPU读取红→绿像素，另由原Particle/UI混排用例验证绘制顺序和免重复上传。具体命令、首次编译失败与终态身份见[B3执行记录](施工文档/证据/361/B3-执行记录.md)。

Player/Editor共用下游能力，Editor独立的常驻属性观察与Presenter缓存接线尚未补齐。本轮不修改项目，不更新360 Provider或旧塔防包。

下一完整块应收敛上游变化传播：反馈清洁帧免全量重建、document/geometry/clip/hit依赖索引、Editor常驻属性入口；再接C生命周期/扩展、D项目迁移、E固定负载和真实窗口性能。B、361均不提前关闭。
