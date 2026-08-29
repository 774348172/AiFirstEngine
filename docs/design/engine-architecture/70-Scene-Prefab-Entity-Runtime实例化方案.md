# Scene / Prefab / Entity 浠?Runtime Package 瀹炰緥鍖栧埌 Rust ECS 鏂规

## 当前归属说明：Projection 术语

本文中如果出现以下历史名称：

```text
RenderExtract
RenderAssetBridge / Render Asset Bridge
Physics2DBridge
RuntimeScene Hydration
AuiRenderExtract / AuiRendererBridge
SpriteRenderer2D ECS-to-RenderProxy Bridge
```

统一按 `110-World-Projection-Adapter统一跨域同步规则.md` 理解为：

```text
RenderProjection
AssetProjection
Physics2DProjection
HydrationProjection
UiProjection
RenderProjectionAdapter<SpriteRenderer2D>
```

这些名称可以作为历史实现名保留，但不再作为新增架构概念扩展。后续新增类型只新增对应 `ProjectionAdapter`，不新增独立 Bridge。

## 闂鏄粈涔?
Runtime 璧勬簮鍔犺浇 v1 宸茬粡瀹屾垚锛?
```text
Runtime Package
  -> RuntimeAssetIndex
  -> RuntimeAssetLoader
  -> RuntimeAssetHandle
```

浣嗚祫婧愮幇鍦ㄥ彧鏄€滃彲鍔犺浇鈥濄€備笅涓€姝ュ繀椤昏В鍐筹細

```text
Scene Asset / Prefab Asset 鍔犺浇鍚庯紝濡備綍鍒涘缓 Rust ECS Entity銆?Scene 鏂囦欢閲岀殑 Entity 鏍戝浣曡惤鍒?World銆?Prefab 濡備綍浣滀负 Entity 妯℃澘瀹炰緥鍖栥€?AssetRef 濡備綍瑙﹀彂璧勬簮鍔犺浇骞剁粦瀹氬埌缁勪欢銆?Scene unload / Prefab despawn 鏃跺浣曞垹闄?Entity 鍜?release 璧勬簮銆?```

杩欎竴姝ユ槸鏈€灏忔父鎴忓惊鐜殑 P0 缂哄彛銆?
## 鍏跺畠寮曟搸鍋氭硶

### UE

鍙傝€冿細

```text
UE婧愮爜鍙傝€?Scene-Level-Actor-Instantiation婧愮爜鍙傝€?md
```

UE 鏄細

```text
ULevel Package loaded
  -> UWorld::AddToWorld
  -> UpdateComponents
  -> InitializeNetworkActors
  -> RouteActorInitialize
  -> BeginPlay
  -> InitializeRenderingResources
```

鐗圭偣锛?
```text
鍒嗛樁娈点€?鏀寔鏃堕棿鐗?/ 澧為噺澶勭悊銆?缁勪欢娉ㄥ唽鍜?Actor 鍒濆鍖栧垎绂汇€?Level 鍙鏄渶鍚庨樁娈点€?```

### Unity

鍙傝€冿細

```text
Unity婧愮爜鍙傝€?Scene-Prefab-GameObject-Instantiation婧愮爜鍙傝€?md
```

Unity 鏄細

```text
SceneManager.LoadScene / LoadSceneAsync
  -> native scene load
  -> root GameObjects
  -> sceneLoaded event

Object.Instantiate / PrefabUtility.InstantiatePrefab
  -> clone GameObject tree
  -> attach to scene / parent
```

鐗圭偣锛?
```text
鎺ュ彛蹇冩櫤绠€鍗曘€?鍚屾 / 寮傛閮芥湁銆?Prefab asset 鍜?Prefab instance 鍖哄垎鏄庣‘銆?澶ч噺缁嗚妭钘忓湪 native 榛戠洅銆?```

### Bevy

鍙傝€冿細

```text
Bevy婧愮爜鍙傝€?Scene-WorldAsset-EntityMap-Instantiation婧愮爜鍙傝€?md
```

Bevy 鏄細

```text
DynamicWorld / WorldAsset
  -> WorldInstanceSpawner
  -> allocate all target Entity
  -> copy / insert components
  -> EntityMap / MapEntities remap internal Entity references
  -> InstanceId tracks spawned entities
```

鐗圭偣锛?
```text
闈炲父閫傚悎 ECS銆?鍏堝缓 Entity 绌哄３锛屽啀鍐欑粍浠躲€?鏄惧紡淇濆瓨瀹炰緥鏄犲皠銆?despawn instance 鍙寜 entity_map 鍒犻櫎銆?```

## 鍙€夋柟妗?
### 鏂规 A锛歎nity-like 绠€鍗?SceneLoader

```text
load_scene(scene_id)
  -> 閫掑綊鍒涘缓 Entity
  -> 鎻掑叆缁勪欢
  -> 璧勬簮寮曠敤鐩存帴鍔犺浇
```

浼樼偣锛?
```text
鏈€绠€鍗曘€?绗竴鐗堝紑鍙戝揩銆?鐢ㄦ埛蹇冩櫤鎺ヨ繎 Unity銆?```

缂虹偣锛?
```text
娌℃湁鏄庣‘瀹炰緥鍖栭樁娈点€?Prefab 鍐呴儴 Entity 寮曠敤涓嶅ソ澶勭悊銆?澶辫触鏃跺緢闅惧洖婊氬崐瀹炰緥鍖栫姸鎬併€?鍚庢湡澶у瀷鍦烘櫙澧為噺鍔犺浇闅炬敼銆?```

### 鏂规 B锛歎E-like 闃舵鍖?Scene Instantiate Job

```text
SceneInstantiateJob:
  LoadAssets
  AllocateEntities
  AttachComponents
  ResolveReferences
  Activate
```

浼樼偣锛?
```text
澶у瀷椤圭洰鑳藉姏寮恒€?浠ュ悗鍙椂闂寸墖 / 鍒嗗抚銆?璇婃柇娓呮櫚銆?```

缂虹偣锛?
```text
绗竴鐗堟瘮 A 澶嶆潅銆?濡傛灉闃舵澶锛孉I 鐢熸垚鍜岃皟璇曚細鏈夌悊瑙ｆ垚鏈€?```

### 鏂规 C锛欱evy-like EntityMap 瀹炰緥鍖?
```text
RuntimeSceneData / RuntimePrefabData
  -> source_to_runtime_entity_map
  -> allocate all Entity skeleton
  -> insert components
  -> remap EntityRef
  -> RuntimeSceneInstance / RuntimePrefabInstance
```

浼樼偣锛?
```text
鏈€閫傚悎 Rust ECS銆?Prefab 鍐呴儴寮曠敤鍙淮鎶ゃ€?despawn / unload 娓呮櫚銆?AI 鑳借鎳?source entity 鍒?runtime entity 鐨勬槧灏勩€?鍜屽綋鍓嶈嚜鐮?ECS 璺嚎鍖归厤銆?```

缂虹偣锛?
```text
闇€瑕佸畾涔?EntityRef remap 瑙勫垯銆?闇€瑕佸疄渚嬭褰曠粨鏋勩€?闇€瑕佹瘮绠€鍗?loader 澶氫竴涓?report銆?```

### 鏂规 D锛欳 + B-min

鏍稿績绠楁硶閲囩敤 Bevy-like EntityMap锛屾墽琛屽澹抽噰鐢ㄦ渶灏?UE-like 闃舵銆?
```text
RuntimeInstanceLoader
  -> SceneInstantiatePlan
  -> SceneInstantiateJob
  -> RuntimeSceneInstance

闃舵鍥哄畾涓?5 涓細
  ResolveAssets
  AllocateEntities
  AttachComponents
  RemapReferences
  Activate
```

浼樼偣锛?
```text
淇濈暀澶у瀷椤圭洰鑳藉姏銆?涓嶈繃搴﹀鏉傘€?AI 鍙嬪ソ锛孯eport 鑳借В閲婃瘡涓€姝ャ€?鍚庢湡鍙墿灞曚负澧為噺瀹炰緥鍖栥€?```

缂虹偣锛?
```text
姣?A 澶氫竴浜涚粨鏋勩€?绗竴鐗堣琛ユ祴璇曞拰璇婃柇瀛楁銆?```

## 鎺ㄨ崘鏂规

鎺ㄨ崘鏂规 D锛欱evy-like EntityMap + UE-like 鏈€灏忛樁娈靛寲銆?
绗竴鐗堜笉瑕佸仛瀹屾暣 streaming锛屼笉瑕佸仛澶嶆潅鐢熷懡鍛ㄦ湡锛屼笉瑕佸仛 Prefab override銆?
绗竴鐗堝彧鍋氾細

```text
RuntimeSceneInstance
RuntimePrefabInstance
RuntimeInstanceId
SourceEntityId -> RuntimeEntityId map
SceneInstantiateReport
PrefabInstantiateReport
RuntimeInstanceLoader
Scene unload / Prefab despawn
Asset handle tracking
```

## 鏍囧噯娴佺▼

### Scene load

```text
load_scene_instance(scene_ref):
  1. RuntimeAssetLoader load(scene asset)
  2. parse RuntimeSceneData
  3. collect AssetRef
  4. load required assets
  5. allocate runtime entities for every scene entity
  6. insert Transform / Hierarchy / Renderable / built-in components
  7. insert project components from schema data
  8. remap EntityRef fields
  9. activate scene instance
  10. return RuntimeSceneInstance
```

### Prefab instantiate

```text
instantiate_prefab(prefab_ref, parent_entity optional, target_scene optional):
  1. RuntimeAssetLoader load(prefab asset)
  2. parse RuntimePrefabData
  3. allocate runtime entities for prefab entity tree
  4. attach to parent / target scene
  5. insert components
  6. remap internal EntityRef
  7. activate prefab instance
  8. return RuntimePrefabInstance
```

### Scene unload

```text
unload_scene_instance(scene_instance_id):
  1. deactivate scene instance
  2. despawn all runtime entities in instance map
  3. release asset handles owned by scene instance
  4. remove instance record
  5. emit report
```

### Prefab despawn

```text
despawn_prefab_instance(prefab_instance_id):
  1. despawn all runtime entities in prefab instance map
  2. release asset handles owned by prefab instance
  3. remove instance record
```

## 鏍稿績鏁版嵁缁撴瀯

```text
RuntimeInstanceId
RuntimeSceneInstance:
  instance_id
  scene_asset_guid
  scene_id
  root_entities
  source_to_runtime_entity
  owned_asset_handles
  state

RuntimePrefabInstance:
  instance_id
  prefab_asset_guid
  root_entity
  parent_entity optional
  target_scene_instance optional
  source_to_runtime_entity
  owned_asset_handles
  state

SceneInstantiateReport:
  request_id
  scene_ref
  stage
  created_entity_count
  loaded_asset_count
  diagnostics[]
  source_to_runtime_entity_debug
```

## 涓轰粈涔堥€傚悎鎴戜滑

鎸変紭鍏堢骇鍒ゆ柇锛?
```text
AI 鍙嬪ソ锛?  鏄惧紡 map / report / stage锛孉I 鑳借拷韪€滆繖涓?Entity 浠庡摢鏉モ€濄€?
澶嶆潅椤圭洰锛?  鏀寔 Prefab 鍐呴儴寮曠敤銆丼cene unload銆佸悗鏈?additive scene 鍜屽閲忓疄渚嬪寲銆?
鍚庢湡鍙淮鎶わ細
  璧勬簮鍔犺浇銆佸疄渚嬪寲銆丒CS 鍐欏叆鍒嗗眰娓呮銆?
绠€鍗曪細
  闃舵鍙湁 5 涓紝涓嶅紩鍏?UE 鍏ㄥ Actor 鐢熷懡鍛ㄦ湡銆?
鏁堢巼锛?  鍏堝悓姝ュ疄鐜帮紝缁撴瀯淇濈暀鍚庣画鎵归噺鍒嗛厤鍜屽垎甯у疄渚嬪寲绌洪棿銆?```

## 涓庡叾瀹冨紩鎿庡姣?
| 椤圭洰 | UE | Unity | Bevy | 鎴戜滑 |
|---|---|---|---|---|
| 鍦烘櫙鍗曚綅 | World / Level | Scene | Scene / DynamicWorld / WorldAsset | RuntimeSceneInstance |
| 瀵硅薄鍗曚綅 | Actor / Component | GameObject / Component | Entity / Component | Entity / Component |
| Prefab/妯℃澘 | Blueprint / Actor Class | Prefab Asset | Scene / WorldAsset | RuntimePrefabData |
| 瀹炰緥鍙ユ焺 | Actor / Level | Scene handle / GameObject | InstanceId | RuntimeInstanceId |
| 鍐呴儴寮曠敤淇 | UObject 寮曠敤浣撶郴 | native object ref | EntityMap / MapEntities | SourceEntityId -> RuntimeEntityId remap |
| 鍔犺浇闃舵 | 寰堢粏銆佸彲鏃堕棿鐗?| 绠€娲佹帴鍙ｃ€乶ative 榛戠洅 | ECS 鍐欏叆娓呮櫚 | 5 闃舵銆乺eport-first |
| AI 鍙鎬?| 寮?| 涓?| 涓珮 | 楂?|
| 绗竴鐗堝鏉傚害 | 楂?| 浣?| 涓?| 涓綆 |

## 绗竴鐗堣竟鐣?
绗竴鐗堜笉鍋氾細

```text
Prefab override / variant銆?杩愯鏃剁紪杈?Prefab asset銆?澶嶆潅 Scene streaming銆?鐪熷疄鍒嗗抚 time-slice銆?璺ㄥ満鏅紩鐢ㄨ嚜鍔ㄤ慨澶嶃€?瀹屾暣 Project Logic lifecycle銆?```

绗竴鐗堝繀椤诲仛锛?
```text
Scene entity tree -> Rust ECS銆?Prefab entity tree -> Rust ECS銆?AssetRef -> RuntimeAssetLoader銆?source entity -> runtime entity map銆?鍐呴儴 EntityRef remap 鐨勬渶灏忚鍒欍€?Scene unload / Prefab despawn銆?缁撴瀯鍖?InstantiateReport銆?headless 鍗曞厓娴嬭瘯銆?```

## 施工状态

本方案已经完成对应施工：

```text
施工文档/已完成/70-当前可自动化施工文档-Scene-Prefab-Entity-Runtime实例化-v1.md
阶段完成记录/2026-06-26-Scene-Prefab-Entity-Runtime实例化-v1/
```

完成结果：

```text
Runtime Package 中的 Scene / Prefab 已能实例化到 Rust ECS，并可被 RenderExtract 读取。
```