# 376 G4项目字节存储与单key提交

2026-10-30，G4源码施工完成。引擎仅处理宿主绑定的稳定身份/profile下的不透明字节；内容编码、索引、保存策略和多key一致性留项目。原content文件替换owner新增运行时子模块，旧authoring helper函数体保持；原RuntimeInstanceLoader负责单活跃worker、按接纳顺序I/O、共享原CPU/I/O预算与128回执，以及领取/放弃、关闭等待和lease释放。

Windows实现从可信固定本地NTFS盘根开始逐组件打开，后续I/O/枚举全部相对已打开目录句柄，拒绝reparse和叶硬链接。稳定身份/profile与相对key分别编码为namespace/固定叶名，包hash不参与。固定lease独占持有；同目录temp完整write/flush/sync后一次POSIX替换，不先搬走旧目标。NotCommitted/Committed/Unknown与真实提交阶段对应，未知不自动重放。真实FFI领取在完整复制后才消费结果，短缓冲可重复尝试，未领payload保持预算、不可逐出；close收束真实worker后才释放。

最终渲染配置一次选择执行265 passed/5 GPU ignored、默认无GPU配置61 passed、content全库31 passed；19存储专项名（含1子进程入口）missing0，各批有重叠不相加。中断测试实际kill/wait并独立owner重开，保存lease/before/after三类握手、exit与65536字节SHA；并发open/旧handle、新打开及真实junction竞态均有断言。五个GPU专项本窗未重跑，前窗G2/G3证据按不变相关实现身份保留。12模块fmt通过，三个大文件原9处格式债保持；content/core Clippy退出0，分别保留1/169告警，含G5尚未接入生产入口的dead_code。

1228源码输入无漂移，12个旧文件修改、5个新模块；旧文件备份12/12匹配窗初，其中逆向还原的备份已明确标记。27份历史/终态子进程回执及108份附件复制并逐SHA校验。当前无所属producer；本run临时目录的精确删除被宿主执行策略拒绝，记`retained_by_host_policy`、未绕过，前窗20锁未重试。未安装、未改冻结V0.0.6/游戏V0.0.5或Git索引。

详细源文件、binary SHA、逐名清单、失败分类、平台依据及清理状态见[本窗施工记录](../../../../.artifacts/376/storage-20261030-085659/00-施工进度.md)、[验收映射](../../../../.artifacts/376/storage-20261030-085659/g4-requirement-map.md)、[平台复核](../../../../.artifacts/376/storage-20261030-085659/platform-review.md)。仅授本机Windows11 build26200/固定NTFS的源码与进程中断合同；不授掉电耐久、多key事务或其它平台资格。

G5正式mutation/capability、可信Player身份/root注入、普通Compiler/DLL导出、实际窗口/独立重启和AI示例出口仍未完成。376总系统partial，正式接口保持CapabilityGap，总施工保留当前。当前范围仍以[总施工](../../施工文档/已完成/376-运行时生成内容通用底座-总施工文档.md)和[54](../../54-当前可自动化施工总入口.md)为准。
