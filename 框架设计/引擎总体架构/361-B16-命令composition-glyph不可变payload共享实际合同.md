# 361 B16 命令/composition/glyph不可变payload共享实际合同

日期：2026-09-27。状态：B16冻结范围已通过；361、Gate B及后续Gate C-E仍在施工。

## 1. 问题与边界

B8已经能复用节点绘制片段和C2文字片段，但复用片段写入新帧时仍深复制全部`AuiDrawCommand`和`AuiTextGlyphQuad`；composition随后又为每条命令重新构造`AuiOverlayDrawItem`，重复复制字符串、裁剪与paint。单点文字变化因此仍在drawing payload层产生与总命令/字形数成正比的复制。

B16只修复这条上游不可变payload生命周期。Renderer逐item扫描、glyph hash/index扫描、最终相邻batch合并和顶点复制是共享payload之后的下游成本，留给下一块量化和处理。未修改schema版本、ABI/Glue、SDK、GPU/RHI协议、安装或塔防项目。

## 2. 实际合同

- `AuiDrawList.commands`以`Arc<AuiDrawCommand>`持有命令；drawing cache复用节点时只复制Arc，节点实际依赖变化时才重建该节点的命令payload。
- `AuiOverlayFrame`与`AuiCompositionStageFrame`的顺序容器持有`Arc<AuiOverlayDrawItem>`。外层容器仍负责stage和透明排序；同一命令Arc且canvas、stage、sort key、effective clips、resolved paint均未变化时，composition复用原item payload。
- `AuiTextGlyphPlan.quads`持有`Arc<AuiTextGlyphQuad>`。C2缓存按item、paragraph、字体registry身份和目标scale失效；未变文字item复用原quad payload，变化item只重建自己的quad。
- Serde对Arc透明，JSON wire继续是原有数组和对象形状，不增加schema字段或版本。
- Renderer对item和glyph先使用`Arc::ptr_eq`识别同源payload，再以值比较兼容外部构造、反序列化和COW后的等值输入；不得只凭裸地址跳过语义检查。
- 缓存仍受`MAX_DRAW_NODES=16384`和`MAX_GLYPH_QUADS=65536`约束；越界或缺失computed/canvas时清除对应retained payload缓存并保守重建。
- `AuiDrawingStats`新增command/item/quad payload built/reused累计计数，只用于开发验收，不进入Serde、RuntimePackage、ABI或项目协议。
- 测试和外部调用若要修改共享payload，必须通过嵌套`Arc::make_mut`执行COW；旧帧保持不可变。

## 3. 实现结果

1002节点、2002命令的retained样本中，单点文字变化结果如下：

| payload | rebuilt | reused | 身份结果 |
| --- | ---: | ---: | --- |
| command | 2 | 2000 | 未变command保持Arc身份，变化节点两条command换新 |
| composition item | 2 | 2000 | 未变item保持Arc身份，变化节点两条item换新 |

双文字item样本中，单点item/paragraph变化只重建1个glyph quad并复用兄弟quad 1个；连续覆盖rect、color、clip和paragraph glyph color变化，未变兄弟均保持Arc身份。字体registry或presentation scale变化继续按真实依赖保守失效。

Renderer现有逐item片段缓存可以直接消费上述身份，同时保留值比较oracle。最终stage扫描、glyph plan hash/index扫描、batch顶点拼接并未由本块消除。

## 4. 验收与非声明

engine AUI 94项、C2 paragraph 27项、Renderer 33项（与AUI筛选重叠5项）、Player AUI 12项、Editor GameView 25项，共186项去重测试通过；三crate定向Clippy退出0，指定文件diff检查通过。JSON/排序、多stage、反馈、字体/paragraph/viewport失效、Renderer无缓存结果及两端consumer由上述套件覆盖。

本合同不声明Gate B完成，不声明整条变化帧已经O(变化量)，也不提供真实窗口、GPU、塔防FPS、安装、导出包或发布资格证据。
