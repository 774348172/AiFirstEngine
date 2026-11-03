# 346 新版 Provider 安装与宿主接入

2026-09-16；用户已选择新版工具链接入、授权安装并重连。本项承接该授权，不重开344/345设计。

## 目标与自审

修正上轮把旧 Provider 当成最新版安装的问题，使实际 Provider 包含345字体源码，并完成安装与真实宿主工具验收。已读取项目与施工技能、当前入口；无新增外部审查文档，无需修改正式方案。继承既有独立九工具、Windows dev、旁路安装和配置备份机制。

必要性与验证经济性：旧文件构建时间2026-09-15 11:38，font_cook源码2026-09-16 15:06，构建身份不合格。最小改动是重建Provider、独立资格化、替换config中唯一command及同步文档。涉及rust现有依赖构建产物、用户config、Provider安装目录和本轮.artifacts/346；不修改引擎架构、项目玩法、美术、字体源码或性能系统。不运行全workspace/Local CI；不宣称干净commit发布。

## Gates

- A：记录旧进程/二进制/config身份；`cargo build -p engine_tool_provider --bin ai_engine_tool_provider_mcp --locked`。构建日志、源码依赖清单和candidate SHA证明不是复用旧文件。预计1–10分钟；编译失败先定位owner。
- B：用新candidate独立MCP执行initialize/list/inspect/check/build/playtest/observe/verify；项目使用laser_house_project以覆盖345字体生产路径。复用344玩法场景，verify窗口120帧。保存完整请求/响应、FontFace产物及Host/DLL身份。预计5–15分钟；上游失败停止后续。独立进程证据不冒充宿主调用。
- C：备份config，仅切换command到已校验新文件，args保持complex_shooter_project；逐字读回、SHA验证与异常回滚。当前旧宿主保留。真实宿主须PID/路径/SHA一致，再经typed inspect/check/build/playtest/observe/verify验收。若无reload接口，先完成安装，明确等待用户重连，不能归档为host_activated。

复用345 Gate D与344终态源码/运行证据，只要其消费的源码/字库/Host/DLL身份不变就不重跑内部127/27测试及人工验收。新Provider导出或宿主身份不同的证据单独建立。连续施工不超过3小时，2小时45分不启新验证。

## 同步与归档

49保存当前真实安装身份，54保留本项唯一执行槽；完成全部Gate后写阶段完成记录，更新施工/阶段README并移至已完成。上次BF5安装预检仅证明旧candidate可启动，不证明345集成；旧证据保留。

## 2026-09-16 19:30 执行结果

- Gate A passed：Provider dev构建1分29秒，SHA-256 `D939C67C0226809369A16777EB8FDD7B6ABB6F5AF748642CA5179AC783D2A277`。175项输入路径清单及关联Cargo manifest已保存；345的16项引擎源码/依赖摘要不变，复用内部回归。未修改产品源码，工作区构建不声明exact-commit发布。
- Gate B passed：独立进程PID19380，9工具；inspect/check/build/playtest/observe/verify均completed。check10.238秒、build433.937秒、playtest37.649秒、observe0.983秒、verify9.186秒。试玩1020 tick、18断言、16捕获，120帧窗口验证passed；实际查看普通及暂停截图。fonts目录10文件、cooked font asset、Host和Engine DLL与344/345验收身份一致，包含digest保护的face-0.bin。完整证据见`.artifacts/346/laser-candidate`，不是宿主typed验收。
- Gate C安装部分passed：新安装目录`C:/Users/zenghaoran/AppData/Local/AiFirstGameEngine/Provider/20260916-346`，直接从安装文件启动预检9工具及complex_shooter_project inspect通过，进程已退出。config逐字备份并只改command，args保持，读回通过；旧文件及PID15424保留。首次安装脚本因正/反斜杠不匹配在任何写入前安全停止，修正精确匹配后通过，无失败配置写入或回滚需求。
- 当时终态`installed_candidate_qualified_host_reconnect_required`：没有可调用的宿主热重载接口，当时真实连接仍为旧PID15424。该等待状态已由下方9月17日真实宿主验收关闭。
- 证据总览：`G:/gameEngin/.artifacts/346/summary.json`；安装事务`installation.json`；源码复用`reused-345-sources.json`；字体交付对比`delivery-font-comparison.json`；旧宿主真实调用`host-old-inspect.json`。本轮约16分钟，测试与预检进程正常退出，旧宿主有意保留，无递归cleanup。

## 2026-09-17 终态

Gate C passed，`host_activated`：PID2516真实typed六步验收全部完成，路径/SHA匹配D939安装版。打飞机1800 tick、16断言、4捕获，同runRef observe和120帧窗口verify通过，截图已检查。config引擎块与安装记录一致，其它会话间配置变化未覆盖。本轮约6分钟，无源码/config修改，无验收失败或重跑；进程正常收尾，Provider继续服务。Gate A/B复用已验收证据。

证据：`G:/gameEngin/.artifacts/346/host-activation/summary.json`；阶段记录`阶段完成记录/2026-09-17-346-Provider-Host-Activation/00-总览.md`。A/B/C闭合，施工文档归档，当前槽清空。
