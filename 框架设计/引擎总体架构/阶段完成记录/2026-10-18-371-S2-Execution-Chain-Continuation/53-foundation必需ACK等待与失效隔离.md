# 371阶段53：foundation必需ACK等待与失效隔离

F11真实双32MiB包的四类失效子例验证完成；这是测试补证，没有产品修复，未核销完整F11/F05或BM11。

## 实际覆盖

只改原integration support/foundation_faults.rs，新增`foundation_f11_pending_and_invalid_required_acks_never_publish_partial_cells`。缺World/Asset required域在dispatch前拒绝，Loading/Prepared不能CPU发布；测试1MiB/帧上传额度制造真实未完成Render ACK，commit_gpu返回NotReady且World不可见。1MiB是测试调度，不改变正式16MiB预算。

分别覆盖foreign renderer ACK、wrong World、World entity ID碰撞、显式abandon_device_epoch。foreign backend拒绝stale_upload_ticket且未提交；wrong World拒绝后原请求仍可完成；碰撞保留外部sentinel且release不得误删；显式设备销毁后ticket清除并明确失败。有效路径泵真实queue completion后才能提交，取消/关闭后reserved、CPU驻留、GPU pending/retiring/resident及World均回零。

显式销毁不是自发device-loss、恢复或迟到callback证明；World/Asset是现有同步准备域，不虚构独立异步ACK线程。Physics/Nav按原后续阶段边界保留。

## 验证与费用

29为fixture字段名和unused import编译错误，非产品红例。30最终1测试/4case通过，构建6.521s、测试进程18.534s（Rust测试体17.68s）、总wall25.229s；31同integration二进制原双cell真实分段ACK/共享退休消费者1项通过；32定向integration Clippy通过、格式通过，修改行无告警。fixture四个owned根由原Drop清理并验证不存在。

integration二进制SHA256 `e655b26aed84c0f0c28e1381af7a6e2356c2244be648c1b30b7d2982d8c74874`；源身份在36-54-source-identity.json，实际命令/原始输出在29–32。复用48 BC3及其它未变owner证据，未重复整套foundation或Release。

## 共同边界与证据

本轮用户授权“开启新一轮，继续推进 S1”，2026-10-24 00:35:48 UTC开始，03:20:48停止新增、03:35:48硬截止。Windows开发源码结果，dirty工作树按实际文件身份记录，不是clean commit或安装资格。构建沿371独立target、Rust 1.96.0、locked/offline/-j1；游戏固定V0.0.5，进程6GiB／图形4GiB／系统available最低4.5GiB保持。374 B路按54登记独立施工，不覆盖其HostLoop/Spine/production修改。

完整证据目录为`.artifacts/371/s1-continuation-20261024-003548`。测试、产品源、生产DLL、临时组合和外部状态分别记身份；下列回执保存实际命令、stdout/stderr和首错。最终13文件fmt通过，Clippy原告警保留、增改行无命中，见53-final-source-and-format.json和54-clippy-modified-line-audit.json。库Clippy不等于cfg(test)函数已被lint。原X1–X6未全部满足，S1/371仍partial，总施工留在当前。

[本轮证据](../../../../.artifacts/371/s1-continuation-20261024-003548/53-final-source-and-format.json)；[测试去重清单](../../../../.artifacts/371/s1-continuation-20261024-003548/55-test-evidence-union.json)。
