# 317 运行失败摘要与诊断信息施工

completed（source_integration_passed）；用户已确认建议并授权修复。316归档后激活；无新外部审查，基线复核与自审通过。2026-09-10 16:16—16:37 CST内完成，约21分钟，未触及19:16截止。

必要性与验证经济性：已确认RuntimeTrace漏规则错误，Player按runtime_advanced覆盖error，Summary仅计数无定位。最低变更为engine_runtime的LogicResult/native adapter/RuntimeTrace/报告数据结构，以及runtime_player_winit既有汇总与退出条件；CLI作为直接序列化consumer。新增字段为向后兼容补充，不增schema/tool/运行服务。复用316正常玩法证据，不运行GPU美术或全workspace/Local CI。

Gate A：先owner红灯覆盖Failed/Unsupported/Skipped、成功写入后失败；原生字段错误位置与原错误码。修复后跑相关RuntimeTrace/native adapter/project logic定向suite。
Gate B：Summary计数与前16条详情/截断/旧JSON兼容，Off无明细但失败状态可信，失败后的成功帧不洗绿；Player定向测试先红后绿。
Gate C：Player lib与runtime_cli受影响consumer回归、fmt/diff；必要时既有项目正常host run确认新产物consumer，不改安装/config，不打开或关闭用户试玩窗口。
Gate D：终态证据、入口与归档一次同步。

预算45—90分钟；任何新诊断分支15分钟不改善证据则收敛，不扩成全局日志/事务系统。命令rust目录cargo test -p engine_runtime --lib <owner-filter> --locked；cargo test -p runtime_player_winit --lib --locked；cargo test -p runtime_cli --lib --locked。证据C:/a317-1616/evidence。每项测试对应漏报、错误状态、详情有界或序列化风险，不机械重复上层已覆盖测试。

## Gate终态与证据

A passed：RuntimeTrace原先只把规则错误码放一般events，gameplay_records无失败记录；有成功writes时甚至不保留错误。新增规则失败record，Failed/Unsupported或实际errors进入失败统计，Skipped正常不计。原生字段写入保留World原错误码、实体/组件/字段位置，失败前的成功写入保留；ABI callback错误保留状态消息。修改LogicResult内部可选failure_location，不改SDK/ABI。字段位置只在错误时构造，未新增字段发现工具。

红灯：trace-red.log两项均失败（期望1/2条失败记录，实际0）；player-red.log真实行为红灯为期望logic=error但实际not_started。Player测试初次调用错误构造名headless导致编译失败，修正为既有headless_surface_gate后才取得行为红灯。第一次并行Cargo命令发生构建目录锁等待，随后保持顺序执行，不把锁等待当产品失败。

B passed：现有Summary追加最多16条failureDetails与omittedFailureCount，短字段256字节、message1024字节，UTF-8边界安全。20次失败保留前16次、遗漏4次，失败总计20；兼容旧JSON缺新增字段。Summary/Off均不保存完整gameplayTraceRecords；Off仅保持最小失败状态，不输出详情。writeCount不把失败写入计作成功。

Player三个运行出口统一使用completion_exit_code，错误状态不被runtime_advanced覆盖；present独立反映呈现事实，即使presented也可以logic=error/exit1。CLI透传新增详情，逻辑失败时exitReason=runtime_logic_failed。无新schema/tool/provider接口，无Trace过滤或全局日志系统扩建。

C passed：最终去重90项测试：RuntimeTrace2 + native adapter5 + ProjectLogic13 + Player45 + CLI25。native测试通过真实ABI callback输出先成功写Transform再失败写unsupportedColor，确认原错误码world.component.unsupported_field、实体failure-fx与字段定位；另覆盖ABI PANICKED状态消息。Player使用现有TestModule/真实headless loop运行3帧，第一帧规则失败或销毁不存在实体，后两帧继续运行，Summary/Off四种组合都保持exit1/logic=error，present仍为presented；Summary精确记录一条失败，不重复计数。CLI验证序列化字段与真实present/失败状态共存。既有成功路径全部保持通过。

命令：cargo test -p engine_runtime --lib runtime_trace::failure_tests --locked（红2/绿2）；同配置project_runtime_native_adapter::tests（最终5）、project_logic::tests（13）；cargo test -p runtime_player_winit --lib --locked（最终45）；cargo test -p runtime_cli --lib --locked（25）；cargo check -p runtime_cli --features real-window --locked。受影响rustfmt --check与git diff --check通过。
成本：Trace绿编译18.33s/测试0s，native最终7.37s/0.11s，ProjectLogic0.13s/0s，Player最终约8s/0.05s，CLI12.71s/0.08s，real-window首次check35.11s、最终增量4.38s。所有日志保留C:/a317-1616/evidence，重复测试不重复计数；后续只有影响输入发生变化的suite更新，未受影响规则语义与CLI序列化证据复用。

D passed：316继续归档且其试玩窗口未操作；317方案、入口、阶段记录同步并归档，当前/待执行为空。没有启动正式宿主run、生成新游戏交付或更新Provider/config/Editor安装；本轮声明源码与直接consumer集成完成，新构建包含修复。原dirty/untracked用户变更保留，不声明clean exact commit资格。

## 源码身份与限制

RuntimeTrace SHA256 90E17A3B3D2F0C81F5D9673C5B5276FBE3A04EF947362079EAA89B62A8B0AF98；native adapter CD0F5FD77D76382BE21A44F54ECE845A1791E114B641D58CB47BBC15E063F4F3；windowed_player报告合同65AB4BD213E9E479E5A79DFB9391652D5A884C5D7A0B2E615CAA670540B20CDB；Player A72003B2B51CB171CD61E80F7A5762208B20E15D22E541B140B85C21C9D71FCA；CLI 1F59FC9C2193D85C9E9DCC414DC76FF36751D7E05CC0D03DD78BDA8ADF8F0A79。
未运行完整workspace/Local CI/性能/GPU截图或安装验收；对应范围未改变或未获本轮资格声明。real-window为编译验证，不冒充实际窗口失败复现。Trace完整收集机制、项目playtest接入、动态实体计数与SDK字段发现均保持后续，Summary仅保留有限失败证据；不宣称227MB全量Trace生成机制已重构。
