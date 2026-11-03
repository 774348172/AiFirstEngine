# 当前可自动化施工文档：Runtime Package v1

> 状态提示：Runtime Package v1 已完成并归档到阶段完成记录。本文档保留为历史施工依据，当前入口看 `49-当前状态与下一步入口.md`。

本文档根据当前框架文档重新筛选“可以直接自动化实现”的任务。

筛选结论：

```text
当前最适合自动化实现的是 Runtime Package v1 纯数据层与 Build Graph 生成入口。
暂不直接实现真实 Rust Runtime。
暂不实现 Native Editor Host MVP。
暂不实现真实 cooked binary。
暂不实现真实 IR -> Rust AOT。
```

## 筛选依据

已经确认的架构规则：

```text
Rust Native Runtime MVP 第一版读取 Runtime Package，而不是编辑器内部 Project Object。
Runtime Package v1 = Normalized Project Runtime View。
Runtime Package v1 第一版只做 Normalize / Strip / Validate。
Editor Play / Preview 必须走 Runtime Package。
Runtime Package v1 可以使用 debug-readable JSON。
Runtime Package v1 不做复杂 Prefab 展开。
Runtime Package v1 不做复杂 Component lowering。
Runtime Package v1 不做字段级 sourceMap。
Runtime Package v1 不做复杂 provenance。
Runtime Package v1 不做 cooked binary。
Runtime Package v1 不做运行时 rule compile。
```

来源文档：

```text
07-Build-Export-Pipeline.md
17-Runtime-FrameLoop.md
18-长期主义实现路径.md
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
39-Rust-Runtime-ECS路线.md
施工文档/已完成/42-当前可自动化施工总清单.md
```

## 当前可以自动化实现的任务

### 任务 1：Runtime Package v1 数据模型

目标：

```text
定义 Runtime Package v1 的 TypeScript 数据结构。
定义 runtime-package.v1 / runtime-scene.v1 / runtime-entity.v1 / runtime-asset-manifest.v1 / runtime-rule-manifest.v1。
提供 normalize / hash / validate 纯函数。
```

建议新增：

```text
src/runtime-package/runtimePackageModel.ts
scripts/test-runtime-package-model.cjs
```

建议 npm script：

```json
"test:runtimepackage": "node scripts/test-runtime-package-model.cjs"
```

第一版结构：

```text
RuntimePackage
  schemaVersion = runtime-package.v1
  packageMode = debug-readable
  project.name
  project.version
  activeSceneId
  scenes[]
  assets
  rules
  contentHash

RuntimeScene
  schemaVersion = runtime-scene.v1
  id
  name
  gravity
  background
  skyColor
  entities[]

RuntimeEntity
  schemaVersion = runtime-entity.v1
  id
  name
  kind
  enabled
  parentId
  siblingOrder
  transform
  mesh?
  camera?
  light?

RuntimeTransform
  localPosition
  localRotation
  localScale

RuntimeMesh
  assetRef
  materialRef?
  visible
  layer

RuntimeAssetManifest
  schemaVersion = runtime-asset-manifest.v1
  assets[]

RuntimeRuleManifest
  schemaVersion = runtime-rule-manifest.v1
  mode = none
  rules = []
  modules = []
```

字段规则：

```text
大部分字段尽量继承 Project Schema 命名。
Transform 例外，Runtime Package v1 提前使用 localPosition / localRotation / localScale。
Project transform.position -> Runtime transform.localPosition。
Project transform.rotation -> Runtime transform.localRotation。
Project transform.scale -> Runtime transform.localScale。
Runtime Package Entity 渲染字段固定使用 mesh.assetRef / mesh.materialRef。
RenderExtract / RenderCommand / RenderSceneState 才解析为渲染资源句柄。
```

验证规则：

```text
activeSceneId 必须存在。
scene id 不可重复。
entity id 在同一 scene 内不可重复。
parentId 必须为 null 或指向同 scene entity。
parent 不能形成循环。
transform 必须存在。
localPosition / localRotation / localScale 必须是有限 number。
mesh.assetRef 必须能在 asset manifest 中找到。
mesh.materialRef 如果存在，也必须能在 asset manifest 中找到。
asset id 不可重复。
asset state 必须为 available / missing / virtual。
rule manifest 第一版必须存在，mode 必须为 none。
```

测试必须覆盖：

```text
valid empty runtime package passes
valid hierarchy + renderable package passes
invalid missing active scene fails
invalid duplicate entity id fails
invalid missing parent fails
invalid parent cycle fails
invalid mesh assetRef fails
hash ignores object key order
normalize sorts scenes / entities / assets deterministically
```

边界：

```text
不接真实 Rust Runtime。
不接 RuntimeBackend。
不写文件系统。
不生成真实 cooked binary。
不改变 Project Schema。
```

### 任务 2：Project -> Runtime Package v1 生成器

目标：

```text
从现有 GameProject / Project Schema 数据生成 Runtime Package v1。
只做 Normalize / Strip / Validate。
```

建议新增：

```text
src/runtime-package/runtimePackageBuilder.ts
scripts/test-runtime-package-builder.cjs
```

建议 npm script：

```json
"test:runtimepackagebuilder": "node scripts/test-runtime-package-builder.cjs"
```

生成规则：

```text
Project name / version / activeSceneId 保留。
Scene id / name / gravity / background / skyColor 保留。
Entity id / name / kind 保留。
Entity enabled 默认 true。
Entity parentId 默认 null。
Entity siblingOrder 默认 0。
Project transform.position 转 Runtime transform.localPosition。
Project transform.rotation 转 Runtime transform.localRotation。
Project transform.scale 转 Runtime transform.localScale。
Project mesh.assetRef / materialRef 保留为 Runtime mesh.assetRef / materialRef。
Project assets 转 RuntimeAssetManifest.assets。
rules manifest 第一版固定为空。
```

Strip 规则：

```text
移除 patchHistory。
移除 selection。
移除 panelLayout。
移除 inspectorState。
移除 consoleLogs。
移除 aiConversation。
移除 previewCameraEditorState。
移除任何明确 editor-only 字段。
```

测试必须覆盖：

```text
starterProject can build runtime package
shooterProject can build runtime package
editor-only fields do not affect runtime package hash
Project transform fields become local transform fields
Runtime package uses mesh.assetRef, not mesh.meshRef
missing asset reference produces validation error
```

边界：

```text
不做复杂 Prefab 展开。
不做 Component lowering。
不做字段级 sourceMap。
不做真实 Asset Cook。
不做 Rule Compile。
```

### 任务 3：Runtime Package v1 文件写出脚本

目标：

```text
提供一个 Node 脚本，把 Project JSON 转成 debug-readable Runtime Package 目录。
该脚本用于测试 Build Graph 未来的 write_runtime_package 阶段。
```

建议新增：

```text
scripts/runtime-package-export.cjs
scripts/test-runtime-package-export.cjs
```

建议 npm script：

```json
"test:runtimepackageexport": "node scripts/test-runtime-package-export.cjs"
```

输出结构：

```text
runtime-package/
  manifest.json
  scenes/
    <scene-id>.json
  assets/
    asset-manifest.json
  rules/
    rule-manifest.json
```

第一版不强制输出：

```text
golden/
```

原因：

```text
Golden Scenario 可以先作为测试 fixture 生成，不必混入所有导出包。
后续 Rust Runtime golden runner 再决定是否把 golden scenario 放入 package。
```

脚本输入：

```powershell
node scripts\runtime-package-export.cjs <project.json> <output-dir>
```

测试必须覆盖：

```text
export writes manifest.json
export writes one scene file per scene
export writes assets/asset-manifest.json
export writes rules/rule-manifest.json
export rejects invalid project
export rejects path outside intended output root if output root policy is added
```

边界：

```text
只写指定 output-dir。
不删除用户资源。
不移动源文件。
不复制真实资产二进制。
不扫描整个项目目录。
不接平台打包。
```

### 任务 4：Build Graph Core 接入 Runtime Package Report-only 阶段

目标：

```text
在现有 Build Graph Core 中加入 cook-project-data / write-runtime-package 的 report-only 阶段。
让 Windows / Web / Android / iOS 导出报告中能看到 Runtime Package v1 生成计划或输出路径。
```

建议修改：

```text
scripts/build-graph-core.cjs
scripts/export-game-win.cjs
scripts/export-game-web.cjs
scripts/export-game-android.cjs
scripts/export-game-ios.cjs
scripts/test-build-graph-core.cjs
```

建议输出：

```text
runtime-package-report.json
runtime-package-report.v1
```

报告字段：

```text
schemaVersion
packageMode
outputDir
sceneCount
entityCount
assetCount
ruleMode
contentHash
warnings[]
```

第一版可以只做 report-only：

```text
不要求平台导出必须携带完整 runtime-package 目录。
不要求 Electron 导出 exe 立刻使用 runtime-package。
只要 Build Graph 能生成报告和可解释路径。
```

如果实现任务 3 已经完成，可以升级为真实写出 Runtime Package 目录。

测试必须覆盖：

```text
Build Graph report includes runtime package stage
runtime-package-report.json has schemaVersion runtime-package-report.v1
invalid project stops before runtime package stage or records failed stage
Windows/Web/Android/iOS export manifest references runtime-package-report.json
```

边界：

```text
不改平台打包规则。
不让当前 TypeScript Runtime 读取 Runtime Package。
不接 Rust Runtime。
```

### 任务 5：Golden Scenario 与 Runtime Package v1 关系补强

目标：

```text
让 Golden Scenario fixture 可以引用 Runtime Package v1 fixture。
第一版只做数据层，不跑 Rust Runtime。
```

建议修改：

```text
src/testing/goldenScenario.ts
scripts/test-golden-scenario.cjs
```

允许新增：

```text
src/testing/runtimePackageFixtures.ts
scripts/test-runtime-package-golden-fixture.cjs
```

规则：

```text
Golden Scenario 测 Runtime Package，而不是编辑器内存 Project Object。
Golden Scenario expected 可以继续包含 entity / component / trace / renderSnapshot。
第一版可在 fixture 中内联 RuntimePackage，也可以引用 manifestPath。
```

测试必须覆盖：

```text
empty_scene_load fixture has runtime package
single_entity_transform fixture checks transform.localPosition.x
renderable_snapshot fixture checks mesh.assetRef -> renderSnapshot.meshRef
fixed_tick_10_frames fixture remains deterministic
```

边界：

```text
不跑 Rust Runtime。
不做 TypeScript Runtime vs Rust Runtime 等价测试。
不改变 ActionSnapshot 数据结构。
```

## 推荐执行顺序

建议按以下顺序自动化实现：

```text
1. Runtime Package v1 数据模型
2. Project -> Runtime Package v1 生成器
3. Runtime Package v1 文件写出脚本
4. Golden Scenario 与 Runtime Package v1 关系补强
5. Build Graph Core 接入 Runtime Package Report-only 阶段
```

如果只做最稳的一批：

```text
1. Runtime Package v1 数据模型
2. Project -> Runtime Package v1 生成器
3. Runtime Package v1 文件写出脚本
```

理由：

```text
前三项是纯数据 / 纯函数 / 受控文件写出。
不碰 Rust Runtime。
不碰编辑器 UI。
不碰平台 SDK。
不改变现有导出行为。
```

## 必须暂停的情况

实现过程中遇到以下情况必须停止，回到架构讨论：

```text
需要修改 Project Schema 核心字段。
需要把 Entity 组件统一改成 components object。
需要决定 Prefab / Overrides 的 runtime 展开规则。
需要修改 RuntimeBackend 接口。
需要让 TypeScript Runtime 改为读取 Runtime Package。
需要接真实 Rust Runtime。
需要接真实 IR -> Rust AOT。
需要接真实 Asset Cook / binary cooked data。
需要让脚本扫描整个项目目录。
需要自动删除、移动、替换用户资源。
```

## 验证命令

每个任务完成后至少执行对应测试和 build。

建议总验证：

```powershell
npm.cmd run test:runtimepackage
npm.cmd run test:runtimepackagebuilder
npm.cmd run test:runtimepackageexport
npm.cmd run test:goldenscenario
npm.cmd run test:buildgraphcore
npm.cmd run build
```

如果任务没有实现某个测试脚本，则不要运行对应命令。

## 文档更新要求

实现完成后更新：

```text
07-Build-Export-Pipeline.md
18-长期主义实现路径.md
38-Rust-Native-Runtime-MVP与TypeScript退役规则.md
施工文档/已完成/43-当前可自动化施工文档-Runtime-Package-v1.md
阶段完成记录/<日期>-Runtime-Package-v1/
```

如果实现结果推翻了字段规则，必须先回到讨论，不允许在代码里偷偷改方向。

## 当前结论

当前可以自动化实现：

```text
Runtime Package v1 纯数据层。
Project -> Runtime Package normalize / strip / validate。
Debug-readable Runtime Package 文件写出。
Runtime Package 与 Golden Scenario 的数据关系。
Build Graph 中 report-only runtime package 阶段。
```

当前不应自动化实现：

```text
真实 Rust Native Runtime。
Native Editor Host MVP。
TypeScript Runtime 改读 Runtime Package。
真实 cooked binary。
真实 IR -> Rust AOT。
真实平台 SDK 打包。
真实热更包。
```

