# 327-R3 Stable Engine Runtime DLL + Project Public API v1

状态：327-R3 已完成归档；2026-09-13 经 335 基线复核限定交付证据。Engine DLL execution owner、Project DLL Loader 与 ABI 失败分支的独立证据保留；旧 `.artifacts/327-r3-formal-delivery` 混合两次测试输出，不能证明单次正式成功导出。单一同源交付资格由 [335 方案](335-单一权威交付布局与导出产物一致性-v1方案.md) 补齐，不重开 Runtime 架构。

## 目标

2026-09-13 构建边界补充：[337固定Player Host预编译复用](337-固定-Player-Host-预编译复用-v1方案.md)已完成，将每项目Host编译收敛为同一引擎构建根内的成品复用，保留本方案公开API/ABI与335/336交付执行合同。实际验收范围及安装状态以337完成记录为准。

采用 Unity/Unreal 式的交付与编译边界：Engine Runtime 由引擎独立构建为稳定的 Windows 动态库，项目层通过公开 Engine API 自由调用引擎能力。项目源码变化不再触发 Engine Runtime 的重新编译，从而降低 Player 首次和增量构建成本。

## 核心模型

```text
Stable Engine Runtime DLL
        ↑
Engine Public C ABI
        ↑
Project SDK / Project Runtime Module
        ↓
Project gameplay, assets, AUI and tests
        ↓
Game Host + RuntimePackage
```

Engine DLL 由 Engine 自己构建、测试和版本化。项目可使用完整公开 API，不要求 AI 或项目作者从人工列出的狭窄目录中选择函数；公开 API 与内部实现分离，项目不得依赖 Engine 私有 Rust crate、内部结构体或内部符号。

## Public API / ABI

公开边界采用稳定、版本化的 C ABI facade，包含 API 表、版本与能力查询、固定布局数据、opaque handles、显式生命周期、结构化错误码和有界缓冲区。禁止暴露 Rust trait、泛型、生命周期、内部 ECS/Renderer 类型、项目玩法类型或未版本化内存所有权。

Project SDK 可在 C ABI 之上提供 Rust-friendly facade，但 facade 不改变 DLL ABI 真相。内存分配与释放必须由同一 owner 完成；API 不兼容时必须 fail-closed 并返回结构化诊断。

## 构建与运行

开发态：Engine DLL 使用已资格化版本；项目只编译 Project SDK/RuntimeModule 和 Generated Glue，Game Host 仅在自身依赖变化时重编译。

首期运行 owner 由独立 `engine_runtime_host` cdylib 提供，产物仍固定命名为 `engine_runtime.dll`。它复用 Engine Runtime 核心与 Native Player 执行实现，但通过版本化 C ABI 接收 RuntimePackage 路径和有界帧请求，并写出结构化运行报告；Game Host 只负责加载、校验、转发和读取报告，不静态接管该运行循环。

发布态：Build Graph 锁定 Engine DLL 版本、Project Module、RuntimePackage 和 Game Host，执行完整链接/打包/交付验证。运行时加载并校验 Engine API 版本、能力位、模块身份和 package manifest。

## 不变项

- AI 可见 Engine tools 仍为现有九项，MCP/Skill 不新增工具或固定调用菜谱。
- Engine 仍只提供项目无关底座；Player/Enemy 等项目语义留在项目层。
- RuntimePackage、Semantic Outcome、334 内部准备缓存和独立工具调用语义保留。
- 不引入脚本 VM、项目专用引擎 API 或工具间等待依赖。

## 兼容与更新

Engine DLL 使用明确的 ABI major/minor 和能力位。minor 兼容在同一 major 内向后兼容；major 不兼容时拒绝加载并给出升级动作。Engine DLL、Project Module 和 RuntimePackage 的身份必须写入交付 manifest 与诊断报告。

## 成功标准

1. `engine_runtime` 不再在项目 Player 编译路径中直接依赖 `project_runtime_sdk` 高层类型。
2. 项目源码或项目测试变化不会重新编译已资格化 Engine DLL。
3. 项目层可以调用完整公开 Engine API，API 由版本化 ABI 保证稳定性。
4. 开发态能单独更新项目模块；发布态仍生成可运行、可验证的 Windows 交付包。
5. ABI、版本、模块或 package 身份不匹配时 fail-closed。

## 范围与风险

首期只实现 Windows dev Engine DLL 和最小公开生命周期/输入/时间/运行时命令合同；不在首期迁移全部 Engine 内部类型，不同时改造 Android、Editor UI 或项目玩法。施工必须先验证 ABI 内存所有权、跨进程/线程边界、DLL 部署和回滚路径，再扩大 API 覆盖。

## 停止条件

若实现需要把项目语义放入 Engine、暴露未稳定 Rust 内部类型、恢复整体 `project_runtime_sdk` 迁移、增加 AI 工具或无法保持 Engine 独立版本化，则停止并重新讨论。
