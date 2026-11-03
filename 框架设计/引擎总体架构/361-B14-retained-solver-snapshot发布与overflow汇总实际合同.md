# 361 B14 retained solver snapshot发布与overflow汇总实际合同

日期：2026-09-26。状态：B14源码范围完成；361、B整体及C-E仍在施工。

## 1. 实际能力

`AuiC2Runtime`现在先完成全部canvas的retained求解、段落错误收集和失败solver清理，再按成功canvas顺序只读借用各solver当前的`AuiLayoutSnapshot`。`c2_layout_result`与`AuiGeometryCache`直接消费这些借用，不再为协调`HashMap`可变借用而clone整份snapshot。1002节点单点变化的C2发布阶段复制节点数从1002降为0；snapshot仍由原solver唯一持有，最终layout仍由原geometry cache发布。

`SnapshotSlots`同时维护按snapshot slot排序的overflow成员索引。节点`overflows_parent`翻转时，通过二分定位只插入或删除对应的公开`overflow_node_ids`成员，不再扫描全部snapshot geometry重建列表。1001节点中单个overflow翻转的汇总扫描数从1001降为0、成员更新数为1，列表顺序与全新solver结果一致。

公开`AuiLayoutSnapshot`字段、Serde形状和owned布局入口未变；新增统计不进入Serde。多canvas按document顺序发布，求解失败的canvas不借用旧snapshot，段落失败移除solver后也不会发布过期snapshot。最终geometry cache既有局部`overflow_count`维护保持不变。

## 2. 已验证边界

- 1002节点retained C2单点可见性变化直接发布solver snapshot借用，最终layout和overlay与全新runtime一致。
- 1001节点单个overflow成员由true变false只更新1项、扫描0项；geometry与overflow列表和全新solver一致。
- retained clean、translation、尺寸/文字变化、Hidden/Collapsed、失败后重试、late multi-canvas、结构重建及原C2语义回归通过。
- 直接cached Presenter没有生产调度器的snapshot/viewport复用证明，继续保守比较source；生产零比较合同仍由`AuiPresentCache` owner测试覆盖。
- engine owner共144项测试通过，engine_runtime Clippy exit 0；既有121条engine_runtime与2条engine_input warning保留，无B14新增warning。

## 3. 未完成范围

本块不处理prepared resolved document的COW、命令/composition/glyph组装复制或Renderer扫描/合批复制，也不修改ABI/Glue、SDK、Renderer/GPU、项目接口或安装。下一整块先量化prepared document COW与剩余组装复制的真实首因，再冻结范围。

安装仍为360，旧塔防包未更新；本块没有真实窗口、GPU、FPS、整帧性能或发布资格声明。
