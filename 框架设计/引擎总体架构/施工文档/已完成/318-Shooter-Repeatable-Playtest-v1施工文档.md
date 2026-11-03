# 318 Shooter Repeatable Playtest v1施工文档

状态：A/B passed，2026-09-10完成归档；用户明确授权接入并重复验收。
正式方案：../../318-Shooter-Repeatable-Playtest-v1方案.md。

## 必要性与验证经济性自审

- 请求：将316临时试玩变成项目默认playtest，并由observe读取保留证据。
- 最小变更：项目RuntimeModule观察函数、manifest、Observations、Tests和项目运行说明。
- 基线：317已归档，observe为空，manifest无场景/观察合同；312已有全部执行能力。
- 无新的外部审查；复用312已冻结合同，方案无需新增架构。用户本轮授权包括项目构建与宿主试玩。
- 排除：新工具/新schema、Provider安装/config、游戏机制扩展、性能资格化、Local CI和全workspace。
- A -> 观察值误报 -> 项目TestWorld正反例与现有玩法测试 -> cargo test项目manifest --locked。
- B -> 实际输入/场景/截图未接通 -> 当前宿主playtest及同runRef observe -> 真实报告、断言和截图。
- 复用317源码诊断回归；不重复其90项测试。新Player构建消费当前源码，旧316二进制证据不冒充新运行。
- 预计A 15分钟，B含构建30分钟，文档10分钟。异常成本先定位最小失败边界，不扩展测试矩阵。

## Gate A：项目合同与测试

实现只读世界观察值，声明合同和默认输入场景。补充能捕获错误分类、失去敌人、过期特效、
未计分等回归的项目测试。运行：
`cargo test --manifest-path samples/complex_shooter_project/RuntimeModule/Cargo.toml --locked`。
检查受影响Rust格式与JSON结构。

已完成：项目12项测试通过（原有10项+观察正反例/缺失状态2项）；格式及diff检查通过。
发现直接consumer测试依赖空observe的文本锚点，同步engine_tool_provider/tests/mcp_process_smoke.rs，
追加既有 `cargo test -p engine_tool_provider --test mcp_process_smoke --locked`，不新增测试框架。

## Gate B：当前宿主验收

调用engine_runtime_playtest({})，使用返回runRef调用engine_runtime_observe。
要求技术/玩法/必需捕获通过，零运行逻辑失败；人工读取截图确认关键画面。
失败保留报告，按首因修复后重跑。记录源码/场景/Player/交付/运行身份。

已完成：当前宿主engine-op-mcp-process-12864-11，3600 tick、37断言、3捕获全部passed，
observe按同runRef返回completed。Player logicStatus=ok/exitCode=0；默认Off未生成失败计数数组，
不把字段缺失解读为统计计数0。新Player包含317粘滞错误及退出路径；复用其90项失败注入证据。
图像人工检查：35帧命中与计分，1800帧持续射击第5波，3600帧第10波/1700分，子弹特效均已清理。

## 收尾

写项目Tests/README，阶段完成记录；同步README、49、54、施工/阶段索引。
Gate通过才移动至已完成，报告真实完成范围，不宣称完整游戏或性能通过。

终态：13项去重Rust测试、37条实际玩法断言、3捕获及同runRef observe全部通过。
MCP测试两次失败分别为旧fixture调用已删除函数、修复代码格式不符；owner级格式复现转绿后，
最终既有MCP测试180.16秒通过。未修改Provider生产实现，不重跑未受测试fixture影响的GPU证据。
完整首因、身份与保留目录见../../阶段完成记录/2026-09-10-318-Shooter-Repeatable-Playtest-v1/00-总览.md。
当前/待执行为空，不自动激活下一系统。
