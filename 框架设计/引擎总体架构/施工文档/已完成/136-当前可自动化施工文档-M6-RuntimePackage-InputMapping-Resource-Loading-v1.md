# 136-当前可自动化施工文档-M6 RuntimePackage InputMapping Resource Loading v1

## 1. 施工目标

按照 `136-M6-RuntimePackage-InputMapping-Resource-Loading-v1方案.md`，把 RuntimePackage 中的项目 `InputMappingAsset` 正式接入 Runtime Player 默认 mapping。

完成后：

```text
Build Pipeline 写入 input manifest。
RuntimePackage Loader 读取并验证 InputMappingAsset。
Runtime Player 优先使用 RuntimePackage.default_input_mapping。
engine_input::InputMappingAsset::gameplay_default() 只作为显式 fallback。
Report 能区分 runtime-package 与 engine-default-fallback。
```

## 2. 不施工范围

本阶段不做：

```text
Input Mapping 编辑器 UI
Runtime rebinding UI
Gamepad / Touch / IME
多玩家输入所有权
动态切换多个 Input Context
复杂 Trigger 图
```

## 3. 代码改造步骤

### 3.1 engine_runtime::runtime_package

新增 RuntimePackage input 结构：

```text
RUNTIME_INPUT_MANIFEST_SCHEMA_VERSION
RuntimeManifestInputIndex
RuntimeInputManifest
RuntimeInputMappingManifestEntry
RuntimePackageInputStatus / 或等价 loader result 辅助结构
```

扩展：

```text
RuntimePackageManifest.input
RuntimePackage.input_manifest
RuntimePackage.input_mappings
RuntimePackage.default_input_mapping
```

Loader 规则：

```text
读取 manifest.input.path。
读取 input/input-manifest.json。
读取 enabled mappings。
验证 defaultMappingId 存在。
调用 InputMappingAsset.validate。
缺失时 fallback 到 InputMappingAsset::gameplay_default，并写 diagnostics warning。
非法 default mapping 第一版产生 error。
```

### 3.2 engine_runtime::runtime_package_builder

Build Pipeline 规则：

```text
继续写入 package/input/{id}.json。
新增写入 package/input/input-manifest.json。
RuntimePackageManifest 写入 input index。
如果 input.input_mappings 为空，写入 engine default fallback mapping，并在 build diagnostics 中 warning。
```

第一版默认：

```text
defaultMappingId = 第一个 input_mappings 的 id。
没有项目 mapping 时 defaultMappingId = input.default。
```

### 3.3 runtime_player_winit

改造点：

```text
headless run path 使用 package.default_input_mapping。
real-window path 加载 package 后使用 package.default_input_mapping。
移除正式路径中的直接 InputMappingAsset::gameplay_default()。
保留单元测试 fixture 中的 gameplay_default。
```

Report 增加或扩展输入摘要字段：

```text
mapping_source
mapping_id
mapping_status
mapping_path
```

第一版允许字段在 `NativeInputSummary` 中表达。

### 3.4 测试

至少新增或更新：

```text
engine_runtime:
  builder 写入 input/input-manifest.json。
  loader 读取 RuntimePackage default_input_mapping。
  项目自定义 mapping 能覆盖 gameplay_default。
  缺失 input manifest 能 fallback 并报告 warning。

runtime_player_winit:
  Player 使用 RuntimePackage mapping，不使用 gameplay_default。
  Report mapping_source = runtime-package。
```

## 4. 验证命令

施工完成后运行：

```powershell
cargo fmt --check
cargo test -p engine_input
cargo test -p engine_runtime
cargo test -p runtime_player_winit
cargo check -p runtime_player_winit --features real-window
```

如果 workspace 中相关依赖触发额外编译错误，需要先定位是否与本阶段有关；无关错误记录在完成记录里，不做无关修复。

## 5. 自审清单

施工前确认：

```text
M6 没有重做输入系统。
M6 没有实现 Input Mapping UI。
M6 没有把项目玩法规则写进引擎。
M6 只把项目 InputMappingAsset 从 RuntimePackage 接到 Player。
```

施工后确认：

```text
RuntimePackage manifest 有 input index。
package/input/input-manifest.json 可读。
load_runtime_package 能暴露 default_input_mapping。
runtime_player_winit 正式路径不再硬编码 gameplay_default。
Report 能说明 mapping 来源。
测试覆盖 runtime-package mapping 与 fallback。
```
