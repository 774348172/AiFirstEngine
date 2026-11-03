# 引擎开发构建反馈（Windows）

本入口用于修改引擎源码后的定向开发验证。项目资产不变时的Compiler/Cook缓存仍由既有owner管理，本脚本不替代它，也不构建游戏或安装Provider。

从仓库根目录用PowerShell 7调用；也可在`rust/`调用脚本。参数使用PowerShell数组，不把shell命令拼入字符串。

```powershell
# CPU原私有owner：仍会构建历史组合lib单测程序，并非native独立快测。
./rust/Invoke-EngineBuild.ps1 -Package engine_runtime -Features real-wgpu `
    -Filter render_geometry::prepare_tasks::

# native边界快反馈：只构建独立native库及这个integration消费者。
./rust/Invoke-EngineBuild.ps1 -Package engine_render_native -Features real-wgpu `
    -TestTarget native_boundary -IncludeIgnored

# 实现收敛后：同一次Cargo构建受影响integration targets。
./rust/Invoke-EngineBuild.ps1 -Package engine_runtime -Features real-wgpu `
    -TestTarget platform_3d_visual,unified_render_execution_u2 -IncludeIgnored

# 不生成测试exe的定向检查。
./rust/Invoke-EngineBuild.ps1 -Mode Check -Package engine_runtime -Features real-wgpu -Lib
./rust/Invoke-EngineBuild.ps1 -Mode Clippy -Package engine_runtime -Features real-wgpu -Lib

# 真正需要无探针运行反馈时，才合并执行原Release构建。
./rust/Invoke-EngineBuild.ps1 -Mode Release -Package engine_runtime_host

# 查看确切命令而不创建缓存、证据或启动Cargo。
./rust/Invoke-EngineBuild.ps1 -Mode Release -Package engine_runtime_host -PlanOnly
```

## 本机迁移后的入口（2026-10-23）

当前本机先在工作区根执行`. ./07-Tools/Enter-EngineEnvironment.ps1 -Lane 371`，该调用会进入`01-Engine/main`并设置本路target；调用上方脚本时显式传`-TargetDir $env:CARGO_TARGET_DIR`。下文I/F盘记录是原机历史路径，不作为本机回退。

Compiler导出在启动producer的同一shell显式设置`CARGO_BUILD_JOBS=1`及当前专属`AIFE_PROJECT_RUNTIME_PLAYER_BUILD_ROOT`；模块清单的Engine SDK依赖应指向当前可信源码根，并在修改后刷新项目sourceRevision。原工程/冻结游戏不随诊断工程改变。新driver须由当前本机源码构建，旧内嵌G盘根的driver不可冒充当前producer。

Android首次完整原生Release编译与后续Gradle阶段分别记录成本。若原生编译已完成但后续阶段失败，可在串行重试的同一shell显式设置`AIFE_ANDROID_CARGO_TARGET_DIR`为该项目`Library/`下已经存在、当前无所属构建使用的私有Cargo target；导出会校验实际规范路径仍属该项目，并继续由Cargo检查源、依赖、target及配置的新鲜度，不直接复用旧`.so`。未设置时仍使用每次导出的独立target；该入口不接收其它项目、源目录、Library根或不存在目录，也不改变原导出截止和APK校验。

阶段41复现本机MSVC链接LNK1181 `.obj`：真实RuntimeGlue同一构建只改Cargo target的verbatim drive表示即可通过。原`project_player_artifact/incremental`保留canonical归属检查，只把传给Cargo的本地drive target转换为等价普通路径；网络UNC不扩改。首错、负对照、7项owner测试及后续导出身份见阶段41证据，不推广为所有verbatim路径都不可用。

## 缓存与配置

- 默认固定`I:/gameEngin-build/371/cargo-target`，仅作本路引擎开发缓存；不要每窗口生成新target。其它并发路应显式使用另一专属target。盘不可用即报错，不静默退回机械盘。
- 需要其它位置时显式`-TargetDir <absolute-path>`。不写全局环境、Cargo配置或安装目录。旧F缓存保留，禁止`cargo clean`作为提速步骤；不整包迁移约102GiB历史debug目录。
- Test/Check/Clippy默认`dev-fast`：继承dev，只把debug信息改成line-tables-only。断言、溢出检查、优化级别和增量行为不削弱。`-Profile test`或`dev`可显式复核原配置。
- Release强制使用原`release`，不提供替换正式profile的隐藏参数。dev-fast结果只证明开发正确性，不能作为13.3/16.67ms性能资格。
- 首次SSD/profile准备是一次性冷成本；稳定profile/feature/target后观察暖反馈。改变profile或feature本来就可能产生不同编译单元，不把它描述成Cargo缓存失效。
- 保留`--locked -j1`；没有峰值依据不提高并发。`RUSTFLAGS`等既有编译环境写入证据，不悄悄清除。正式性能来源还需核对这些输入及源码身份。
- Cargo工作目录固定为`rust/`，使仓库`rust-toolchain.toml`生效；测试工作目录仍为仓库根，与现有371直接消费者一致。需要已安装的固定工具链，本入口不负责安装工具链。

## 已拆出的 owner（371 §0.28）

按实际修改选择编译目标，以下测试不再需要构建整个 `engine_runtime`：

```powershell
# 资源句柄/generation/原生租约/退休，以及mip布局、生成和采样语义。
./rust/Invoke-EngineBuild.ps1 -Package engine_render_resources

# FontBundle、shaping和字体generation。
./rust/Invoke-EngineBuild.ps1 -Package engine_text

# 内容摘要、安全包路径、原子文件/目录发布及发布manifest。
./rust/Invoke-EngineBuild.ps1 -Package engine_content
```

旧 `engine_runtime::{render_resource,font_bundle,text_shaping,font_generation,canonical_digest,runtime_package_path,atomic_file_replace,atomic_directory_publish,release_package_manifest}` 路径保持为同一实现的重导出；texture mip/sampler仍可从 `engine_runtime::runtime_texture` 消费。项目不必改import。原owner的单测已随实现迁移，不能继续用旧package下的同名filter期待执行它们；零测试仍是失败。正式workspace测试自动发现三个新成员，无需再抄一套固定回归菜谱。

三者不反向依赖runtime；text只依赖content及既有字体库。`real-wgpu`仍由runtime/Player控制，不为这些CPU owner添加假GPU feature。owner内部修改先快测，收敛后再验证实际runtime消费者；依赖变更后的集成仍可能重编下游。

同一资源测试诊断消息修改，本机SSD/dev-fast从原runtime的18.359秒变成独立owner的0.758秒；无修改0.365秒。仅支持这一内循环。当前主runtime Release单元仍339.14秒，未证明比历史308.43秒更快；不能把整次Release418.09秒与上轮冷缓存645.29秒直接相减作为拆分收益。第二块主渲染编译边界仍partial，见[记录](../框架设计/引擎总体架构/阶段完成记录/2026-10-20-371-Build-Boundary/00-总览.md)。

## 构建产物与执行

371 §0.29生产编译链为 `engine_runtime -> engine_render_native -> engine_runtime_core`，装配也直接依赖core；旧公开模块路径重导出同一Rust类型。native拥有设备、队列、上传、粒子GPU和真实完成点，core仍包含CPU准备、Graph、World、UI与加载，并未全部解耦。core/native的lib单测关闭以避免反向产品依赖；对它们默认调用不带`-TestTarget`的Test模式会得到零测试失败。显式选用真实integration消费者，或使用Check/Clippy。`engine_runtime --lib`只在test二进制组合同一份实现源码，保留旧私有断言，编译成本仍高。

同SSD/dev-fast/real-wgpu、同一生产native消息小改动与原compatibility测试，构建82.6359→2.8388秒；core为fresh，实际重编native、兼容装配和测试。该单次局部反馈减少约79.80秒，不能推广为所有改动/Release/整轮施工提速，也不能作为帧预算证据。完整结果及既有流式失败见[本轮记录](../框架设计/引擎总体架构/阶段完成记录/2026-10-21-371-Native-Build-Boundary/00-总览.md)。

Test先执行一次`cargo test --no-run --message-format=json-render-diagnostics --timings`，再运行Cargo实际返回的exe，避免硬编码hash或再次编译。`-Lib`可与`-TestTarget`合并；仅选integration时不额外构建lib单测程序。

`-Filter`对本次所有测试exe生效：使用过滤时选择匹配的目标；不要把仅存在于lib中的过滤器用于integration。零测试、构建失败、测试失败及超时均报失败，保留首错并停止后续测试。默认单线程执行，不默认运行ignored测试；真实GPU消费者需要时显式`-IncludeIgnored`。

默认每次调用新建`.artifacts/371/build-feedback/<timestamp>-<mode>/`，可显式给新的`-EvidenceDir`。包含计划、完整Cargo与各测试stdout/stderr、构建/测试独立时长、测试exe SHA、manifest/lock身份、Cargo timings副本及失败信息。源码身份由本轮施工记录另行冻结，不能用manifest哈希冒充全部源码身份。`formalQualified=false`表示入口本身不颁发资格。

单进程默认20分钟上限，可显式调整`-TimeoutSeconds`；超时只终止该调用创建的进程树。启动昂贵步骤前仍须核对施工窗口剩余时间。timings显示Cargo单元和关键路径，不能单独给出精确的链接/I/O/分页归因。

## 减少一轮内的构建次数

先形成owner最小首红，修改收敛后合并实际消费者终验。相同源码/feature/profile已有上层覆盖时复用，测试名称不同不自动形成新证据。不要把全workspace或完整Compiler作为内循环。

性能诊断先定义互斥阶段、嵌套关系和必要工作量，一版探针尽量收齐。保存诊断DLL和来源后恢复源码，恢复动作本身不要求立即构建；待产品修改完成再做一次合并无probe Release。若旧信息不足必须补probe，先写清新增问题、预计构建成本及退出点。禁止跨来源拼接profile，也不能复用旧DLL冒称新源码结果。

同源码无修改再次调用应主要复用缓存；仍重编时先检查源码/feature/profile/环境/target与Cargo freshness，不直接清缓存或重跑整套。

Windows复制备份恢复源码可能同时恢复旧mtime，导致Cargo误复用期间构建的诊断变体。此类恢复应核对字节并刷新实际恢复文件的修改时间；合并到下一次必要构建，核对实际测试名/数量，不为恢复另起完整构建或清缓存。
