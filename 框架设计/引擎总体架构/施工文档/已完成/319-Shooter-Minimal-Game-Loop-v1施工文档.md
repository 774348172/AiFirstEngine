# 319 Shooter Minimal Game Loop v1施工文档

状态：2026-09-10 A/B passed并归档；用户本轮明确授权实现和试玩。
正式方案：../../319-Shooter-Minimal-Game-Loop-v1方案.md。

## 必要性与验证经济性自审

请求：敌机攻击、受伤、死亡、重开完整小游戏闭环。最小修改只在complex_shooter_project：
RuntimeModule、敌弹Prefab/贴图引用、输入、AUI、场景数据、观察合同、默认试玩场景/README。
既有碰撞、deferred命令、颜色写入和PanelVisible可复用，不新增引擎接口/工具。
用户已确认上轮建议，无新外部审查；318当前基线一致，项目旧改动全部保留。
排除：Core玩法、新脚本/工具、安装/config、性能和全workspace/Local CI。
Gates：A逻辑合同 -> 项目单元正反例；B真实输入和呈现 -> 当前宿主playtest/observe、截图。
复用318引擎/MCP基础证据，项目源码变化只重验项目与新Player，不重验无关安装。
预计A40分钟、B含构建30分钟、收尾10分钟；异常先定位owner，不扩大架构。起始23:35，3小时上限。

## A 项目实现与回归

补敌弹/血量/无敌期/死亡冻结/重开清理，AUI与typed观察合同。
项目测试覆盖：敌弹方向/有界冷却、同帧多弹只扣一次、无敌期间吞弹、不误伤或加分、
死亡冻结、重开实体去重/状态恢复、非死亡Enter不重置、既有射击/计分/清理回归。
命令：cargo test --manifest-path samples/complex_shooter_project/RuntimeModule/Cargo.toml --locked。
受影响Rust格式和JSON检查。

## B 当前宿主验收

engine_runtime_playtest默认场景，真实闲置挨弹 -> HP下降 -> 死亡 -> Enter重开 -> 再次射击。
根据实际报告确认测试时间点，不能放宽死亡/重开断言绕过失败；截图含战斗、结算、恢复。
engine_runtime_observe使用同runRef；技术/玩法/捕获通过，保存结果、身份和首因。
必要时打开本轮验收Player供用户试玩，不替换Provider/config。

## 收尾

写阶段记录、同步README/49/54/施工与阶段入口，所有Gate通过才归档。报告完整玩法范围与仍未测性能。

终态：16项去重项目Rust测试通过；当前宿主1680 tick、22断言、4捕获、observe全部通过。
第74 tick受伤/384 tick死亡/1200 tick重开/后续恢复射击计分；运行51.245秒、退出0，无失败重试。
截图人工读取确认受伤、结算可见与重开后正常战斗。详见319阶段完成记录的身份与边界。
当前无有效施工计划，安装/config不变，性能资格未启动。
