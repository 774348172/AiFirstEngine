# 333 Independent Tools + Shared Prepared Artifact Cache v1

状态：已废弃（2026-09-12）。由 334 取代，避免重复施工。

## 目标

在保持九项 Engine tool 独立调用的前提下，让 `run`、`playtest`、`build` 在输入身份完全一致时复用 Compiler 已验证的 prepared artifact，减少重复编译和测试准备成本。

## 强制边界

- 工具不等待其它工具的 ToolResult；每次调用都能独立完成自己的准备和动作。
- 共享的是 Compiler artifact、身份和缓存，不是 Agent 状态、调用顺序或工作流状态。
- 不新增 `prepare` 工具，不暴露内部阶段，不改变九项工具面。
- 缓存命中必须验证 projectRevision、sourceDigest、dependencyDigest、buildProfile、toolchainDigest 和 artifact 完整性。

## 独立调用模型

```text
tool call
  -> validate current project identity
  -> lookup prepared artifact
  -> hit: consume artifact
  -> miss: prepare locally, publish artifact
  -> execute this tool's action
  -> return this tool's result
```

## 失效条件

项目 revision、RuntimeModule 源码、依赖锁文件、Build Profile、toolchain 或 artifact/evidence 任一变化时，缓存不得复用。

## 验收

证明 `run`、`playtest`、`build` 可独立调用；命中时不重复执行可复用阶段；未命中时仍可自行完成；工具间无 ToolResult 依赖；缓存错误和身份漂移 fail-closed。
