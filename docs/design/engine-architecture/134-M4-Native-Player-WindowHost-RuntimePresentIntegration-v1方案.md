# 134-M4 Native Player WindowHost / Runtime Present Integration v1 方案

## 问题是什么

M4 不是重新设计 Player Runtime。

已有规则：

```text
117-Runtime-WGPU-Surface注入-WindowedPlayerPresent-v1方案.md
118-WindowedPlayer-Runtime-v1完整方案.md
133-M3-Native-Windowed-Player-Productization-v1方案.md
```

当前缺口是把 M3 的产品化入口真正接到 native window host：

```text
run-native-player --mode windowed
  -> Native Player Host
  -> OS Window
  -> wgpu Surface
  -> RuntimePackage load
  -> World hydration
  -> EngineHostLoop continuous tick
  -> RuntimeRenderer / RHI command plan
  -> Surface acquire / submit / present
  -> WindowedPlayerRunReport
```

现有 `editor_window_winit::windowed_runtime_present` 已经有真实 surface smoke 和 headless present backend，但它仍在 editor window crate 下，而且真实路径主要跑 `minimal_world()`，不是从真实 `RuntimePackage` 启动。M4 要把 Player Host 做成正式 runtime platform 层，而不是继续让 editor crate 承担 Player 职责。

## 其他引擎怎么做

| 引擎 | 对应结构 | 对我们的启发 |
|---|---|---|
| Unity | Player exe + Data；PlayerLoop；底层 graphics backend 创建窗口并 present | Player 是独立产品运行入口，读取构建数据，不依赖 Editor UI |
| Unreal Engine | Cook/Stage/Package；GameViewport/SceneViewport；RenderThread；RHI Present | Window/Viewport/RenderThread/RHI 分层，不让 World 或 Gameplay 直接碰 OS window |
| Godot | Exported executable + pck；MainLoop；DisplayServer；RenderingServer | 项目数据包、主循环、显示服务分层，窗口错误属于显示层报告 |
| Bevy | App/World/Schedule；WinitPlugin runner；Render/WGPU surface | Rust 路线里 runner/window 和 App/World 分离，headless 与 windowed 只在 runner 层分叉 |

共同结论：

```text
核心 runtime 不创建 OS window。
Player/native host 创建窗口和 surface。
World/FrameLoop/Renderer/RHI 主线必须复用。
headless 自动化和 windowed 用户运行只在 Window/Surface/Present 层分叉。
```

## 可选方案对比

| 方案 | 内容 | 优点 | 缺点 | 判断 |
|---|---|---|---|---|
| A | 让 `runtime_cli` 直接依赖 `editor_window_winit`，复用现有真实窗口代码 | 实现最快 | Player 依赖 Editor crate，长期边界错误，后期会把 editor/window/player 混在一起 | 不选 |
| B | 新建 `runtime_player_winit` crate，作为正式 Native Player Host；复用/迁移现有 windowed runtime present 思路 | 边界清晰，像 Unity/UE 的独立 Player 平台层，后续可多平台扩展 | 多一个 crate，需要接 runtime_cli 和测试 | 推荐 |
| C | 把真实 window host 下沉到 `engine_runtime` | 调用最短 | 违反 117/118/133，runtime core 依赖 winit/wgpu surface，后续 RHI/平台会失控 | 不选 |

## 推荐方案

选择 B，但第一版做 C-min 落地。

标准结构：

```text
runtime_cli
  -> run-native-player
  -> NativePlayerPathResolver
  -> runtime_player_winit

runtime_player_winit
  owns:
    NativePlayerWindowConfig
    NativePlayerWindowHost
    winit EventLoop
    OS Window
    wgpu Surface
    surface resize/lost/acquire/present
    input event collection boundary
    report adapter

engine_runtime
  owns:
    RuntimePackage load
    RuntimeScene -> World hydration
    EngineHostLoop
    RuntimeRenderer
    RenderThreadFrameOutput
    RHI command plan
    WindowedPlayerRunReport foundation
```

第一版目标：

```text
run-native-player --mode windowed
  在 real-window feature 下进入 runtime_player_winit
  从 M3 解析出的 RuntimePackage 路径加载真实 package
  hydrate World
  创建 OS window + wgpu surface
  连续 tick 至少 1 帧
  将 RHI command plan submit 到 surface view
  present
  写 WindowedPlayerRunReport / native host report
```

默认无 feature 或自动化环境：

```text
run-native-player --mode windowed
  不假装成功
  返回 native_window_host_required 或 feature_not_enabled
  写结构化 report
```

## 关键规则

1. `engine_runtime` 不依赖 `winit`，不创建 OS window，不拥有 `wgpu::Surface`。
2. `runtime_player_winit` 是正式 Player Host，不是 Editor Host。
3. `runtime_player_winit` 可以依赖 `engine_runtime` 的 `real-wgpu` feature，但不能被 `engine_runtime` 反向依赖。
4. `runtime_cli run-native-player --mode windowed` 必须复用 M3 的 package/report discovery。
5. `run-native-player --headless-gate` 必须继续存在，作为自动化测试主路径。
6. windowed user run 和 headless gate 必须共用：

```text
RuntimePackage -> World Hydration -> EngineHostLoop -> RenderProjection -> RuntimeRenderer/RHI
```

7. 两条路径只允许在这里分叉：

```text
Window / Surface / Present adapter
```

8. 不允许为了可见画面绕过 RuntimePackage 手写临时 scene。
9. 不允许新增项目玩法 API。M4 只处理窗口、surface、runtime present、报告。
10. 真实 OS window 测试必须 feature-gated / ignored smoke；默认单元测试使用 headless surface backend。
11. 所有失败必须进入结构化报告，至少包括：

```text
package
scene
world
logic
render
rhi
window
surface
present
```

## Report 规则

M4 第一版允许两层报告：

```text
WindowedPlayerRunReport
  给 editor/runtime_cli/AI 读，延续 118/133 的主报告

NativeWindowHostReport
  给 M4 内部定位 window/surface/present 细节
  可以被嵌入或摘要进 WindowedPlayerRunReport diagnostics
```

报告必须能回答：

```text
package 是否找到
package 是否加载成功
world 是否 hydration 成功
是否创建 window
surface 是否 configure 成功
是否 acquire surface frame
RHI submit 是否成功
是否 present
完成了几帧
失败在哪一层
```

## 施工 Gate

### Gate 1：新建 runtime_player_winit crate

```text
新增 crate。
默认 feature 不依赖 winit/wgpu。
real-window feature 才启用 winit/wgpu/pollster/engine_runtime real-wgpu。
```

### Gate 2：Headless Native Player Host

```text
实现 headless surface backend adapter。
从真实 RuntimePackage load + hydrate World。
运行 EngineHostLoop。
输出 NativeWindowHostReport。
```

### Gate 3：真实 Windowed Host

```text
real-window feature 下创建 winit EventLoop / OS Window / wgpu Surface。
调用 RealWgpuBackend::execute_plan_to_surface_view。
present 一帧或有限帧。
```

### Gate 4：runtime_cli 接入

```text
run-native-player --mode windowed
  feature enabled:
    调 runtime_player_winit
  feature disabled:
    保持结构化 native host required report
```

### Gate 5：测试

```text
默认测试：
  package discovery 仍通过
  headless native host 从 package 跑一帧
  missing package 报告正确
  feature disabled windowed 报告正确

feature smoke：
  cargo check -p runtime_player_winit --features real-window
  cargo test -p runtime_player_winit --features real-window real_windowed_player_smoke -- --ignored --nocapture
```

## 为什么适合我们

| 优先级 | 判断 |
|---|---|
| AI 友好 | 入口、host、surface、present 都有结构化报告，AI 能直接定位失败层 |
| 复杂项目 | Player Host 不认识玩法，只跑 RuntimePackage 和引擎主线 |
| 可维护 | 不让 editor crate 继续承担 Player 职责，也不污染 engine_runtime |
| 简单 | 第一版只做 native player host、有限帧、单窗口、单 surface |
| 效率 | 真实 windowed 路径直接走 RHI command plan -> surface present，不走临时 UI 或解释路径 |
| 多平台 | 后续可以新增 `runtime_player_windows` / `runtime_player_macos` / `runtime_player_web` 或 backend feature，不推翻 runtime core |

## 明确不做

M4 v1 暂不做：

```text
多窗口 / 多 surface
完整 frame pacing
全平台发布安装器
复杂输入映射全链路
真实 asset texture/mesh 高级资源绑定补完
脚本热更新
编辑器 Play Mode 体验重做
```

这些属于后续系统，不应塞进 M4 让规则膨胀。

## 自审结论

本方案没有推翻 117/118/133，而是把它们收敛到真实 Native Player Host。

检查结果：

```text
未让 engine_runtime 依赖 window。
未让 Player 继续挂在 editor crate 上。
未新增项目玩法规则。
保留 headless 自动化路径。
保留 feature-gated real window smoke。
明确了 report 层级和失败定位。
```

暂未发现必须补充的架构规则。下一步可以生成施工文档。
