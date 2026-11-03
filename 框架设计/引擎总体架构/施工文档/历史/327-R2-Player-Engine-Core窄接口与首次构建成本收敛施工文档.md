# 327-R2 Player Engine Core 窄接口与首次构建成本收敛施工文档

状态：已被 327-R3 替代（2026-09-12），Gate A 停止记录保留；本文不再施工。

## 施工边界

只调整 Engine Core 与 Project SDK 的编译依赖边界及失效报告；不增加 AI 工具，不引入 DLL/脚本 VM，不改变 RuntimePackage、工具独立调用或项目玩法。

## 必要性与验证经济性审计

- 请求目标：降低首次 Player 构建中约 42 秒 Host build 与约 22 秒 tests compile 的重复失效范围。
- 最小因果变更：抽取少量通用接口，修正 Engine→Project 反向依赖，并按身份拆分构建触发。
- 明确排除：整体迁移 `project_runtime_sdk`、动态模块、Runtime 重写、发布配置变更。
- Gate 证据：每个 Gate 使用最窄 owner 测试和依赖图证据；不以完整宿主作为编辑循环。

## Gate A：依赖边界基线

盘点当前反向依赖、公共类型和 Cargo 失效范围，落盘 `.artifacts/327-r2-gate-a-boundary.json`。若发现方案假设错误，停止后更新正式方案。

## Gate B：窄接口抽取

抽取并迁移最少的项目无关类型；Engine 只消费窄接口，Project SDK 反向依赖 Engine Core。运行 owner 单元测试和 `cargo metadata --locked`。

证据：`.artifacts/327-r2-gate-b-interface.json`。

## Gate C：首次构建失效范围

在不改变最终交付形态的前提下，验证源码、测试、生成 glue 和 Host 依赖变化的触发范围，并记录各阶段耗时。

证据：`.artifacts/327-r2-gate-c-timing.json`。

## Gate D：单一 Game.exe 与独立性回归

验证 run/build/delivery verify、反向依赖 fail-closed、接口漂移 fail-closed，确认九项工具和 Skill 工作流不变。

证据：`.artifacts/327-r2-gate-d-regression.json`。

## 激活与归档

只有当前施工槽为空且完成激活前复核后，才能从 `待执行/` 移到 `当前/`。Gate A-D、受影响 owner 回归和整体回归全部通过后，生成完成记录、同步入口并归档本文。
