# Runtime 资源加载系统方案

本文定义 `Runtime Package / AssetRegistry / cooked asset` 如何接入 Rust Runtime 的真实资源加载。

优先级：

```text
1. AI 友好
2. 支持复杂项目
3. 后期可维护、可修改
4. 简单，减少隐藏规则
5. 效率与多平台
```

## 问题是什么

我们已经有：

```text
Project Schema
Asset DB / Importer / AssetRegistry
Build Graph / Runtime Package
Rust Runtime / ECS / FrameLoop
RenderCommand / RenderSceneState / RHI-min
```

但还缺一条真实链路：

```text
Runtime Package 中的 AssetRef
  -> RuntimeAssetIndex
  -> cooked asset / bundle
  -> Rust Runtime 中可用的 Texture / Mesh / Material / Audio / Scene / Prefab 数据
  -> Renderer / Audio / SceneInstantiator 等系统使用
```

这个系统不是“什么时候加载”的项目规则系统。它是 Runtime 资源基础设施：

```text
项目侧决定：
  什么时候 load
  load 哪些 AssetSet / AssetRef
  什么时候 release
  场景切换时保留哪些资源
  是否显示 loading / 黑屏

引擎侧负责：
  AssetRef 解析
  Runtime Package / bundle mount
  cooked asset 查找
  依赖加载顺序
  同步 / 异步加载 API
  解码
  GPU / audio / native resource prepare
  handle 生命周期
  diagnostics / trace
```

## 成熟引擎怎么做

### UE

源码参考：

```text
UE源码参考/RuntimeAssetLoading-AssetManager-Streamable-Pak-IoStore.md
```

UE 采用：

```text
FSoftObjectPath / FPrimaryAssetId
  -> AssetRegistry / AssetManager
  -> FStreamableManager / FStreamableHandle
  -> Package / Async Loading
  -> Pak / IoStore
```

优点：

```text
支持大型项目、异步加载、软引用、Primary Asset、热更新包、chunk、IoStore。
AssetManager / StreamableHandle 对生命周期和加载状态有清晰管理。
AssetRegistry 能查依赖和反向引用。
```

缺点：

```text
体系非常重。
大量规则绑定 UObject / Package / Project Settings / Cook / Plugin / Chunk。
AI 直接生成或修改这套配置成本很高。
```

### Unity

源码参考：

```text
Unity源码参考/RuntimeAssetLoading-Resources-AssetBundle-Addressables.md
```

Unity 主要路线：

```text
Resources
AssetBundle + AssetBundleManifest
Addressables + AssetReference + AsyncOperationHandle
SceneManager.LoadSceneAsync
```

优点：

```text
用户入口简单。
Addressables 的 AssetReference / AsyncOperationHandle 比直接操作 bundle 友好。
AssetBundle 是构建产物，资源和包分离。
```

缺点：

```text
Resources 对大型项目不可维护。
AssetBundle 手动依赖管理容易出错。
Addressables 能力强，但 catalog / provider / profile / group 体系对 AI 和普通用户仍偏复杂。
```

### Bevy

源码参考：

```text
Bevy源码参考/13-RuntimeAssetLoading-AssetServer-Handle-RenderAsset.md
```

Bevy 采用：

```text
AssetServer.load(path)
  -> Handle<T>
  -> AssetLoader / LoadContext
  -> Assets<T>
  -> LoadState / DependencyLoadState
  -> RenderAsset extract / prepare
```

优点：

```text
结构轻。
Handle / LoadState / dependency state 清晰。
RenderAsset 把 CPU asset 和 GPU resource 分开。
```

缺点：

```text
路径式加载不适合作为我们正式 Runtime 引用。
对 cooked package / 大型热更包 / 平台包的完整方案不如 UE / Unity 成熟。
```

### Godot

公开资料参考：

```text
ResourceLoader
ResourceUID
PackedScene
load_threaded_request / load_threaded_get_status / load_threaded_get
```

可学习：

```text
ResourceLoader 提供统一资源加载入口。
ResourceUID 解决资源移动后的稳定引用问题。
PackedScene 是场景 / 预制体运行时实例化入口。
线程加载状态可查询。
```

不照搬：

```text
Godot Resource 面向脚本和编辑器资源统一模型。
我们的 Runtime 应消费 cooked 后的 RuntimeAssetIndex，不直接消费完整编辑器资源对象。
```

## 公开资料补充

除本地源码外，本方案还参考以下公开资料方向：

```text
Unity Manual:
  AssetBundle 加载、AssetBundleManifest 依赖查询、Addressables AssetReference / AsyncOperationHandle。

Unreal Engine Documentation:
  Asset Manager、Asynchronous Asset Loading、Asset Registry、Pak / IoStore packaging。

Bevy Docs / docs.rs:
  AssetServer、Handle、LoadState、AssetLoader、RenderAsset。

Godot Documentation:
  ResourceLoader、ResourceUID、PackedScene、threaded resource loading。
```

公开资料的用途：

```text
确认成熟引擎对用户暴露的概念。
确认源码中未完全展开的运行时行为，例如 Unity Addressables。
不以公开教程替代本地源码结构分析。
```

公开资料链接：

```text
Unity AssetBundle dependencies:
  https://docs.unity3d.com/6000.0/Documentation/Manual/AssetBundles-Dependencies.html

Unity AssetBundleManifest.GetAllDependencies:
  https://docs.unity3d.com/6000.4/Documentation/ScriptReference/AssetBundleManifest.GetAllDependencies.html

Unity Addressables AsyncOperationHandle:
  https://docs.unity3d.com/Packages/com.unity.addressables@2.0/manual/AddressableAssetsAsyncOperationHandle.html

Unreal Asset Management:
  https://dev.epicgames.com/documentation/unreal-engine/asset-management-in-unreal-engine

Unreal Asynchronous Asset Loading:
  https://dev.epicgames.com/documentation/unreal-engine/asynchronous-asset-loading-in-unreal-engine

Bevy AssetServer:
  https://docs.rs/bevy/latest/bevy/asset/struct.AssetServer.html

Bevy asset module:
  https://docs.rs/bevy/latest/bevy/asset/index.html
```

## 可选方案

### 方案 A：Unity Addressables 式

```text
AssetRef / key
  -> Runtime catalog
  -> provider
  -> AsyncOperationHandle
```

优点：

```text
用户概念直观。
加载、释放、依赖、远程下载都能统一。
```

缺点：

```text
provider / catalog / profile / group 容易变成第二套复杂系统。
AI 要同时理解 catalog、provider 和 package，规则偏多。
```

### 方案 B：UE AssetManager 式

```text
AssetRef / PrimaryAssetId
  -> RuntimeAssetRegistry
  -> AssetManager
  -> StreamableHandle
  -> Package backend
```

优点：

```text
大型项目能力最强。
软引用、异步加载、包挂载、热更、依赖查询都有成熟参照。
```

缺点：

```text
如果照搬 UE，会引入过多配置、继承和历史层。
AI 修改成本高。
```

### 方案 C：Bevy AssetServer 式

```text
AssetServer.load(path)
  -> Handle<T>
  -> Loader
  -> Assets<T>
```

优点：

```text
实现快，结构清楚。
Rust 生态适配好。
```

缺点：

```text
路径式引用不符合我们 AssetRef / cooked package 规则。
复杂项目的包、版本、热更和构建追踪不够强。
```

### 方案 D：我们的压缩版 AssetManager

```text
AssetRef / AssetSet
  -> RuntimeAssetIndex
  -> RuntimePackageMountTable
  -> RuntimeAssetLoader
  -> RuntimeAssetHandle
  -> DecodedAsset
  -> Domain Prepared Resource
```

优点：

```text
吸收 UE 的分层能力。
吸收 Unity Addressables 的用户入口。
吸收 Bevy 的 handle / load state / render asset 分离。
删除 catalog provider 复杂度、UObject 复杂度、路径式 runtime 引用。
AI 只需要理解 AssetRef、AssetSet、RuntimeAssetIndex、LoadRequest、Handle、Diagnostics。
```

缺点：

```text
需要自研 RuntimeAssetIndex / Loader / Handle / Diagnostics。
第一版需要把边界定死，否则容易重新长成复杂系统。
```

推荐：方案 D。

## 推荐方案

正式规则：

```text
Runtime 不读取完整编辑器 Asset DB。
Runtime 只读取 Runtime Package 中的 RuntimeAssetIndex / bundle_table / cooked_asset_table / dependency_table。
AssetRef 是稳定引用，不是路径，不是运行时对象指针。
RuntimeAssetHandle 是加载后的运行时句柄。
项目逻辑允许调用受控 load / load_async / release API。
项目逻辑不能直接读 cooked 文件，不能直接绕过 RuntimeAssetLoader。
依赖由 Build / Cook 生成，Runtime 只校验和执行。
同步和异步都支持；同步加载必须在 diagnostics 中标记。
分阶段加载流程由项目侧 LoadPlan / SceneLifecycle 决定，不进入 RuntimeAssetLoader 规则。
```

最小数据结构：

```text
RuntimePackageManifest
  package_id
  package_version
  platform
  schema_version

RuntimeAssetIndex
  asset_guid -> RuntimeAssetRecord

RuntimeAssetRecord
  asset_guid
  asset_id
  asset_type
  sub_asset_id
  version
  cooked_asset_id
  bundle_id
  loader_kind
  dependencies[]
  hash
  size
  flags
  source_map_debug

BundleRecord
  bundle_id
  mount_id
  uri
  hash
  version
  mounted

CookedAssetRecord
  cooked_asset_id
  bundle_id
  offset/path
  size
  compression
  hash

RuntimeAssetHandle
  handle_id
  asset_guid
  asset_type
  cooked_asset_id
  bundle_id
  runtime_resource_id
  state
  generation
  ref_count
```

加载流程：

```text
1. mount_runtime_package(package)
2. 读取 manifest / runtime_asset_index / bundle_table / cooked_asset_table
3. 项目侧发起 load(asset_ref) 或 load_async(asset_ref)
4. RuntimeAssetLoader resolve AssetRef
5. 校验 type / subAsset / version
6. 检查 bundle 是否 mounted
7. 按 dependency_table 加载依赖
8. 读取 cooked bytes
9. 调用 loader_kind 对应 decoder
10. 写入 decoded asset cache
11. 需要 GPU / audio / native resource 时进入 domain prepare
12. 返回 RuntimeAssetHandle，状态 Ready / Failed
```

释放流程：

```text
release(handle)
  -> 校验 generation
  -> ref_count--
  -> 如果 ref_count 为 0，标记可释放
  -> domain resource 先释放 GPU / audio / native resource
  -> decoded asset cache 释放
  -> handle state = Released
```

热更 / patch 第一版规则：

```text
Runtime 支持 mount 新 Runtime Package / patch package。
已加载 handle 不自动变成新资源。
默认替换策略是 next-load 生效。
强制替换必须经过项目侧明确 reload / replace 请求。
旧 handle 的 generation / version 不变，避免静默替换导致 BUG 难查。
```

## Diagnostics

每个加载请求必须产生最小诊断记录：

```text
request_id
asset_ref
stage
state
error_code
bundle_id
cooked_asset_id
loader_kind
dependency_chain
handle_id
generation
sync_load
source_map_debug
recommended_action
```

错误码第一版：

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

AI 查错路径：

```text
用户说“飞机贴图没出来”
  -> AI 查 RenderFrameReport
  -> 找到 material / texture handle not ready
  -> 查 AssetLoadDiagnostics
  -> 定位 asset_ref / bundle_id / cooked_asset_id / dependency_chain
  -> 回到 RuntimeAssetIndex.source_map_debug
  -> 定位 Project Asset / Prefab / Scene 字段
```

## 和其他引擎对比

| 项目 | UE | Unity | Bevy | 我们 |
|---|---|---|---|---|
| 用户引用 | FSoftObjectPath / PrimaryAssetId | AssetReference / path / bundle asset | path -> Handle | AssetRef / AssetSet |
| Runtime 索引 | AssetRegistry / PackageStore | AssetBundleManifest / Addressables catalog | AssetServer internal info | RuntimeAssetIndex |
| 加载句柄 | FStreamableHandle | AsyncOperationHandle / ResourceRequest | Handle<T> | RuntimeAssetHandle |
| 底层包 | Pak / IoStore | AssetBundle | file / processed asset source | RuntimePackage / Bundle / cooked asset |
| 依赖 | AssetRegistry / AssetManager | Manifest / Addressables | LoadContext dependency | Build Cook dependency_table |
| GPU 准备 | Render resource init | native renderer | RenderAsset prepare | Domain Prepared Resource |
| AI 友好 | 中等，强但重 | 中等，Addressables 较好 | 高但偏程序员 | 高，结构显式且少规则 |
| 大项目能力 | 很强 | 强 | 中等 | 目标强 |
| 第一版复杂度 | 很高 | 中高 | 低 | 中 |

## 为什么适合我们

```text
AI 友好：
  AssetRef / RuntimeAssetIndex / LoadRequest / Handle / Diagnostics 都是结构化数据。
  AI 可以解释资源为什么没加载，而不是猜路径或读底层包。

复杂项目：
  支持 cooked asset、bundle mount、依赖表、版本、patch package、typed handle。
  不依赖编辑器 Asset DB，导出运行和编辑器预览可以共用 Runtime Package。

可维护：
  Build/Cook 负责生成依赖，Runtime 负责执行。
  项目侧 LoadPlan 负责什么时候加载，RuntimeLoader 不掺业务生命周期。

简单：
  不做 UE 完整 PrimaryAsset 继承体系。
  不做 Unity Addressables 完整 provider/catalog/profile 体系。
  不做 Bevy 路径式 Runtime 引用。

效率：
  RuntimeAssetIndex 是精简索引。
  Release 版可把 JSON/目录包替换为二进制 container，AssetRef / handle 层不变。
```

## 第一版边界

第一版做：

```text
本地 Runtime Package mount
本地 bundle table / cooked asset table
AssetRef resolve
sync / async API 数据结构
dependency_table 校验和拓扑加载
typed RuntimeAssetHandle
decoded asset cache
domain prepare 预留接口
diagnostics
headless test
```

第一版不做：

```text
远程下载
CDN
加密
真实压缩分块 streaming
后台 patch 下载
运行时重新 cook
完整 Addressables catalog
完整 UE AssetManager 配置系统
直接读取编辑器 Asset DB
```

## 小型验证结论

验证 demo：

```text
验证Demo/RuntimeAssetLoading/runtime_asset_loading_validation.js
```

测试覆盖：

```text
复杂依赖：scene -> prefab -> mesh/material/texture/vfx
缺失 bundle：bundle_not_mounted
循环依赖：cyclic_dependency
patch mount：next-load 生效，旧 handle 不静默替换
release generation：旧 handle 释放失败
```

结论：

```text
这套方案能覆盖复杂项目所需的基本资源加载闭环。
它没有把“何时加载、何时卸载、如何显示 loading”塞进引擎底层。
它保留了 AI 查错需要的结构化路径。
```
