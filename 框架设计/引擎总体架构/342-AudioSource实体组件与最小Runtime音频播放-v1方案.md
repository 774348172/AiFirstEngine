# 342 AudioSource 实体组件与最小 Runtime 音频播放 v1

状态：2026-09-14 方案B施工完成，Gate A/B/C通过并归档，`source_and_windows_dev_delivery_passed`。用户确认实际回录无爆音、无拖音；[完成记录](阶段完成记录/2026-09-14-342-AudioSource-Runtime-Audio-v1/00-总览.md)与[归档施工](施工文档/已完成/342-AudioSource实体组件与最小Runtime音频播放-v1施工文档.md)保留完整证据。338已恢复当前，仍未完成、未归档；Provider未安装、宿主配置未改。

依据：[权威架构 v1.1](00-AI-First-Game-Engine-权威架构设计-v1.1.md)的 Project Game SDK、Deep Engine Capability、RuntimePackage 与组件边界；[338 项目记录](施工文档/历史/338-激光小屋Windows能力验证-v1施工文档.md)的真实音频缺口。339、340、341 已完成证据继续有效。

## 1. 选定方向

声音资源与发声对象分开：AudioClip 是可复用的音频资产；AudioSource 是实体上的纯数据组件，保存音频引用与音量；Runtime Audio Module 负责解码后的播放、暂停、停止和生命周期。项目通过公开 SDK 控制某个实体的 AudioSource。

像给游戏对象安装一个扬声器：项目决定何时按播放键，引擎负责把声音送到设备，并在对象销毁时收好这个扬声器。

长期保留的是“资产 → 实体组件 → 项目命令 → Runtime 执行”这条链。本期把它做通，范围为 Windows 非空间短音效。AudioSource 不带 2D 后缀，但本版不读取 Transform 做距离、方向或衰减计算；实体仍遵循既有 Transform/Hierarchy 规则。

## 2. 施工前基线与本方案增量

| 已有基础 | 本方案的最小增量 |
|---|---|
| Audio 资源类型、AssetRef、RuntimePackage asset index / cooked table | 将真实 WAV 音源纳入快照、依赖与 payload，增加真实 PCM 解码 |
| Scene/Prefab 的通用 components、共同 HydrationProjection | 注册强类型 `engine.audio_source`，复用已有组件容器与实例化路径 |
| SDK deferred mutation、native adapter、session commit | 增加针对 AudioSource 的三个命令入口 |
| 实体/Prefab/Scene 生命周期及 Windows Player | Runtime 管理 Source 播放状态和设备输出，随对象生命周期释放 |

当前 `neutral_assembler.rs::append_asset` 对非 texture 资产只装配 `.asset` 描述字节；`RuntimeAssetLoader::ensure_decoded` 当前只保存读取字节数，不能证明音频解码。`runtime_instance_loader.rs::collect_entity_asset_refs` 目前只收集 mesh/sprite 引用，音频必须加入同一实例资源持有/释放链。

旧 `07-Build-Export-Pipeline.md` 中的 HTMLAudioElement / JavaScript 后端属于历史 TypeScript 原型，不是本轮 Rust Native 音频基础，也不作为已有能力复用。

## 3. 成熟引擎参考与取舍

| 参考 | 具体源码与调用链 | 本方案采用的部分 |
|---|---|---|
| Unity AudioSource | [公开 bindings](I:/UnityCode/UnityCsReference-master/UnityCsReference-master/Modules/Audio/Public/ScriptBindings/Audio.bindings.cs:841)：组件持有 clip/volume；`Play()` → native `PlayHelper`，另有 Stop/Pause/UnPause | 资产与 Source 分开、按对象控制播放；不把设备句柄交给项目 |
| Godot AudioStreamPlayer | [play](I:/godotcode/godot-master/godot-master/scene/audio/audio_stream_player.cpp:109) → `play_basic()` → `AudioServer::start_playback_stream()`；[内部生命周期](I:/godotcode/godot-master/godot-master/scene/audio/audio_stream_player_internal.cpp:87) 管播放数量、暂停、删除时停止 | Source 与内部播放状态分开，释放由 Runtime 负责；首版固定每 Source 一个声音 |
| 对照项目激光小屋 | [main.gd::create_audio](G:/gameEngin/samples/laser_house_godot/main.gd:164) 创建四个 WAV/AudioStreamPlayer，事件调用 `.play()` | 四个普通 Source 足以覆盖本轮四类短音效，无需先建设音频编辑器 |

本地 Godot 参考为 4.8-dev 源码快照，Unity 参考是 C# 公开绑定，不据此声称掌握当代 Unity 的完整 native 后端。官方对照：[Godot 4.4 AudioStreamPlayer](https://github.com/godotengine/godot/blob/4.4-stable/scene/audio/audio_stream_player.cpp)、[Unity Audio bindings](https://github.com/Unity-Technologies/UnityCsReference/blob/master/Modules/Audio/Public/ScriptBindings/Audio.bindings.cs)。本轮联网复取超时，具体判断使用已核对的本地源码。

本引擎组件保持纯数据；下面的 `audio_source(...).play()` 是 SDK 命令写入器，组件本身不增加行为方法。Godot 的 Node 方法、Unity 的对象模型不原样搬入 ECS。

## 4. 项目侧最小合同

### 4.1 音频资产

AudioClip 是内容概念，继续使用已有 `audio` 资产种类。新增 `audio-asset.v1` 项目描述，沿用 `.asset` 文件与 AssetRef/GUID 规则：

```json
{
  "schemaVersion": "audio-asset.v1",
  "assetId": "audio-jump",
  "assetGuid": "feef8eb9-8055-4f24-8dd6-cef8529f1475",
  "sourceAudio": "Assets/Audio/jump.wav"
}
```

示例 GUID 仅作格式示范，实际项目为各资产分配自己的稳定 GUID。`sourceAudio` 是项目内源文件定位，不是 Runtime 播放接口。首版支持 RIFF/WAVE PCM16、单/双声道、44.1/48 kHz；完整短音频解码入内存。缺文件、错误格式、截断数据、非法采样信息在 check/prepare 时报告，不能当 JSON 描述或空声音继续通过。

### 4.2 实体组件

组件只增加两个字段；下面片段放入既有 Scene/Prefab 实体的 `components`：

```json
{
  "componentType": "engine.audio_source",
  "data": {
    "clipRef": {
      "id": "audio-jump",
      "type": "audio",
      "guid": "feef8eb9-8055-4f24-8dd6-cef8529f1475"
    },
    "volume": 0.4
  }
}
```

`clipRef` 必填，指向可解析的 audio 资产；`volume` 为有限线性增益，范围 `[0,1]`，缺省为 1。同一实体最多一个 AudioSource。播放实例、进度、暂停状态和设备 handle 不写入 Scene，也不反写资产。首版没有自动播放，加载后保持停止、未暂停。

首版配置在实例加载时确定，不增加运行时换 clip、音量自动化或专用字段编辑接口。

### 4.3 三个 SDK 命令

以下公开 SDK 写法已在本轮源码实现；安装宿主是否可用仍服从49的安装身份，不由源码通过推断：

```rust
context.mutations().audio_source(&source_entity).play();
context.mutations().audio_source(&source_entity).stop();
context.mutations().audio_source(&source_entity).set_paused(true);
context.mutations().audio_source(&source_entity).set_paused(false);
```

| 命令/情况 | 首版确定语义 |
|---|---|
| `play()` | 从头播放绑定 clip；同一 Source 再次调用则替换旧播放、从头重播，不累积声音 |
| 不同 Source 的 `play()` | 允许同时发声；相同 clip 的解码数据可以共享，各自进度独立 |
| `stop()` | 停止并释放当前播放实例；重复停止无副作用，不隐式解除暂停标志 |
| `set_paused(true/false)` | 暂停/继续当前进度；对无播放实例的 Source 也保存暂停标志，恢复不会自行新建播放 |
| 暂停状态下 `play()` | 替换为从零开始的单个暂停实例，保持暂停；没有命令积压队列，恢复最多继续这个实例 |
| 自然播放结束 | 回到停止状态并释放播放实例；组件和共享资产仍可用于下一次播放 |

音频的 `play()` 明确采用从头重播语义；341 Animator2D 的同名 play 保留进度，两者不混用。

命令复用现有 FixedUpdate/AUI session callback 提交通道；回调返回 `HandlerStatus::Applied` 才提交，NoOp/失败不播放。准备和验证阶段不能产生声音。接受后的命令按提交顺序只消费一次，多次 Present 不重复发声。不扩展 Rule IR，也不增加 AI 工具或工具调用前置步骤。

## 5. Runtime 与资源责任

完整链路为：

```text
项目 WAV + audio.asset + Scene/Prefab AudioSource
  → 既有 Compiler 快照、校验与装配
  → RuntimePackage 的音源字节、资产引用与组件数据
  → 既有资产加载 + HydrationProjection
  → SDK 提交的 Source 命令
  → Runtime Audio Module
  → Windows 音频输出
```

- Compiler 只消费 immutable SourceView；WAV 字节必须进入 sourceAudio 依赖和内容摘要，修改音源可使相关产物失效。使用既有 payload、asset index、cooked table，不新建 Audio Manifest、独立 Compiler 或缓存系统。
- AudioSource 沿 `RuntimeEntity.components` 装配，在共同 hydration 路径解为强类型 ECS 组件；复用 `engine.collider2d` 已有做法，不给 RuntimeEntity 再添并行的顶层音频字段。Scene/Prefab 的音源引用都进入现有实例资产持有/释放。
- 真实 PCM 解码进入既有资源加载责任域，只补 Audio 分支。相同 clip 的解码数据按现有资产身份共享，保留到相关资源持有者释放；运行期间只读交付包。Renderer、SceneLoader 和项目 DLL 都不持有音频设备。
- Runtime Audio Module 拥有 Source 播放状态、内部播放实例与输出适配；跨 ECS 同步使用既有 World Projection/Projection Adapter 规则，不新增独立 Bridge 或全套同步框架。生产组合由现有 Engine Runtime DLL/Player 承载，不新建音频 DLL。
- 输出适配优先采用成熟 Rust 音频库的 WAV/播放能力，例如 rodio/CPAL；具体版本与 feature 在施工时锁定，只启用本版必需项。设备线程、重采样及混合由成熟库承担，不自制设备驱动或混音器，不把第三方类型暴露到 SDK。
- 不含 AudioSource 的项目不要求打开音频设备。包含 AudioSource 的窗口路径在运行准备时检查输出可用性，并在首次游戏音效前准备 clip；读不到包、无法解码或设备输出失败必须进入现有诊断/失败摘要，不能静默降级后宣称有声验收通过。

SDK 增加 typed intent 后更新既有 contract digest 和相关 consumer。复用现有 C ABI/Generated Glue；没有音频的旧项目继续有效，新音频交付必须绑定包含本能力的 Runtime/DLL，不能把源码完成等同于旧安装宿主已支持。

## 6. 必要的生命周期

播放时间由音频设备的采样时钟推进；Fixed Tick 决定何时提交业务事件，渲染帧不充当音频时钟。Runtime 每轮控制更新都能处理停止/恢复和生命周期清理；即使本帧没有 Fixed Tick，也不能让旧声音滞留。

Source 随实体的实际 runtime identity 管理。实体禁用、AudioSource 移除、实体/Prefab 销毁、Scene 实例卸载时，Runtime 自动停止对应播放并释放持有；重新启用从停止、未暂停状态开始，不自动续播。session/Host 结束释放剩余播放及输出。清理由引擎负责，不要求项目在销毁对象前手动 stop 才能安全运行。

准备音频命令时捕获实际 RuntimeEntityId 及 generation，消费时复核，避免已排队命令命中新创建的同名实体。现有 SDK deferred wire 主要携带稳定实体 ID，本方案不冒称已解决所有 SDK 陈旧 handle；只保证本次已准备音频命令不跨实体世代执行，不顺带重构全局 handle 合同。

游戏暂停与“重新开始”仍是项目语义。激光小屋持有四个 Source 引用：暂停/恢复时调用各 Source 的 `set_paused`；重开时 `stop` 并解除暂停。可在项目函数中封装这几行代码，Runtime 不识别房间、死亡计数或 `house.paused`。四类音效在事件发生的边沿触发，不按持续状态每帧重播。

## 7. 必须证明的最小结果

| 风险/目标 | 必要验收 |
|---|---|
| 资产看似入包但没有真实声音数据 | Scene 与 Prefab 的 clipRef/volume 保真，真实 WAV 可解码；缺失/错误音源有定位；脱离项目源目录仍可播放；修改 WAV 内容会失效相关产物 |
| 重复播放、暂停或提交错误 | 同 Source 重播、不同 Source 并行、暂停/继续/停止；NoOp/失败提交不播放，已接受命令不因多次 Present 重放 |
| 对象退役后仍有声音 | 覆盖禁用/移除/销毁、Prefab 销毁、Scene 卸载、同名实体重建及零 Fixed Tick 更新；只验音频消费者，不借机扩建 Scene 系统 |
| 只有 playing 计数，没有实际输出 | 激光小屋四类事件通过同一 Windows 交付实际发声，暂停/重开正确；保留真实设备输出证据和事件对应关系，完成听感复核 |

确定性测试使用内部测试输出适配验证命令与释放；headless 结果明确不证明设备发声。真实 Windows 验收优先用设备回录与事件时间对照，再做听感复核；离线生成 WAV、命令次数或 `playing=true` 不能替代输出证据。设备不可用时保留明确未完成状态。

对照音色可按 Godot 项目现有四个 PCM16 音色离线生成项目 WAV（跳跃 0.18s、开关 0.35s、死亡 0.38s、通关 0.85s），使用相近音量，避免把音源素材差异误判为引擎播放差异。声音合成属于项目素材制作，不进入引擎 Core。

339/340/341 不重做；完整 workspace、性能基准、安装宿主升级不作为本方案的默认验收前置。后续施工只验证新增 owner、直接消费者和一次真实音频交付。

## 8. 范围与自审

首期范围到“实体 Source 控制短音效并随生命周期释放”为止。BGM/流式播放、loop/autoplay、单 Source 多声部、pitch/曲线、3D/Listener、混音台/总线/效果器、播放完成事件、音频编辑器、压缩转码矩阵与 Android 适配均不进入本版字段、API、任务或验收，不预留空壳实现。

- 长期入口：采用用户选定的实体 AudioSource；资产、组件、行为和设备责任分开。
- 项目使用成本：两个组件字段、三个命令入口；AI 沿用文件创作与现有九项工具，Skill 仅给轻量示例。
- 最小必要性：真实音源装配、typed Source、命令消费和自动释放都是闭合播放链必需项；复杂混音与编辑工具不属于本轮。
- 兼容性：沿用 RuntimePackage 组件容器、实例资源链与既有 C ABI；新增 SDK 合同及新版二进制消费仍需验证，不宣称零构建成本或旧宿主自动获得能力。
- 风险已限定：实际解码、失败摘要、命令只消费一次、对象世代及零 Fixed Tick 清理均有对应验收；不把设备时钟变成确定性玩法时钟。
- 施工状态：342已完成归档；证据见完成记录，后续执行槽以54为准。338保持未完成，不由音频子系统完成推断完整项目完成。

方案生成时的自审结论（2026-09-14）：已核对组件 hydration、实例资源持有/释放、SDK intent 与 RuntimeEntityId 世代实现，并完成独立只读复审；未发现需要扩大范围或改变 B 方向的问题。方案生成时尚未运行构建或音频测试，后续实际施工结果由本页顶部及施工文档维护。
