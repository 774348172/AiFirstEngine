# AI First Game Engine V0.0.3 介绍

## 它要解决什么问题

传统游戏引擎主要围绕人工点击编辑器、编写脚本和手工维护工程关系设计。AI 可以生成代码，却很难稳定理解“当前项目真实状态是什么”“一次修改会影响哪些资产”“修改失败后如何回滚”。AI First Game Engine 尝试把这些问题变成引擎的一等能力。

引擎的核心方向是：项目对象采用稳定结构表达，修改通过受控命令和补丁进入，构建产物可以追溯到输入，运行时只消费明确的 RuntimePackage，验证结果以结构化报告返回。AI 使用和人类编辑器共享的工程接口，而不是绕过引擎直接篡改内部状态。

## 核心链路

```text
自然语言目标 / 可视化操作
        -> AI Tool Catalog / 结构化项目变更
        -> Scene / Prefab / Asset / Rule / AUI
        -> ProjectRuntimePackageAssembler
        -> RuntimePackage
        -> Rust Native Runtime + Project RuntimeModule
        -> World / Projection / Renderer / AUI
        -> 可检查的诊断、报告与回滚引用
```

## 主要设计

### RuntimePackage 是运行输入

运行时不扫描项目源码目录，也不依赖编辑器内存中的临时对象。项目资产经过装配和构建后形成 RuntimePackage，运行时加载明确的 manifest、资源索引、场景和规则数据，让编辑器预览、导出 Player 和自动验证尽量共享真实链路。

### 项目逻辑与引擎底座分离

引擎 Core 提供 ECS、场景、输入、渲染、UI、资源和构建能力，不内置塔防、射击、敌人、武器等具体玩法。复杂项目逻辑由项目侧 Rust RuntimeModule 与受限规则资产承载。

### 项目 Rust 模块使用稳定 ABI

`project_runtime_abi` 与 `project_runtime_sdk` 使用窄 C ABI、版本、能力位和合同摘要隔离宿主与项目模块。V0.0.3 让生成式 Editor 项目组合也从公开 ABI entry 建立 linked runtime set，减少对项目 crate 私有 Rust API 的依赖。

### AI 面向能力目录，而不是编辑器内存

AI Gateway 暴露可枚举的工具描述、结构化输入、项目摘要、审批/副作用信息和结果诊断。AI 可以检查项目、发起受控修改并使用 opaque rollback reference 回滚，但不能直接持有 Editor、ECS 或渲染器内部对象。

### Editor 与 Runtime 明确分层

Editor 负责 authoring、预览、项目组织和报告；Runtime 负责加载发布输入并执行游戏。渲染、物理和 UI 同步通过 Projection 边界传递，项目逻辑不直接操作 GPU handle 或编辑器对象。

### Windows 与 Android 共享运行底座

Windows Player 和 Android GameActivity Player 共享 RuntimePackage、项目模块、World、AUI 与呈现合同。V0.0.3 对 Android x86_64 明确选择 GL backend，并修复 GL 多页字体纹理路径；这不会建立 Android 专用的 Scene、Rule 或 UI 模型。

## V0.0.3 可用于什么

- 阅读、构建和评估 Rust Native Runtime 与 Native Editor 源码；
- 使用结构化 Scene、Prefab、Asset、Rule、Input、AUI 和 RuntimePackage 合同；
- 研究项目 Rust RuntimeModule 的 ABI/SDK、缓存、加载与 Editor 组合方式；
- 构建 Windows 原生编辑器、运行时 CLI 和开发用 Player；
- 在配置外部 Android 工具链后研究 ARM64/x86_64 debug APK 开发链；
- 通过 MCP Gateway 研究能力目录、受控项目检查、修改和回滚接口；
- 结合随包设计资料理解当前实现边界和后续路线。

## V0.0.3 不是什么

它不是成熟的 Unity、Unreal 或 Godot 替代品，也不是开箱即用的无代码游戏平台。本包不带样例项目或预编译程序；Android 能力仍是 dev APK 纵切；设计文档中的后续方案也不等于已实现功能。接口、ABI 与工程格式仍可能快速变化。

设计资料入口见 [DESIGN_INDEX.md](DESIGN_INDEX.md)，版本的准确来源和排除范围见根目录 [RELEASE-MANIFEST.json](../RELEASE-MANIFEST.json)。
