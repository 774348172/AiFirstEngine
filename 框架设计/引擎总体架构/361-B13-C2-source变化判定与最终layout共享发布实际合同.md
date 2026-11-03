# 361 B13 C2 source变化判定与最终layout共享发布实际合同

日期：2026-09-26。状态：B13源码范围完成；361、B整体及C-E仍在施工。

## 1. 实际能力

生产`AuiPresentCache`现在只在同一retained实例、项目snapshot复用、viewport未变、虚拟列表交互未变且无虚拟列表物化时，向C2发布有界的内容变化slot。C2以该owner证明跳过整份`AuiDocument`相等比较、结构兼容扫描和全部solver节点同步；1003节点中的单点typed内容变化只检查1个source节点，纯translation共享同一source身份且检查0个节点。

无法证明来源的公共`AuiC2Runtime::layout(&AuiDocument)`、项目snapshot替换、viewport/theme/字体/结构变化、实例替换和外部COW输入继续走完整比较、验证和失效。优化不依赖裸地址，不新增项目手工dirty API；变化提示只存在于原调度器与C2 owner内部。

最终geometry由`AuiGeometryCache`以`Arc<AuiLayoutResult>`持有并直接发布到`AuiRuntimePresentOutput`，paragraph Map也由C2以`Arc<HashMap<...>>`发布。变更使用COW保持仍被外部持有的旧输出不可变；调度器一旦越过clean/failure复用分支并确定重建，就释放内部已过期的`last_present`，避免缓存自身迫使变化帧复制完整`computed_nodes`或paragraph Map。公共owned layout入口保留兼容语义。

## 2. 已验证边界

- 1003节点生产Player路径中，单点文字变化`source_nodes_compared == 1`，最终`computed_nodes`底层storage地址保持；绑定文字不被retained属性覆盖。
- 同一实例后续Text、Interactable、Hidden/Visible变化复用prepared binding/theme/viewport结果；旧输出document若仍被调用方持有则通过COW保持不可变。
- 纯translation检查0个source节点；Presenter输出layout与C2当前发布layout为同一`Arc`，未变paragraph Map跨帧保持同一`Arc`。
- snapshot变化、viewport/DPI变化、实例替换、字体资源COW、直接document变化、失败重试、虚拟列表、Player与Editor生产consumer回归保持。
- source比较计数只描述变化判定实际访问量，不把解析、文档COW或后续绘制工作伪装成已消失。

## 3. 未完成范围

本块没有消除无法证明来源时的完整source比较，也不处理prepared resolved document自身的COW、solver snapshot clone、overflow汇总、命令/composition/glyph组装复制或Renderer扫描/合批复制。没有新增schema、ABI/Glue、SDK、Renderer/GPU协议或项目接口。

下一完整块先量化solver snapshot借用协调与overflow汇总的真实成本，再决定是否作为同一因果链施工；不直接发明persistent vector或扩大到Renderer。安装仍为360，旧塔防包未更新；本块没有真实窗口、GPU、FPS或发布资格声明。
