# Project Game SDK

公开项目接口与注册例子见 `src/lib.rs`。项目规则使用 `WorldRead` 与 deferred mutations，不直接持有引擎世界。

`kinematic2d::slide_aabb(body, displacement, solids)` 提供纯数据的轴对齐矩形运动辅助：输入世界坐标中心、半尺寸与本次位移，输出受阻后的中心、受阻轴及脚下 solid 索引。先处理 X，再处理 Y；调用方负责时间、重力、平台位移、触发器和世界写回。

它要求初始位置无穿透，不处理斜坡、旋转矩形、穿透恢复或刚体冲量。参考 `samples/laser_house_project/RuntimeModule/src/game.rs` 的平台承载与跳离；这些游戏规则不属于 SDK。

`context.mutations().audio_source(&entity)` 返回 `AudioSourceWriter`，控制实体上的 `engine.audio_source`。组件在 Scene/Prefab 中配置音频 `clipRef` 与 `[0,1]` 音量；命令不接受文件路径或临时换音源。

```rust
context.mutations().audio_source(&entity).play();
context.mutations().audio_source(&entity).stop();
context.mutations().audio_source(&entity).set_paused(true);
context.mutations().audio_source(&entity).set_paused(false);
```

在 FixedUpdate/AUI session 回调中提交，回调必须返回 `HandlerStatus::Applied`；NoOp、拒绝、失败和准备阶段都不播放。`play()` 从头重播该 Source 的唯一播放实例，保留暂停标志；`stop()` 释放播放实例而不解除暂停；`set_paused` 暂停/继续当前进度，恢复不会自行播放。不同 Source 可同时发声。

实体禁用、组件移除、销毁或 Scene 卸载时，Runtime 自动停止并释放声音；重新启用保持停止。项目负责游戏暂停、重开等业务事件，只在事件发生时提交命令，不按持续状态每帧重播。342 首版为 Windows 非空间短音效（PCM16 WAV，单/双声道，44.1/48 kHz）；设备输出失败会进入运行诊断，headless 控制验收不证明实际发声。
