# 361 C2：跨DLL事件callback与卸载清理实际合同

日期：2026-09-27。范围：Project Runtime ABI/SDK、Project Game SDK、Generated Glue、AUI常驻会话及原生模块适配器。接续[361总施工文档](施工文档/当前/361-AUI-R2-总施工文档.md)的Gate C，不代表模板、reparent、自定义measure/paint或整个Gate C完成。执行证据见[C2记录](施工文档/证据/361/C2-执行记录.md)。

## 1. ABI与callback ID合同

Project Runtime ABI由1.3升级为1.4。Host函数表增加单一`ui_subscribe`和`ui_cancel`入口，Module函数表增加单一`ui_event`入口，并以`PROJECT_RUNTIME_CAP_UI_EVENTS`声明能力。函数表短表、能力位与函数指针不一致、ABI minor不一致或合同digest不一致均失败关闭。

跨DLL只传版本化序列化值：`UiCallbackId`、`UiNotification`、filter、scope、token和节点handle。项目通过`ProjectGameDefinition::ui_event(id, handler)`注册稳定非零ID；零ID、重复ID和未知ID拒绝。Host不保存项目Rust闭包、trait object或函数地址，Generated Glue只按callback ID查找项目handler并在调用期创建`GameCallContext`。

这是函数表布局变化，不提供1.3二进制兼容层。所有旧1.3项目模块必须由当前Compiler/Generated Glue重建为1.4后才能加载；源码接口之外的旧DLL会被明确拒绝。

## 2. 订阅事务与事件分发

`ui_subscribe`和`ui_cancel`都属于当前项目回调的暂存段。只有项目输出为Applied、没有rejected action且Host UI段没有失败时，属性、订阅和取消才一起提交；Rejected、Faulted、panic、无效handle、容量超限或宿主拒绝均不发布暂存订阅/取消。

订阅token由对应实例预留，整个会话最多4096个订阅。Accepted后沿用C1稳定注册顺序和Instance/Active scope；取消立即阻止后续listener，旧token、跨实例token和过期handle不能命中。事件按通知逐个调用模块`ui_event`，每个Host stage最多4096次callback，重入或预算超限失败关闭。事件handler的项目输出与普通session输出合并；普通可选阶段callback缺席时直接返回事件输出，不把已经执行的项目mutation/status降为NoOp。handler内UI写入仍经过独立的接受/拒绝边界。

## 3. 无通知属性写

属性wire升级为`aui-update.v3`，每个`UiPropertyUpdate`携带`notify: bool`。普通setter与`*_without_notify`便利入口共享同一个稀疏buffer、handle/property合并、4096属性上限、256 KiB文字预算、4 MiB wire上限以及一次`ui_stage`提交。

同一属性最后一次setter同时决定最终值和notify策略。无通知写仍更新常驻值、布局/绘制失效和后续读取，只是不生成Changed；因此项目可以在事件handler中执行绑定回填而不会形成Changed反馈循环。

## 4. session与模块卸载顺序

`AuiSession::shutdown`先停止当前dispatch，丢弃pending属性和pending订阅操作，再清空订阅、dispose实例并释放document/instance表。`AuiSession::Drop`重复执行同一幂等清理；`LoadedProjectRuntimeSession::Drop`显式先调用shutdown，之后字段析构才释放session lease和DLL lifetime guard。

清理之后实例、通知、订阅和暂存操作均为零，旧token与callback ID没有可分发owner。Host只在模块仍有有效lease且处于调用期Host context时调用`ui_event`，不会把项目代码地址保存在AUI实例中。

## 5. 明确未完成

C2不实现模板多实例、reparent、坐标转换、结构事务、绑定解绑、自定义measure/layout/paint或圆形控件。下一整块应冻结模板实例身份、结构变更事务和reparent前后坐标语义，再进入项目自定义布局/绘制扩展。

本合同不声明真实窗口、GPU、安装、塔防迁移、FPS或整帧性能完成。安装仍为360，既有塔防交付仍不包含ABI 1.4源码。
