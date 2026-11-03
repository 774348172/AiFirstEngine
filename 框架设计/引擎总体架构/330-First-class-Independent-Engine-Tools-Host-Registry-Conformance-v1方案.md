# 330 First-class Independent Engine Tools + Host Registry Conformance v1

状态：用户已确认采用方案 A+B，正式方案。
日期：2026-09-12

## 1. 目标

将 Engine 已稳定的深能力逐个注册为宿主 AI 可直接发现和调用的独立 typed tools。Native Provider 直接接入真实 Host Tool Registry；MCP 仅逐工具兼容投影。两者共享同一份最小 Engine-owned Tool Definition、Engine Tool Kernel、权限、dispatch、结果 envelope 与诊断语义。

## 2. 强制调用拓扑

```text
ChatGPT Host Tool Registry
  ├── engine_project_inspect
  ├── engine_project_check
  ├── engine_project_mutate
  ├── engine_project_rollback
  ├── engine_runtime_run
  ├── engine_runtime_playtest
  ├── engine_runtime_observe
  ├── engine_project_build
  └── engine_delivery_verify
        ↓
  Native Provider Projection
        ↓
  Engine Tool Kernel
```

MCP 只允许提供同名、同 schema、同结果语义的逐工具兼容投影，并直接进入同一个 Engine Tool Kernel。禁止单一 MCP/Gateway 工具通过 `toolId`、`operation` 或字符串路由二次分发；禁止向 AI 暴露 `catalog/execute`。

## 3. 设计范围

### 3.1 Engine-owned Tool Definition

建立最小共同定义，作为 Native Provider 与 MCP Adapter 的来源：工具名、typed 输入 schema、typed 输出 envelope、capability grant、错误码、取消标记和版本标识。第一阶段不建设通用 schema 编译器，不新增独立 registry 真相。

### 3.2 Native Provider

为九个工具提供直接宿主注册接口。宿主可逐个列出工具并完成参数校验、调用授权和结果接收。Native Provider 不理解完整对话、不规划任务、不拥有项目状态。

### 3.3 MCP Compatibility Adapter

按工具逐个投影到 MCP。MCP transport、session 和协议包装不得改变工具名称、输入、输出、错误或取消语义。MCP 不得成为 Engine 工具统一上游。

## 4. 九项默认工具

```text
engine_project_inspect
engine_project_check
engine_project_mutate
engine_project_rollback
engine_runtime_run
engine_runtime_playtest
engine_runtime_observe
engine_project_build
engine_delivery_verify
```

`capture` 继续作为 `engine_runtime_observe` 的 typed 请求。`prepare`、`refresh`、`snapshot`、generated glue、RuntimePackage 装配、缓存和证据落盘继续是工具内部阶段，不注册为独立工具。

## 5. 一致性合同

Native Provider 与 MCP Adapter 对同一输入必须保持：

- 工具名称和 schema 一致；
- capability 与授权边界一致；
- Engine revision、artifact、delivery identity 一致；
- Result envelope、诊断码、失败分支和 cancellation 语义一致；
- 不产生第二份 mutation、cache、receipt 或 project authority。

## 6. 分阶段施工

### Gate A：Definition Baseline

盘点九项工具当前 Kernel definition、dispatch、schema、结果和错误；建立 Engine-owned 最小定义并证明没有新增工具或二级路由。

### Gate B：Native Direct Registration

实现 Native Provider 的逐工具注册，接入真实 Host Tool Registry；验证宿主实际列出九项独立工具。

### Gate C：MCP Per-tool Projection

将九项工具逐个投影到 MCP，验证 MCP 仅为兼容 Adapter，未引入统一大工具或内部路由字段。

### Gate D：Equivalence

使用同一 project revision 和 typed 输入，分别执行 Native 与 MCP；比较结果、诊断、revision、artifact、delivery identity 和取消语义。

### Gate E：Real Host Conformance

在真实 ChatGPT 宿主执行 inspect、mutate、run、playtest、observe、build、delivery_verify 完整链路，保留 Tool Registry、调用结果和身份一致性证据。

## 7. 明确不做

- 不新增 `engine_execute` 或统一 MCP 大工具；
- 不向 AI 暴露 `catalog/execute`；
- 不把 Engine 内部阶段机械拆成更多工具；
- 不新增 Provider Registry 作为能力真相；
- 不改变 Runtime、Compiler、Project SDK、RuntimePackage 或项目玩法；
- 不进行 Editor authority migration 或生产安装替换。

## 8. 验收标准

方案完成必须同时证明：真实宿主直接看到九项独立 typed tools；Native 与 MCP 逐工具投影没有语义漂移；完整调用链可完成；失败与取消可诊断；Engine Core 不依赖宿主或 MCP transport；工具总数没有因 R5 增加。
