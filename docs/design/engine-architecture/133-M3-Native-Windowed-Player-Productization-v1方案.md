# 133-M3 Native Windowed Player Productization v1 方案

## 问题是什么

M3 解决的不是重新设计 Player Runtime，也不是把窗口系统塞进 `engine_runtime`。

现有文档已经确认：

```text
117-Runtime-WGPU-Surface注入-WindowedPlayerPresent-v1方案.md
118-WindowedPlayer-Runtime-v1完整方案.md
128-Playable-Windows-Export-Vertical-Slice-v1方案.md
130-复杂打飞机编辑到Windows可玩项目缺失能力当前基线.md
```

当前缺口是产品化入口：

```text
RuntimePackage
  -> Native Windowed Player exe
  -> 双击 exe / 命令行启动
  -> 自动找到 data/runtime_package
  -> 加载 package
  -> 跑 FrameLoop / Logic / RenderProjection / RHI 主线
  -> Window / Surface / Present
  -> 输出可诊断 report
```

第一版必须把路径发现、报告、失败定位、headless 自动化验证固定下来。真实 OS window / wgpu surface 继续属于 native host 层，不下沉到 `engine_runtime`。

## 其他引擎怎么做

| 引擎 | 对应结构 | 对我们的启发 |
|---|---|---|
| Unity | BuildPlayer 生成 Player exe + Data；PlayerLoop 运行逻辑；图形后端负责窗口和 swap/present | 可执行程序和数据目录是产品边界；编辑器验证和最终 Player 应共用同一份构建产物 |
| Unreal Engine | Cook / Stage / Package；GameInstance / GameViewportClient；RenderThread / RHI；平台层创建窗口 | Runtime/Renderer/Platform 分层清楚；Game 层不直接拥有底层窗口资源 |
| Godot | 导出 exe + pck/data；OS / DisplayServer 管理窗口；MainLoop 跑项目 | 项目数据包和平台显示服务分离；运行报告应能定位包加载或显示层问题 |
| Bevy | App/World/Schedule 与 WindowPlugin/WinitPlugin 分离；RenderPlugin 处理 GPU present | `World/App` 主线和 `Window/Winit` runner 分离，headless 与 windowed 只在 runner/window adapter 分叉 |

结论：成熟引擎不会让核心 runtime 直接散落处理 OS window、包路径、平台发布细节。它们都有一个产品化启动层，把数据包、窗口、主循环和报告接起来。

## 可选方案对比

| 方案 | 内容 | 优点 | 缺点 | 结论 |
|---|---|---|---|---|
| A | 继续只用 `runtime_cli --package` 手动运行 | 最简单 | 不是用户级 Player，不能双击 exe，也不能验证发布包结构 | 不选 |
| B | 在 `engine_runtime` 里直接实现 windowed player | 调用路径短 | 破坏现有边界，把 winit/wgpu surface 下沉到 runtime，后期多平台/RHI 会变脏 | 不选 |
| C-min | 建立 Native Player 产品化入口，路径发现/报告/host handoff 固化；真实 window host 仍在平台层 | 长期边界正确，能自动化测试，AI 容易定位错误 | 第一版真实窗口仍可 gated，需要后续 native host 接入 | 选择 |

## 推荐方案

采用 C-min：

```text
NativePlayerLaunchRequest
  package_override: Option<Path>
  report_override: Option<Path>
  frame_limit: u64
  mode: HeadlessGate | Windowed

NativePlayerPathResolver
  1. --package 显式覆盖
  2. current_exe_dir/data/runtime_package
  3. current_dir/data/runtime_package

NativePlayerHostAdapter
  HeadlessGate:
    RuntimePackage -> WindowedPlayerHost::run_headless_gate
  Windowed:
    当前 C-min 返回 native_window_host_required 结构化报告
    后续由 native window host 接入同一 request/report 契约

Report
  1. --report 显式覆盖
  2. current_exe_dir/reports/windowed-player-run-report.json
  3. current_dir/reports/windowed-player-run-report.json
```

## 关键规则

1. `engine_runtime` 不创建 OS window，不直接依赖 winit/wgpu surface。
2. `runtime_cli` / native player entry 负责产品化启动：路径发现、报告写入、命令参数、host handoff。
3. headless 自动化和真实 windowed 用户模式必须共用：

```text
RuntimePackage -> World Hydration -> EngineHostLoop -> RenderProjection -> RuntimeRenderer/RHI
```

4. 两条模式只允许在这里分叉：

```text
Window / Surface / Present adapter
```

5. 第一版必须能区分：

```text
package_missing
package_load_error
native_window_host_required
runtime_success
report_write_error
```

6. 所有字段使用引擎基础概念，不引入 bullet / enemy / health 等项目玩法概念。

## 为什么适合我们

| 优先级 | 判断 |
|---|---|
| AI 友好 | 入口、路径、报告结构固定，AI 能从 report 直接判断失败层 |
| 复杂项目 | 不把项目规则塞进 Player；复杂度留在 RuntimePackage / Asset / Rule / Render 主线 |
| 可维护 | `engine_runtime` 继续纯运行时，native host 层接平台差异 |
| 简单 | 不新增第二套 runtime，不新增玩法 API |
| 效率 | 真实 windowed 后续直接走同一 RHI/RenderThread 主线，不经过解释层或临时 UI 路径 |

## M3 v1 完成标准

```text
run-native-player --headless-gate
  能从 data/runtime_package 默认发现 package
  能用 --package 覆盖 package
  能写默认 report
  能用 --report 覆盖 report
  package 缺失时写结构化失败 report
  windowed 模式当前明确返回 native_window_host_required
```

M3 v1 完成的是产品化启动契约，不宣称真实商业级窗口 player 已完成。真实 window/surface/present 的 native host 接入，是 M3 后续或 M4/M5 的施工内容，但必须复用本文件定义的 request/report/path contract。
