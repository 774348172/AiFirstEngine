# V0.0.3 设计资料索引

本目录收录随 V0.0.3 源码快照发布的设计资料。它们用于解释目标、边界、数据合同和演进决策，不是“所有内容均已实现”的承诺。

## 权威关系

```text
实际产品行为：本标签下的 Rust 源码 + RuntimePackage/ABI/schema
版本范围与来源：RELEASE-MANIFEST.json + RELEASE_NOTES.md
设计意图与决策：docs/design/ 下的方案文档
```

文档与代码不一致时，以本标签源码和可复现验证结果为当前实现事实。部分方案描述后续能力、历史迁移或验证方法，应按文档自身状态阅读。

## 推荐阅读顺序

1. `engine-architecture/01-目标与核心原则.md`
2. `engine-architecture/03-系统分层与混合数据模型.md`
3. `engine-architecture/04-引擎能力边界与蓝图.md`
4. `engine-architecture/05-逻辑系统边界-DSL-IR-RustAOT-ECS.md`
5. `engine-architecture/06-资源系统架构.md`
6. `engine-architecture/07-Build-Export-Pipeline.md`
7. `engine-architecture/15-Scene-Entity-Component-Prefab数据模型.md`
8. `engine-architecture/16-ECS写入与项目规则边界.md`
9. `engine-architecture/21-Runtime-Core-Boundary.md`
10. `engine-architecture/100-AUI-AI-First-Runtime-UI-System方案.md`
11. `engine-architecture/110-World-Projection-Adapter统一跨域同步规则.md`
12. `engine-architecture/194-Gameplay-Rule-Asset-and-Rust-Framework-Two-Layer-Mental-Model方案.md`
13. `engine-architecture/195-Gameplay-Rule-Asset-Rust-Framework-IR-Redline-and-AUI-Logic-Boundary方案.md`
14. `engine-architecture/253-AI-Capability-First-Tool-Kernel-Agent-Owned-Planning-v1方案.md`
15. `engine-architecture/254-AI-Tool-Gateway-Codex-Adapter-v1方案.md`
16. `engine-architecture/259-External-Codex-Authoring-Readiness-Connection-Recovery-Deep-Mutation-Contract-v1方案.md`
17. `engine-architecture/260-Project-Runtime-Session-AUI-Intent-Dispatch-Fixed-Step-Lifecycle-v1方案.md`
18. `engine-architecture/262-Trusted-ProjectRust-Editor-Composition-Artifact-v1方案.md`
19. `engine-architecture/304-Android-Native-Player-Dev-APK-Vertical-Slice-B-v1方案.md`
20. `engine-architecture/305-Android-Player-Vulkan-GLES-Fallback-Android17-16KB-B-min-v1方案.md`
21. `engine-architecture/306-Android-GL-MultiPage-Font-Texture-B-min-v1方案.md`

## 收录范围

- `design/engine-architecture/`：229 份基础架构、正式系统方案、数据合同和验证设计。
- `design/rendering/`：渲染技术路线资料。
- `design/editor-interaction/`：编辑器交互设计、可视参考和生成脚本。

复杂打飞机、塔防等名称可能作为压力测试场景出现在引擎方案中，但对应项目资产、RuntimeModule、Player 和测试证据未随包发布。

为避免发布本机信息，少量方案中的绝对开发路径已泛化为 `<repository-root>`、`<run-root>`、`<CODEX_HOME>` 等占位符；方案语义未改变。

## 明确排除

- 当前状态入口、自动化施工入口和施工优先级队列；
- 施工文档、阶段完成记录、运行证据、资格化产物和过程性施工/代码审查报告；
- 历史失败归档和包含瞬时本机状态的盘点报告；
- OpenCode 整仓镜像以及 Unity、Unreal、Godot、Bevy 源码参考；
- 样例项目自身的 Design、Assets、RuntimeModule 和 Player。

这些排除项不是 V0.0.3 的产品合同，也不应由公开源码包承担维护和许可边界。

文件名中的“方案审查”表示已经纳入正式设计体系的方案比较或边界审查，不属于上述过程性报告，因此可以随设计集发布。
