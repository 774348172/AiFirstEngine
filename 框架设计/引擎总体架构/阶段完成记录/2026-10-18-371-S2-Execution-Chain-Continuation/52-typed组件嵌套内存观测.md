# 371阶段52：typed组件嵌套内存观测

原X4的五类typed组件可访问backing观测补齐，未宣称进程内存下降或完整内存覆盖。

## 改动与合同

`archetype.rs`、`runtime_entity_hydration.rs`和新私有`typed_memory_tests.rs`沿原observe_memory接入Renderable网格/材质/layer字符串、Sprite引用、Animator controller/registry及bool参数键、Audio/Particle的RuntimeAssetRef字段和Particle参数键。使用原cpu_string/去重合同，按实际capacity而非字符串长度计量，包括空字符串预留容量；共享AssetRef helper复用到已有空间路径。

只观测已有owner，不新增分配器、schema或常驻扫描。std树节点、allocator、Arc控制块仍opaque，原Partial保留。Transform/Collider等纯标量不虚构nested backing。

## 验证与身份

22两个原owner边界真实首红：Prepared queue及commit后World漏记实际地址/capacity；23修复后10 hydration项通过，24同二进制6 archetype项、25的1 instance memory项通过，共17项。两新边界按256/4096容量、空/非空、重复观测和别名去重、prepared→World实际存储移动、despawn后旧allocation消失核对。无unsafe解引用或保活副本。

26 core real-wgpu库Clippy通过，wall11.773s；原3个core告警保留，修改行无命中。三文件格式通过。测试二进制SHA256 `00f0f464b5ffdfb686a63c3a1317e554141b421d228366a9fdfcb1f9fb39ec92`；产品和测试文件各自SHA在23-typed-memory-source.json，最终再对53清单核验。只影响显式观测，不为本块重建Release；18–20旧DLL仍只证明原身份。

## 共同边界与证据

本轮用户授权“开启新一轮，继续推进 S1”，2026-10-24 00:35:48 UTC开始，03:20:48停止新增、03:35:48硬截止。Windows开发源码结果，dirty工作树按实际文件身份记录，不是clean commit或安装资格。构建沿371独立target、Rust 1.96.0、locked/offline/-j1；游戏固定V0.0.5，进程6GiB／图形4GiB／系统available最低4.5GiB保持。374 B路按54登记独立施工，不覆盖其HostLoop/Spine/production修改。

完整证据目录为`.artifacts/371/s1-continuation-20261024-003548`。测试、产品源、生产DLL、临时组合和外部状态分别记身份；下列回执保存实际命令、stdout/stderr和首错。最终13文件fmt通过，Clippy原告警保留、增改行无命中，见53-final-source-and-format.json和54-clippy-modified-line-audit.json。库Clippy不等于cfg(test)函数已被lint。原X1–X6未全部满足，S1/371仍partial，总施工留在当前。

[本轮证据](../../../../.artifacts/371/s1-continuation-20261024-003548/53-final-source-and-format.json)；[测试去重清单](../../../../.artifacts/371/s1-continuation-20261024-003548/55-test-evidence-union.json)。
