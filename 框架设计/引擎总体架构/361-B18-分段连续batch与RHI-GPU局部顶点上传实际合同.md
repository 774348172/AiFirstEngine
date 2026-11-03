# 361 B18 分段连续 batch 与 RHI/GPU 局部顶点上传实际合同

日期：2026-09-27。

## 1. 已兑现边界

B18关闭B17量化出的最后一条顶点复制/上传链：相邻同材质UI项目仍保持一个有序batch和一个RHI draw，但batch不再为了连续`Arc<Vec<RenderDrawVertex>>`复制全部项目顶点。Renderer现在发布保持原透明顺序的`UiVertexStream`，其中每个投影片段继续持有不可变顶点chunk；RenderGraph与RHI只转发该流，不展平CPU顶点。

`UiProjectionCache`仍是唯一lineage owner。相同batch key和相同chunk形状下，变化流沿用稳定`stream_id`、递增revision，并携带前一版不可变数据Arc及可信变化范围。1001个同材质item单点变化时仍只有一个最终batch，只替换一个chunk、复用1000个chunk，`vertices_copied`不增长。

## 2. RHI与WGPU合同

- `UiVertexStream`的进程内identity/revision/previous Arc/changed ranges不进入Serde语义；JSON继续把`vertices`编码为原有扁平数组。
- `compile_render_graph_to_rhi_plan`克隆分段流并保持数据Arc与lineage，不增加draw或资源更新命令。
- WGPU resident buffer以稳定stream identity索引。只有当前resident数据Arc正是流声明的previous、revision连续、总顶点数不变且范围有界时，才对既有`VERTEX | COPY_DST` buffer执行`queue.write_buffer`。
- 首帧、新identity、反序列化/外部输入、跳版plan、顶点数或chunk形状变化均完整创建/上传；不只信任可伪造的revision。
- clean流的数据Arc相同，不上传；局部更新与clean命中都继续复用同一buffer。两条真实WGPU绘制路径仍各执行一个batch一个draw。
- Headless consumer继续通过`RhiDrawPayload`的完整顶点语义Hash消费画面内容；lineage元数据不改变确定性结果。

## 3. 统计与容量

`UiProjectionStats`新增`stream_chunks_reused/stream_chunks_changed`，与既有`vertices_copied`分开；`UiVertexUploadStats`新增`full_uploads/partial_uploads/partial_uploaded_bytes`。这些是内存计数，不生成逐帧字符串或文件。

GPU resident数量和总字节仍受原1024 buffer、32 MiB预算约束。超过预算的draw可见性不受影响，但保持瞬时完整上传。CPU fragment/chunk复用不等于GPU局部写入，只有WGPU统计能证明后者。

## 4. 失败回退与兼容性

插删、batch key/顺序变化、顶点数变化、chunk边界变化、过期lineage和Serde round-trip都不进行局部patch。顶点扁平迭代、长度、索引、Eq、Hash和JSON数组语义保持，直接RenderGraph/RHI consumer已编译/测试。

本块不增加每item draw call，不跨不相邻材质合批，不改变alpha、clip、纹理或字体顺序，不增加公开AUI、ABI/Glue、SDK或项目dirty接口。真实窗口、安装、塔防包与整帧FPS不属于B18声明。

## 5. 验收结论

当前源码在真实WGPU适配器上证明：1001×6顶点首帧完整上传，单点变化只上传6个顶点，clean帧上传0字节；跳过一版lineage和JSON输入完整回退。真实像素刷新测试证明局部写入后颜色从红变绿且resident buffer数量保持1。B18源码合同完成；361的Gate B由B1–B18完成，后续进入Gate C，最终项目性能由D/E单独验收。
