# 107-复杂项目验证与样例游戏装配 C-min 方案

## 1. 问题是什么

94 到 106 已经补齐了一批引擎底座能力：

```text
ECS / Generic Component / CommandBuffer
Physics2D Foundation
Sprite2D / RenderCommand / RuntimeRenderer
Input Mapping
AUI Runtime UI
Prefab / Scene / Runtime Package
Trace / Golden Scenario
Editor Authoring
Build / Runtime Package Completion
```

现在最重要的问题不是再增加一个细小系统，而是验证这些系统能否组合成一个真实项目闭环。

本系统目标是：

```text
用一个复杂打飞机样例项目，验证当前所有已实现引擎系统是否能串起来。
```

但引擎层不能新增任何项目玩法概念。`player / enemy / bullet / health / score / wave` 只能出现在样例项目数据、项目 schema、项目 rule 中，不能成为引擎 API。

## 2. 成熟引擎参考

### Unity

Unity 通常通过：

```text
Sample Project
Scene / Prefab / Asset 工作流
PlayMode Test
BuildPipeline.BuildPlayer
BuildReport
```

来验证一整套功能是否能支撑项目，而不是只依赖单元测试。

### Unreal Engine

UE 通常通过：

```text
Sample Project
Automation Test
Functional Test
Gauntlet
BuildCookRun
```

来验证从内容、Cook、Stage、Run 到自动化报告的完整链路。

### Godot

Godot 使用：

```text
Demo Project
Scene / Resource / Export Preset
Editor run / native run
```

来验证项目级闭环。

### Bevy

Bevy 更偏向：

```text
Examples
ECS integration tests
App schedule tests
```

优点是轻量，缺点是编辑器、资产、打包链路覆盖较少。

## 3. 可选方案

### 方案 A：继续只做孤立单元测试

优点：

```text
简单
维护成本低
```

缺点：

```text
无法发现系统之间的断点
无法证明 Runtime Package / Runtime / Render / Trace 能形成闭环
AI 难以判断一个复杂项目到底哪里流转不通
```

不推荐。

### 方案 B：硬编码一个打飞机 Demo

优点：

```text
短期视觉结果明确
```

缺点：

```text
容易把 bullet / enemy / health / score 写成引擎概念
后期会污染引擎边界
```

不推荐。

### 方案 C-min：复杂项目验证与样例游戏装配

结构：

```text
Sample Project Description
  -> Engine-neutral Validation Fixture
  -> Runtime Package Build
  -> Runtime Load
  -> Input Resolve
  -> ECS Query / Write / Command
  -> Physics2D Sync / Collision Pair
  -> Render Extract / RenderCommand / RuntimeRenderer
  -> AUI Layout / Overlay
  -> Trace / Golden-style Report
  -> Gap Report
```

推荐采用。

## 4. 第一版验证范围

第一版样例场景是“复杂打飞机验证项目”，但代码中使用中性表达：

```text
controlled_entity
spawned_entity
target_entity
state component
score component
collision pair
action.move
action.fire
```

验证流程：

```text
1. 创建样例 Runtime Package 输入
2. 构建 Runtime Package
3. 加载 Runtime Package
4. 创建 Runtime World
5. 解析 InputMapping，得到 ActionSnapshot
6. 通过 ECS API 修改 Transform
7. 通过 GameplayCommandBuffer 创建 projectile-like entity
8. 同步 Physics2DWorld
9. 检测碰撞对
10. 用项目动态组件表达 state / score 修改
11. 提取 RenderCommand 并应用到 RenderSceneState
12. 构建 RuntimeRenderer headless frame
13. 构建 AUI draw list / overlay
14. 输出 ComplexProjectValidationReport
```

## 5. 报告规则

报告必须区分三类状态：

```text
passed：当前真实代码已经跑通
gap：当前引擎还缺真实能力或真实接入
simulated：验证中临时用 fixture 代替真实系统
```

第一版不允许为了让报告变好而隐藏缺口。

## 6. 非目标

第一版不做：

```text
真实游戏玩法框架
真实 AI 生成完整项目
真实素材导入与图片生成
真实窗口交互
真实项目规则编译
真实复杂关卡编辑器
真实多场景大型项目压力测试
```

这些后续系统应该由报告暴露出来，再独立讨论。

## 7. 正式规则

```text
复杂项目验证系统是验证门禁，不是 gameplay 系统。
打飞机只是样例项目内容，不进入引擎 API。
验证必须尽量覆盖当前已完成的引擎系统。
验证报告必须同时记录已跑通能力和流转缺口。
后续每完成一个大系统，都应该能被加入复杂项目验证门禁。
```

