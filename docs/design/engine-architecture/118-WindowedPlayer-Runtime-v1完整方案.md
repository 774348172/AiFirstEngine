# 118-Windowed Player Runtime v1 主线方案

## 问题

117 已经完成真实 `wgpu::Surface` 注入和一帧 present：

```text
WindowedRuntimeHost
  -> winit Window / wgpu Surface
  -> RuntimeRenderer
  -> RhiCommandPlan v2
  -> RealWgpuBackend
  -> present
```

但这还不是完整 Player Runtime。完整 Player 需要从真实 RuntimePackage 启动，把场景、资源、World、逻辑、渲染、RHI、窗口 present 串成正式主线：

```text
RuntimePackage
  -> Runtime World
  -> Scene Lifecycle
  -> Asset Load
  -> GPU Resource Binding
  -> RenderProjection
  -> RuntimeRenderer
  -> RhiCommandPlan
  -> RealWgpuBackend
  -> Windowed Player Present
  -> Input / Time / Physics / Gameplay Frame
```

本阶段选择方案 C：完整主线一次定型，但施工必须一个小模块一个小模块推进，每个模块完成后先测试，通过后再进入下一个模块。

当前 118 施工完成后的边界必须明确：

```text
118 已完成 engine-side gate。
它不是最终真实 Windowed Player。
engine_runtime 不创建 OS window，不直接拥有 winit / wgpu Surface。
真实 package-to-window Player 仍是后续缺口。
```

## 其它引擎对比

| 引擎 | 对应路线 | 可借鉴点 | 不照搬点 |
|---|---|---|---|
| Unreal Engine | GameInstance / World -> GameViewport / SceneViewport -> Scene / PrimitiveProxy -> RenderThread -> RHI Viewport Present | Player 入口、World、Viewport、RenderThread、RHI 分层清楚 | 不第一版做完整 GameInstance、Slate、复杂 viewport、多平台 RHI 全量能力 |
| Unity | PlayerLoop -> Scene / GameObject / Renderer -> Camera / RenderPipeline -> Graphics backend -> Player Window | 用户不直接接触底层图形 API，Renderer 组件自然进入渲染管线 | 不做 MonoBehaviour/IL2CPP 路线，不把项目脚本和底层渲染绑定 |
| Bevy | App / World -> Extract -> RenderApp / RenderGraph -> RenderDevice / RenderQueue -> Window Surface | Rust + ECS + wgpu 技术栈接近，Main World 与 Render World 分离 | 不把 wgpu 作为长期唯一抽象，不照搬 Bevy plugin 复杂度 |
| Godot | SceneTree -> RenderingServer -> RenderingDevice -> DisplayServer Window | 场景逻辑、渲染服务、窗口服务分层 | 不第一版做完整 Server 架构和节点系统 |

## 最终选择

选择完整方案 C，但执行方式是：

```text
架构一次到位。
实现分 gate 落地。
每完成一个 gate 必须测试。
测试通过后才能进入下一个 gate。
```

这不是临时 B+，也不是为了快速显示画面绕过 RuntimePackage。所有可见内容必须从 RuntimePackage / World / Projection / Renderer / RHI 主线进入窗口。

## 标准结构

```text
WindowedPlayerHost
  owns:
    OS window / event loop / surface host
    EngineHostLoop
    RuntimePackage instance
    Runtime World
    PlayerRunReport

RuntimePackageLoader
  loads:
    manifest
    active scene
    asset manifest
    rule manifest

RuntimeWorldHydrator
  maps:
    RuntimeScene
      -> ECS World

RuntimeAssetLoader
  loads:
    Runtime AssetRef
    cooked asset records
    texture / mesh / material data

RuntimeGpuResourceBinder
  maps:
    TextureAsset -> GPU texture
    MeshAsset -> GPU buffer
    MaterialAsset -> pipeline / bind group descriptor

RenderProjection
  maps:
    ECS render-facing components
      -> RenderProxy common + typed payload

RuntimeRenderer
  maps:
    RenderProxy / RenderView
      -> RenderGraph
      -> RhiCommandPlan

RealWgpuBackend
  executes:
    resource upload
    draw commands
    surface present
```

## Gate 拆分

### Gate 1：WindowedPlayerHost 入口

```text
新增独立 PlayerHost 结构。
不依赖 EditorHost。
可 headless 测试。
真实窗口路径 feature-gated。
```

验收：

```text
能构造 PlayerHostRequest。
能生成 PlayerRunReport。
能对 windowed 模式给出明确 native host required / feature gate report。
```

### Gate 2：RuntimePackage Load

```text
从真实 RuntimePackage 目录加载。
package / manifest / scene / asset / rule 错误全部进入结构化 report。
```

### Gate 3：World Hydration

```text
RuntimeScene -> ECS World。
记录 entity / component / projection 计数。
```

### Gate 4：Runtime Asset Load

```text
AssetRef -> RuntimeAssetRecord / CookedAssetRecord。
Texture / Mesh / Material 最小加载。
```

### Gate 5：GPU Resource Binding

```text
Texture -> GPU texture。
Mesh -> GPU buffer。
Material -> pipeline / bind group descriptor。
重复资源走缓存。
丢失资源进入 report。
```

### Gate 6：RenderProjection

```text
ECS render-facing components -> RenderProxy typed payload。
Sprite2D / MeshRenderer 至少各有最小可见路径。
```

### Gate 7：RuntimeRenderer

```text
RenderProxy -> RenderGraph -> RhiCommandPlan。
可见对象必须生成 draw command。
```

### Gate 8：RealWgpuBackend Present

```text
RhiCommandPlan -> surface。
真实 ignored smoke 打开窗口并 present 一帧。
```

### Gate 9：Player FrameLoop

```text
Time / Input / ECS Systems / Physics / Render 顺序稳定。
连续多帧 report 可追踪。
```

### Gate 10：Playable Sample Gate

```text
用复杂打飞机样例验收引擎底座。
不能为了样例增加敌人、子弹、分数等项目专用引擎 API。
```

## 强制边界

```text
不能为了打飞机增加专用引擎 API。
不能把 Player 入口塞进 EditorHost。
不能让 RuntimeRenderer 直接依赖 winit / wgpu Surface。
不能绕过 RuntimePackage 直接手写临时场景。
不能跳过 report / test。
不能新增独立 Bridge，跨域同步继续归口 Projection / ProjectionAdapter。
```

## 报告规则

`WindowedPlayerRunReport` 必须按层定位问题：

```text
request
package
asset
scene
world
logic
input
physics
projection
render
rdg
rhi
surface
present
```

AI 查问题时必须能从 report 判断失败层，不需要读完整底层源码。

## 为什么适合我们

AI 友好：

```text
完整链路每层有 report，AI 能定位问题，不靠隐含调用栈猜。
```

复杂项目能力：

```text
和 UE / Unity 一样保留 Player、World、Renderer、RHI、Window 分层，大项目不会被一个临时 runner 卡死。
```

长期可维护：

```text
方案 C 一次确认正式主线，但施工分 gate，避免一次性大爆炸。
```

简单度：

```text
不为项目玩法增加引擎规则，只补通用底座。
```

性能：

```text
RuntimeRenderer 不碰窗口，WindowedPlayerHost 不碰项目玩法，RealWgpuBackend 只做 GPU 执行。后续 D3D12 / Vulkan / Metal backend 可以替换底层。
```
