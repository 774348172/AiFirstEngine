# 129-Editor Build / Export Workspace v1 方案

## 问题定义

`128-Playable-Windows-Export-Vertical-Slice-v1` 已经跑通底层导出链路：

```text
Saved Editor Project
  -> RuntimePackageBuilder
  -> Build/Windows/dev staged package
  -> WindowedPlayer headless gate
  -> desktop-export-report.json
```

但编辑器工作区还缺一个正式入口。用户不能在 Native Editor 里点击导出、查看输出目录、查看导出报告，所以“编辑器里制作项目并导出 Windows 可玩版本”的闭环还不完整。

本系统只做编辑器 Workspace 对现有导出能力的接入，不重新做第二套 Build Pipeline。

## 其他引擎参考

| 引擎 | 对应系统 | 关键做法 |
|---|---|---|
| Unity | Build Settings / BuildPipeline.BuildPlayer | 编辑器 UI 选择平台和 profile，底层仍由统一 BuildPipeline 执行 |
| Unreal Engine | Project Launcher / Cook / Stage / Package / UAT | 编辑器入口组织 profile 和报告，真正执行进入统一 staged build 链路 |
| Godot | Export Presets / Export | 编辑器管理 export preset，底层导出器统一生成目标平台包 |
| 我们 | BuildExportModel + DesktopExportPipeline | 编辑器只产生命令和报告视图，执行复用 128 的 `DesktopExportPipeline` |

结论：成熟引擎不会让每个面板私自实现打包逻辑，而是让编辑器 UI 作为统一 build pipeline 的入口。

## 方案选择

采用 `B-C min`：

```text
Unity-like Build / Export Workspace UI
+ UE-like staged pipeline / report model
+ 直接调用 128 DesktopExportPipeline
```

第一版只支持：

```text
windows-dev profile
ExportDesktopPackage
OpenBuildOutput
OpenBuildReport
BuildExportModel
SelfUiRenderer Build Export panel
```

第一版不做：

```text
installer
signing
store package
multi-platform profile
full async build queue
shell open file explorer
second build pipeline
```

## 正式规则

1. `DesktopExportPipeline` 是 v1 唯一 Windows desktop export 执行入口。
2. Editor Workspace 只持有 `BuildExportModel`，不直接暴露 WGPU、runtime 内部对象或项目玩法规则。
3. UI 命令统一走 `UiCommandPayload`：

```text
ExportDesktopPackage { profile_id }
OpenBuildOutput
OpenBuildReport
```

4. `ExportDesktopPackage` 调用 `DesktopExportPipeline::export`，并把结果保存为 `last_desktop_export_report`。
5. `OpenBuildOutput` / `OpenBuildReport` 第一版只把路径写入 Console，不直接调用系统 shell。
6. Renderer 只渲染 `BuildExportModel`，按钮只输出 command id，不执行导出。
7. Input Router 负责把按钮 hit region 转成 `UiCommandPayload`。
8. Command System / EditorSession 负责命令执行、事务记录和 Console 反馈。

## 数据流

```text
SelfUiRenderer Build Export panel
  -> HitRegion(command_id)
  -> EditorInputRouter
  -> UiCommandPayload
  -> EditorCommandSystem
  -> EditorSession
  -> DesktopExportPipeline::export
  -> DesktopExportReport
  -> BuildExportModel.last_report
  -> Console summary / UI report summary
```

## 为什么适合我们

- AI 友好：所有导出入口、profile、报告摘要都是结构化数据。
- 复杂项目可维护：编辑器只接入 pipeline，不让 UI 膨胀成第二套导出系统。
- 长期主义：未来多平台 profile、异步队列、平台 SDK、签名、商店包都可以挂到同一个 build/export workspace 下。
- 简单：v1 只保留一个 `windows-dev` profile 和三个命令，避免过早设计复杂发布系统。

## 已完成施工

完成记录见：

```text
阶段完成记录/2026-07-01-Editor-Build-Export-Workspace-v1/00-总览.md
施工文档/已完成/129-当前可自动化施工文档-Editor-Build-Export-Workspace-v1.md
```
