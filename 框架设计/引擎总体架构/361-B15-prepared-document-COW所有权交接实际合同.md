# 361 B15 prepared document COW所有权交接实际合同

日期：2026-09-26。状态：B15冻结范围已通过；361、Gate B及后续Gate C-E仍在施工。

## 1. 问题与边界

生产`AuiPresentCache`已在决定重建后释放内部旧`AuiRuntimePresentOutput`，但Presenter仍先从`prepared_input`浅克隆`AuiPreparedInput`，C2 runtime又以`source`持有同一`Arc<AuiDocument>`。单点retained内容变化随后调用`Arc::make_mut`时，至少两个内部强引用仍然存在，因此无调用方观察旧帧也会复制完整prepared document。

B15只修复这条prepared input/C2 source所有权链。命令拼接、composition stage组装、glyph汇总/hash和Renderer扫描/合批复制位于后续drawing链，不属于同一首因，未纳入本合同。未修改schema、ABI/Glue、SDK、Renderer/GPU、安装或塔防项目。

## 2. 实际合同

- 调度器证明snapshot、retained实例、viewport、虚拟列表及文档结构允许局部复用后，Presenter必须从缓存移出prepared input，而不是先制造一个额外强引用。
- retained内容变化前，若C2 `source`与prepared document共享同一Arc，runtime必须释放该内部source引用；精确slot变化提示优先于同一Arc身份判断，使既有solver/geometry cache仍按真实变化节点更新。
- 没有外部旧输出或旧document Arc时，单点Text/Visibility/Interactable更新必须原地修改prepared document，`nodes`存储身份保持，完整document复制节点数为0。
- 调用方仍持有旧输出或旧document Arc时，`Arc::make_mut`必须执行COW；新旧document存储身份分离，旧帧内容保持不可变。不得以强制原地写破坏快照语义。
- 纯translation不修改prepared document，继续直接共享Arc且复制节点数为0。
- snapshot/binding/theme/viewport/字体/结构/虚拟列表、无法证明来源的直接调用及retained实例替换继续走既有保守重建或比较路径。
- `AuiC2Runtime::prepared_document_nodes_copied()`是开发验收统计：每次present重置；仅在本次prepared document COW确实需要完整克隆时报告document节点数。它不进入Serde、RuntimePackage、ABI或项目协议。

## 3. 实现结果

`AuiRuntimePresenter`复用分支改为`prepared_input.take()`，局部内容变化完成后再将共享prepared input放回缓存。`AuiC2Runtime`在`Arc::make_mut`前识别并释放相同source，在变化后恢复新source；`layout_shared`先消费有界source change hint，再判断Arc身份。这样内部owner不再自行触发COW，外部观察者仍自然触发COW。

1003节点生产样本结果：

| 场景 | prepared document复制节点数 | 存储语义 |
| --- | ---: | --- |
| 缓存独占，单点Text变化 | 0 | `nodes.as_ptr()`保持 |
| 调用方持有旧document Arc，单点内容变化 | 1003 | 新旧地址分离，旧节点不变 |
| 纯translation | 0 | prepared document Arc直接共享 |

## 4. 验收与非声明

engine retained/C2/真实字体144项、Player生产AUI 12项、Editor GameView 25项，共181项去重测试通过；三crate定向Clippy退出0，指定文件diff检查通过。测试覆盖Text、Visibility、Interactable、translation、binding变化、viewport、实例替换、失败恢复、最终layout/overlay/hit和两端共用consumer。

本合同不声明命令/composition/glyph或Renderer剩余复制已消除，不声明Gate B完成，也不提供真实窗口、GPU、塔防FPS、安装、导出包或发布资格证据。
