# 317 运行失败摘要与诊断信息修复

用户2026-09-10确认先修运行失败摘要与诊断信息。316失败证据：颜色写入失败而failedRecordCount=0；长跑两次重复销毁只有计数，定位需要227MB Trace。

沿现有LogicResult -> RuntimeTrace -> Player报告链修复，不新增工具、服务或报告版本。Failed/Unsupported规则结果写入失败记录，Skipped不是失败；原生字段写入保留原始错误码及实体/组件/字段位置，已完成写入不从报告中丢失。命令应用失败复用既有记录，不重复计数。

现有Summary追加failureDetails（前16条）、omittedFailureCount；详情字符串有界，包含frameIndex/phase/ruleId/operation/entityId/componentType/fieldPath/errorCode/message。Summary不保留全量成功Trace。旧JSON缺新增字段可读取。失败总数不因详情截断而丢失。

逻辑状态失败后保持error，不被后续成功帧覆盖；窗口/无窗口输出不得仅因present成功而exit0。Off不生成失败详情或长日志，但仍保持最小错误状态和非零退出；保留正常退出/截图/呈现语义。

实现口径：一次失败的规则调用计一次，保留首个错误；失败前的成功写入仍在Trace中，writeCount只统计成功写入。规则调用的ABI错误保留原消息/状态码，延迟命令复用现有command_apply错误码与实体定位。CLI失败原因明确为runtime_logic_failed，不把presented当失败原因。failureDetails的短字段最多256 UTF-8字节，message最多1024字节（含截断标记），前16条之外计入omittedFailureCount。字段写入位置属于LogicResult内部诊断，不改变SDK/ABI/schema版本。

排除本轮：Trace筛选、完整日志系统、动态实体计数、SDK字段补全、项目playtest接入、性能与安装升级。验证以真实规则/字段/命令失败经过owner到Player/CLI序列化为准，不破坏现有项目来造失败。
