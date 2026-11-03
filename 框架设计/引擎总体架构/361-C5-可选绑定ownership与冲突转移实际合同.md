# 361 C5：可选绑定 ownership 与冲突/转移实际合同

日期：2026-09-28。范围：Project Runtime SDK 属性批次、AUI 常驻实例与 session 事务、原生模块适配器、Generated Glue 及 Player `AuiPresentCache` 直接消费。接续[361 总施工文档](施工文档/当前/361-AUI-R2-总施工文档.md)的 Gate C；执行证据见[C5 记录](施工文档/证据/361/C5-执行记录.md)。本合同不代表绑定字段通知、真实窗口、塔防迁移或 Gate D/E 已完成。

## 1. ABI 与 wire 合同

Project Runtime ABI 函数表保持 1.6，ownership 复用既有 `ui_stage` Host 槽，不新增 Host/Module 函数指针。属性 wire 升级为 `aui-update.v5`，在普通属性与 custom 属性之外增加 `ownershipUpdates:[handle, property, owner]`；owner 仅为 `Binding` 或 `Controller`。

普通、custom 与 ownership 更新共用每批 4096 项、4 MiB wire 上限和 callback 接受边界。ownership key 在批次内必须唯一；句柄、generation、属性类型和作者绑定声明由 Host 继续验证。Controller 接管必须与同一 handle/property 的首个普通属性值同批出现；归还 Binding 时同批不得直接写该属性。重复 key、无绑定声明、过期句柄、非法组合或容量超限使整批失败关闭。

SDK 通过 typed ownership 操作写入批次，不暴露引擎指针。函数表版本未变，但 wire schema 进入 Project Runtime 合同 digest，因此旧 digest 的 1.6 项目模块仍必须重建。

## 2. 单 owner 与事务语义

每个动态 AUI 实例分别持有属性 owner，默认按作者资产的 binding 声明归 `Binding`。Binding owner 下直接 setter 返回带 handle/property 的 `BindingOwned`；Controller 必须以“接管 + 首值”一次提交，避免引擎猜测当前绑定表现。接管后普通 setter 继续使用既有通知、同值消除和失效合同。

归还 Binding 会删除该属性的 Controller retained override，并按属性类别登记保守失效；默认不伪造 Changed 通知。相同 owner 的重复转移是幂等操作，不增加 presentation revision。所有转移服从既有项目 callback 原子边界：Rejected、Panic、后续结构草稿失败或任一更新验证失败时，owner、值、布局、命中和呈现均不发布半份状态。

节点删除、模板实例回收、session shutdown 和模块卸载沿既有实例生命周期释放 owner 状态，不建立全局 binding registry、第二 snapshot owner 或最后写入者获胜规则。

## 3. Presenter 优先级与恢复

Controller owner 存在时，`AuiPresentCache` 使用 retained override，后续 `ProjectUiStateSnapshot` 变化不能覆盖该值。归还 Binding 后，下一次 present 必须解析最新 snapshot，而不是恢复接管前的旧值。

增量 Presenter 原先只枚举当前仍存在的 Controller override；override 被删除时没有值可供增量枚举，导致画面残留 Controller 文本。C5 由 `AuiC2Runtime::retained_value_removed` 显式记录这一低频删除事件，并在该帧重建 prepared binding input。普通属性变化仍走局部增量，clean frame 和无关节点不额外重建。

直接 consumer 已验证以下序列：snapshot A 显示 A；Controller 接管并显示 Controller 值；snapshot 更新为 B 时仍显示 Controller 值；归还 Binding 后显示最新 B。

## 4. 明确未完成

C5 不实现 pressed/hover 主题内部状态、动画属性覆盖层、custom 私有属性 ownership、绑定直接读取 ECS、按字段绑定通知或按需刷新。也不运行真实窗口、GPU、安装、塔防包、FPS 或 Gate D/E 验收。Gate C 尚未关闭；下一整块按正式方案第 6 节进入 C6 绑定字段通知与按需刷新。
