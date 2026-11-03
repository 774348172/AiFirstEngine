# 361 C1：事件顺序屏障、active语义与生命周期订阅scope实际合同

日期：2026-09-27。范围：AUI常驻实例owner、typed属性wire与Player/Editor直接consumer。接续[361总施工文档](施工文档/当前/361-AUI-R2-总施工文档.md)的Gate C，不代表模板、reparent、自定义measure/paint或整个Gate C完成。执行证据见[C1记录](施工文档/证据/361/C1-执行记录.md)。

## 1. 独立状态合同

节点现在分别保存`active`、`visibility`和`interactable`：

- `active`是节点自身值；有效active为自身与祖先active的合取。父节点停用会使整个子树失活，子节点自己的active值不被覆盖。
- inactive子树不参与布局、绘制和命中；重新启用后恢复节点原有的Visible/Hidden/Collapsed和interactable值。
- Hidden仍占布局空间但不绘制/命中，Collapsed不占布局，interactable=false仍保留布局和绘制。三者不能互相代替。
- active边沿复用原`AuiLayoutSolver`、最终geometry、UiProjection和交互有效可见性清理链，没有第二布局或输入owner。

`UiProperty::Active`及`UiNodeRef::set_active`通过原`ui_stage`批次传输。`aui-update.v2`只在原有有界属性列表中增加`active:bool`；4096属性、256 KiB文字和4 MiB wire上限保持。Project Runtime ABI表、HostApi布局和ABI 1.3不变，SDK合同digest随schema正常变化，旧模块以digest不匹配失败关闭。

## 2. 提交与通知屏障

同一`AuiEdit`内仍以`handle+property`合并纯属性赋值；同值、写后回到原值均不提交、不失效、不通知。成功提交后才排入通知，失败或丢弃编辑段不会暴露暂存值。

Changed包含target、property、old/new和User/Programmatic/Binding来源。`edit_without_notify`更新属性及表现但不产生Changed，用于绑定回填等无通知路径。一个提交中的Changed按稳定property顺序排队；Active Changed之后才排有效active边沿对应的Enable/Disable，因此生命周期形成明确屏障，不递归进入监听器。

## 3. 生命周期与订阅scope

`subscribe`返回带实例身份和单调ID的token；`cancel_subscription`幂等，跨实例或已经取消的token返回false。订阅保存稳定callback ID，不保存闭包、trait object或跨DLL裸指针。

- Instance scope持续到dispose；Active scope在目标失活时暂停，恢复后继续使用原token，不重复注册。
- 生命周期订阅建立后排入该目标的一次Mount，并在当前有效active时排一次Enable。有效active边沿各排一次Disable/Enable；dispose对仍active目标先Disable再Dispose，重复dispose不重复通知。
- 同一事件按token注册顺序分发。事件开始时冻结本次最大token，分发中新订阅从下一事件起生效；取消会立即移除尚未执行的监听器。
- `begin_dispatch`在已有分发期间返回Reentrant；listener执行不通过递归dispatch绕过队列。dispose事件完成且队列清空后清除订阅，旧句柄和token都不再有效。
- 无匹配订阅时不积累Changed/lifecycle队列，普通控件没有默认逐帧Update或常驻全局消息总线。

## 4. 呈现与兼容

常驻属性向呈现文档物化时，Visibility先恢复节点自己的显隐，Active再覆盖有效停用；因此false→true不会错误变成Visible。C2文档以Collapsed表达inactive并进入原增量solver；legacy `aui-document.v2`只更新其支持的`visible`字段，不写入C2-only `layout_visibility`，避免合法旧文档被错误判成schema混用。

现有交互owner继续按最终有效可见几何清理hover、capture、drag、focus与输入编辑状态；已有“隐藏后首次清理、后续不重复”回归保持。C1不新建捕获表或事件路由器。

## 5. 明确未完成

C1的callback ID只在引擎实例owner内分发。普通Project Module尚不能跨DLL注册/执行listener，也没有跨DLL无通知setter便利接口；模块卸载与callback表清理将在C2以有界ID协议和Glue接入。模板多实例、reparent/坐标转换、结构事务、自定义measure/radial layout/paint、绑定解绑和扩展非法输出隔离仍属于后续Gate C块。

本合同不声明真实窗口、安装包、塔防迁移或FPS完成；这些仍由361的D/E验收。
