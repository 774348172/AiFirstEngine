# V0.0.3 Release Notes

发布日期：2026-08-29

V0.0.3 是 AI First Game Engine 的第三个引擎源码预览版。本版没有新增一套 AI Tool API；AI Gateway 的主要能力与 V0.0.2 相同。变化集中在 Android/GL 呈现、项目原生模块的 Editor 组合接线、Play/Input 一致性，以及设计资料的可交付整理。

## 运行与平台改进

- Android x86_64 Player 显式选择 GL backend；Android ARM64 和桌面平台继续使用 primary backend。
- Player 报告新增 `graphics.backend_selected` 信息诊断，便于判断实际采用的图形后端。
- GL backend 的多页 Bitmap/MSDF FontBundle 改为每页独立 D2 纹理，避免纹理数组层在部分 GL 环境下采样错误。
- 保留非 GL backend 的纹理数组路径，避免无必要地扩大其它平台的资源与 binding 变化。

## Editor 与项目模块改进

- 生成式 Editor 项目组合通过项目模块导出的 `aife_project_runtime_entry_v1` ABI 表建立 `LinkedProjectRuntimeSet`，移除对项目 Rust 专用 `linked_set()` 的要求。
- 已绑定 linked composition 时，Play 可用性不再被仅适用于动态准备路径的 RuntimeModule blocker 错误禁用。
- 同一输入已经命中 Editor UI 交互时，不再因 GameView route 的 `UiConsumed` 提前返回；Editor UI 未命中时仍由 GameView 消费。
- 真实窗口 authority scenario 会保留并报告最后一个不可执行原因，包括控件 disabled 的具体原因，而不是只返回通用超时。

## 发布与文档

- 16 个引擎产品 crate 统一使用版本 `0.0.3`。
- MCP `serverInfo.version` 改为读取 Cargo 包版本；工具实现协议自身的 `1.0.0` 版本保持不变。
- 首次加入筛选后的设计资料：229 份引擎架构/方案文档、1 份渲染路线文档、9 项编辑器交互设计资料。
- 新增 `docs/DESIGN_INDEX.md`、`RELEASE-MANIFEST.json` 和文件级 `SHA256SUMS.txt`。
- 发布历史延续 V0.0.1/V0.0.2 的独立源码仓库，不包含预编译程序。

## 资源与依赖封印

- 内置中文 FontPack 为 10 个已经存在的汉字补齐 `engineZhCnCatalog` 来源标签，并通过仓库正式 producer 重新生成 glyph lock、bundle metadata 和 cooked pages。
- FontPack 字符规模保持 1,357 codepoints / 1,223 Han，仍为 3 个 Bitmap 页和 7 个 MSDF 页；同一 producer 连续两次生成的 12 个 cooked 文件 SHA-256 完全一致。
- `Cargo.lock` 按当前主工作区的锁定闭包裁剪为 16 个发布 crate 和它们的外部依赖，不沿用 V0.0.2 的传递依赖锁定版本；这属于源码快照依赖封印变化，不表示新增公开 API。

## 来源说明

本版是从主仓库工作区快照冻结，而不是从 clean exact commit 直接归档：

- 基准 commit：`acaffa8cb95bbb54d093bbf45835a59315c82da5`
- 基准分支：`codex/255-capability-catalog`
- 捕获时工作区：174 个 tracked 变更、224 个 untracked 条目

因此基准 commit 只能说明快照的父状态，不能单独还原本版。最终可发布内容以独立发布仓库中带 `v0.0.3` annotated tag 的 commit 和随包 SHA-256 清单为准。

## 发布范围

- 包含 16 个引擎产品 crate、运行资源和最小通用 fixture。
- 包含筛选后的架构、渲染和编辑器交互设计资料。
- 排除所有样例、项目专用 RuntimeModule/Player、过程性施工/代码审查/阶段证据和第三方源码镜像。
- 排除构建缓存、安装器和预编译二进制。

## 兼容性与限制

- 主要编辑器平台为 Windows 10/11 x64。
- Android 仍是 debug APK 开发链，不包含 release AAB、正式签名和商店发布。
- 固定工具链为 Rust 1.96.0。
- 设计文档包含已实现合同和后续方案；文档存在不代表对应能力已经交付。
- API、项目格式、项目 ABI 与 RuntimePackage schema 暂不承诺跨版本兼容。
- 当前许可证状态仍为 `UNLICENSED`。
