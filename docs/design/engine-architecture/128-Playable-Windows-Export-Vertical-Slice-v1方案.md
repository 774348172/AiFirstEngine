# 128-Playable Windows Export Vertical Slice v1 方案

## 1. 问题定义

本系统解决的不是单个编辑器按钮，也不是单个 Runtime 模块，而是完整纵向链路：

```text
Editor Project
  -> Saved Project Data
  -> RuntimePackageBuilder
  -> Windows package staging
  -> Player executable
  -> RuntimePackage load
  -> Windowed/Headless player gate
  -> Build / Runtime / Trace reports
```

第一版目标是让一个由编辑器保存的项目可以被构建成 Windows 桌面包，并能通过自动化 gate 验证它可以运行。

本系统不引入打飞机专用 API。`Player / Enemy / Bullet / Health / Score / Wave` 等仍属于项目侧，由 Project Schema / Rule / Prefab / Asset / AUI 定义。

## 2. 成熟引擎对比

### Unity

Unity 的链路是：

```text
Scene / Prefab / AssetDatabase / Script
  -> BuildPipeline.BuildPlayer
  -> exe + Data
```

关键点是编辑器保存内容可以稳定进入 Player 构建产物。

### Unreal Engine

UE 的链路是：

```text
Content / Map / Blueprint / C++
  -> Cook
  -> Stage
  -> Package
  -> Windows executable
```

关键点是 Cook / Stage / Package 边界清晰，报告可追踪。

### Godot

Godot 的链路是：

```text
Scene / Resource / Script
  -> Export Preset
  -> executable + pck/data
```

关键点是 Scene / Resource 既是编辑器创作入口，也是导出数据来源。

### 我们的差异

我们已有 RuntimePackageBuilder、RuntimeScene Hydration、WindowedPlayer engine-side gate、WGPU surface gate、Native Editor 基础面板。缺的是把它们收敛成一条可重复的产品级构建主线。

## 3. 推荐方案：C-min

采用长期结构下的最小可玩 Windows 导出切片：

```text
EditorSavedProjectToRuntimePackageGate
  -> RuntimePackageValidationGate
  -> WindowsPackageStageGate
  -> WindowedPlayerLaunchGate
  -> ExportedGameSmokeGate
  -> BuildRuntimeReportGate
```

## 4. 第一版边界

### v1 必须做

```text
从 ProjectManifest / default scene 读取编辑器项目
把 EditorSceneDocument 转换为 RuntimeScene
构造 RuntimePackageBuildInput
调用现有 RuntimePackageBuilder
stage Windows package 目录
复制或记录 runtime_cli/player exe
写 package-manifest / desktop-export-report
用 staged runtime_package 跑 headless player gate
输出 AI 可读报告
```

### v1 不做

```text
安装器
代码签名
商店包
压缩包优化
多平台导出
完整真实项目规则编辑器
完整 texture/mesh/material 产品级 cook
Steam / itch / Store 集成
```

## 5. 目录规则

第一版输出目录：

```text
<project>/Build/Windows/<profile>/
  Game.exe 或 runtime_cli.exe
  data/runtime_package/
  data/assets/
  reports/desktop-export-report.json
  reports/build-runtime-package-report.json
  reports/runtime-package-validation-report.json
  reports/windowed-player-run-report.json
  package-manifest.json
```

如果当前构建环境不能复制真实 exe，允许第一版记录 `playerExecutableStatus=not_found`，但必须仍然能通过 `cargo` 或当前测试二进制执行 headless gate。

## 6. 核心规则

```text
1. Export 不读取 UI 临时状态，只读取保存后的项目文件。
2. RuntimePackageBuilder 是唯一 RuntimePackage 生成入口。
3. Windows package stage 不修改 RuntimePackage 内部 schema。
4. Player gate 必须读取 staged runtime_package，而不是内存 fixture。
5. 所有阶段都必须写报告，报告要能被 AI 直接定位失败层。
6. v1 可以 headless 验证可运行性，但目录结构必须按真实 Windows package 设计。
```

## 7. 验收标准

```text
给定一个 ProjectLauncher 创建的项目
  -> desktop export 成功
  -> data/runtime_package/manifest.json 存在
  -> package-manifest.json 存在
  -> desktop-export-report.json 存在
  -> WindowedPlayer headless gate 能读取 staged package
  -> 报告里能看到 package / scene / world / render / rhi 状态
```

## 8. 后续升级

v1 通过后，再继续：

```text
真实 WindowedPlayer native host
真实 texture/sprite GPU upload
Project Rule Authoring / Compile / Runtime Execute Gate
Build And Run UI
Exported Shooter Golden Scenario
```

