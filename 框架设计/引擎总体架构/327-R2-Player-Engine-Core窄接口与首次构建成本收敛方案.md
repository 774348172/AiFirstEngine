# 327-R2 Player Engine Core 窄接口与首次构建成本收敛方案

状态：已被 327-R3 替代（2026-09-12）。本方案的窄接口抽取方向停止，不再作为施工依据。

## 目标

借鉴 Unity“稳定 Engine + 独立 Project Assembly”的分层思想，收敛 Rust Engine 与 Project SDK 的反向依赖，减少首次 Player 构建中不必要的 tests compile 与 Host build，同时保持最终单一 `Game.exe` 交付。

## 方案

抽取少量项目无关、稳定且 schema-first 的 Engine Core 类型与接口。Engine Runtime/Host 只依赖该窄接口；Project SDK、RuntimeModule 与 Generated Runtime Glue 依赖 Engine Core，不反向让 Engine 消费项目高层业务类型。开发构建按源码、测试、生成 glue 和 Host 依赖的失效身份分别触发；发布仍执行完整链接和交付验证。

## 不变项

- 不新增 AI 可见工具，仍保持现有九项工具面。
- 不引入 DLL、脚本 VM、动态 Assembly 或 Live Coding。
- 最终交付仍是单一可运行 `Game.exe`。
- 不把 Player/Enemy 等项目玩法语义移入 Engine。
- RuntimePackage、334 内部缓存、Skill 与工具独立调用语义不变。

## 边界

仅处理 `engine_runtime`、`runtime_player_winit`、Project SDK/RuntimeModule 和 Generated Glue 之间的编译依赖；不重写 Runtime、Renderer、AUI 或项目玩法。

## 成功标准

1. Engine 不再直接依赖 Project SDK 高层业务类型。
2. 项目源码或测试变化不会无条件使稳定 Engine Core 失效。
3. 首次构建报告能分别记录 tests compile、Host build 与链接成本。
4. 单一 Game.exe 的运行、build、delivery verify 结果保持一致。
5. 反向依赖、身份漂移和接口不兼容均 fail-closed。

## 风险与停止条件

若抽取需要引入项目语义、公共大接口、DLL/脚本 VM、工具面变化，或无法保持单一 Game.exe，则停止并回到方案讨论。
