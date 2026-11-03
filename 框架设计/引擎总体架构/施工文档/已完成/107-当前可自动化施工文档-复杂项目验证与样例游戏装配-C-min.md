# 107-当前可自动化施工文档-复杂项目验证与样例游戏装配 C-min

## 1. 施工目标

新增一个可自动化运行的复杂项目验证门禁，用复杂打飞机样例覆盖当前已实现引擎系统，并输出缺口报告。

## 2. 施工范围

### 2.1 文档

```text
新增 107-复杂项目验证与样例游戏装配-C-min方案.md
新增阶段完成记录
新增流转问题报告
```

### 2.2 代码

```text
engine_runtime::complex_project_validation
lib.rs 挂载模块
最小自动化测试
```

### 2.3 验证链路

```text
RuntimePackageBuilder
load_runtime_package
InputMapping / InputResolver
World / WorldApi / GameplayCommandBuffer
Physics2DBridge / CollisionPairReport
RenderExtract / RenderCommand / RenderSceneState
RuntimeRenderer headless
AUI Layout / DrawList / Overlay
ComplexProjectValidationReport
```

## 3. 验收标准

```text
cargo fmt --check
cargo test -p engine_runtime complex_project_validation
cargo test -p engine_runtime --no-run
```

如果 Windows Application Control 拦截测试二进制执行，需要记录为环境阻塞，而不是代码失败。

## 4. 施工边界

```text
不新增 engine 内置 bullet/enemy/health/score API。
不做真实 AI 项目生成。
不做真实素材导入。
不做真实窗口运行。
不做完整玩法系统。
```

