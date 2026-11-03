# 336 Engine Runtime DLL 可见试玩与语义执行最小接入 v1

状态：用户于 2026-09-13 确认最小方案，文档自审和最小实现已完成；源码/交付及安装独立预检通过，当前对话连接重载单列 deferred。见[完成记录](阶段完成记录/2026-09-13-336-Engine-DLL-Visible-Semantic-Execution-v1/00-总览.md)。

## 1. 目标与已确认缺口

让同一正式 Windows dev 交付包的普通窗口运行和 Semantic playtest 都由 Engine Runtime DLL 执行，使用原样 complex_shooter_project 的正式场景证明输入、玩法、截图和交付闭环。

335 已完成交付一致性，证据限于改为 test.frame 的 headless 夹具。当前 runtime_cli 的 packaged 分支忽略 mode，固定调用 `aife_engine_runtime_run_headless_v1`；Semantic worker 则提前分发，直接调用 EXE 内的 runtime_player_winit。Engine DLL 尚未启用 real-window。因此 335 通过不代表 DLL 窗口/语义执行已接入。

权威依据：架构 v1.1 第 5.9/5.10/5.11、G4/G5；327-R3 稳定 DLL 及 335 固定交付布局继续有效。240 旧队列已关闭，无竞争中的新系统。

## 2. 最小实现

1. 在现有 engine_runtime_host cdylib 增加一个版本化、有大小上限的执行入口，接收 C ABI 的字节指针/长度，内容复用已有 NativePlayerWindowRunRequest 和可选 PlaytestScenario。普通 run 与 semantic 仅为该入口内的有限分支，不支持任意 operation/tool 路由。报告/截图目录沿用既有请求和 owner。
2. Host 在调用期间持有输入字节；DLL 解析并拥有内部 Rust 对象，只通过 repr(C) 状态/帧数返回，报告仍用已有 JSON 文件。ABI 不传 Rust trait、Vec、Arc 或分配器所有权。不新增 crate 或外部工具 schema；共享传输定义放在既有双方依赖的运行 crate 内。
3. 保留旧 headless ABI 供兼容；新入口增加明确能力/版本检查。旧 DLL、不支持的请求、坏载荷、缺失/错误项目模块、执行失败或报告失败必须明确失败。不得在窗口请求下静默改为 headless，不得在正式包 semantic 失败后回退静态 Runtime。
4. DLL 启用 real-window，内部复用现有普通窗口/无窗口与语义窗口/无窗口函数。DLL 自己加载 staged Project RuntimeModule，并在实际报告里记录 execution owner。输入、帧预算、GameView target、capture、Summary/Trace 和性能采样参数不得丢失。超时、子进程生命周期继续由既有 bounded process owner 管理。
5. runtime_cli 的正式包普通入口和 Semantic worker 共同调用 DLL。非交付的 in-process owner 测试与已有 release 合同保持原语义。沿用 335 的 manifest、DLL/payload 摘要和 DeliveryRef；不重新实现交付校验。

实现复核收敛：正式包的未知/缺失 schema 直接失败，既有 release 格式保持原分流；正式 dev 包拒绝运行外部 payload。报告核对 schema、package、mode、状态、帧数及实际 owner。项目产物缓存命中时也检查 Engine DLL 新入口能力，不兼容沿原 invalidated/rebuild 分支处理，不改变缓存键或增加缓存层。正式窗口能力证据只覆盖本轮默认 real-window 构建，不把 no-default-features 视作窗口产物。

施工检查补充：`native_window_report` 原来为生成报告再次调用静态 `WindowedPlayerHost::run_headless_gate`，会实际重复执行。改为既有空报告构造器再映射 DLL 返回字段；只增加该构造器的可见性，不新增报告 schema，未由 Native report 提供的层不伪造成功。

## 3. 原样项目与宿主验收

复用项目 manifest 指向的 `Tests/game-loop.scenario.json`、`Observations/project.observations.json`：1800 tick、16 条断言、12 个输入、4 次捕获，含射击、受伤、死亡、tick1500 Enter 重开及清理。Tests/README 旧 22 条说明已过时，以实际 JSON 为准。保留原项目源字节，禁止用 test.frame 或放宽断言冒充完整项目。

使用既有 Compiler/Provider 入口在本轮独占目录构建交付、显式 windowed verify、playtest 和 observe。先保存初次成功交付、源码身份与报告，再做耗时测量的副本修改。检查普通窗口模式、输入/截图及 DLL execution owner，核对 16 断言/4 捕获证据。

通过后沿用既有 Provider 安装/激活机制更新宿主使用的版本，保留旧安装可恢复。用户本轮确认包含此前建议的更新与耗时复测；无需再次询问普通修复或安装执行许可。先准备并验证新产物再切换，记录实际安装与当前宿主观察分别成立到哪一层；无法自动完成客户端重连时不声称当前连接已升级，不为重连新建 daemon/connector。

构建测量三种情况：本轮首次项目构建、同源无修改重复、项目副本源码小改后的构建；分别记录 Engine DLL 构建、项目/Host 编译、准备、导出和验收成本。已有 SDK/依赖缓存必须如实标注，不能把测试 suite 时间或暖缓存当全冷耗时；没有等条件旧基线则只报告新绝对数和复用行为，不编造收益。

## 4. 外部参考与非目标

延续 327-R3/335 已研究的 Unity Player/DLL 分工和 Godot 模板运行原则。本轮采纳的是运行 owner 与项目模块分离：输入与显示适配归运行层，项目只提供玩法。Godot 本地固定 tag 源码 `main/main.cpp`、`platform/windows/godot_windows.cpp` 中 setup/start/MainLoop/DisplayServer 的组合可作为既有运行实现复用的参考；不把 Godot 的静态链接方式当成本项目 DLL ABI 模板。Unity 6 Windows Player 文档只证明 EXE/UnityPlayer.dll/Data 职责，不推断其未公开内部 API。

不增加九项 AI 工具，不新建通用 RPC、Runner、渲染器、输入系统、缓存架构、热更新、整包事务或项目玩法。不在本轮删除全部 Host 编译类型依赖，不承诺尚未测量的首次构建收益。Android、Editor、全 workspace、Local CI 和六引擎 benchmark 不在范围。

## 5. 正式方案自审

2026-09-13 主审通过：用户已选定，无需重新讨论。最小失败反事实是 mode/semantic 请求进入真实 DLL owner；现有四个执行函数可复用，无需新深模块。传输只在已有 C ABI 边界，Runtime 对象在 DLL 内创建销毁。原样项目是本轮验收必要范围，独占副本及现有临时产物机制保证可恢复。335 证据和未改变的 ABI 负向可复用，只更新变动入口和真实窗口所需证据。未发现适用于 336 的外部审查文件；施工前独立复核若发现实现细节错误，直接收敛文档和代码，不扩架构。
