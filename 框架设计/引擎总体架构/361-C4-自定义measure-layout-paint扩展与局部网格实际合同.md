# 361 C4：自定义 measure/layout/paint 扩展与局部网格实际合同

日期：2026-09-28。范围：Project Runtime ABI/SDK、Project Game SDK、Generated Glue、AUI常驻实例/布局/绘制、原生模块适配器及直接Player/Editor consumer。接续[361总施工文档](施工文档/当前/361-AUI-R2-总施工文档.md)的Gate C，不代表绑定ownership、复杂自定义hit、塔防项目迁移或Gate D/E完成。执行证据见[C4记录](施工文档/证据/361/C4-执行记录.md)。

## 1. ABI、注册与wire合同

Project Runtime ABI由1.5升级为1.6。Module函数表新增单一`ui_extension`批处理回调，Host函数表新增单一`ui_invalidate`入口；旧1.5模块、短表和digest不一致均失败关闭。`aui-extension.v1`统一Describe/Measure/Layout/Paint四阶段，不暴露Rust trait、Taffy节点或GPU对象。

每批最多256节点，请求与响应各最多1 MiB；每类型最多128项typed属性，标识符最多128字节；每容器最多4096个直接子矩形，每节点最多4096顶点和12288索引。非Describe请求携带注册所得非零`callback_id`；引擎与Project Game SDK共同核对type、schema、callback和属性名称/类型，浮点与颜色必须有限。响应ID、阶段、容量、有限值及mesh索引在整批安装前验证。`aui-update.v4`在原有有界属性批次中增加`custom_updates`；项目通过`UiWriteBuffer::set_custom_property`写typed私有属性，同一callback内按handle+name合并，同值写不增加revision或跨DLL工作。

Project Game SDK以`ProjectGameDefinition::ui_extension`注册稳定type/schema/callback和handler，拒绝重复type、重复callback、重复属性、零callback及无失效类别属性。Generated Glue只读分发扩展批次，不创建`GameCallContext`，扩展callback得到空Host context，不能写UI、改树、重入扩展或推进玩法。

## 2. 常驻状态、失效与失败语义

Custom作者资产只保存type、schema和typed属性；Measure、直接子项local rect及Paint mesh属于`AuiInstance`运行时缓存，不写回源资产。初始挂载先Describe全部Custom节点，缺失type、schema不兼容或属性不匹配会定位到document/node/type并拒绝挂载。

`AuiInstance`维护独立有界`custom_dirty`集合；清洁present cache命中直接返回，不构造扩展frame、不扫描全部Custom节点也不跨DLL调用。`request_measure`合并为Measure/Layout/Paint，`request_repaint`只进入Paint；descriptor为每项typed属性声明Measure/Layout/Paint/Hit失效，纯重绘不会重新measure/layout。

Measure/Layout/Paint分别保存`properties + stage input + resource identity`。相同payload但identity变化只更新identity并清dirty，不增加presentation revision；callback失败时，仅当旧输出身份仍匹配当前输入才允许last-good，否则立即清除旧阶段输出。响应缺失、重复、阶段错误、非有限值、非法直接子项或越界mesh索引时整批不发布半份结果。

扩展输出与descriptor registry归项目runtime session所有。session drop先清空registry并shutdown AUI session，实例、dirty、运行时输出和订阅随之释放；Presenter只复制已接受的运行输出，不持有DLL callback地址。

## 3. 布局与真实绘制链

Custom measure作为既有`AuiLayoutSolver`的叶子固有尺寸进入Taffy。Custom layout在Taffy先产生最终容器矩形后执行，收到该容器的最终size及直接children的实际measured size；返回值只覆盖已验证的直接children local rect，最终geometry继续供clip、hit和draw共同消费，未建立第二布局器。约束、最终容器或child尺寸改变才重新调用对应阶段；相同presented frame第二次为零callback。

Custom paint输出版本化局部顶点、UV、线性色、索引和可选已打包资源身份。局部mesh通过`AuiDrawCommand::DrawMesh`进入既有overlay/UiProjection、`AuiRendererBridge`和`runtime_renderer`；Rect/Image/Text图元分别进入既有`DrawRect/DrawImage/DrawText`路径，没有第二Renderer。圆形进度样本仍以真实索引三角形进入Renderer vertices，不以多个矩形冒充网格。

`AuiPresentCache`在Player headless、Player windowed与Editor GameView入口执行单根最多8轮稳定化。前7轮允许发布合法变化，第8轮以`allow_publish=false`探测；若仍变化则不安装新输出，保留最后已发布frame并产生`aui_extension.stabilization_limit_exceeded`结构化partial诊断。

径向布局样本验证直接子项最终位置；两个动态模板实例同时持有不同custom measure输出并保持handle、runtime缓存与生命周期隔离。报告保存Describe/Measure/Layout/Paint节点数、跨DLL调用数及请求/响应字节数。

## 4. 明确未完成

C4不实现绑定ownership/解绑转移、复杂自定义hit callback、10k虚拟列表项目验收、塔防UIBattle迁移、真实窗口、GPU截图、FPS、安装或生产二进制替换。Gate C尚未关闭；下一整块按361完整方案冻结可选绑定ownership与冲突/转移语义，不进入Gate D/E。
