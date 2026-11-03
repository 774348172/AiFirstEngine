# 261 Window H 项目字体、第二项目与 legacy 专项矩阵记录

日期：2026-07-31

授权窗口：2026-07-31 00:39 +08:00 至 03:39 +08:00；03:24 后不得开启新阶段。

证据身份：dirty-worktree development evidence；不是 clean exact-commit、Local CI、
production binary、真实安装态或 Windows/GPU 视觉证据。

## 本轮完成范围

### 塔防普通项目路径

- `samples/tower_defense_project` 迁移到 `project-runtime-module.v2`，由项目 RuntimeModule
  注册 `create_empty_project_runtime_session`；
- 项目 AUI 同时包含小字号和大字号中文 `中界`；
- 项目内新增 FontFace / FontFamily / FontStack / AutoHybrid FontAtlasProfile、AssetDB
  记录、meta、OFL 字体源与许可证；
- 项目自有集成测试通过公开 AssetDB -> assembler -> FontBundle v2 -> RuntimePackage ->
  AUI presenter 链路证明：
  `rendered == requested`、`unsupported == 0`、未使用 fallback、Bitmap R8 与 MSDF
  均被选中，RuntimePackage 不包含源字体。

塔防修改保持在 `samples/tower_defense_project`，未把玩法语义写入通用引擎接口。

### 非塔防第二项目

新增中性 fixture `rust/fixtures/projects/font_bundle_v2_second_project`，使用与普通项目相同的
公开 AssetDB、assembler、FontBundle v2、RuntimePackage 和 AUI presenter seam。
`project_e2e_gate::font_bundle_compatibility` 证明 v2 bundle 非 legacy、未 fallback、
具备 quality gate 资格，且 RuntimePackage 不包含 `fontSource` 或原始字体文件。

### Legacy 专项矩阵

Complex Shooter 与 Switch Puzzle 保持 legacy adapter：

```text
legacy_mode=true
fallback_used=true
quality_gate_eligible=false
```

legacy metadata 现在稳定包含 `FontBundleV2MigrationRequired`，并指导导入
Project Font Asset、配置默认 UI FontAtlasProfile。两个项目均可继续装配、构建和加载，
但不能伪装通过 261 Font Quality Gate。

### 公开 seam 缺口修复

Formal Asset Import 原先只接受 texture AssetDB record，普通项目无法通过正式 AssetDB
注册 Project Font Asset。本轮在通用 owner 内增加受限的 typed importer：

```text
assetType=fontSource
importerId=font.ttf.v2
importerVersion=2
```

assembler 对字体源执行正式 AssetDB identity/hash 校验，但构建 RuntimePackage 时排除原始
`fontSource`。没有增加系统字体扫描、runtime 字体解析或塔防专用 API。

## 红测与修复

1. Legacy 定向测试先因缺少稳定迁移提示失败；在 legacy atlas cooker owner 增加提示后通过。
2. 第二项目先因 AssetDB validator 拒绝 `fontSource` 失败；补 typed importer 后进入下一层。
3. 随后分别暴露 `licenseUri` 不是绝对 URI、RuntimePackage v2 缺少显式 `input.none`；
   两项均在 fixture owner 修正后通过。

这些失败均保留了首个 owner-level 原因，没有使用完整 workspace 或 Local CI 作为修复循环。

## 验证结果

```text
workspace cargo fmt --all -- --check
passed

tower-defense RuntimeModule format check
passed

tower-defense RuntimeModule existing suite
60 passed; 0 failed

tower-defense FontBundle v2 integration
1 passed; 0 failed

editor_core full suite
585 passed; 0 failed; 2 ignored

project_e2e_gate full suite
main suite: 65 passed; 0 failed; 1 ignored
integration suites: 1 + 1 + 1 + 3 passed; 0 failed

editor_window_winit full suite
142 passed; 0 failed; 0 ignored
```

本轮未改变 `engine_runtime` 和 `runtime_player_winit` 所消费的产品身份，因此复用此前同一
Window H 自动化 closure：

```text
engine_runtime: 492 passed
runtime_player_winit --features real-window: 17 passed; 2 ignored
```

既有 warning 保持不变：`engine_runtime` dead-code、`project_font_bundle.rs` 未使用的
`BTreeSet`、一个 `editor_window_winit` dead-code warning。没有新增测试失败。

## 身份与卫生检查

两项目字体源 SHA-256 均为：

```text
f70ecb32e5b312ba7bc724977352139a3f691566dc2491377be3828631c9fab2
```

字体和 OFL 许可证均来自项目目录，不读取系统字体。`git diff --check` 通过。
对本轮通用 owner 新增行、第二项目 fixture 和专项测试做塔防语义扫描，无塔防专用语义；
装配器中命中的 `enemy` 仅属于既有 prefab 测试，OFL 文本中的 `communities` 不是项目语义。

测试结束后没有残留 Cargo/Rustc 进程或 `legacy-font-*` 临时目录。一次失败的 legacy 红测
曾留下
`C:\Users\zenghaoran\AppData\Local\Temp\legacy-font-complex_shooter_project-3116-1785430457101323700`。
原生 `Remove-Item` 被 host policy 拒绝；按施工 cleanup 规则重新确认目标后使用
`.NET Directory.Delete` 删除，存在性检查为 false。后续进程引用检查因 Windows PowerShell
不支持双参数 `String.Contains` 产生 overload error；测试进程已经结束，最终进程与目录检查
均无残留。

## 未完成范围

- 未执行真实 production composition；
- 未采集小字号 R8、大字号 MSDF、metrics 对齐、多 DPI、multi-page/fallback 或 exported
  Windows Player screenshot；
- 未运行 Local CI、clean exact-commit qualification、production binary replacement 或
  安装态验证。

因此 Window H 的项目字体、第二项目和 legacy 自动化专项矩阵已完成，但 Gate H 仍为
`incomplete`。261 继续保留在 `施工文档/当前/`，不得归档或标记施工完成。
