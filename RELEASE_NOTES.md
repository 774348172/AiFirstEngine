# V0.0.2 Release Notes

发布日期：2026-08-25

V0.0.2 是 AI First Game Engine 的第二个引擎源码预览版。本版重点把项目 Rust 逻辑从仓库内静态示例推进为有明确 ABI/SDK、构建身份和宿主生命周期的原生模块，并建立 Windows 连续 Player 与 Android dev APK 两条平台运行路径。

## 主要新增

- `project_runtime_abi`：版本化 C ABI、能力位、opaque handle、字节输入输出和稳定合同摘要；
- `project_runtime_sdk`：项目模块描述、Rule/AUI/fixed-update/UI-state/observation 数据合同与 FFI 辅助；
- 项目原生模块独立构建、内容摘要、缓存、加载与编辑器非阻塞准备；
- `runtime_player_android`：GameActivity 生命周期、触摸映射、RuntimePackage APK asset materialization；
- Android ARM64 与 x86_64 emulator debug APK dev export 基础。

## 运行与呈现改进

- Windows Player 使用持续 Runtime/World/GPU 会话，不再按帧重建主要运行状态；
- GameView/Player 的竖屏 contain、DPI 与输入反解共享同一 presentation identity；
- AUI clean frame 支持差分缓存，固定步进 catch-up 与可见帧 publication 解耦；
- 修正 MSDF atlas 上下方向、物理像素 AutoHybrid 选择和无 outline 空白 glyph metrics；
- 改进 Editor GameView 即时 AUI action、纹理 binding、hover 稳定性与 session-bound UI-state resolve。

## 发布范围

- 16 个引擎产品 crate，统一版本 `0.0.2`；
- 保留引擎运行所需字体、主题和最小 ABI/font fixture；
- 不包含塔防、复杂打飞机、开关谜题或任何其它样例项目；
- 不包含项目专用 RuntimeModule/Player、内部施工文档、验证证据、缓存、安装器、二进制或压缩包。

## 兼容性与限制

- 主要编辑器验证平台：Windows 10/11 x64；
- Android 当前只覆盖 debug APK 开发链，不包含 release AAB、正式签名和商店发布；
- 固定工具链：Rust 1.96.0；
- API、项目格式、项目 ABI 与 RuntimePackage schema 暂不承诺跨版本兼容；
- 当前许可证状态仍为 `UNLICENSED`。
