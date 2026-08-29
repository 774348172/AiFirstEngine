# 187-Project Rule Artifact & Module Lifecycle B-min 方案

## 1. 系统定义

本系统正式命名为：

```text
Project Rule Artifact & Module Lifecycle v1
```

第一版采用：

```text
B-min：完整产物生命周期框架的最小落地。
```

它不是重新发明一套项目脚本系统，也不是把项目规则运行时改成动态 DLL 热加载。它解决的是：

```text
Canonical Rule IR
  -> Rust AOT 派生产物
  -> RuleArtifactManifest
  -> RuleArtifactRegistry
  -> RuleModuleLoader / Lifecycle
  -> RuleModuleRegistry
  -> ProjectLogicRunner
```

一句话：

```text
IR 仍然是唯一真相层；Rust AOT、manifest、registry、module lifecycle 都只是让 IR 派生产物安全进入 Runtime 的工程边界。
```

## 2. 为什么需要这一层

186 已经定义了 Project Rule Asset Pipeline，重点是项目规则如何从资产进入 Runtime。

187 进一步收敛的是“编译产物和模块生命周期”：

```text
如果只有 IR 和手写 RuleModuleRegistry，手写注册很容易长期变成第二真相层。
如果直接动态加载 DLL，第一版会过早承担 ABI、卸载、热替换、跨平台和安全问题。
所以第一版需要 manifest + registry + lifecycle 边界，但默认 module_kind 仍然是 StaticRegistry。
```

## 3. 参考引擎对比

### Unity

Unity 的对应心智是：

```text
C# Script / Assembly 编译
Serialized Script Reference
Domain / Assembly 加载
PlayerLoop 调用脚本入口
```

可学习点：

```text
脚本源码不是运行时唯一入口，编译产物、程序集、引用关系和生命周期必须被管理。
编辑器预览和 Player 运行应尽量走同一套脚本产物路径。
```

不照搬：

```text
不引入 C# Domain Reload 心智。
不把脚本源码或手写注册作为长期真相层。
```

### Unreal Engine

UE 的对应心智是：

```text
Blueprint Asset / C++ Source
Generated Class / 编译产物
Module 加载
UObject / Actor 生命周期
ProcessEvent / Tick
```

可学习点：

```text
资产、生成产物、模块加载、对象生命周期之间有明确边界。
运行时不直接相信编辑器临时状态，而是通过编译/注册后的产物执行。
```

不照搬：

```text
第一版不做 UE 式完整模块系统、Live Coding、Blueprint VM、反射宏体系。
```

### Godot

Godot 源码里的对应心智是：

```text
Script Resource
ScriptServer / Language 注册
ResourceLoader
GDExtension / Module 初始化与关闭
SceneTree 生命周期
```

可学习点：

```text
脚本语言、扩展模块、资源加载、生命周期注册都有清晰管理边界。
动态扩展不是随意加载 DLL，而是有初始化、校验、注册、关闭边界。
```

本系统最应学习 Godot 的点：

```text
manifest / registry / lifecycle discipline，而不是直接复制 Node、GDScript 或 Variant 调用模型。
```

### Bevy

Bevy 的对应心智是：

```text
Plugin
App / World
Schedule / System
Commands / apply_deferred
```

可学习点：

```text
系统注册和执行计划需要集中生成，运行时按计划执行。
```

不照搬：

```text
不把完整 Rust Schedule / Plugin 复杂度暴露给 AI 和普通项目作者。
```

## 4. 我们的正式规则

### 4.1 真相层规则

唯一真相层：

```text
Canonical Rule IR
```

以下内容全部不是规则真相层：

```text
Generated Rust Source
Rust AOT crate
RuleArtifactManifest
RuntimeRuleManifest
RuleModuleRegistry
ProjectLogicRunner
```

第一版可以手写 Rust AOT 注册函数，但必须被标记为：

```text
IR 派生产物占位
```

含义是：

```text
手写函数只模拟未来 IR -> Rust 代码生成器的输出形态。
它不能新增 IR 没有表达的业务语义。
它不能绕开 manifest / artifact_id / ir_hash / ABI 校验。
它不能成为新规则真相层。
```

### 4.2 产物结构规则

B-min 至少需要以下概念：

```text
RuleArtifactManifest
RuleArtifactManifestEntry
RuleArtifactRegistry
RuleModuleLifecycle
RuleModuleRegistry
```

最小字段：

```text
artifact_id
rule_id
ir_hash
abi_version
compiler_version
module_kind
generated_source_path
artifact_path
status
diagnostics
```

其中：

```text
artifact_id = rule-artifact:{rule_id}:{ir_hash}
```

### 4.3 module_kind 规则

第一版只允许真实执行：

```text
StaticRegistry
```

保留但不执行：

```text
DynamicValidationHost
```

DynamicValidationHost 的定位：

```text
未来动态模块验证入口占位。
第一版只允许 manifest 识别和校验，不允许 Runtime 真实动态加载、卸载或热替换。
```

### 4.4 生命周期规则

RuleModuleLifecycle 第一版最小状态：

```text
Declared
Validated
Registered
Ready
Rejected
```

含义：

```text
Declared：manifest 中声明了产物。
Validated：artifact_id / ir_hash / ABI / module_kind 校验通过。
Registered：StaticRegistry 已经把 rule_id 注册到 RuleModuleRegistry。
Ready：ProjectLogicRunner 可以按 RuntimeRuleManifest 执行。
Rejected：校验失败或注册缺失。
```

### 4.5 Editor Preview / Player Runtime 统一规则

编辑器预览和导出的 Windows Player 必须走同一条规则执行路径：

```text
RuntimeRuleManifest
  -> RuleArtifactRegistry validate
  -> RuleModuleRegistry
  -> ProjectLogicRunner
```

不得出现：

```text
编辑器一套临时规则执行。
Player 一套正式规则执行。
测试里绕过 manifest 直接执行项目逻辑。
```

### 4.6 不做范围

B-min 不做：

```text
运行时编译项目规则
真实 DLL 热加载
跨平台动态卸载 / reload
复杂脚本调试器
第二套 Interpreter
项目侧玩法规则 API 膨胀
```

## 5. 与 186 的关系

186 负责：

```text
ProjectRuleAsset / Canonical Rule IR / RuntimeRuleManifest / ProjectLogicRunner 主链路。
```

187 负责：

```text
Rule IR 派生产物如何被识别、校验、注册、进入运行生命周期。
```

两者关系：

```text
186 是规则资产流水线。
187 是规则产物生命周期。
```

## 6. 方案自审

### 合乎规则

通过。方案保持 Canonical Rule IR 为唯一真相层，没有把 Rust AOT、manifest 或 registry 升级为业务规则来源。

### 合乎长期设计

通过。保留 module_kind 和 lifecycle 边界，未来可以扩展动态模块，但第一版不会被动态加载复杂度拖住。

### 合乎实现可行性

通过。当前代码已经有 RuntimeRuleManifest、RuntimeRuleModuleEntry、RuleModuleRegistry、RuleCompiler、ProjectLogicRunner，本次只需补产物 manifest / registry / 校验和少量接入。

### 合乎简单度

通过。第一版只执行 StaticRegistry，不引入真实 DLL、Interpreter 或 runtime compilation。

### 风险

主要风险：

```text
如果测试继续直接手写 registry，可能绕开 artifact lifecycle。
```

治理规则：

```text
后续新增项目规则测试必须优先覆盖 manifest + artifact registry + ProjectLogicRunner 路径。
```

