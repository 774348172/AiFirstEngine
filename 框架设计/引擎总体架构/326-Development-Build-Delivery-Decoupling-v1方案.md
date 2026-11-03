# 326 开发构建与交付验收解耦

目标：把 `engine_project_build` 的快速开发构建与 Player 启动验收分开。构建仍生成完整交付和可验证 delivery；普通 build 默认不启动 Player，`engine_delivery_verify` 继续承担显式进程验收。runtime_run 保持原有运行语义。

范围仅为已有 DesktopExportRequest/RunOptions 的布尔策略透传和对应诊断，不新增工具、不变工具数量、不改变 RuntimePackage 或发布交付合同。正式 release 与显式 verify 仍可启动 Player。

收尾合同：不启动验收不等于允许缺失Player；复制失败必须导致导出失败。普通build不发布未生成的运行报告引用，显式verify追加报告时避免重复。低层RunOptions/BuildRequest/DesktopExportRequest继续默认验收，只有Provider开发build显式关闭；DesktopExportRequest反序列化保持同一默认值。
