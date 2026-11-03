# 压力包SDK边界与复用刷新

2026-10-25 10:33 UTC：340正常Compiler刷新已终态失败，首因是staging复制源树时D盘空间不足（os112），Cargo尚未执行；不是新的运行内存失败。345已将15587个项目源文件（1312923463bytes）及原引擎编译缓存1533文件（1499698910bytes，排除workspace.lock）复制到明确owned的C盘临时目录，原件与prepared-engine回执保持。347正常Compiler沿原D盘Cook缓存、当前SDK与原sourceRevision在C盘副本导出中，尚未授新包或压力资格。346核正run-owned压力脚本确实消费可选新计划，342回执不再作为该行为的证明。S1/371仍partial，原十小时与安装/游戏V0.0.5边界保持。

以下保留各自时间的历史记录；当前状态以上述实际终态为准。

2026-10-25 10:09 UTC。原S1-X3范围，S1/371仍partial，十小时施工边界保持。

324 K1/10%六窗三组成本均通过，320–324全部生产者已终态。没有为了失败挑绿重跑280/321/323，也没有新增CPU生产优化。

331以当前3082f423 DLL和正常Compiler generic Game读取原1.5×K0包，332在0.890秒内有界退出1，framesCompleted=0。第一错误为`engine_runtime.project_module_load_failed`，内层`project_runtime.native_module_invalid: native module ABI/SDK digest mismatch`。私有采样峰129667072bytes，窗口和场景尚未启动；这不是压力内存超标或有效压力窗口，更不能沿此前1.5×超内存结论解释本次首因。其它旧包不直接批量执行同一已知兼容风险。

339通过正常Compiler sourceDigest，146.161秒，摘要`sha256:815dbea9c844da3adda754bd4c01c22f7345e41eb4d14b0cf8081204e25fa168`与原47请求完全相同。保留原项目、原已导出包及332首错，340正常Compiler fresh export使用当前SDK和原outputParent下的`.lit-cook-cache`，只用新的runId与Build目录。不存在手工替换模块、改ABI接受条件或重写最终Bundle。

原request600000ms单阶段边界保持。外层runner给snapshot/prepare与独立有界build预留1500秒，依据本次源摘要146秒和旧大包准备成本，避免660秒外层提前杀死仍合法的多个阶段。此调整只适用于run-owned编排，不改产品schema或semantic120秒边界；完成后先审cache/实际成本与新旧scene/mesh/material/texture身份，再决定其它包。

340当前运行中，未取得新包压力通过。原33/70的a914 DLL bounded720帧与像素证据保持原身份，不授当前完整倍率/耗尽矩阵。337已离线复现旧20000对象/16pixels/4×4 oracle；334新helper漏近灰色像素的首错与336修前代码保留，未造成新的Player重试。

完整BM11、BM12、可访问backing、流式三轮及实际OS显示仍未关闭；315权限5分支停止。没有新生产代码、Cook重做或安装成功声明，游戏仍固定V0.0.5。
