# 371阶段54：真实渲染线程延迟与冻结帧GPU消费

F04真实worker延迟、三版本冻结与GPU消费子集通过；未宣称三帧GPU同时未完成或完整F04关闭。

## 最小测试范围

render_thread_worker.rs新增cfg(test, real-wgpu)私有模块hook，新f04_tests.rs使用原streamed_cells包fixture、ProductionGeometryCache.snapshot及实际dispatcher资源锁；兼容unit_composition.rs只加test-only self crate别名以复用既有fixture。不修改B所有production.rs或HostLoop，不新增调度器/runner。

持实际Arc<Mutex<RenderResourceManager>>锁阻塞专用OS worker，连续交付三份不可变packet：material红/蓝/红、x=-0.3/0/0.3、alpha=0/0.5/0.75及各自frame/tick/view身份。默认最大lag2：第二帧不等待、第三帧要求等待第一帧；reset_world逻辑释放后旧snapshot弱引用仍可升级，50ms锁阻塞期间无完成输出。

放锁后实际FIFO准备，消费者反序取结果；这不是反向执行任务。三份plan进入RTX4090/Vulkan的独立64²目标，三次execute间不主动poll/readback/wait，最后统一真实回读验证红蓝与变换差异。观测submitted3/completed2/in_flight1，明确没有证明三帧同时unfinished。真实GPU shutdown后退休归零，worker正常关闭、drop旧outputs/cache/dispatcher后manager resident/pending/failed归零，fixture不存在。

## 验证和剩余边界

34 API路径/默认构造/字段及35 schemaVersion错误均属fixture错误，非产品红例。36新增1项通过，总wall22.233s、测试体0.77s；37同二进制12个既有worker消费者通过，wall3.055s（测试体2.01s）。38只跑库target Clippy，wall12.152s；不包含cfg(test)函数，测试体的保证是实际编译运行及fmt检查。文件身份和unit二进制SHA见36-54-source-identity.json。

剩余F04要求：mesh替换、实际cell卸载、三帧延迟完成期间关闭及完整组合；本例逻辑cache release不替代它们。F05真实设备恢复/旧callback仍未资格。本测试补证不能变成产品性能或安装声明。

## 共同边界与证据

本轮用户授权“开启新一轮，继续推进 S1”，2026-10-24 00:35:48 UTC开始，03:20:48停止新增、03:35:48硬截止。Windows开发源码结果，dirty工作树按实际文件身份记录，不是clean commit或安装资格。构建沿371独立target、Rust 1.96.0、locked/offline/-j1；游戏固定V0.0.5，进程6GiB／图形4GiB／系统available最低4.5GiB保持。374 B路按54登记独立施工，不覆盖其HostLoop/Spine/production修改。

完整证据目录为`.artifacts/371/s1-continuation-20261024-003548`。测试、产品源、生产DLL、临时组合和外部状态分别记身份；下列回执保存实际命令、stdout/stderr和首错。最终13文件fmt通过，Clippy原告警保留、增改行无命中，见53-final-source-and-format.json和54-clippy-modified-line-audit.json。库Clippy不等于cfg(test)函数已被lint。原X1–X6未全部满足，S1/371仍partial，总施工留在当前。

[本轮证据](../../../../.artifacts/371/s1-continuation-20261024-003548/53-final-source-and-format.json)；[测试去重清单](../../../../.artifacts/371/s1-continuation-20261024-003548/55-test-evidence-union.json)。
