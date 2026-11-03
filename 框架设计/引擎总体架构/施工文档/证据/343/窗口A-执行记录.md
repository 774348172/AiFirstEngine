# 343 窗口A：作者合同与Cook执行证据

日期：2026-09-14。状态：窗口A通过，343整体未完成；本文件不是343归档完成记录。工作树已有大量前序更改；本记录是当前工作树开发集成证据，不绑定clean exact commit，不代表已安装Provider或GPU运行通过。

## 首次失败与处理

命令工作目录：`G:/gameEngin/rust`。

1. `cargo test -p engine_runtime --locked particle_effect -- --nocapture`首次未启动测试：新增直接Naga依赖尚未同步锁文件。Cargo报`cannot update the lock file ... because --locked was passed`。归类施工配置遗漏；在已有Naga26锁定包基础上仅增加project_authoring_execution的依赖边，不更新其它包版本。
2. 同命令首次实际测试，编译52.56秒，测试0.01秒：8 passed / 1 failed。首因：Serde对内部tag的unit variant会忽略附加字段，即使enum标了deny_unknown_fields。原始关键输出：

```text
thread 'particle_effect::tests::particle_effect_rejects_unknown_fields_types_and_units' (20852) panicked at crates\engine_runtime\src\particle_effect\tests.rs:38:9:
accepted /emitters/0/shape/unknown
test result: FAILED. 8 passed; 1 failed; 0 ignored; 0 measured; 602 filtered out; finished in 0.01s
```

修复：将Point/Integrate/Billboard这类空参数variant改为空struct variant，保持JSON作者格式不变，严格拒绝未知字段。保留原断言并续跑，不放宽合同。

3. Compiler定向首轮：编译1分47秒，测试3.70秒，12 passed / 1 failed。包测试夹具漏了已有RuntimePackage v2必须显式声明的InputMappingAsset：

```text
particle_effect_cook_package_owns_exact_program_bytes_without_author_tree
assertion left == right failed: left: Failed, right: Success
code: digest_scope_violation
message: RuntimePackage v2 requires an explicit project InputMappingAsset; use input.none for projects without input.
```

修复只补测试夹具的`InputMappingAsset::new("input.none", [], [], [])`，不修改Builder约束。其余12项包含真实Naga语法/类型和作者行定位已通过；继续定向修复包测试后跑终态集。

## 修复复验和终态

包夹具修复后先定向跑`cargo test -p project_authoring_execution --locked particle_effect_cook_package_owns_exact_program_bytes_without_author_tree -- --nocapture`：1 passed，编译2分37秒，测试0.06秒。随后增加正式Compiler的同快照消费检查和生成字段错误定位，再做最终集合；不重复计数这个已包含的包测试。

```powershell
cargo test -p engine_runtime -p project_authoring_execution --locked particle_effect -- --nocapture
cargo test -p project_authoring_execution --locked --test game_project_compiler_interface game_project_compiler_interface_prepare -- --nocapture
cargo test -p project_authoring_execution --locked --test game_project_compiler_interface game_project_compiler_check_validates_without_assembling_runtime_package -- --exact --nocapture
```

终态原始结果摘要：

```text
Finished test profile [unoptimized + debuginfo] in 3m 02s
engine_runtime: 9 passed; 0 failed; test time 0.01s
project_authoring_execution unit: 14 passed; 0 failed; test time 3.68s
game_project_compiler_interface particle_effect_check_and_prepare_share_snapshot_program_validation:
1 passed; 0 failed; test time 3.02s

Existing prepare consumer regression: build/fresh 0.71s; 3 passed; 0 failed; test time 3.86s
Existing check consumer regression: build/fresh 0.22s; 1 passed; 0 failed; test time 0.01s
```

共28项去重通过。Compiler为真实Naga26解析/类型校验，不是字符串模拟；普通数学/函数/只读参数可用，entry point、资源binding、隐藏可写全局、坏签名、坏路径/资源引用均拒绝。语法/类型错误分别测试作者实际行；生成的非法参数名也回到描述字段。

效果组件/播放尚未接入：入包的是经验证的WGSL行为函数库及typed效果数据，不是已经派发运行的GPU效果。P1/P2编译部分得到支持，P2实际执行、P3–P8及P9最终DLL不变证据仍未完成。现有快照测试实证旧lease在磁盘源改坏后仍准备原程序，新lease的check/prepare定位同一作者行；局部效果字节不因另一效果修改而变化。

新文件定向rustfmt检查通过；涉及已跟踪入口/锁文件的`git diff --check`通过。343方案与施工文档的本地链接检查通过。编译中的3条dead_code警告来自原有project_logic/world，不是新增粒子代码；未扩成无关清理或全workspace/Local CI。

窗口从21:44起保守计时，22:17终态记录（Asia/Shanghai，约33分钟；包含文档、施工和验证）。无安装/config/生产Provider修改；无真实GPU或窗口程序启动。失败包夹具留下的唯一空临时目录已验证为空、非reparse后非递归移除；后续包夹具和Compiler fixture自行清理。结束核查无本轮cargo/rustc剩余进程。

## 源码与测试身份

以下路径均相对`G:/gameEngin/rust`，SHA-256为窗口A终态工作树内容。该身份不等同历史HEAD或安装二进制；已有无关工作树变更完整保留。

| 文件 | SHA-256 |
|---|---|
| crates/engine_runtime/src/particle_effect.rs | D6017DE7577903C05F84DD57B1510D5F48AB1CA3AEACC90B6CAACE16A9E89C4D |
| crates/engine_runtime/src/particle_effect/validation.rs | B4FB31087E82675143AD8FD68DF0F95205FA219C389E430A23D6EC57E26BE648 |
| crates/engine_runtime/src/particle_effect/tests.rs | BD8CC2B24C5AED2C94C04B0A7B5C6BD13FDEAB5507347D33A3EA3F76131FF138 |
| crates/engine_runtime/tests/fixtures/particle-effect.json | 8CDC15EE85CC8E4B318243DFC3D01CCBDBD41CE871FC96F459D38037C388B97C |
| crates/engine_runtime/src/lib.rs | 660777EBDEE32E4C8D7BDDE76EDEE788845D68BD60058B0DFB912F495D32964C |
| crates/project_authoring_execution/src/particle_effect_cook.rs | 76C6C0886CE7B4C1048DD04A8F66C52A0732E0E98DDE6525635F27B421109806 |
| crates/project_authoring_execution/src/particle_effect_cook/tests.rs | CCF71B2E4FE96ACE63B8B028AE5707942ED72EF127E303BEE8997113944303C7 |
| crates/project_authoring_execution/src/game_project_compiler.rs | 97C1D9AAB8C3E601A90F8ADDB8B6B1D0A750341BC1F657D65534555FB92EC910 |
| crates/project_authoring_execution/src/neutral_assembler.rs | 2913CC8A50924613275FAB560A0F7ADEC39B8D0C9EC835A7C04149EB4FFDE1FC |
| crates/project_authoring_execution/tests/game_project_compiler_interface.rs | AE54B2F14871183588BEF7DA31B9084D3E14C45B85FA5A6271512E318163F82C |
| crates/project_authoring_execution/src/lib.rs | 6654A28AEA14DC5A42CDF8478B876385452D07C84FE24161080B177B17392DF0 |
| crates/project_authoring_execution/Cargo.toml | CD9B93AD2EDD0813AD4C0BA850AC3DD1FEE64E435D23AC6BB0254BF0FB2D38E8 |
| Cargo.lock | 191AC9AEF1D7A2D9F0329521AD4B89BBC6339E80176431687881EA25987216AB |

下一窗口按当前施工文档进入Gate B：由引擎生成GPU入口，持久粒子buffer、模拟/存活/事件边界及小规模真实GPU回读反例；不以本轮函数库校验代替实际执行。
