# 当前可自动化施工文档：Runtime 资源加载系统 v1

> 状态：已确认可施工。本文档只定义本轮自动化施工范围，不继续扩展完整热更、远程下载、二进制 Bundle、平台 Cook。长期架构规则以 `68-Runtime资源加载系统方案.md`、`06-资源系统架构.md`、`07-Build-Export-Pipeline.md`、`21-Runtime-Core-Boundary.md` 为准。

## 1. 施工目标

本轮目标是把 `Runtime Package / AssetRegistry / cooked asset` 接入 Rust Runtime 的真实资源加载最小闭环。

目标链路：

```text
Runtime Package
  -> RuntimeAssetIndex
  -> RuntimePackageMountTable
  -> RuntimeAssetLoader
  -> RuntimeAssetHandle
  -> DecodedAsset cache
  -> Domain Prepared Resource stub
  -> Diagnostics / tests
```

本轮完成后，Rust Runtime 应能：

```text
读取 Runtime Package 中的 runtime_asset_index / bundle_table / cooked_asset_table / dependency_table。
mount 本地 Runtime Package / bundle。
通过 RuntimeAssetRef 解析资源。
同步加载资源并返回 RuntimeAssetHandle。
支持 async API 的数据结构和状态，但第一版可以用同步执行模拟完成。
校验依赖、bundle、cooked asset、类型、subAsset。
生成结构化 AssetLoadDiagnostics。
支持 release(handle) 和 generation 防旧 handle 误用。
支持 patch package mount 的 next-load 生效规则。
用 headless Rust 单元测试验证完整闭环。
```

本轮不是实现完整 Runtime 资源系统，而是建立长期正确的底座。

## 2. 参考引擎对比

| 引擎 | 对应机制 | 可借鉴点 | 本轮取舍 |
|---|---|---|---|
| Unreal | `FSoftObjectPath` / `AssetRegistry` / `UAssetManager` / `FStreamableHandle` / Pak / IoStore | 软引用、运行时索引、加载 handle、包挂载、依赖查询、异步加载 | 学分层和 handle，不复制 UObject / PrimaryAsset / Chunk / IoStore 完整复杂度 |
| Unity | AssetBundle / AssetBundleManifest / Addressables `AssetReference` / `AsyncOperationHandle` | AssetRef 用户入口、Bundle 是构建产物、加载 handle、Release 语义 | 学 AssetRef + handle，不复制 Addressables catalog / provider / profile |
| Bevy | `AssetServer` / `Handle<T>` / `LoadState` / `AssetLoader` / `RenderAsset` | Rust handle、load state、依赖状态、CPU asset 与 GPU prepare 分离 | 学轻量状态和 RenderAsset 分离，不采用路径式 Runtime 引用 |
| Godot | `ResourceLoader` / `ResourceUID` / `PackedScene` / threaded load | 统一加载入口、稳定 UID、线程加载状态 | 学统一入口和 UID，不采用完整 Resource 对象模型 |

本轮路线：

```text
UE-like 运行时索引和包挂载
+ Unity-like AssetRef / handle 用户心智
+ Bevy-like Rust typed handle / load state / prepared resource 分离
+ AI-first diagnostics
```

## 3. 硬规则

```text
Runtime 不读取完整编辑器 Asset DB。
Runtime 只读取 Runtime Package 中的运行时资源索引。
项目逻辑只能通过 RuntimeAssetLoader 的受控 API load / load_async / release。
项目逻辑不能直接读 cooked 文件。
AssetRef 是稳定引用，不是路径，不是运行时对象指针。
RuntimeAssetHandle 是运行时句柄，必须有 generation。
依赖由 Build / Cook 生成，Runtime 只校验和执行。
第一版支持本地 Runtime Package + 本地 cooked asset + 本地 bundle mount。
同步加载允许存在，但必须在 diagnostics 标记 sync_load。
分阶段加载、Loading UI、场景切换策略属于项目侧，本轮不做。
```

## 4. 本轮边界

### 本轮做

```text
A0 Runtime 资源数据模型。
A1 RuntimePackage loader 扩展读取 runtime_asset_index / bundle_table / cooked_asset_table / dependency_table。
A2 RuntimePackageMountTable / RuntimeAssetIndex 查询。
A3 RuntimeAssetLoader / RuntimeAssetHandle / RuntimeAssetLoadState。
A4 dependency_table 校验、拓扑加载、循环依赖诊断。
A5 decoded asset cache + domain prepared resource stub。
A6 Rust 单元测试和 npm 测试脚本。
A7 文档入口同步。
```

### 本轮不做

```text
远程下载 / CDN。
后台 patch 下载。
加密。
真实压缩分块 streaming。
真实二进制 Bundle container。
运行时重新 cook。
完整 Addressables provider / catalog / profile。
完整 UE AssetManager 配置系统。
真实 GPU texture / mesh upload。
真实 audio backend。
Scene / Prefab 实例化到 ECS 的完整链路。
IR Interpreter / Rust AOT。
```

## 5. 建议代码落点

优先在现有 Rust crate 内施工：

```text
rust/crates/engine_runtime/src/runtime_asset.rs
rust/crates/engine_runtime/src/runtime_asset_loader.rs
rust/crates/engine_runtime/src/runtime_asset_diagnostics.rs
rust/crates/engine_runtime/src/runtime_package.rs
rust/crates/engine_runtime/src/lib.rs
```

可选补充：

```text
rust/crates/engine_runtime/src/runtime_asset_prepare.rs
```

脚本：

```text
scripts/test-rust-runtime-asset-loading.cjs
```

package script：

```text
test:rustruntimeassetloading
```

## 6. 数据结构要求

### 6.1 RuntimeAssetRef

现有：

```text
RuntimeAssetRef:
  id
  type
```

本轮兼容扩展：

```text
RuntimeAssetRef:
  id
  type
  guid optional
  sub_asset optional
```

规则：

```text
如果 guid 存在，优先用 guid resolve。
如果 guid 不存在，第一版允许 id resolve，用于兼容现有 Runtime Package fixture。
type 必须校验。
```

### 6.2 RuntimeAssetIndex

Rust 数据结构最小字段：

```text
RuntimeAssetRecord:
  asset_guid
  asset_id
  asset_type
  sub_asset_id optional
  version
  cooked_asset_id
  bundle_id
  loader_kind
  dependencies
  hash optional
  size optional
  flags
  source_map_debug optional
```

兼容规则：

```text
如果 Runtime Package 仍只有 assets/asset-manifest.json 的旧 RuntimeAssetManifest，则 loader 可以从旧 manifest 派生最小 RuntimeAssetRecord。
派生字段：
  asset_guid = asset.id
  asset_id = asset.id
  asset_type = asset.type
  cooked_asset_id = asset.id
  bundle_id = asset.bundleId
  loader_kind = asset.type
  dependencies = []
```

这只是兼容桥，不是长期结构替代。

### 6.3 BundleRecord

```text
BundleRecord:
  bundle_id
  mount_id optional
  uri
  hash optional
  version optional
  mounted
```

第一版：

```text
uri 可以是 Runtime Package 内相对目录。
mounted 由 RuntimePackageMountTable 运行时维护，不依赖 JSON 初始值。
```

### 6.4 CookedAssetRecord

```text
CookedAssetRecord:
  cooked_asset_id
  bundle_id
  path optional
  offset optional
  size optional
  compression optional
  hash optional
```

第一版：

```text
如果 path 存在，从 package_dir/path 读取。
如果 path 不存在，可以用 source / asset id 派生一个 fake decoded asset，用于旧 manifest 测试。
不实现 offset 二进制读取。
```

### 6.5 RuntimeAssetHandle

```text
RuntimeAssetHandle:
  handle_id
  asset_guid
  asset_id
  asset_type
  sub_asset_id optional
  cooked_asset_id
  bundle_id
  runtime_resource_id optional
  state
  generation
  ref_count
  loader_kind
  version
```

状态：

```text
Loading
Ready
Failed
Released
```

## 7. Diagnostics 要求

新增 AssetLoadDiagnostics 最小字段：

```text
request_id
asset_ref_id
asset_guid optional
asset_type
stage
state
error_code optional
bundle_id optional
cooked_asset_id optional
loader_kind optional
dependency_chain
handle_id optional
generation optional
sync_load
recommended_action optional
```

stage：

```text
resolve
dependency
bundle_mount
read
decode
prepare
apply
release
```

error_code：

```text
missing_asset_ref
type_mismatch
sub_asset_missing
bundle_not_mounted
cooked_asset_missing
dependency_missing
cyclic_dependency
decode_failed
gpu_prepare_failed
release_generation_mismatch
release_in_use
version_mismatch
sync_load_in_hot_path
```

规则：

```text
每次 load / release 都应可产生 diagnostics。
测试中必须覆盖至少 bundle_not_mounted、cyclic_dependency、release_generation_mismatch。
```

## 8. 任务拆分

### A0：新增 Runtime Asset 数据模型

改动：

```text
新增 runtime_asset.rs。
定义 RuntimeAssetRecord / BundleRecord / CookedAssetRecord / RuntimeAssetIndex / RuntimePackageMountTable。
定义 RuntimeAssetHandle / RuntimeAssetLoadState。
在 lib.rs 导出模块。
```

测试：

```text
Rust 单测：能构造 RuntimeAssetIndex，并通过 id / guid resolve。
Rust 单测：type mismatch 返回错误。
```

### A1：扩展 RuntimePackage loader

改动：

```text
runtime_package.rs 增加可选字段读取：
  runtime_asset_index
  bundle_table
  cooked_asset_table
  dependency_table
保留旧 RuntimeAssetManifest 兼容。
加载 RuntimePackage 后能构建 RuntimeAssetIndex。
```

测试：

```text
Rust 单测：旧 manifest 能派生最小 index。
Rust 单测：新 runtime_asset_index JSON 能读取完整字段。
```

### A2：实现 mount table

改动：

```text
RuntimePackageMountTable:
  mount_package
  mount_bundle
  unmount_bundle
  is_bundle_mounted
```

规则：

```text
第一版 mount_package 只注册 bundle records。
默认是否自动 mount 由测试 fixture 明确指定，不做隐藏自动 mount。
```

测试：

```text
未 mount bundle 时 load 返回 bundle_not_mounted。
mount 后同一资源可继续加载。
```

### A3：实现 RuntimeAssetLoader

改动：

```text
新增 runtime_asset_loader.rs。
实现：
  load(asset_ref)
  load_by_id(asset_id, asset_type)
  load_async(asset_ref) 数据结构入口
  release(handle)
  get_handle_state(handle)
  diagnostics()
```

第一版：

```text
load_async 可以立即执行并返回 Ready/Failed handle，不要求真实 async executor。
decoded asset 可以是 DecodedAsset { asset_type, bytes_len, source_debug }。
```

测试：

```text
加载 texture / material / prefab 返回 Ready handle。
重复 load 增加 ref_count 或返回新的 handle，二者选一种并在测试中固定。
release 正常把 handle 标记 Released。
旧 generation release 返回 release_generation_mismatch。
```

推荐第一版选择：

```text
每次 load 返回新的 RuntimeAssetHandle。
cache 按 cooked_asset_id 共享 decoded asset。
release(handle) 只释放 handle 引用；cache 在无 handle 引用后释放。
```

这样比“同一资源复用同一 handle”更容易查 BUG。

### A4：依赖加载和循环检测

改动：

```text
RuntimeAssetLoader 在读取根资源前先按 dependency_table / record.dependencies 加载依赖。
使用 recursion stack 检测循环。
dependency_chain 写入 diagnostics。
```

测试：

```text
scene -> prefab -> material -> texture 全部 Ready。
缺失依赖返回 dependency_missing。
循环依赖返回 cyclic_dependency。
```

### A5：Domain Prepared Resource stub

改动：

```text
新增 PreparedRuntimeResource / RuntimeAssetPrepareResult。
loader_kind 为 texture / model / material / audio / scene / prefab 时生成 domain stub。
不做真实 GPU / audio 上传。
```

规则：

```text
prepare 是 decoded asset 之后的独立 stage。
prepare 失败必须使用 gpu_prepare_failed 或 decode_failed，不混到 resolve 阶段。
```

测试：

```text
texture load 后能得到 PreparedRuntimeResourceKind::Texture。
material 依赖 texture 后再 prepare。
```

### A6：新增端到端测试脚本

改动：

```text
新增 scripts/test-rust-runtime-asset-loading.cjs。
package.json 增加 test:rustruntimeassetloading。
```

脚本执行：

```text
cargo test -p engine_runtime runtime_asset
cargo test -p engine_runtime runtime_asset_loader
```

可接受简化：

```text
如果 cargo test 名称过滤不稳定，可以直接 cargo test -p engine_runtime。
```

测试 fixture 应覆盖：

```text
完整依赖加载。
未 mount bundle。
循环依赖。
patch package next-load 生效。
release generation mismatch。
```

### A7：文档同步

施工完成后必须：

```text
新增 阶段完成记录/2026-06-26-Runtime资源加载系统-v1/。
施工文档从 当前/ 移到 已完成/。
更新 49-当前状态与下一步入口.md。
更新 54-当前可自动化施工总入口.md。
更新 18-长期主义实现路径.md。
更新 施工文档/README.md。
如实现中改变长期规则，回填 68-Runtime资源加载系统方案.md。
```

## 9. 最小验收标准

施工完成必须满足：

```text
npm run test:rustruntimeassetloading 通过。
npm run test:rustruntimepackage 仍通过。
npm run test:rustruntime 仍通过。
```

Rust 单测至少覆盖：

```text
resolve by id / guid。
type mismatch。
bundle_not_mounted。
dependency_missing。
cyclic_dependency。
load scene dependency chain。
release_generation_mismatch。
patch next-load 生效，旧 handle 不静默替换。
```

文档验收：

```text
当前施工入口能从 49 / 54 / 施工文档 README 找到。
68 仍是长期规则入口。
不新增与 68 冲突的新规则。
```

## 10. 风险控制

禁止为了通过测试临时引入：

```text
Runtime 直接读取编辑器 Asset DB。
项目逻辑直接读 cooked 文件。
AssetRef 保存 runtime object pointer。
加载失败 silent fallback。
旧 handle 自动重绑定 patch 后的新资源。
把 Scene 实例化和资源加载混成一个大函数。
把真实 GPU upload 塞进本轮。
```

如果实现时发现 Runtime Package 当前 JSON 缺少正式字段：

```text
允许做旧 manifest -> RuntimeAssetIndex 的兼容派生。
必须在 diagnostics / comments 中标记为 compatibility path。
不允许把旧 manifest 继续定义成长期真相。
```

## 11. 完成后下一步

本轮完成后，下一套系统级施工文档应围绕：

```text
Scene / Prefab / Entity 从 Runtime Package 实例化到 Rust ECS。
```

原因：

```text
资源能被 Runtime 加载后，最小游戏循环的下一段瓶颈就是 Scene / Prefab / Entity 落到 ECS。
```
