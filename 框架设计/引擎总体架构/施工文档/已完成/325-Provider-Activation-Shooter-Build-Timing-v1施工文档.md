# 325 修复版Provider安装与完整打飞机构建计时施工

2026-09-11 17:03激活；18:04完成归档。用户已授权外部安装切换和完整项目构建。方案见../../325-Provider-Activation-Shooter-Build-Timing-v1方案.md。

进度：Gate A-D全部通过；新版宿主真实首次70253ms、重复16025ms，完整交付与typed inspect/build通过。细节见../../阶段完成记录/2026-09-11-325-Provider-Activation-Shooter-Build-Timing-v1/00-总览.md。

必要性：324源码尚未进入安装Provider，当前PID16460运行旧版。最小范围为编译engine_tool_provider、安装副本替换、完整complex_shooter_project首次/重复构建与交付复验。保留配置和旧二进制；不改玩法或SDK，不跑Editor/Local CI。324两个源码哈希未变，复用其9项owner回归。

Gate A：cargo build -p engine_tool_provider --bin ai_engine_tool_provider_mcp --locked；候选stdio initialize/list/inspect证明9项与正确项目，二进制SHA256封存。编译超过10分钟复核。

Gate B：完整项目windows-dev首次和重复构建，frameLimit=1，结构化report计时与cache统计；一轮现有语义playtest验收。记录实际profile，不能将dev性能称为优化版。

Gate C：保留旧安装文件并替换为已验证候选，文件哈希对齐，配置哈希不变；确认旧Provider空闲后重连，typed inspect操作PID与新文件身份绑定。无公开重连能力时不反复杀进程；记为installed_ready/host_reload_pending并给用户重连步骤，保留已完成证据。

Gate D：阶段记录及49/54/README同步；只有宿主激活与计时完成才能归档完成。约20分钟预计；三小时硬上限。自审通过：同一已授权任务，不新增额外批准门，不扩展全局安装或测试范围。
