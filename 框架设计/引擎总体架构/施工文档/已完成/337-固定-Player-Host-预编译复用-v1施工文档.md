# 337 固定 Player Host 预编译复用 v1 施工文档

状态：2026-09-13 两次文档自审及Gate A/B/C全部通过，`source_integration_passed`，已归档；安装态未更新。

依据：[正式方案](../../337-固定-Player-Host-预编译复用-v1方案.md)已完成自审。54 与当前/待执行目录无有效施工，旧254证据文件不占槽；240历史队列已关闭。工作树有大量既有修改，保持原样，仅在下列 owner 增量施工，不提交/清理用户文件。

## 必要性与验证经济性自审

- 请求：新项目不重复构建固定 Host/引擎依赖；正常交付仍可运行、验证。
- 最小因果改动：runtime_cli 的固定 bin/descriptor；project_player_artifact 的引擎产物准备、模块构建、内部身份与报告；现有直接 consumer 测试。
- 排除：新 tools/公共 schema/ABI/API 目录、整体 Runtime 迁移、安装 Provider或修改宿主配置、Android/release、Editor、Local CI、玩法/美术、全 workspace 和长性能复测。
- 复用证据：335交付清单/摘要失败合同、336 DLL ABI与报告 owner 合同及原样场景输入；输入/owner变化只更新相应证据。旧54秒是不同缓存条件，不能当严格同条件收益。
- 预计开发与验证 45–100 分钟；三个 Gate 合计上限遵循施工 Skill 三小时。单次真实链路预计3–8分钟，超时先保留首因并回最小owner，不反复跑完整链路。若需新增架构边界则停止扩展并报告。

| Gate | 风险/失败合同 | 最小可变红验证 | 终态证据 |
|---|---|---|---|
| A 固定 Host | descriptor 仍静态或读取错误 DLL；正式运行分流回归 | CLI 参数/路径/失败 owner测试，cargo test -p runtime_cli --locked | 固定bin编译、测试输出 |
| B 构建收敛 | 项目仍编重型引擎；错误复用/身份失效；交付坏布局 | project_player_artifact owner+incremental，现有desktop_export_owner增加跨目录复用及身份断言 | 引擎/项目步骤、缓存身份/摘要、消费者结果 |
| C 真实闭环与收尾 | 测试替身掩盖原样玩法；无实际节省证据 | 源码Provider独占运行原样项目build/短窗口verify/playtest/observe，第二项目首次构建与小改复用测量 | 同次完整交付、16断言/4捕获、三类时间及范围说明 |

## Gate A：固定 Host 与真实 DLL descriptor

涉及 `rust/crates/runtime_cli/Cargo.toml`、`src/lib.rs`、固定 binary/helper 及其 tests。提供固定 ai_project_runtime_player；保留 descriptor flag，显式路径不回退，默认定位复用既有正式包布局；其余参数原样进入 run_from_env。禁止改变 Engine/Public SDK/ABI。

命令（rust目录）：`cargo test -p runtime_cli --locked`。真实 DLL success/mismatch 由 Gate B/C 承接，避免 owner 内重建多套项目。

## Gate B：引擎成品准备与项目模块独立编译

涉及 `project_authoring_execution/src/project_player_artifact.rs`、该目录内部 helper / incremental，及 `tests/desktop_export_owner.rs`。引擎准备使用项目无关工作区、输入身份/两个产物摘要校验和既有锁；一次构建固定 Host + Engine DLL。去掉每项目 Host 生成/编译，仅编 RuntimeGlue/已有ABI模块；保留 tests compile 开关和 staging/publish。产物版本标记排除旧静态Host。修正引擎输入摘要覆盖 engine_runtime_host 与构建配置。

命令（rust目录）：

```powershell
cargo test -p project_authoring_execution --locked --lib project_player_artifact
cargo test -p project_authoring_execution --locked --test desktop_export_owner
```

验证引擎输入/工具链/平台/环境改变不误命中，项目路径/源码不改变引擎身份，缺文件/坏摘要不命中；复用同一引擎产物后项目 Cargo 无引擎/Host编译。格式仅检查实际修改 Rust 文件；如需扩大consumer范围须有具体失败原因。

## Gate C：原样交付、测量、归档

使用既有源码 Provider，允许为验证构建开发二进制，不替换安装或真实配置。独占目录复制项目源码（排除Library/Build/target等派生目录）；运行 build(windows-dev,frameLimit=1)，立即保存内部报告，保留同次成功交付完整副本。短窗口120帧与默认game-loop.scenario 1800tick/16断言/4捕获、observe证明正式DLL执行。

第二个新项目目录复用同一引擎预编译根；同项目注释改动再构建，记录Host/DLL身份与项目编译步骤。引擎首次准备、项目冷工作区、项目增量分别计时；无需重复1800tick场景。使用Stopwatch保存数值elapsedMs；bounded超时不是成功样本。既有测试覆盖相同条件不得机械重跑。

完成后生成 `阶段完成记录/2026-09-13-337-Fixed-Player-Host-Reuse-v1/00-总览.md`，同步 README、00地图/阅读顺序、49、54和两个目录README，将本文件通过apply_patch移至已完成。记录源码完成与安装未更新的区别，不宣称当前宿主已使用337。

## 施工文档自审

2026-09-13 主审通过：逐项对照正式方案与两份只读审查结论，Gate均对应具体失败风险。无需新增ABI/Tool/公共schema，默认数据布局已修正。现有模块加载保持DLL句柄生命周期；真实descriptor成功/错项目由集成承接，owner只测参数/定位/失败。已有desktop_export_owner夹具不冒充原样游戏，C单次原场景承接真实声明。

预算内执行三Gate，不用全workspace/Local CI或安装资格扩大范围。当前无有效冲突，54已同步。独立实现分配只按文件owner并行，Cargo验证统一串行避免争锁；自审完成，允许开工。

## 执行记录

Gate A passed：63项CLI回归通过；首次并发两条500ms超时夹具未输出启动标记，定向串行复测及完整串行回归通过，未改断言。终态日志 `.artifacts/337-runtime-cli-tests.log`。

Gate B passed：13项artifact owner/incremental + 3项desktop_export_owner通过。独立审查修复legacy cdylib/custom lib.name、公开inspect嵌套布局与三个文件封存校验。首次引擎嵌套路径造成MSVC LNK1181，去掉额外目录层后首项目通过；第二项目夹具缺Compiler prepare已修回正式链路。终态 `.artifacts/337-player-artifact-terminal.log`、`.artifacts/337-desktop-export-terminal.log`；后者编译8.79秒、测试182.93秒。失败日志保留，不计为新成功次数。

Gate C passed：源码Provider编译11.40秒；原样项目窗口120帧、1800tick/16断言/4捕获/observe通过，59文件交付副本运行前后摘要一致，原项目66源文件保持。首次/第二新项目/注释小改构建95.060/33.668/21.548秒，项目Cargo9.083/9.255/1.293秒；首次Engine准备63.030秒，后两次无引擎Cargo步骤、Host/DLL摘要相同。见[完成记录](../../阶段完成记录/2026-09-13-337-Fixed-Player-Host-Reuse-v1/00-总览.md)及 `.artifacts/337-fixed-host-run/evidence/337-summary.json`。79项Rust去重测试通过，独立进程均退出；安装/config未变。当前/待执行无有效施工。
