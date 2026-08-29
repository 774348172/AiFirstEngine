<!-- Current Update: This historical stage record originally pointed to Asset DB / Importer MVP as the next stage. That direction has already completed its first architecture and implementation pass. The current mainline is Project Logic IR Interpreter / Rust AOT 与 Rust ECS 正式接入. Formal rule document: 31-Project-Logic-Runner-IR-RustAOT-ECS方案.md -->
# 闃舵 6 鏀舵暃锛氱湡瀹?Runtime System 鎺ュ叆 IR

鏈枃妗ｈ褰?IR Interpreter MVP 杩涘叆鐪熷疄 runtime system 鍚庣殑鏀舵暃缁撴灉銆?
## 鐩爣

闃舵 6 鐨勭洰鏍囦笉鏄妸鎵€鏈夌帺娉曡剼鏈兘杩佺Щ鍒?IR锛岃€屾槸楠岃瘉涓€鏉￠暱鏈熻竟鐣屾槸鍚︽垚绔嬶細

```text
Runtime System 璐熻矗 ECS 璇诲啓鍜岃皟搴?Canonical Rule IR 璐熻矗椤圭洰瑙勫垯璁＄畻
IR Interpreter 璐熻矗寮€鍙戞湡 / 楠岃瘉鏈?/ 鐑洿瑕嗙洊鎵ц
Runtime Trace 璐熻矗鎶婄郴缁熸墽琛屽拰 IR rule 鎵ц鏆撮湶缁欑敤鎴蜂笌 AI
```

杩欎釜杈圭晫鍙互閬垮厤鎶?IR 鍋氭垚瀹屾暣鑴氭湰璇█锛屼篃閬垮厤璁?IR 鐩存帴鎿嶄綔 ECS锛岄檷浣庨暱鏈熷鏉傚害銆?
## 宸叉帴鍏ョ殑鐪熷疄绯荤粺

### Spin3D

瑙勫垯锛?
```text
rotationY = rotationY + speed * deltaTime
```

鎺ュ叆鏂瑰紡锛?
```text
script.Spin3D system
  -> 璇诲彇 Transform.rotation.y 鍜?script.params.speed
  -> 缁勮 IR input
  -> interpretFunctionRule(prototype.spin3d.rotation)
  -> 鍐欏洖 Transform.rotation.y
  -> 鍐欏叆 FrameContext.irTrace
```

楠岃瘉浠峰€硷細

```text
璇佹槑 Transform 绫荤粍浠舵暟鎹彲浠ラ€氳繃 IR 瑙勫垯璁＄畻銆?璇佹槑鐪熷疄 system 鍐呴儴 IR trace 鍙互杩涘叆 RuntimeTraceReport.irRules銆?```

### GameScore

瑙勫垯锛?
```text
score = Number(score ?? 0)
lives = Number(lives ?? 3)
```

鎺ュ叆鏂瑰紡锛?
```text
script.GameScore system
  -> 璇诲彇 script.params.score / lives
  -> 鍦?system 杈圭晫澶勭悊杈撳叆姝ｈ鍖?fallback
  -> 缁勮 IR input
  -> interpretFunctionRule(prototype.game_score.normalize)
  -> 鍐欏洖 script.params.score / lives
  -> 鍐欏叆 FrameContext.irTrace
```

楠岃瘉浠峰€硷細

```text
璇佹槑 IR 涓嶅彧閫傚悎 Transform 婕旂ず鏁版嵁锛屼篃鑳界鐞嗛」鐩剼鏈弬鏁般€?璇佹槑鍚屼竴濂?Runtime Trace 鍙互鍚屾椂鎵胯浇澶氫釜鐪熷疄 IR rule銆?```

### Projectile Cleanup Lifetime

瑙勫垯锛?
```text
shouldDestroy = projectile.age > projectile.lifetime
```

鎺ュ叆鏂瑰紡锛?
```text
engine.Cleanup system
  -> 璇诲彇 RuntimeState.age 鍜?ProjectileMover.params.lifetime
  -> 缁勮 IR input
  -> interpretFunctionRule(prototype.projectile.cleanup_lifetime)
  -> 鏍规嵁 output.shouldDestroy 鍙?destroy command
  -> 鍐欏叆 FrameContext.irTrace
```

楠岃瘉浠峰€硷細

```text
璇佹槑 IR 鍙互鍙備笌缁撴瀯鍙樺寲鍓嶇殑鍐崇瓥锛屼絾涓嶇洿鎺ユ搷浣?ECS World銆?璇佹槑瀹炰綋閿€姣佷粛鐢?Runtime System 鍜屽懡浠ら槦鍒楁墽琛岋紝IR 鍙緭鍑哄彲瀹℃煡鍒ゆ柇缁撴灉銆?璇佹槑灏忓瀷 projectile 鐢熷懡鍛ㄦ湡瑙勫垯鍙互鍦ㄤ笉淇敼 IR 鏍稿績璇箟銆佷笉淇敼 ECS 璋冨害瑙勫垯鐨勬儏鍐典笅鍙楁帶鎺ュ叆銆?```

## 褰撳墠瀹炵幇鏂囦欢

```text
src/engine/systems.ts
src/engine/frameLoop.ts
src/engine/runtime.ts
src/runtime-backends/typescript/TypeScriptRuntimeBackend.ts
scripts/test-runtime-backend.cjs
```

## 褰撳墠娴嬭瘯

宸叉墽琛岋細

```powershell
npm.cmd run test:runtime
npm.cmd run test:scenario
npm.cmd run test:interpreter
npm.cmd run test:ir
npm.cmd run test:schema
node scripts\validate-project.cjs $env:TEMP\ai-first-validation-smoke.json
npm.cmd run build
```

缁撴灉锛?
```text
鍏ㄩ儴閫氳繃銆?```

褰撳墠宸茶鐩栵細

```text
starter project tick 鍚?Coin.rotation.y 澧炲姞 2.8 / 60銆?RuntimeTraceReport.irRules 鍖呭惈 prototype.spin3d.rotation銆?shooter project tick 鍚?GameScore 瑙勫垯鎵ц銆?RuntimeTraceReport.irRules 鍖呭惈 prototype.game_score.normalize銆?GameScore score = "42" / lives 缂虹渷鏃朵細姝ｈ鍖栦负 score = 42 / lives = 3銆?expired projectile tick 鍚庣敱 engine.Cleanup 閿€姣併€?RuntimeTraceReport.irRules 鍖呭惈 prototype.projectile.cleanup_lifetime銆?```

## 璁捐缁撹

闃舵 6 宸茬粡瓒冲鏀舵暃锛屽彲浠ヨ繘鍏ヤ笅涓€闃舵銆?
鍘熷洜锛?
```text
宸茬粡鏈変笁涓湡瀹?runtime system 鎺ュ叆 IR銆?绯荤粺瑕嗙洊涓嶅悓鏁版嵁闈細Transform Component銆丼cript Params銆丷untimeState 涓?command 鍐崇瓥銆?IR trace 宸茬粡浠庣湡瀹?system 鍐呴儴杩涘叆 RuntimeTraceReport銆?娌℃湁鏂板澶嶆潅璋冨害瑙勫垯銆?娌℃湁璁?IR 鐩存帴璇诲啓 ECS銆?娌℃湁鎶婅В閲婂櫒鎵╁紶鎴愯嚜鐢辫剼鏈瑷€銆?```

## 涓嶇户缁墿澶ч樁娈?6 鐨勫師鍥?
涓嶅缓璁户缁縼绉绘洿澶氬綋鍓嶇帺娉曡剼鏈€?
鍘熷洜锛?
```text
缁х画杩佺Щ Collision / Spawn 浼氱壍娑夊鏉傜粨鏋勫彉鍖栥€佺鎾炶涔夊拰璧勬簮瀹炰緥鍖栥€?杩欎簺闂灞炰簬 Runtime Core / Asset / Build / Rust Runtime 鐨勫悗缁樁娈碉紝涓嶅簲璇ュ杩?IR Interpreter MVP銆?闃舵 6 鐨勭洰鐨勫凡缁忚揪鎴愶細璇佹槑杈圭晫鎴愮珛锛岃€屼笉鏄畬鎴愭墍鏈夌帺娉曠郴缁熻縼绉汇€?```

## 涓嬩竴闃舵寤鸿

寤鸿杩涘叆闃舵 9锛欰sset DB / Importer MVP銆?
鍘熷洜锛?
```text
Rust Runtime MVP 涔嬪墠锛孭roject Schema 涓?AssetRef 宸茬粡瀛樺湪锛屼絾杩樼己鐪熷疄 Asset DB銆?Asset DB 鏄悗缁?Rust Runtime銆丅uild Graph v2銆佽祫婧愮儹鏇淬€丄I 璧勬簮鐢熸垚鐨勫叡鍚屽墠缃潯浠躲€?瀹冩瘮鐜板湪鐩存帴鍐?Rust Runtime 鏇磋兘鍑忓皯鍚庣画杩斿伐銆?```

闃舵 9 鐨勬渶灏忓疄鐜板簲鍖呮嫭锛?
```text
asset.meta.json 鏁版嵁妯″瀷
GUID
source path
asset type
importer type
dependencies
asset state
external file sync
import lock
```

绗竴鐗堜笉鍋氬畬鏁?UI锛屼笉鍋氱湡瀹炲浘鐗?/ glTF 杞崲锛屽彧鍏堝浐瀹氭暟鎹ā鍨嬨€佹壂鎻忋€佸悓姝ャ€佹牎楠屽拰娴嬭瘯銆?
