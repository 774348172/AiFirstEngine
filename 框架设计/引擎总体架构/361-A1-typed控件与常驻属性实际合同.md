# 361 A1：typed控件与常驻属性实际合同

日期：2026-09-24。范围：361 R2首段源码基础，不代表生产Player已接入。

## 实际接口

`project_runtime_sdk::aui`新增UiNodeHandle（instance:u64、slot:u32、generation:u32）、UiNodeRef、UiTextRef、UiButtonRef、UiRead/UiWrite与紧凑UiError。它们是模块内Rust接口，不是C ABI布局，也不作为当前wire协议传输。此阶段没有升级ABI版本或修改已生成Glue。

`engine_runtime::aui_instance::AuiInstance`持有不可变共享源资产、加载后属性表、节点ID索引和去重失效队列。mount时执行结构校验；旧v1/v2无C2字段的文档仅在校验副本中适配v3树校验，不回写schema；旧schema携带C2字段仍拒绝。每实例分配不复用的身份；A1节点generation为1，不提供删除/复用接口。dispose使全部句柄失效。

真实Rust用法（不需要直接依赖Renderer）：

```rust
use project_runtime_sdk::aui::UiTextRef;

let text = UiTextRef::try_new(&instance, instance.node("gold")?)?;
let mut edit = instance.edit();
text.set_text(&mut edit, "100")?;
assert_eq!(text.text(&edit)?, "100");
edit.commit()?;
```

示例中的instance/edit由引擎owner创建；A2将用SDK/Glue适配到项目回调安全提交边界。普通项目最终不必自己管理引擎实例或调用commit。当前这里只证明同一接口可跨SDK与engine_runtime源码consumer工作，没有宣称真实DLL已接通。

## 属性和失效

| 属性 | 可用节点/初值 | 登记失效 |
| --- | --- | --- |
| Text | Text节点，源text或空串 | measure/layout/paint |
| Translation | 任意节点，初值[0,0]，区别于源布局offset | transform/hit |
| Interactable | 任意节点，源interactable | style/paint/hit |
| Visibility | 任意节点，按源layout_visibility与visible初始化 | paint/hit；进入或退出Collapsed再登记layout |

这些是属性层失效原因；B阶段consumer负责依赖传播、几何命中同步和最终绘制，不可直接拿该表声称后端更新已完成。active、结构变更、控件值Changed和自定义绘制尚未实现。

## 提交与容量

- edit是独占可变借用，stage时检查实例/slot/generation、属性类型、binding所有权和有限数值，因此提交期间不能并发改变这些条件。
- BTreeMap只存已触及且偏离已提交值的属性；恢复原值移除条目。读回优先读暂存值；只克隆所读属性，不克隆整树。
- 同一静默属性段合并重复赋值，不跨事件/结构屏障合并；A1还没有后两类操作。
- 任一write失败即清空该段、保留首个错误，后续读写及commit返回该错误。丢弃edit也不提交；失败不能留下半份引擎状态。
- 每段最多4096个不同属性，最多256 KiB暂存UTF-8文本；替换或撤回属性释放段内预算。上限适用于本段，不代表整个UI缓存容量。
- 每个变更节点只入队一次；drain_invalidations复用owner队列和调用方Vec，只访问已排队节点。清洁drain无节点遍历；不涉及GPU提交成本。
- Text/visibility/interactable对应旧binding被标为绑定owner，直接setter拒绝。RectOffset绑定不占post-layout Translation。绑定执行和解绑操作留在后续阶段。

## 尚未完成的生产链

A2必须接通host/module ABI、生成引用、自动回调提交及真实模块consumer；B接入布局、文字、命中与UiProjection；C完成事件/生命周期/扩展；D/E完成真实项目与性能。现有Player/Editor仍走旧路径，不能拿A1测试解释为塔防已经提速。

验证见[执行记录](施工文档/证据/361/A1-执行记录.md)，总任务见[当前施工文档](施工文档/当前/361-AUI-R2-总施工文档.md)。
