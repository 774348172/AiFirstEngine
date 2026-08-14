# V0.0.1 Release Notes

发布日期：2026-08-14

V0.0.1 是 AI First Game Engine 的首个引擎源码预览版。发布目标是提供一个不携带临时游戏项目、可独立构建和评估的 Rust 引擎代码基线。

## 亮点

- Rust Native Runtime 与 Native Editor Host 主线；
- RuntimePackage 驱动的项目运行与构建基础；
- ECS、场景、Prefab、资源、输入、规则与项目 RuntimeModule 边界；
- WGPU 2D 渲染、纹理、字体、AUI 与 GameView 基础；
- AI capability catalog、受控项目修改与 MCP Gateway 基础；
- Windows 原生编辑器工作区和项目 authoring 基础。

## 发布清理

- 移除临时塔防项目；
- 移除全部样例项目和项目专用 RuntimeModule/Player；
- 移除历史原型、内部施工/审查文档、验证证据、构建缓存和二进制；
- 统一发布 crate 版本为 `0.0.1`；
- 增加面向 GitHub/Gitee 的公开 README、介绍、变更记录与发布指引。

## 兼容性

- 主要验证目标：Windows 10/11 x64；
- 固定工具链：Rust 1.96.0；
- API、项目格式与 RuntimePackage schema 暂不承诺跨版本兼容。

