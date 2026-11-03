# 237-当前可自动化施工文档-Release Package Polish / Metadata / Icon / Layout v1

> 状态：当前唯一可执行施工文档；施工文档自审通过；尚未开始 Gate A。  
> 生成日期：2026-07-11。  
> 正式方案：`../../237-Release-Package-Polish-Metadata-Icon-Layout-v1方案.md`。  
> 方案选择：用户已确认 B-min+。  
> 前置完成记录：`../../阶段完成记录/2026-07-10-Save-Reload-Rebuild-Consistency-Gate-v1/00-总览.md`。  
> 施工规则：一次只执行一个 Gate；当前 Gate 的实现、定向测试和证据全部通过后，才能进入下一 Gate。

## 1. 施工目标

把现有 Windows dev export：

```text
Game.exe
data/runtime_package/
reports/
package-manifest.json
```

收敛为 B-min+ Windows portable release package：

```text
Build/Windows/x86_64/release/ComplexShooter/
  ComplexShooter.exe
  package-manifest.json
  data/
    runtime_package/
      ...
```

最终必须形成以下真实闭环：

```text
BuildProfile v2
+ application metadata
+ icon AssetRef
  -> ReleasePackagePlan
  -> 唯一 ProjectRuntimePackageAssembler
  -> verified RuntimePackage
  -> copied Runtime executable staging copy
  -> Windows VERSIONINFO + multi-size icon stamp/readback
  -> relative release-package-manifest.v1
  -> inventory/hash/formal load/process verification
  -> shared safe directory publish
  -> ReleasePackageReport
  -> Build panel / Report Panel build.release_package
```

用户双击 `ComplexShooter.exe` 时必须在无参数、有效 release manifest 条件下进入 native player；用户运行默认 report Off，不要求发布目录可写。

## 2. 固定边界

### 2.1 本轮必须交付

```text
BuildProfile v2 release schema、validation 和 windows.dev v1 兼容。
application display/executable/company/description/version/copyright/icon AssetRef。
Windows PE VERSIONINFO、StringFileInfo、ICON/GROUP_ICON 写入与读回验证。
release-package-manifest.v1、entrypoint、file roles、relative paths 和 canonical payload hash。
portable directory staging、验证、single-writer publish、rollback 和 last-good 保留。
无参数 packaged release 启动、Runtime report Off、manifest-driven verifier。
Build Release Package command/service/UI、ReleasePackageReport、build.release_package provider。
复杂打飞机 release package E2E 与完整回归。
```

### 2.2 本轮禁止扩入

```text
Bootstrap Launcher 或第二进程代理。
ZIP、installer、MSIX、Steam/Store package。
代码签名、证书、timestamp server。
自动更新、delta package、资源加密、DRM。
PDB/symbol server/crash upload。
通用跨平台 Packaging Framework。
把 RuntimePackage 改成单文件 archive。
第二套 ProjectRuntimePackageAssembler、RuntimePackage manifest 或 payload publisher。
```

未来方案 C 只保留 manifest 合同升级点：

```text
B-min+:
  ComplexShooter.exe roles=[entrypoint,runtime]

C:
  ComplexShooter.exe roles=[entrypoint,launcher]
  bin/ai_engine_runtime.exe roles=[runtime]
```

本轮不得实现 Launcher interface、child lifecycle、exit-code proxy 或多 Runtime router。

## 3. 施工前基线与审查输入

### 3.1 已确认代码基线

```text
rust/crates/editor_core/src/project_runtime_package_assembler.rs
  BUILD_PROFILE_SCHEMA_VERSION 仍为 build-profile.v1。
  BuildProfile 仍只有 dev/runtime package/gate 字段。

rust/crates/editor_core/src/desktop_export.rs
  DesktopExportRequest::windows_dev 是现有 dev 入口。
  stage_player_executable 固定输出 Game.exe。
  DesktopPackageManifest / DesktopExportReport 保留开发兼容语义。

rust/crates/runtime_cli/src/lib.rs
  普通无参数调用仍返回 missing arguments。
  native player 当前默认解析/写入 reports 路径。

rust/crates/runtime_cli/src/exported_player_verification.rs
  resolve_verification_paths 固定寻找 Game.exe。

rust/crates/editor_core/src/report_panel.rs
  已有 build.export、build.and_run、validation.save_reload_rebuild provider。

rust/crates/editor_core/src/asset_browser.rs
  已有 Texture/Sprite AssetRef picker、类型过滤和结构化提交链路。

rust/crates/engine_runtime/src/canonical_digest.rs
rust/crates/engine_runtime/src/runtime_package_path.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs
  236 已提供 canonical digest、safe path、single-writer staging/publish、rollback 和 formal load。
  publish 实现当前仍嵌在 RuntimePackageBuilder 内部。
```

### 3.2 外部审查结论

生成本文档前已检索当前审查目录，没有发现适用于系统 237、Release Package、Metadata/Icon/Layout 的外部审查文档。

结论：

```text
没有外部“必须修改”或“施工约束”输入。
正式 237 方案无需因外部审查修改。
本文档只吸收正式方案自审和 236 已完成合同，不机械套用其它系统审查。
```

如果施工开始后用户提供新的 237 审查文档，必须先暂停当前 Gate，分类审查结论；存在“必须修改”时先更新正式方案和本文档，再继续施工。

## 4. 共享合同与模块归属

### 4.1 RuntimePackage 真相不变

```text
ProjectRuntimePackageAssembler
  -> RuntimePackageBuildInput
  -> RuntimePackageBuilder
  -> runtime-package.v1
```

237 不扫描项目目录拼第二份 runtime payload，不从 Editor 内存对象直接写最终包，也不改变 RuntimePackage contentHash/schema。

### 4.2 Release manifest 共享 schema

新增窄模块：

```text
rust/crates/engine_runtime/src/release_package_manifest.rs
```

只承载 Runtime 与 Builder 都要读取的版本化 DTO、file role、relative path validation、entrypoint/runtimePackage 解析和 canonical payload hash 合同。它不负责编排构建、不读取项目源目录、不写 Editor report，也不演化成通用跨平台 packaging layer。

```text
rust/crates/editor_core/src/release_package.rs
```

负责 Windows B-min+ 的 plan、staging、resource stamping 调用、outer manifest/inventory、verification、publish 和 ReleasePackageReport。Runtime CLI 不得反向依赖 `editor_core`，两侧不得复制 manifest struct 或 validator。

### 4.3 共享安全目录发布原语

从 236 的 `runtime_package_builder.rs` 抽取一个共享深模块：

```text
rust/crates/engine_runtime/src/atomic_directory_publish.rs
```

职责仅限：

```text
same-parent/same-volume staging 与 backup ownership。
每个 final output 的 single-writer guard。
orphan recovery。
pre/post publish validation callback。
final -> backup、staging -> final、last-good rollback。
owned-path cleanup containment。
结构化 stage/path/error/next action。
确定性 fault injection 测试。
```

`RuntimePackageBuilder` 必须改为复用该原语并保持 236 所有 publish 测试通过；release outer directory 再复用同一个原语。禁止复制 rename/backup/lock/rollback 代码。

## 5. Gate A - BuildProfile v2 / Release Plan

### 涉及文件

```text
rust/crates/editor_core/src/project_runtime_package_assembler.rs
rust/crates/editor_core/src/release_package.rs                       # 新增
rust/crates/editor_core/src/lib.rs
samples/complex_shooter_project/BuildProfiles/windows.release.json   # 新增
```

### 任务

```text
1. 保留 build-profile.v1 的 windows.dev typed load 与现有 DesktopExport 行为。
2. 增加 build-profile.v2 application/release/architecture schema；release build 只接受完整 v2。
3. application 至少包含 displayName、executableName、companyName、fileDescription、displayVersion、Windows file/product 四段版本、copyright 和 icon AssetRef。
4. release v1 固定 layout=portable-directory-v1、includeReports=false；includeDebugSymbols 默认 false。
5. executableName 禁止绝对路径、分隔符、`.`/`..`、尾随点/空格、Windows 保留设备名和大小写折叠 collision。
6. Windows file/product version 各为四个 0..65535 整数；不得从 displayVersion 猜测。
7. projectName 只能用于创建 release profile 的初始建议，不在每次 build 隐式覆盖 release identity。
8. 生成纯数据 ReleasePackagePlan：profile/target/architecture/application/output layout/icon AssetRef；此阶段不写文件、不改 exe。
9. v1/v2 unknown field/default/invalid matrix 必须有结构化 diagnostic 和 next action。
10. complex shooter 新增稳定 windows.release.json；windows.dev.json byte 内容与 v1 行为不变。
```

### Gate 测试

在 `G:\gameEngin\rust` 执行：

```powershell
cargo test -p editor_core build_profile_v2
cargo test -p editor_core project_runtime_package_assembler
cargo test -p editor_core desktop_export
```

### 通过条件

```text
windows.dev.json 继续按 build-profile.v1 装配并通过现有 desktop_export 测试。
windows.release.json 只能通过 v2 完整 schema 进入 ReleasePackagePlan。
invalid executable/version/path/architecture/release flags 全部 fail closed。
诊断定位 profile path + field + reason + next action。
Gate A 不生成 release directory，不引入 PE 或 UI 施工。
```

### 失败即停

任一 v1 兼容测试、v2 invalid matrix 或 plan deterministic test 失败时，不进入 Gate B；不得通过放宽 release validation 或把 v1 静默升级为 release 绕过失败。

### Gate A 施工记录（2026-07-11）

状态：`已完成`。

```text
新增 build-profile.v2 application/release/architecture typed schema，并保留 build-profile.v1 dev 反序列化与序列化形状。
新增集中式 BuildProfile validation issue，覆盖 schema、target、runtime mode、release identity、Windows executable name、icon AssetRef 和 portable layout。
新增纯数据 ReleasePackagePlan 与复杂打飞机 windows.release.json；本 Gate 不写发布目录。
BuildRecipe digest 改为记录实际 profile.schemaVersion，避免 v1 输入被最新 v2 常量改变 digest。
```

```powershell
cargo test -p editor_core build_profile_v2                 # 2 passed
cargo test -p editor_core project_runtime_package_assembler # 8 passed
cargo test -p editor_core desktop_export                    # 5 passed
```

## 6. Gate B - Icon Asset / Windows PE Resource Stamping

### 涉及文件

```text
rust/Cargo.toml
rust/Cargo.lock
rust/crates/editor_core/Cargo.toml
rust/crates/editor_core/src/windows_executable_resources.rs           # 新增
rust/crates/editor_core/src/release_package.rs
rust/crates/editor_core/src/lib.rs
samples/complex_shooter_project/Assets/app-icon.asset                 # 新增
samples/complex_shooter_project/Assets/Images/app-icon.png            # 新增，正方形 >=256x256
samples/complex_shooter_project/BuildProfiles/windows.release.json
```

### 任务

```text
1. 评估并固定 editpe = 0.2.3；记录 BSD-2-Clause、transitive dependencies、Windows PE resource read/write 能力和 Cargo.lock 结果。
2. 若 editpe 0.2.3 无法保留无关资源、写入 VERSIONINFO 或 ICON/GROUP_ICON，停止 Gate 并更新正式方案；禁止临时手写 PE parser 或切到外部 rcedit/rc.exe。
3. icon 必须从 BuildProfile AssetRef -> typed texture asset descriptor -> source PNG/ICO 解析；禁止长期 raw path-only icon。
4. PNG 必须是正方形且至少 256x256；生成 16/32/48/64/128/256 尺寸，缩放过程确定性且保留 alpha。
5. 新模块只接收已校验 metadata、resolved icon bytes 和 staging executable path。
6. 写入 ProductName、CompanyName、FileDescription、ProductVersion/FileVersion string、fixed four-part versions、copyright、ICON/GROUP_ICON。
7. 只修改 staging copy；施工测试必须证明 engine runtime template/source binary hash 不变。
8. stamp 后立即用同一 library 的 readback API 读取并比较全部 metadata、fixed versions 和 icon sizes。
9. fixture 必须是测试 owned executable copy；已有 manifest 和至少一个无关资源不得丢失。
10. 形成 release_resource_stamp_failed / release_resource_readback_mismatch 等结构化结果，不把 library error 当唯一用户信息。
```

### Gate 测试

在 `G:\gameEngin\rust` 执行：

```powershell
cargo test -p editor_core windows_executable_resources
cargo test -p editor_core release_icon_asset
cargo test -p editor_core desktop_export
cargo metadata --format-version 1 --locked
```

### 通过条件

```text
依赖固定在 Cargo.lock，license/provenance 记录可审查。
AssetRef missing/dangling/wrong type/decode failure/non-square/too-small 全部阻断 release。
owned PE copy 的 metadata、fixed version、GROUP_ICON 尺寸读回与 BuildProfile 完全一致。
无关 PE 资源保留，stamped exe 仍可被 Windows 加载。
源 runtime template hash 前后相同。
现有 DesktopExport Game.exe staging 不被资源写入副作用改变。
```

### 失败即停

依赖能力、license、resource preservation、readback 或 source-template immutability 任一不满足时停止，不进入 Gate C；不得用“Explorer 看起来有图标”代替 readback。

### Gate B 施工记录（2026-07-11）

状态：`已完成`。

```text
固定 editpe = 0.2.3、BSD-2-Clause、default-features=false、features=[std]；新增锁定依赖仅 debug-ignore 1.0.5 与 foldhash 0.2.0。
新增 typed texture-asset.v1 icon AssetRef resolver；complex shooter 新增 app-icon.asset 与 1024x1024 PNG 品牌源图。
使用既有 png crate 确定性生成 16/32/48/64/128/256 六尺寸 ICO，避免 editpe 内建集合缺少 64px。
新增 VERSIONINFO/StringFileInfo/fixed versions/GROUP_ICON stamp 与强制 readback。
owned PE fixture 预置 manifest 和 RCDATA；stamp 后二者保留，源 template byte-for-byte 不变。
```

```powershell
cargo test -p editor_core windows_executable_resources # 2 passed
cargo test -p editor_core release_icon_asset            # 1 passed
cargo test -p editor_core desktop_export                # 5 passed
cargo metadata --format-version 1 --locked              # passed
```

## 7. Gate C - Release Manifest / Portable Layout / Shared Safe Publish

### 涉及文件

```text
rust/crates/engine_runtime/src/atomic_directory_publish.rs             # 新增，从 236 抽取
rust/crates/engine_runtime/src/release_package_manifest.rs             # 新增共享 schema
rust/crates/engine_runtime/src/runtime_package_builder.rs
rust/crates/engine_runtime/src/canonical_digest.rs
rust/crates/engine_runtime/src/runtime_package_path.rs
rust/crates/engine_runtime/src/lib.rs
rust/crates/editor_core/src/release_package.rs
rust/crates/editor_core/src/lib.rs
```

### 任务

```text
1. 抽取 atomic_directory_publish；RuntimePackageBuilder 先切换到共享实现，保持 236 行为和 diagnostics code 兼容。
2. 定义 release-package-manifest.v1：application、target、launch.userFrameLimit、entrypoint、runtimePackage、runtimeContentHash、releasePayloadHash、files[]；userFrameLimit=null 表示用户运行直到窗口关闭，自动化 verifier 仍显式传有限 --frames。
3. files[] 每项包含 package-relative `/` path、size、sha256、roles；v1 role 至少支持 entrypoint/runtime/launcher/runtime-payload。
4. B-min+ 的品牌 exe 同时具有 entrypoint/runtime role；消费者只依赖 entrypoint/role，不依赖两者永远相同。
5. entrypoint 必须是唯一 entrypoint role 文件；runtimePackage 必须位于 package root；所有 path 使用共享 safe path/collision validator。
6. package-manifest.json 自身不进入 files/releasePayloadHash，避免自引用。
7. releasePayloadHash 对按 path 排序后的 path + file sha256 使用长度分帧 canonical SHA-256；路径、文件内容变化必须敏感，report/timestamp/output absolute path 不得进入。
8. Release assembler 复用唯一 ProjectRuntimePackageAssembler 和 RuntimePackageBuilder，得到已 formal-load 验证的 RuntimePackage；不得复制其 inventory/hash。
9. staging 只包含 stamped entrypoint、package-manifest.json 和 data/runtime_package runtime-required payload；排除 reports、trace、project source、cache 和 debug symbol。
10. outer pre-validation 检查 manifest/schema/path/inventory/hash/PE readback/RuntimePackage formal load；post-publish 从 final 再执行同一核心验证。
11. outer publish 复用 atomic_directory_publish：single writer、orphan recovery、last-good rollback、stale removal、owned cleanup。
12. publish 或 report 失败不得污染 final；ReleasePackageReport 写到 project `.aife`，不写发布目录。
13. 相同输入执行两次必须得到相同 releasePayloadHash；stale sentinel 在成功 publish 后消失。
14. fault injection 覆盖 staging write、pre-verify、rename、post-verify 和 rollback，不能依赖偶发文件权限错误。
```

### Gate 测试

在 `G:\gameEngin\rust` 执行：

```powershell
cargo test -p engine_runtime atomic_directory_publish
cargo test -p engine_runtime runtime_package_publish
cargo test -p engine_runtime runtime_package_path
cargo test -p engine_runtime release_package_manifest
cargo test -p editor_core release_package
cargo test -p editor_core desktop_export
```

### 通过条件

```text
RuntimePackageBuilder 复用共享 publisher 后，236 publish/path/digest/formal-load 回归保持通过。
release manifest 不包含 packageDir/projectRoot/reportsDir/stagingDir 等本机绝对路径。
entrypoint/file roles/runtimePackage/inventory/hash 可以从 final 独立验证。
Windows 大小写、尾随点/空格、保留名、重复 claim、absolute/traversal/symlink escape 全部 fail closed。
并发 publisher 只有一个成功，测试有硬超时且不死锁。
失败保留或恢复 last-good；成功 final 不含 stale payload、reports 或 source。
没有第二套 RuntimePackage assembler、content hash 或 publish 实现。
```

### 失败即停

共享 publisher 抽取导致任何 236 测试回归，或 outer publish 无法证明 rollback/containment/inventory 一致时停止，不进入 Gate D。不得复制旧函数恢复通过。

### Gate C 施工记录（2026-07-11）

状态：`已完成`。

```text
从 RuntimePackageBuilder 抽取 atomic_directory_publish：same-parent staging/backup、single-writer、orphan recovery、pre/post validation、rollback、owned cleanup 和 fault injection 由 RuntimePackage 与 release outer directory 共用。
新增 release-package-manifest.v1 共享 schema、entrypoint/runtime/launcher/runtime-payload roles、relative path/collision validator 和 canonical framed releasePayloadHash。
ReleasePackageBuilder 复用唯一 ProjectRuntimePackageAssembler/RuntimePackageBuilder，在 outer staging 内完成 RuntimePackage、stamped entrypoint、manifest、inventory/hash 与双重验证。
release final 排除 RuntimePackage reports、Editor report、source 和 stale payload；报告写入 package 外 owned path。
两次 clean build releasePayloadHash 一致；PE template byte hash 不变；staging/final 均通过 PE readback 与正式 RuntimePackage loader。
```

```powershell
cargo test -p engine_runtime atomic_directory_publish # 2 passed
cargo test -p engine_runtime runtime_package_publish  # 2 passed
cargo test -p engine_runtime runtime_package_path     # 6 passed
cargo test -p engine_runtime release_package_manifest # 2 passed
cargo test -p editor_core release_package             # 4 passed
cargo test -p editor_core desktop_export              # 5 passed
```

## 8. Gate D - No-arg Packaged Entrypoint / Manifest-driven Verifier

### 涉及文件

```text
rust/crates/runtime_cli/src/lib.rs
rust/crates/runtime_cli/src/exported_player_verification.rs
rust/crates/runtime_cli/tests/
rust/crates/engine_runtime/src/release_package_manifest.rs
rust/crates/editor_core/src/release_package.rs
rust/crates/editor_core/src/desktop_export.rs
```

### 任务

```text
1. CLI 无参数时只检查 current_exe parent/package-manifest.json；必须是有效 release-package-manifest.v1、entrypoint 指向 current exe、runtimePackage containment 通过，才进入 packaged release mode。
2. 缺失或无效 release manifest 时保持既有 CLI usage/missing arguments 行为；不得扫描 cwd 猜 package。
3. packaged release mode 从 manifest.runtimePackage 启动 native player，cwd 固定 package root；launch.userFrameLimit=null 映射为无限用户会话，禁止回退 CLI 默认 1 帧。
4. 用户无参数模式 report Off：不创建 reports、log、cache 或 JSON，不因 package root 只读而失败。
5. Editor/test 显式 `--report` 时才生成 Summary/Trace，并允许 report 位于 package 外的 project `.aife` 或 owned temp root。
6. exported verifier 先识别 release manifest，并从 manifest.entrypoint/runtimePackage 获取路径；release 分支禁止回退 Game.exe。
7. verifier 显式传 `--package <resolved manifest.runtimePackage>`，不依赖 exe_dir 猜测；process cwd 为 package root。
8. desktop-package-manifest.v1 + Game.exe 只保留明确 legacy/dev 分支，现有 DesktopExport/Build & Run 测试必须继续通过。
9. process report 记录 manifest schema、entrypoint role、resolved relative paths、cwd、exit code/reason；Summary 不暴露本机绝对路径。
10. Windows process test 对 release entrypoint 执行无参数启动并有硬超时；还要执行显式 headless gate，正式 loader 成功且 exit 0。
11. 测试证明用户无参数模式没有在发布目录产生 report 文件。
```

### Gate 测试

在 `G:\gameEngin\rust` 执行：

```powershell
cargo test -p runtime_cli packaged_entrypoint
cargo test -p runtime_cli release_package_verification
cargo test -p runtime_cli exported_player
cargo test -p editor_core build_and_run
cargo test -p editor_core desktop_export
```

### 通过条件

```text
有效 release package 无参数启动进入 packaged native player；无效普通 CLI 环境仍输出 usage。
release verifier 全程由 manifest 驱动，不拼接 Game.exe。
用户模式在只读 package root 下不尝试写 report。
显式验证模式通过正式 RuntimePackage loader，process exit 0，cwd/entrypoint/runtimePackage 证据完整。
旧 dev package、Game.exe、Build & Run 兼容路径仍通过，且代码分支明确可测试。
所有 child process 都有硬超时、wait/kill/handle cleanup，不遗留后台进程。
```

### 失败即停

无参数模式发生 cwd 猜测、发布目录写入、release Game.exe fallback，或 legacy dev 回归时停止，不进入 Gate E。

### Gate D 施工记录（2026-07-11）

状态：`已完成`。

```text
runtime_cli 无参数入口只从 current_exe parent 读取 release-package-manifest.v1；验证 current exe/entrypoint role/runtimePackage containment/runtime manifest 后进入 packaged native-player mode。
release launch.userFrameLimit=null 映射为 u64::MAX，用户 packaged session 不再继承 CLI 默认 1 帧；用户模式 report Off，发布目录不创建 reports/log/cache/JSON。
exported verifier 先区分 release 与 legacy desktop manifest；release entrypoint/runtimePackage 完全由 manifest 驱动，child 显式接收 --package，parent/child report 默认写到包外 owned temp root。
desktop-package-manifest.v1 + Game.exe 固定路径仅保留在明确 legacy/dev 分支，原有 DesktopExport 与 Build & Run 行为不变。
新增真实零参数子进程测试：从非 package cwd 启动 branded entrypoint，30 秒硬超时和 kill/wait cleanup 生效，并证明发布目录无 report；默认测试 binary 未启用 real-window 时明确得到 native_host_required，不把该结果伪装为真实窗口成功，真实窗口成功由 Gate E release E2E 继续证明。
```

```powershell
cargo test -p runtime_cli packaged_entrypoint            # 3 passed
cargo test -p runtime_cli release_package_verification   # 2 passed（unit 1 + process 1）
cargo test -p runtime_cli exported_player                # 6 passed（unit 5 + process 1）
cargo test -p editor_core build_and_run                  # 4 passed
cargo test -p editor_core desktop_export                 # 5 passed
```

## 9. Gate E - Editor / Report Panel / Complex Shooter E2E

### 涉及文件

```text
rust/crates/editor_core/src/release_package.rs
rust/crates/editor_core/src/services/build_service.rs
rust/crates/editor_core/src/session.rs
rust/crates/editor_core/src/editor_command.rs
rust/crates/editor_core/src/editor_command_registry.rs
rust/crates/editor_core/src/ui_model_composer.rs
rust/crates/editor_core/src/report_panel.rs
rust/crates/editor_core/src/tests/build_service_tests.rs

rust/crates/editor_ui_model/src/build_export.rs
rust/crates/editor_ui_model/src/command.rs
rust/crates/editor_ui_model/src/workflow_command.rs
rust/crates/editor_ui_model/src/lib.rs

rust/crates/editor_ui_renderer/src/panels/build_export.rs
rust/crates/editor_ui_renderer/src/tests.rs
rust/crates/editor_window_winit/src/command_system.rs
rust/crates/editor_window_winit/src/tests/

rust/crates/project_e2e_gate/src/release_package.rs                 # 新增
rust/crates/project_e2e_gate/src/lib.rs
rust/crates/project_e2e_gate/src/tests.rs

samples/complex_shooter_project/BuildProfiles/windows.release.json
samples/complex_shooter_project/Assets/app-icon.asset
samples/complex_shooter_project/Assets/Images/app-icon.png
```

### 任务

```text
1. Build 面板读取 BuildProfile v2，显示 release identity、architecture、icon AssetRef、portable output preview 和 validation diagnostics。
2. 新增 Build Release Package command/payload/registry/service；UI 只发命令，不直接拼目录、写 manifest 或修改 exe。
3. icon field 复用 235 Asset Browser Picker 的 Texture/Sprite AssetRef、filter、confirm/cancel/transaction 语义。
4. profile 编辑复用既有 command/service/undo/save 边界；保存成功才清 dirty，外部 source revision/hash 变化 fail closed。
5. Build Release Package 与 Build & Run 分离：release 默认完整验证和发布，但不为用户常驻启动游戏。
6. ReleasePackageReport schema 固定 release-package-report.v1，包含 status/reportLevel/projectId/profile/target/architecture/application/runtime/entrypoint/resource/layout/payloadHash/verification/diagnostics/nextAction。
7. report artifact 默认写 `<project>/.aife/reports/release-package/latest.json`，用 atomic_file_replace 提交；失败不覆盖上一份完整 report。
8. EditorSession 只缓存 active-project report；OpenProject/显式 refresh 时读取，compose/frame 不做磁盘 IO；projectId 不匹配显示 NotRun。
9. Report Panel 注册 build.release_package；Off 不收集，Summary 只显示产品名/版本/entrypoint relative path/hash/status/first next action，Trace 才显示绝对 staging/output/process/resource readback 证据。
10. diagnostic 至少覆盖正式方案列出的 profile/identity/version/icon/template/stamp/readback/manifest/path/collision/entrypoint/launch/runtime-load/hash/publish/rollback 错误。
11. complex shooter E2E 必须从样例 typed profile 和 AssetRef 构建到 owned temp final，不修改仓库样例源文件。
12. E2E 验证品牌 exe 名、PE metadata/icon readback、relative manifest、roles、payload hash、formal RuntimePackage load、no-arg launch、explicit verification、report Off 和 stale replacement。
13. E2E 对同一输入两次 clean release build 比较 releasePayloadHash；仓库 sample 前后 byte hash 相同。
14. E2E report 由真实 builder/verifier 结果组成，不手写 expected passed witness。
```

### Gate 测试

在 `G:\gameEngin\rust` 执行：

```powershell
cargo test -p editor_ui_model build_export
cargo test -p editor_core build_service
cargo test -p editor_core report_panel
cargo test -p editor_core release_package
cargo test -p editor_ui_renderer build_export
cargo test -p editor_window_winit build_export
cargo test -p project_e2e_gate release_package
```

### 通过条件

```text
用户可在 Build 产品面查看/编辑 release profile、选择 icon AssetRef 并执行 Build Release Package。
command -> service -> plan -> RuntimePackage -> stamp -> manifest -> verify -> publish 链路唯一且可审查。
Report Panel build.release_package 的 Off/Summary/Trace、active project cache 和绝对路径分档正确。
complex shooter final 目录只含发布 payload，双 build hash 一致，stale payload 消失。
无参数 branded entrypoint 和显式 verifier 都成功；用户模式发布目录无 report。
样例源文件、engine executable template 和 existing dev output 不被测试修改。
新增 filter 命令确实执行至少一个目标测试；0 tests filtered 不算通过。
```

### 失败即停

产品面绕过 service、report frame 读盘、E2E 修改样例、hash 非确定、process 不可回收或任一 release 证据缺失时停止，不进入 Gate F。

### Gate E 施工记录（2026-07-11）

状态：`已完成`。

```text
Build 产品面新增 windows-release typed profile read model，显示 application identity、architecture、icon AssetRef、portable relative output preview 和 validation 状态；Build Release、Pick Icon、Save Profile 均经 UiCommand -> registry -> EditorSession service。
release icon 复用 Asset Browser Texture/Sprite picker、confirm/cancel 和 source hash guard；确认后只改内存 draft，Save Release Profile 再校验外部 source hash 并用 atomic_file_replace 提交，保存成功才清 dirty。
Build Release 与 Build & Run 分离；ReleasePackageBuilder 默认在 outer staging 上运行 manifest-driven headless process gate，失败不 publish，成功报告 process/child exit、frames 和包外 report path。
release-package-report.v1 新增 application/runtime/entrypoint/resource/layout/payloadHash/verification typed evidence；默认 artifact 位于 project .aife，EditorSession 仅在 OpenProject/RefreshReports 读取并校验 active projectId，frame compose 只读 cache。
Report Panel 注册 build.release_package；Off 不收集，Summary 仅显示产品名/版本/relative entrypoint/hash/status/first next action，Trace 才显示 absolute output/manifest/process/resource readback 证据。
E2E 暴露 DesktopExport 曾在 RuntimePackageBuilder 完成后复制 generic asset 的旁路；已把 generic runtime payload bytes 和 digest 收回 RuntimePackageBuildInput -> RuntimePackageBuilder，并删除 DesktopExport post-build copy，font-main 等 payload 由唯一 builder 生产。
共享 atomic_directory_publish 对 Windows process image section 的短暂 access-denied/sharing violation 增加 5 秒有界 rename retry；single-writer、rollback 和 hard bound 保持不变，连续两次真实 E2E 通过。
complex shooter E2E 使用 test-only Runtime binary 调用同一个 runtime_cli::run_from_env（不是 Launcher），真实验证 branded no-arg native window、explicit headless verifier、PE metadata/icon、relative manifest/roles/hash/formal load/report Off/stale replacement/双 build deterministic hash/sample 与 template 不变。
```

```powershell
cargo test -p editor_ui_model build_export       # 2 passed
cargo test -p editor_core build_service           # 10 passed
cargo test -p editor_core report_panel            # 9 passed
cargo test -p editor_core release_package         # 7 passed
cargo test -p editor_ui_renderer build_export     # 1 passed
cargo test -p editor_window_winit build_export    # 2 passed
cargo test -p project_e2e_gate release_package    # 1 passed（连续执行 2 次）
```

附加直接受影响回归：

```powershell
cargo test -p engine_runtime atomic_directory_publish                              # 2 passed
cargo test -p engine_runtime runtime_package_builder_owns_generic_asset_payload_bytes # 1 passed
cargo test -p editor_core project_runtime_package_assembler                        # 8 passed
cargo test -p editor_core desktop_export                                            # 5 passed
cargo fmt --all -- --check                                                          # passed
```

## 10. Gate F - 整体回归、文档、完成记录与归档

### 涉及文件

```text
框架设计/引擎总体架构/237-Release-Package-Polish-Metadata-Icon-Layout-v1方案.md
框架设计/引擎总体架构/00-文档地图.md
框架设计/引擎总体架构/49-当前状态与下一步入口.md
框架设计/引擎总体架构/54-当前可自动化施工总入口.md
框架设计/引擎总体架构/227-复杂打飞机可自由编辑并Windows打包运行-系统讨论优先级.md
框架设计/引擎总体架构/施工文档/README.md
框架设计/引擎总体架构/施工文档/当前/237-当前可自动化施工文档-Release-Package-Polish-Metadata-Icon-Layout-v1.md
框架设计/引擎总体架构/阶段完成记录/README.md
框架设计/引擎总体架构/阶段完成记录/2026-07-11-Release-Package-Polish-Metadata-Icon-Layout-v1/00-总览.md  # 全部通过后新增
```

### 任务

```text
1. 执行格式检查、所有受影响 crate 全量测试和历史重点回归。
2. 汇总 Gate A-E 的实际实现文件、editpe 锁定版本/license、diagnostics、process/publish 证据和 deferred 边界。
3. 确认 final release package、报告和测试临时目录无残留污染，所有 child process/lock 已释放。
4. 只有全量通过后才生成完成记录、更新方案/入口/227，并归档本文档。
5. 若实际实现修正了架构合同，先回填 237 正式方案，再写完成记录；禁止只在完成摘要口头说明。
```

### Gate 测试

在 `G:\gameEngin\rust` 分命令执行：

```powershell
cargo fmt --check
cargo test -p engine_input
cargo test -p editor_ui_model
cargo test -p editor_core
cargo test -p editor_input
cargo test -p editor_ui_renderer
cargo test -p editor_wgpu_renderer
cargo test -p editor_window_winit
cargo test -p engine_runtime
cargo test -p runtime_cli
cargo test -p runtime_player_winit
cargo test -p project_e2e_gate
```

必须额外复核以下历史重点：

```powershell
cargo test -p editor_core desktop_export
cargo test -p editor_core build_and_run
cargo test -p runtime_cli exported_player
cargo test -p engine_runtime runtime_package_publish
cargo test -p engine_runtime runtime_package_path
cargo test -p editor_core report_panel
```

### 测试执行规则

```text
每个命令单独记录结果和耗时，不把多命令最后一个 exit code 当整体结果。
新增 filter 必须确认实际执行目标测试；0 tests filtered 不算通过。
process/concurrency/fault-injection 测试必须有硬超时，完成后确认没有 child/lock/staging 残留。
失败先修当前 Gate，不跨 Gate 堆积问题，不用 ignored 跳过新增 release 必测项。
只修 237 或本轮直接暴露的回归，不整理无关 dirty worktree。
Explorer icon cache 目测只能作为 local-only smoke，不能替代 PE readback/process evidence。
```

### 全部通过后才允许的文档操作

```text
1. 生成 阶段完成记录/2026-07-11-Release-Package-Polish-Metadata-Icon-Layout-v1/00-总览.md。
2. 在完成记录中写入 Gate A-F 实际文件、依赖版本、测试命令/数量、deferred 边界和已知限制。
3. 更新 237 正式方案状态为已施工完成，并回填任何实际设计修正。
4. 更新 00-文档地图.md、49-当前状态与下一步入口.md、54-当前可自动化施工总入口.md。
5. 更新 施工文档/README.md、阶段完成记录/README.md。
6. 更新 227，把 P2-2 标为完成并选择下一讨论入口。
7. 把本文档从 施工文档/当前/ 移到 施工文档/已完成/。
8. 确认 施工文档/当前/ 重新为空，且没有第二份 active plan。
```

### 通过条件

```text
全部格式检查、crate 全量测试和历史重点回归 exit 0，且没有用 0 tests filtered 或新增 ignored 冒充通过。
Gate A-E 的 manifest/path/hash/PE/process/publish/report 证据可在完成记录追溯。
237 正式方案、00/49/54、施工 README、阶段记录 README 和 227 状态一致。
完成记录只描述真实已执行结果，deferred 能力没有被标成已完成。
本文档已移动到 已完成/，施工文档/当前/ 为空且入口不再指向 active 237。
```

### 失败即停

任一 Gate、格式检查、crate 全量测试、历史重点回归或文档一致性检查未通过时，不得生成“已完成”阶段记录，不得移动本文档，不得把 237/P2-2 标为完成；修复并重跑失败范围及其受影响回归。

### Gate F 施工记录（2026-07-11）

状态：`已完成`。

全量命令均分开执行并记录耗时，最终结果：

```text
cargo fmt --check：通过，1.626s。
engine_input：26 passed，0.536s。
editor_ui_model：52 passed，1.164s。
editor_core：340 passed，1 ignored（既有 env-gated），65.058s。
editor_input：29 passed，0.613s。
editor_ui_renderer：12 passed，0.274s。
editor_wgpu_renderer：14 passed，9.902s。
editor_window_winit：86 passed，16.590s。
engine_runtime：426 passed，20.716s。
runtime_cli：22 unit + 4 process integration passed，12.661s。
runtime_player_winit：17 passed，8.956s。
project_e2e_gate：41 unit + 1 release integration + 3 consistency integration passed，29.332s。
```

历史重点回归：

```text
editor_core desktop_export：5 passed，0.852s。
editor_core build_and_run：4 passed，0.861s。
runtime_cli exported_player：5 unit + 1 process integration passed，0.606s。
engine_runtime runtime_package_publish：2 passed，0.314s。
engine_runtime runtime_package_path：6 passed，0.212s。
editor_core report_panel：9 passed，0.230s。
```

Gate F 首次 `editor_input` 全量运行暴露 Build 面板多行按钮与 Asset Browser 共用命中矩形；已把 Build 与 Asset Browser 分配到独立稳定列，并扩充真实路由测试覆盖 Build Release、Pick Icon、Save Profile。受影响的 `editor_input`、`editor_ui_renderer`、`editor_window_winit` 全量回归与最终格式检查均已重跑通过。

卫生检查：目标 Runtime/Cargo 子进程为 0；工作区（排除 `rust/target`）与系统 temp 精确扫描均无 `.publish.lock`、`.staging-*` 或 `.backup`；样例 Reports 只有既有 `.gitkeep`，既有 dev Build 和 source/template hash 未被 release E2E 修改。

Gate A-F、整体回归、正式方案回填、完成记录和入口同步均已完成；本文档可归档。

## 11. 施工诊断最小集合

ReleasePackageReport 至少稳定区分：

```text
release_profile_missing
release_profile_schema_unsupported
release_identity_invalid
release_executable_name_invalid
release_version_invalid
release_icon_asset_missing
release_icon_decode_failed
release_player_template_missing
release_resource_stamp_failed
release_resource_readback_mismatch
release_manifest_invalid
release_path_escape
release_path_collision
release_entrypoint_missing
release_entrypoint_launch_failed
release_runtime_package_load_failed
release_payload_hash_mismatch
release_publish_busy
release_publish_failed
release_publish_rollback_failed
```

每条 diagnostic 必须至少有：

```text
code
domain/stage
message
relative source or package path（Summary）
absolute owned path（仅 Trace）
next action
```

## 12. 工作树与安全规则

当前仓库存在大量用户已有改动。施工期间必须：

```text
不 reset、checkout 或回退非 237 改动。
修改重叠文件前先读当前内容，基于现状小范围 patch。
所有测试 output/staging/backup/report 使用经过 containment 校验的 project-owned 或 temp root。
不删除或移动由本 Gate 之外逻辑创建的目录。
只修改 staging executable，不修改 rust/target 中的源 template。
不把 sample project Build/ 或 .aife 派生产物提交为 source fixture。
```

## 13. 施工文档自审

审查结论：`通过，可以从 Gate A 开始施工`。

```text
方案一致性：采用用户确认的 B-min+；BuildProfile v2、AssetRef icon、direct stamped Runtime entrypoint、relative layout、manifest roles、no-arg launch 和 report 均进入 Gate。

外部审查：没有发现适用于 237 的外部审查文档，因此正式方案无需审查驱动修改；该判断已显式记录。

范围控制：明确排除 ZIP、installer、signing、store、auto-update、archive format 和 Bootstrap Launcher；未来 C 只保留 entrypoint/file roles 边界。

真相边界：ProjectRuntimePackageAssembler 和 RuntimePackageBuilder 仍是唯一 RuntimePackage 生产链；release 只装配外层目录。

共享 schema：ReleasePackageManifest 放在 Runtime/Editor 都可依赖的窄模块，避免 runtime_cli 依赖 editor_core 或复制 schema。

发布复用：明确从 236 抽取 atomic_directory_publish；RuntimePackage 和 outer release 共用 lock/staging/rollback，不复制发布逻辑。

236 回归：Gate C 和 Gate F 都强制执行 runtime_package_publish/runtime_package_path/desktop_export/build_and_run 回归。

PE 安全：editpe 固定版本、license/API/resource preservation、staging-only、readback 和 process load 都有 Gate；不手写 parser，不依赖外部 Windows SDK 工具。

运行成本：用户 packaged Runtime report Off；Editor Summary/Trace 和显式验证才生成证据，不把 report 变成运行热路径常驻层。

测试完整性：Gate A-F 每个 Gate 都有精确文件、任务、命令、通过条件和失败即停；最后有 crate 全量回归。

可判定性：manifest/path/roles/hash/readback/formal load/process/report 均有结构化证据，不依赖人工目测。

工作树安全：测试限定 owned temp/output root，禁止改样例源和 template，禁止整理或回退无关 dirty changes。

归档规则：只有 Gate A-F 与整体回归全部通过才生成完成记录、更新入口并移动本文档；失败时保持当前状态。
```

自审未发现与正式 237 方案冲突、范围扩大、测试缺失或归档条件不清的问题。正式施工必须从 Gate A 开始。
