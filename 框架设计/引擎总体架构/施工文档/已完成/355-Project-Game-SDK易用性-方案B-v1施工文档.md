# 355 Project Game SDK 易用性方案 B 施工

日期：2026-09-19。依据：[正式方案](../../355-Project-Game-SDK易用性-方案B-v1方案.md)。用户已授权方案、自审、施工文档、自审、直接施工。54当前槽为空，历史254证据不占槽。

## 目标与必要性审计

目标是355源码集成；不安装、不发布、不重跑349人工验收。

| 已证实问题/需求 | 最小修改owner | 排除事项 |
|---|---|---|
| 缺可选FixedUpdate运行失败 | engine_runtime native adapter与直接回归 | 不要求所有项目空回调 |
| Rule能误调用媒体writer | project_game_sdk typed rule/context、adapter | 不重做所有回调，不取消Runtime防线 |
| 输入Down/Pressed语义混淆 | engine_input预设别名；project_runtime_sdk phase helper | 不改变旧义，不新增全局状态 |
| Click/Submit重复绑定 | engine_runtime AUI事件/mapper，editor_core模板 | 不改变底层焦点/消费，不全帧去重 |
| 示例与知识脱节 | SDK README、元数据、Cargo example、glue集成测试 | 不新建知识服务 |

涉及文件：上述crate相关src、README、example/test；project_authoring_execution generated_runtime_glue测试；必要的runtime_player_winit replay消费者测试。产品修改前保存涉及文件快照和dirty基线到`.artifacts/355/`，不覆盖已有用户工作。无新runner，使用现有Cargo target。

## Gates与最小验证

命令工作目录`G:/gameEngin/rust`，使用`--locked`，日志保存`.artifacts/355/`。测试编译时间与执行时间分开报告。

| Gate | 风险/合同 | 最便宜可红测试与终态证据 | 预计成本/复核条件 |
|---|---|---|---|
| A | 可选入口与故障终态 | 先运行新增native adapter回归确认红，再修复；`cargo test -p engine_runtime --lib project_runtime_native_adapter --locked` | 5–15分钟，冷编译超过估计先审视缓存，不扩suite |
| B | typed注册/阶段限制/输入兼容 | `cargo test -p project_game_sdk --locked`(含doc负例)；`cargo test -p project_runtime_sdk --locked`；`cargo test -p engine_input --locked`；`cargo run -p project_game_sdk --example sdk_usability --locked` | 10–25分钟，不加泛型框架 |
| C | Activate映射、焦点消费及直接consumer | `cargo test -p engine_runtime --lib aui --locked`；`cargo test -p editor_core --lib aui_activate --locked`（包含编译，替代重复check）；`cargo test -p runtime_player_winit --lib ordered_input --locked` | 10–25分钟，发现其它owner需确认因果后才扩 |
| D | generated glue真实编译、旧项目兼容与格式 | `cargo test -p project_authoring_execution --lib generated_runtime_glue --locked`；受影响文件rustfmt检查；现有neon/shooter项目模块编译(若可在既有配置复用) | 10–25分钟，不升级为DLL发布或安装 |

复用：353已证明有序回放生产链路，不重跑窗口/GPU/声音；354安装身份不变，保持其现有证据。新增SDK/Runtime/AUI行为不得套用旧二进制证据。涉及输入消费的当前源码测试提供本次增量证明。

总代价估计45–90分钟；超过分支估计检查最小必要路径；遵守施工技能连续三小时上限，2h45停止新实现/验证并保存状态。

## 施工顺序

1. 自审本文件、激活54，保存基线。
2. A红测试/修复/绿；B受限接口与输入及示例；C激活与consumer；D集成/兼容/格式。
3. 检查改动范围，保留失败首因，不削弱断言；证据未齐不归档。
4. 完成后写阶段记录、同步49/54/地图/阅读顺序/施工及完成索引，将本文移动已完成。源码与安装状态分开。

## 施工文档自审

2026-09-19 自审通过。复核方案五部分均有owner与验收，无新增架构边界。A使用native adapter直接测试，B负例用rustdoc compile_fail而非新增编译runner，C复用353 `ordered_input`过滤与AUI既有消费测试，D复用已有glue真实编译测试。无外部审查待处理。源代码工作区已有大量历史dirty内容，本次按文件前置快照检查增量，不清理、不回滚既有改动。若编译发现直接consumer穷尽match，仅做对应适配。

## 执行记录

已激活，开始Gate A；测试日志与文件前置快照保存`.artifacts/355/`。

A：新增回归先红(Faulted != NoOp)，修复后native adapter 10测试通过。B：SDK/runtime SDK/input共57单测+3编译负例通过，可执行样例通过。C首轮80通过/1失败：禁用但仍持焦点按钮收到Submit。已回填方案，限定在既有Submit生成处修复可交互性检查，再跑定向测试及C终态suite，不扩其它UI系统。

C：定向2测试通过后补手柄覆盖，AUI终态81测试通过。直接消费者发现editor_core service事件解码与GameView可点击目标快照只认旧事件；最小适配Activate，并新增经现有RuntimePackage打包/加载后的目标发现与混合绑定去重验收。editor_core定向测试替代check，不重复编译验证。

C终态：editor消费者1项、353 ordered_input 9项通过。
D终态：glue 7项（实际独立编译与typed ABI调用）、原霓虹实验室29项、原复杂打飞机21项通过；11文件格式检查通过。最终强化可选入口测试再次通过，不重复计数。源码去重218项加可执行SDK示例通过；完整日志/耗时/增量源码SHA256见`.artifacts/355/summary.json`。

最终自审通过，A–D完成。输入旧义、旧handler、UI低层事件保持，已修禁用Submit首因；无待解决失败。只声明源码集成，354安装未变。阶段记录：`阶段完成记录/2026-09-19-355-SDK-Usability-B/00-总览.md`；归档后无当前施工。
