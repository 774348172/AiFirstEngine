# AI First Game Engine 权威架构设计 v1 自审报告

> 审查对象：`00-AI-First-Game-Engine-权威架构设计-v1.md`。
> 审查日期：2026-08-31。
> 最近复审：2026-09-02；对象仍为v1，完成Engine workflow与默认工具面收敛复审。
> 审查性质：正式顶层架构自审，不是施工文档，不授权代码施工。
> 结论：通过顶层 authority 资格；不具备全量代码施工资格。

## 1. 输入完整性

| 输入 | 结论 |
| --- | --- |
| `00-AI-First-Game-Engine-权威产品需求-v1.md` | 已确认作为最高级产品需求 |
| Harness-Native Game Project Compiler研究方案v0.3 | 已吸收总体Module、No-Editor、SDK/Compiler/Outcome和迁移方向 |
| v0.3自审报告 | 已吸收普通文件revision、lease/drift、Host native/MCP边界、平台路线等残留问题 |
| 253-259既有正式方案 | 已做保留、降级和替代分类 |
| 264/292/RuntimePackage/Build等基础 | 已明确作为后续子设计实现基础，不重复发明 |

## 2. 需求覆盖

| 需求主题 | 架构落点 | 结果 |
| --- | --- | --- |
| 引擎适配AI | AD-001、AD-002、Engine Tool Provider | 通过 |
| 同一个Agent Tool Loop | AD-001、AD-002、Host Adapter | 通过 |
| Provider不是第二个Agent | AD-001、5.2 | 通过 |
| No-Editor默认路径 | AD-003、3、G1 | 通过 |
| Editor只作验证 | AD-004、5.13 | 通过 |
| AI直接写真实游戏 | AD-005、7 | 通过 |
| Native Runtime和移动交付 | AD-006、5.9、5.11 | 通过 |
| 游戏质量优于单纯Web | AD-007、5.10、12、G6 | 顶层覆盖，效果待benchmark证明 |
| 安全、审计和回滚 | 5.3、7、13 | 通过 |
| 单一工程真相 | 5.4、6、G3 | 通过 |
| 不重写Godot | 19 | 通过 |

## 3. 深Module审查

### 3.1 Engine Tool Provider

Interface只暴露具体typed tools，隐藏Host projection、approval translation、Kernel invocation和result mapping。删除Module后复杂度会散落到多个Host Adapter，具备Depth和Locality。

结论：通过。

### 3.2 AuthoringProjectContext

HeadlessProjectProvider与EditorProjectAdapter构成两个真实Adapter，seam不是为测试虚构。Module集中identity、revision、snapshot、refresh、mutation和drift，避免双authority。

结论：通过顶层设计；具体lease、并发和文件refresh语义必须进入正式子设计。

### 3.3 Project Game SDK

定位为项目侧稳定编程Interface，不复制脚本语言；生成胶水隐藏ABI、descriptor和registration样板。

结论：通过顶层设计；最小符号集和跨Player/Android兼容仍需子设计。

### 3.4 Game Project Compiler

Interface收敛为inspect/check/prepare，隐藏增量Rust、asset cook、RuntimePackage、cache和diagnostics；不拥有AI计划和玩法判断。

结论：通过顶层设计；prepare与mutation commit的lease/drift必须在子设计中冻结。

### 3.5 Semantic Outcome

将run/observe/playtest/capture/replay集中为结果闭环，区分Technical、Gameplay、Visual和Delivery Outcome，不把日志或截图单独当成完成证明。

结论：通过顶层设计；query schema、预算、source mapping和privacy仍需子设计。

## 4. 关键冲突检查

| 冲突 | 处理 |
| --- | --- |
| 旧01默认自然语言+可视化Editor | 旧01已移入历史；新架构固定No-Editor默认路径 |
| 旧02强线性Feature Spec/Patch Plan | 旧02已移入历史；治理按风险选择 |
| 254 Editor-hosted Gateway | 降为可选Host Adapter内部实现 |
| 255 `catalog/execute` | 允许Kernel内部保留；禁止AI可见二次调用 |
| 256 Editor instance identity | 只保留给EditorProjectAdapter |
| 259 Editor/Gateway mutation binding | 经验保留，authority迁移到AuthoringProjectContext |
| 292 SDK以Editor首次打开为主要目标 | 作为基础保留，后续扩展Headless/Player/Android一致性 |
| 264紧凑标量Observation | 作为source保留，不等于完整Semantic Outcome |

结论：没有发现仍由两个顶层文档同时拥有同一authority的设计意图。

## 5. 文档迁移审查

- 权威产品需求状态已改为用户确认；
- 新权威架构已进入文档地图、阅读顺序、README和当前状态入口；
- 旧01/02已物理移入`历史文档/`并增加历史状态；
- 10/20/252/254/256/259已增加新authority定位；
- 240旧优先级队列已标记关闭，不再选择新Headless架构施工项；
- 54明确新authority切换不授权引擎施工；
- 既有Tower P1-2项目施工未被扩大或修改。

结论：通过。

## 6. 初次审查时尚未解决的问题

> 本节是2026-08-31初次审查快照。后续309-A至309-F和310完成状态以当前入口、完成记录及第8节复审为准，不得用本节旧状态覆盖后续事实。

1. Host-native Tool Provider的真实Codex/OpenCode注册Interface；
2. MCP compatibility与native tool call的精确分界；
3. Host approval与Engine Grant的一次性用户决策映射；
4. AuthoringProjectContext的进程owner、lease、lock和crash recovery；
5. 普通文件写入到canonical revision的原子refresh合同；
6. Project Game SDK最小符号集、版本和generated glue；
7. Compiler增量依赖图、cache identity和source diagnostics；
8. Observation query、input timeline、replay和privacy；
9. Windows/Android/未来移动平台各自的No-Editor Gate；
10. 六路径benchmark的固定模型、预算、素材和人工审核协议。

这些问题已经被放入后续正式子设计和资格Gate，不影响顶层架构生效，但阻止直接生成全量施工。

## 7. 初次审查时的施工资格判断

```text
顶层权威架构：qualified
旧顶层authority历史化：qualified
正式子设计：not_created
施工文档：not_created
引擎代码迁移授权：none
production/config/external state授权：none
```

以上是2026-08-31初次审查结论，已由后续正式子设计和完成记录推进。当前施工资格以第8节及`49-当前状态与下一步入口.md`、`54-当前可自动化施工总入口.md`为准；仍不得把本架构文档直接转换成一次性巨型施工。

## 8. 2026-09-02 Engine workflow与工具面收敛复审

### 8.1 修订结论

本次修订不改变No-Editor、AuthoringProjectContext、Project Game SDK、Compiler、RuntimePackage、Rust Native Runtime、Semantic Outcome、Build/Delivery和Optional Editor的authority。它只修正原v1在“工具少而深”与“显式引擎阶段”之间表达不清的问题，因此保持v1，不建立顶层v2迁移。

正式职责收敛为：

```text
Skill                 公开条件化引擎使用工作流和失败恢复
Engine tools          暴露模型可停止、分支、修复或改变策略的决策控制点
Engine Implementation 隐藏refresh、lease、prepare、glue、package assembly和worker等机械步骤
Host AI               持有用户目标、项目计划、工具选择和最终判断
```

### 8.2 默认工具面自审

v1目标默认工具族冻结为9项：inspect、check、mutate、rollback、run、playtest、observe、build、delivery verify。capture作为observe的typed请求；项目创建进入CLI/模板；项目绑定默认来自workspace/cwd或Host配置。

当前`engine_tool_provider`注册的20项工具不是新的authority。以下现有投影必须在后续正式子设计和施工中收敛：普通search/read、evidence read、浅文本references/symbols、三个共用文本搜索Implementation的UI查询工具、日常project open以及同步调用不需要的operation observe/cancel。当前代码未因本次文档修订自动改变，不能误报已完成工具裁剪。

### 8.3 工作流边界自审

- 通过：显式check/run/playtest/observe/build/verify为模型提供中间反馈和失败恢复，不把完整引擎压成黑盒。
- 通过：prepare及其内部生成、编译、装配步骤不要求AI机械编排。
- 通过：ToolResult可以返回基于当前事实的局部下一步及原因，但不能生成目标级项目计划。
- 通过：Skill不执行引擎能力、不保存项目状态、不成为第二Agent。
- 通过：252 Intent/WorkItem/ChangeSet继续只作可选复杂任务治理，不恢复为所有游戏开发的默认前门。

### 8.4 施工资格

```text
顶层v1文档修订：qualified
下层309-F工具合同收敛方案：not_created
代码施工授权：none
当前Provider工具裁剪完成度：not_started
```
