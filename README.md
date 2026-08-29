# AI First Game Engine

**V0.0.3 source preview** | Rust native runtime + native editor | Windows + Android dev

AI First Game Engine 是一个面向 AI 协作创作的实验性游戏引擎。它把项目资产、场景、规则、UI、构建输入和诊断结果组织成可验证的结构化数据，让 AI 在明确边界内创建、检查和修改项目，而不是直接操作不可审计的编辑器内部状态。

V0.0.3 是引擎源码预览版。本版重点改善 Android x86_64 模拟器的 GL 呈现、GL 多页字体纹理、项目原生模块在 Editor 组合中的 ABI 接线，以及 Play/Input/真实窗口诊断的一致性；同时首次随源码包提供经过筛选的架构、渲染与编辑器交互设计资料。

[详细介绍](docs/INTRODUCTION.zh-CN.md) | [设计资料索引](docs/DESIGN_INDEX.md) | [版本说明](RELEASE_NOTES.md) | [来源清单](RELEASE-MANIFEST.json) | [发布指引](PUBLISHING.md)

## 目前包含

- Rust Native Runtime：ECS world、场景加载、固定帧更新、RuntimePackage、渲染与项目运行时接口。
- Native Rust Editor：项目启动器、可停靠/浮动工作区、层级、检查器、资产浏览、GameView、构建与报告面板。
- GPU/UI：`wgpu` 渲染路径、Sprite2D 纹理、AUI 运行时 UI、Animator2D、字体与中文默认字体包。
- AI 工具入口：能力目录、项目检查、受控修改/回滚、MCP Gateway 组件。
- 项目构建基础：项目资源装配、RuntimePackage、Windows Player staging 与导出底座。
- 项目原生模块：`project_runtime_abi` / `project_runtime_sdk`、模块身份校验、缓存与非阻塞准备基础。
- Android dev：GameActivity Player、触摸/生命周期接线、RuntimePackage materialization 与 ARM64/x86_64 debug APK 导出基础。
- 设计资料：229 份正式架构与方案文档、1 份渲染路线文档、9 项编辑器交互设计资料。

设计文档既包含已经实现的合同，也包含后续方向，不能把“文档中存在”理解成“V0.0.3 已全部实现”。源码和实际验证结果优先于路线文档。详见 [设计资料索引](docs/DESIGN_INDEX.md)。

## V0.0.3 重点变化

- Android x86_64 Player 显式选择 GL backend，并在运行报告中记录实际图形 backend；其它平台继续使用 primary backend。
- GL backend 的多页 Bitmap/MSDF 字体改为每页独立 D2 纹理，避开部分 GL 环境对纹理数组层采样的不一致。
- 内置中文 FontPack 刷新本地化目录来源归因并由正式 producer 重新封印；字符规模保持 1,357 codepoints / 1,223 Han。
- 生成式 Editor 项目组合通过稳定 `aife_project_runtime_entry_v1` C ABI 接入项目模块，不再依赖项目 crate 暴露 Rust 专用 `linked_set()`。
- Linked Editor 组合的 Play 可用性、Editor UI 与 GameView 的输入优先级、真实窗口 authority 诊断更一致。
- MCP `serverInfo.version` 现在跟随 Cargo 包版本。
- 新增经过筛选的设计资料与文件级 SHA-256 清单。
- `Cargo.lock` 从当前工作区锁定闭包重新裁剪为 16 个发布 crate；传递依赖锁定版本可能与 V0.0.2 不同。

## 发布边界

本目录只包含引擎产品源码、运行资源、最小通用 fixture 和筛选后的设计资料，不包含：

- `samples/**`、项目专用 RuntimeModule/Player 或任何样例工程；
- 当前施工文档、阶段完成记录、过程性施工/代码审查报告、运行证据或本机状态入口；
- OpenCode、Unity、Unreal、Godot、Bevy 等第三方源码镜像或研究副本；
- legacy TypeScript/Electron 原型、缓存、安装器、预编译二进制和构建产物。

本包是从一个有未提交修改的主工作区冻结出的独立源码快照，不声称等同于主仓库基准 commit。准确来源见 [RELEASE-MANIFEST.json](RELEASE-MANIFEST.json)；最终包内容以本独立仓库的 `v0.0.3` 标签为准。

## 环境要求

- Windows 10/11 x64；
- Rustup；固定 Rust `1.96.0`，并安装 `rustfmt`；
- Visual Studio 2022 Build Tools、“使用 C++ 的桌面开发”和 Windows SDK；
- 支持 DirectX 12、Vulkan 或目标平台 GL 路径的显卡与近期驱动。

Android dev 导出还需要 JDK 17、Android SDK 35、NDK 28.1、Gradle bootstrap 和相应 Rust Android target；引擎不会静默安装这些外部工具。

## 构建与运行

```powershell
cd rust
cargo build --locked --release -p editor_host --features real-window,real-wgpu-surface
./target/release/editor_host.exe --real-window
```

基础源码检查：

```powershell
cd rust
cargo metadata --locked --no-deps --format-version 1
cargo fmt --all -- --check
cargo check --locked --workspace --all-features
```

本源码包排除了项目样例，因此主仓库中依赖样例路径或项目专用模块的集成测试不属于本包的可执行测试集合。

## 目录结构

```text
docs/
  DESIGN_INDEX.md            设计资料入口与适用范围
  design/                    架构、渲染、编辑器交互设计
rust/
  crates/                    16 个引擎产品 crate
  fixtures/                  最小通用 ABI/font fixture
  resources/                 字体、主题和内置运行资源
RELEASE-MANIFEST.json        版本、来源、范围与排除项
SHA256SUMS.txt               发布文件 SHA-256 清单
```

## 已知限制

- Windows 是主要编辑器平台；Android 当前只承诺 debug APK 开发链，不包含 AAB、商店发布和正式签名流程。
- 本版本不承诺 API、项目格式、项目 ABI 或 RuntimePackage schema 的跨版本兼容。
- 3D、完整物理、音频、复杂动画、完整可视化脚本和成熟资产导入链尚未形成稳定产品面。
- 这是源码包，不含已资格化的 `editor_host.exe` 或其它预编译程序。

## 许可证

当前发布状态为 `UNLICENSED`，源码与随包项目文档可查看，但未授予复制、修改、再分发或商业使用许可。详见 [LICENSE](LICENSE)。内置字体和语料等第三方资源保留其原始许可证和声明。

## English summary

AI First Game Engine V0.0.3 is an experimental source preview of a Rust native runtime and editor. It improves Android x86_64 GL selection, GL multi-page font textures, ABI-based linked project composition, Play/Input diagnostics, and ships a curated design-document set. This engine-only archive excludes sample projects, project-specific runtime modules, internal construction evidence, third-party source mirrors, binaries, and caches. The project is currently `UNLICENSED` and not production-ready.
