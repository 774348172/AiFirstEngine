# Changelog

本文档记录公开发布版本的主要变化。

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

