# 361 B2 常驻UI到Player呈现实际合同

日期：2026-09-25。范围为一整条源码功能链：宿主接受typed属性 → Player呈现缓存失效 → 原Taffy/真实字体 → 位移、矩形裁剪、命中与原UiProjection → 控件反馈。不是361全部B、真实窗口或FPS验收。

## 所有权与生产接入

项目继续使用A1/A2的typed引用/setter，ABI保持1.3。`AuiInstance`只在成功且非空提交时推进内部presentation revision；同值、未提交和失败事务不推进。实例保留显式写过的属性槽集合，当前值仍存在原NodeState中，不累积修改历史或再复制一份文本。集合最多为当前节点数×4；dispose同时释放布局缓存与该集合。

`presentation_key()`返回实例identity与提交revision，已释放实例返回None。`EngineHostLoop::retained_ui()`只读暴露已接受的会话UI；没有开放绕过stage/finish的可变实例入口。`AuiC2Runtime::observe_retained()`独立观察，不drain布局、paint或hit队列；相同key为常数时间比较，不扫描/克隆实例节点。

Player的headless、real-window持续帧及有界运行入口均从实际HostLoop取其当前文档实例。沿用manifest首个文档的原选择规则；未新增多文档合成规则。旧snapshot的Reuse不能遮蔽typed变化；旧snapshot失败时，已有上一份snapshot仍可供独立typed变化重建，旧绑定维持上一有效值。新实例或dispose清除旧呈现属性，不以文档ID相同判定实例相同。

事件/更新回调仍遵循既有帧顺序：本帧呈现准备/命中后执行回调，成功值在下一次呈现准备消费；不是本轮实现了C的同帧事件通知屏障。代码修改尚未装入当前360 Provider或旧游戏可执行文件。

## 解析、布局与文字

原binding resolver、theme、viewport、RendererBridge保留。typed覆盖仅来自成功的显式setter，未设置的字段不覆盖绑定结果；A1对binding-owned字段的拒绝仍有效。顺序为binding → typed属性 → theme → viewport → layout → post-layout translation → draw/glyph。旧v2仍走原兼容布局；v3在原AuiC2Runtime内部改用B1的`for_instance/update_instance_node/solve_instance`，没有增加第二套布局算法或渲染器。

解析后的结构、rect、slot、layout、字体style、rich spans或viewport根约束改变时重建solver；仅文本/visibility改变时更新常驻slot。Visible/Hidden保留占位；Collapsed驱动原Taffy display。Translation/Interactable不参与基础布局；测试中纯平移measure、源装载、solver几何收集均为0。最终呈现几何组装不包含在这个“solver零工作”声明内。

ParagraphContext建立节点索引，消除原每次测量按字符串线性找节点；measure读取`AuiMeasureInput.text`，不再错误地读取静态源文本。最终段落约束来自已经计算的可见矩形，文字平移进入裁剪区后也能生成段落。字体、换行、ellipsis和rich text继续使用原paragraph cache及其既有entry/byte/item上限。

反馈重绘继续消费同一份Arc段落，不退回另一种单行shape路径。Button/InputField生成的`:label`绘制项解析回源节点，普通内嵌按钮文本和InputField占位文字均能使用最终段落。

字体缺失保留旧geometry-only接口的部分几何和diagnostics，不让固定尺寸控件突然失去几何；发生测量错误的canvas solver会被移除，下一次调用重新测量，不能把缺字体的零测量缓存当成成功clean结果。资源在一个RuntimePackage/呈现缓存生命期内保持不变，换包创建新cache；运行期热替换同一registry尚未纳入本段合同。

## 最终几何与输入

Translation在基础布局之后累加父子偏移，同步节点矩形、祖先clip形状的位置、clip矩形交集和滚动条几何。重新计算可见性，允许节点从裁剪区域外移入；布局solver的基础snapshot不被改写。绘制和命中消费同一个最终layout，原位置不能继续触发已经移动的节点。

Interactable=false仍按既有合同允许`HitNonInteractable`识别节点，不把“禁用”改成“几何不存在”。圆角clip的位置/半径传至原Renderer；本轮不宣称补齐历史命中系统的圆角精确边缘判定。

Player C2输入报告使用实际Presenter layout，移除原来仅为报告再次运行无字体旧布局的开销。旧v2滚动报告兼容行为保留。

## 有界缓存与剩余性能工作

每个Player cache持有当前实例的最新属性投影、每canvas一个solver、一个源解析文档和原有有界paragraph cache；不存历史revision。实例切换清空投影，有变化时仍会复制当前显式属性；不是逐修改日志。

本轮贯通生产消费和正确性，不把以下成本藏进“增量完成”声明：变化帧仍做解析后document比较/校验及slot遍历，canvas snapshot仍可能全量clone，最终几何、draw/glyph列表仍全量组装；控件反馈与Renderer上传尚未整体增量化。无变化Player的present cache复用不等于整帧所有模块均零扫描。

后续B3应按完整“变化传播与绘制复用”块推进：消除上述全量成本，固定clip/hit/paint依赖索引，接通反馈与Renderer片段/上传复用及Editor consumer；随后C事件扩展、D项目迁移、E真实窗口和负载验收。见[执行记录](施工文档/证据/361/B2-执行记录.md)及唯一[总施工文档](施工文档/当前/361-AUI-R2-总施工文档.md)。
