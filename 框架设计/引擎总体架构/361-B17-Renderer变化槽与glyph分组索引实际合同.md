# 361 B17 Renderer变化槽与glyph分组索引实际合同

日期：2026-09-27。状态：B17冻结范围已通过；361、Gate B及后续Gate C-E仍在施工。

## 1. 问题与边界

B16已经让命令、composition item和glyph quad成为可共享的不可变payload，但Renderer在任一stage外层Arc变化后仍比较全部item；glyph plan外层Arc变化后仍扫描全部quad并重建`item_id -> indices`索引。随后所有item fragment再次合并为连续batch，`Arc::make_mut`还会复制受影响batch的顶点。单点变化因此同时包含“可消除的重复扫描”和“当前连续RHI buffer合同下不可避免的batch复制”两类成本。

B17只修复前者并量化后者。不修改RHI命令、GPU上传协议、draw call数量、schema版本、ABI/Glue、SDK、项目dirty接口、安装或塔防项目。

## 2. 实际合同

- 有界`AuiDrawingCache`在既有composition排序组装点记录每个stage的前后`Arc<Vec<Arc<AuiOverlayDrawItem>>>`身份与变化槽；C2 glyph cache记录实际变化/删除的item分组。
- `AuiCompositionFrame.projection_hint`只在进程内传递上述事实，`serde(skip)`且不参与帧语义相等；JSON wire、RuntimePackage、报告schema和外部项目协议均不增加字段。
- Renderer仅在hint的前一stage、当前stage、前一glyph plan和当前glyph plan Arc身份全部与自己的缓存一致时消费hint。外部构造、反序列化、手工COW、过期hint、结构不匹配或超预算输入继续完整扫描。
- 可信hint路径从原fragment槽表中原地替换变化槽；未变fragment不逐项比较。item id槽表随变化槽维护，glyph变化item通过该表定位对应投影槽。
- glyph索引改为`item_id -> Arc<Vec<Arc<AuiTextGlyphQuad>>>`，不再保存会因前方字形数变化而整体偏移的flat indices。可信glyph hint只替换变化分组；保守路径仍从完整plan重建。
- glyph稳定hash改为item叶子加有序哈希树。变化item只重算自己的叶子和树路径，未变叶子复用；非缓存路径用同一算法生成相同结果。旧的逐quad `format!`、临时字符串数组和`join`已删除。
- `UiProjectionStats`新增hint槽、fallback stage扫描、glyph分组更新、fragment合并和顶点复制累计计数；`AuiDrawingStats`新增glyph hash叶子built/reused计数。统计只用于开发验收，不序列化。
- 最终输出继续按透明顺序合并相邻同key fragment，并继续向RHI提供连续`Arc<Vec<RenderDrawVertex>>`。B17不宣称该层已局部化。

## 3. 实现结果

1001个Rect item的单点变化：可信hint路径`items_compared=1`、`items_projected=1`、`hint_slots_consumed=1`、`fragments_reused=1000`；没有hint的同一手工COW输入仍`items_compared=1001`并增加一次fallback stage扫描。两条路径均与无缓存oracle的batch、诊断、裁剪和顺序一致。

1001个Text item/1001个glyph的单点glyph变化：只更新1个glyph分组、只比较并重投影1个item，`glyphs_indexed`不增加。双文字生产cache样本中，每次变化只构建1个hash叶子并复用兄弟叶子1个；cached/uncached plan hash保持一致。B16的1002节点/2002命令生产样本会发布精确变化槽`[1000, 1001]`。

计数同时证明剩余成本没有被掩盖：上述1001 item单点变化仍需合并1001个fragment，巨大同材质batch仍产生连续顶点复制。若要消除它，下一块必须评审分段batch、draw call和RHI/GPU局部上传边界，不能只改Renderer统计。

## 4. 验收与非声明

engine AUI 94项、C2 paragraph 27项、Renderer 35项（与AUI筛选重叠5项）、Player AUI 12项、Editor GameView 25项，共188项去重测试通过；三crate定向Clippy退出0，B17无新增warning，指定文件`git diff --check`通过。Serde round-trip、外部COW回退、插删重排、字体/viewport失效、反馈路径及两端consumer由上述套件覆盖。

本合同不声明Gate B完成，不声明变化帧端到端O(变化量)，也不提供真实窗口、GPU、塔防FPS、安装、导出包或发布资格证据。
