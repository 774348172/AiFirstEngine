# Changelog

本文档记录公开发布版本的主要变化。

## [0.0.3] - 2026-08-29

### Added

- Player 图形 backend 选择诊断。
- GL 多页 Bitmap/MSDF FontBundle 的独立 D2 页纹理路径。
- 经过筛选的架构、渲染和编辑器交互设计资料。
- 发布来源清单、设计索引和文件级 SHA-256 清单。

### Changed

- Android x86_64 Player 固定选择 GL backend，其它目标继续使用 primary backend。
- 生成式 Editor 项目组合改为通过稳定 C ABI entry 接入项目原生模块。
- Linked composition 的 Play actionability、GameView 输入路由和真实窗口诊断更加一致。
- MCP server 对外软件版本跟随 Cargo package version。
- 公开发布 crate 版本统一为 `0.0.3`。
- 内置中文 FontPack 更新本地化目录来源归因并由正式 producer 重新封印，字符规模保持不变。
- `Cargo.lock` 按当前工作区锁定闭包重新裁剪；传递依赖锁定版本与 V0.0.2 存在变化。

### Packaging

- 设计资料集只收录正式架构与方案，排除当前状态入口、施工队列、过程性审查报告和历史失败归档。
- 源码包保持引擎-only 边界，不纳入样例项目、项目专用 RuntimeModule/Player、构建缓存和内部证据。

## [0.0.2] - 2026-08-25

### Added

- 新增 `project_runtime_abi` 与 `project_runtime_sdk`，为项目 Rust 原生模块提供稳定、可校验的宿主边界。
- 新增项目原生模块构建、身份封印、缓存和编辑器非阻塞准备基础。
- 新增 `runtime_player_android`、GameActivity 生命周期/触摸接线和 Android debug APK 导出纵切。
- 新增 Android ARM64 与 x86_64 emulator dev profile 基础。

### Changed

- Windows Player 改进为持续窗口会话，并统一竖屏 contain 呈现与输入坐标。
- AUI clean frame 支持差分缓存，固定步进与可见呈现解耦。
- 修正 MSDF atlas 方向、物理像素 AutoHybrid 选择和空白 glyph metrics。
- 改进 Editor GameView 即时 AUI 输入、纹理 binding、hover 稳定性和项目 UI state 条件解析。
- 公开发布 crate 版本统一为 `0.0.2`。

### Removed

- 发布包继续排除所有样例项目、项目专用 RuntimeModule/Player、构建缓存和内部施工证据。

## [0.0.1] - 2026-08-14

### Added

- 首个 Rust Native Runtime 与 Native Editor Host 源码预览。
- ECS、RuntimePackage、项目资源装配、WGPU 渲染、AUI、输入与 AI Gateway 基础。
- Windows 构建说明、引擎介绍和双远端发布指引。

### Changed

- 公开发布 crate 版本统一为 `0.0.1`。

### Removed

- 临时塔防项目以及其它样例游戏和项目专用代码。
- 内部施工材料、历史原型、验证产物、二进制和压缩产物。
