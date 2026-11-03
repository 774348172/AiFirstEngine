# 334 Internal Prepared Artifact Cache — No New Tool v1

状态：用户已确认方案。日期：2026-09-12。

## 目标

在不增加 AI 可见工具、不改变九项工具独立调用的前提下，复用 `run`、`playtest`、`build` 之间相同身份的内部准备产物，降低重复测试编译和 Player Host 构建成本。

## 核心原则

- Prepared Artifact Cache/Lease 是 Engine 内部对象，不是工具、Provider、Skill 或 AI 参数。
- AI 仍只看到并独立调用现有九项工具。
- 工具不等待其它工具的 ToolResult；命中缓存则消费，未命中则自行准备。
- 不新增 `prepare`、`cache_lookup` 或 `prepared_artifact` 工具。

## 内部调用模型

```text
独立工具调用
  → 校验 projectRevision 和构建身份
  → Engine 查询内部 Prepared Artifact Cache
  → 命中：复用并执行本工具动作
  → 未命中：本工具自行准备并发布缓存
  → 返回本工具自己的 ToolResult
```

## 缓存身份与失效

缓存身份至少包含 projectRevision、sourceDigest、dependencyDigest、buildProfile、toolchainDigest、engineSdkDigest 和 generatedGlueDigest。任一身份变化、产物校验失败或证据缺失时必须失效并重新准备。

## 不变项

默认工具数量仍为九项；不暴露内部准备阶段；不引入工具间调用依赖、Agent 状态或固定 Skill 工作流；不改变 Runtime、Project SDK、RuntimePackage 和项目玩法。

## 验收

证明独立调用仍可成功；相同身份可复用 prepared artifact；命中时跳过 tests compile/Host build；未命中时自包含完成；缓存损坏和身份漂移 fail-closed。
