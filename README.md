# AI First Game Engine

**V0.0.2 source preview** | Rust native runtime + native editor | Windows + Android dev

AI First Game Engine 是一个面向 AI 协作创作的实验性游戏引擎。它把项目资产、场景、规则、UI、构建输入和诊断结果组织成可验证的结构化数据，让 AI 能够在明确边界内创建、检查和修改项目，而不是直接操作不可审计的编辑器内部状态。

V0.0.2 在首个源码预览版上补齐项目 Rust 原生模块 ABI/SDK、持续窗口 Player、AUI 差分呈现和 Android dev APK 纵切。它适合源码阅读、架构评估和早期功能试用，目前仍处于快速开发阶段，不建议直接用于生产项目。

[English summary](#english-summary) | [详细介绍](docs/INTRODUCTION.zh-CN.md) | [版本说明](RELEASE_NOTES.md) | [发布到 GitHub/Gitee](PUBLISHING.md)

## 目前包含

- Rust Native Runtime：ECS world、场景加载、固定帧更新、RuntimePackage、渲染与项目运行时接口。
- Native Rust Editor：项目启动器、工作区、层级、检查器、资产浏览、GameView、构建与报告面板。
- GPU/UI：`wgpu` 渲染路径、Sprite2D 纹理、AUI 运行时 UI、字体与中文默认字体包。
- AI 工具入口：可枚举能力的工具目录、项目检查与受控修改基础、MCP Gateway 组件。
- 项目构建基础：项目资源装配、RuntimePackage、Windows Player staging 与发布相关底座。
- 输入与交互：键鼠输入映射、编辑器输入路由和运行时 UI 交互基础。
- 项目原生模块：稳定的 `project_runtime_abi` / `project_runtime_sdk` 边界、模块身份校验与编辑器非阻塞准备基础。
- Android dev：GameActivity Player、触摸/生命周期接线、RuntimePackage materialization 与 ARM64/x86_64 debug APK 导出基础。

## 发布边界

本目录只包含引擎产品代码和运行所需资源，不包含：

- 临时塔防项目或任何 `samples/tower_defense_project` 内容；
- 复杂打飞机、开关谜题等样例项目与项目专用 RuntimeModule/Player；
- 历史 TypeScript/Electron 原型；
- 内部架构施工文档、审查材料、阶段证据和本机构建产物；
- 安装器、预编译二进制或压缩包。

## 环境要求

- Windows 10/11 x64；
- Rustup；仓库固定 Rust `1.96.0`，首次构建时 rustup 会按 `rust/rust-toolchain.toml` 获取工具链；
- Visual Studio 2022 Build Tools，安装“使用 C++ 的桌面开发”和 Windows SDK；
- 支持 DirectX 12 或 Vulkan 的显卡与近期驱动。

Android dev 导出还需要 JDK 17、Android SDK 35、NDK 28.1、Gradle bootstrap 和相应 Rust Android target；这些工具不会由引擎静默安装。

## 构建与运行

```powershell
cd rust
cargo build --locked --release -p editor_host --features real-window,real-wgpu-surface
./target/release/editor_host.exe --real-window
```

只检查源码和依赖闭包：

```powershell
cd rust
cargo fmt --all -- --check
cargo check --locked --workspace --all-features
```

构建运行时命令行入口：

```powershell
cd rust
cargo build --locked --release -p runtime_cli --features real-window
```

## 目录结构

```text
rust/
  crates/
    engine_runtime/          运行时、ECS、RuntimePackage、渲染
    engine_input/            运行时输入状态与映射
    project_runtime_abi/     项目 Rust 原生模块稳定 C ABI
    project_runtime_sdk/     项目运行时模块 SDK 与数据合同
    editor_core/             编辑器领域逻辑与项目工作流
    editor_ui_model/         编辑器 UI 状态模型
    editor_ui_renderer/      编辑器 UI 绘制列表
    editor_wgpu_renderer/    编辑器 WGPU 后端
    editor_window_winit/     原生窗口与事件循环
    editor_host/             原生编辑器程序入口
    ai_tool_gateway/         AI/MCP Gateway
    runtime_cli/             运行时命令行入口
    runtime_player_android/  Android GameActivity Player
  resources/                 字体、主题和内置运行资源
```

## 已知限制

- Windows 是主要编辑器平台；Android 当前只承诺 debug APK 开发纵切，不包含 AAB、商店发布和正式签名流程。
- 本源码包刻意移除了项目样例，因此仓库内部依赖样例工程的集成测试不属于此发布包。
- AI Gateway 仍是早期接口，配置、权限和宿主集成可能在后续版本变化。
- 3D、完整物理、音频、复杂动画、完整可视化脚本和成熟资产导入链仍未形成稳定产品面。
- V0.0.2 不承诺 API、项目格式、项目 ABI 或 RuntimePackage schema 的跨版本兼容。

## 许可证

当前发布状态为 `UNLICENSED`，源码可查看，但未授予复制、修改、再分发或商业使用许可。详见 [LICENSE](LICENSE)。内置字体等第三方资源保留其原始许可证和声明。

## English summary

AI First Game Engine V0.0.2 is an experimental source preview built around a Rust native runtime and editor. This release adds a stable project-runtime ABI/SDK boundary, nonblocking native-module preparation, continuous Windows player presentation, differential AUI presentation, and an Android GameActivity/debug-APK development path. This engine-only release excludes all sample games, temporary tower-defense content, internal construction evidence, legacy prototypes, binaries, and archives. The project is currently `UNLICENSED` and not production-ready.

