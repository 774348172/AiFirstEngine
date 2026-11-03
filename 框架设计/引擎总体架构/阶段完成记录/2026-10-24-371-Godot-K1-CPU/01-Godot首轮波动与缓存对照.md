# Godot 首轮波动与缓存对照

2026-10-24。用户要求先判断 Godot 首轮与后两轮差异，再决定是否研究本引擎性能差距。本次只研究已有 Godot 对照，未启动本引擎、重建、修改产品源码或调整预算。371/S1 的 partial 状态保持。

## 判断

**后两轮更接近本机这组负载可重复达到的正常稳态。首轮是真实慢运行，保留其结果，但不能据此认定 Godot 正常稳态必须消耗约25 ms，也不能据此放下本引擎的性能分析。**

原后两轮与本轮七次补测共九次，视口 CPU P95 为7.282–8.690 ms，应用帧间隔 P95 为11.534–13.278 ms。这里的“稳态”仅指加载后120帧预热、600帧采样短窗，不授长期持续、首次安装冷启动或OS实际显示资格。

**历史首轮的唯一根因尚未定位。** 本轮削弱了“Godot用户缓存为空，所以整个采样窗必然慢至25 ms”的解释；外部进程CPU时间支持额外等待参与放大墙钟，但不足以区分线程调度、同步锁、GPU／驱动等待，不能直接认定某一种。

## 同一程序与绘制量

旧证据：`.artifacts/371/cpu-godot-k1-20261024-0603`。本轮证据：`.artifacts/371/godot-round-variance-20261024-0627`。

直接复用原Godot 4.7.1 stable official Release导出，未改脚本或重新导出。嵌入资源的`godot-k1.exe` SHA256仍为`70e130421b7bbb7fe903bd5ac0278c9fdc7858e9477535165996f7de3b72da8b`。RTX4090、Forward+/Vulkan、1765×993、关闭vsync、同一720帧相机回放；10000独立网格、256材质、64张2048² RGBA8无mip纹理、1000万三角形。

重新统一断言原三轮和新七轮：均720帧、600样本、每采样帧10000 draws/1000万primitives/10000 objects，Release身份、截图和exit0通过。十张截图SHA256完全相同：`24aec1723dc47f5fe3faf4ae8cbd8976b05a57eb70b06354547fd3a7add1e98a`。没有用裁剪、换相机、降低数量或换程序解释快结果。

## 缓存与顺序对照

仅为子进程设置独立APPDATA；W复制原Godot shader/pipeline缓存，A和C各自初始为空，最后返回原APPDATA。没有清空NVIDIA驱动缓存或OS文件缓存，所以这里的“空缓存”只指Godot应用用户缓存。

单位ms，均为600帧P95；应用帧间隔包含等待。

| 运行 | Godot用户缓存 | 视口CPU | 应用帧间隔 | Shader cache miss | 编译进程竞争快照 |
| --- | --- | ---: | ---: | ---: | --- |
| 历史第一轮 | 原环境，当时已有缓存 | 24.925 | 38.883 | 未开启verbose，未知 | 未记录到所筛选进程 |
| 历史第二轮 | 原环境 | 7.512 | 11.843 | 未记录 | 未记录到所筛选进程 |
| 历史第三轮 | 原环境 | 8.690 | 13.278 | 未记录 | 未记录到所筛选进程 |
| 01-warm-first | W，首先运行 | 7.772 | 12.108 | 0 | 空 |
| 02-empty-A-first | A，初始空 | 8.170 | 12.774 | 68 | cargo/rustc，11次非空 |
| 03-warm-second | W，第二次 | 8.207 | 12.656 | 0 | cargo/rustc，6次非空 |
| 04-empty-C-first | C，初始空 | 8.459 | 13.151 | 68 | cargo/rustc，5次非空 |
| 05-A-second | A，第二次 | 7.814 | 12.140 | 0 | 空 |
| 06-warm-third | W，第三次 | 7.282 | 11.534 | 0 | 空 |
| 07-original-user | 返回原APPDATA | 8.350 | 12.986 | 0 | cargo/rustc，2次非空 |

空A/C日志均显示启动PSO cache为0.0 MiB，各有68次shader cache miss，全部发生在warmup_complete前，采样窗内为0。已有缓存运行启动PSO cache为4.6 MiB、miss为0。由此可以确认空应用缓存也能在本场景预热后进入约8 ms的视口CPU P95，而非必然持续25 ms。

02/03/04/07存在其他任务的编译进程，**不能把各轮之间零点几毫秒的差值解释为纯缓存效应**。这些运行用于证明快结果仍可出现；01/05/06未记录到所筛选竞争进程，三次视口CPU P95为7.772/7.814/7.282 ms，支持快水平可重复。筛选器只检查cargo.exe/rustc.exe/link.exe/Game.exe，不覆盖所有系统、驱动或GPU进程。

新七轮采样期窗口的前台观测均为100%，优先级均为32768（`ABOVE_NORMAL_PRIORITY_CLASS`，高于正常；不是BELOW_NORMAL）。观察脚本没有调用SetPriorityClass。全机CPU busy约8.13%–12.05%，不能用低总利用率排除局部核心或线程调度问题。历史首轮没有前后台、优先级、频率或线程调度遥测，因此无法用这些新观测反推其当时状态。

原环境缓存并非本次对照才产生：主要pipeline cache为4,783,907 bytes，mtime为2026-10-23 11:41:05 UTC。采用Windows扩展路径完整扫描，前后69个文件的大小、SHA和mtime均相同，无增删。因此不能把历史首轮称为第一次无Godot应用缓存启动；同样不能据此声称驱动当时也一定命中了缓存。

## 首轮额外时间在哪里

原600帧每60帧分段，首轮各段视口CPU P95为25.034、24.600、23.274、23.383、23.461、29.854、21.200、23.816、22.886、22.684 ms。最后60帧仍慢，排除了“只被最初几帧尖峰拉高P95”的解释。

外部GetProcessTimes记录的是全进程实际执行CPU时间。按采样器观察到的warmup_complete阶段取近似窗口：

| 历史运行 | 墙钟秒 | 进程CPU秒 | CPU/墙钟 |
| --- | ---: | ---: | ---: |
| 第一轮 | 13.547 | 8.2500 | 0.609 |
| 第二轮 | 5.906 | 6.1875 | 1.048 |
| 第三轮 | 6.734 | 6.5625 | 0.975 |

首轮相对第二轮，实际CPU时间约1.33倍，墙钟却约2.29倍。说明额外经过时间不能全部解释为同等倍数的实际CPU工作；调度或各种等待值得怀疑。多线程进程CPU/墙钟可以大于1；这也不是CPU利用率百分比。外部0.1秒采样窗口不精确等于内部600帧边界，不能据此做逐帧等待归属。

历史三轮该窗口GetProcessIoCounters增量均0，未见普通进程文件I/O持续拖慢；它不排除内存映射缺页或驱动内部活动。新七轮CPU/墙钟约0.993–1.049，同样不证明没有任何等待。

## 官方机制核对与限制

官方Godot管线编译文档区分shader中间格式编译和驱动pipeline编译，并说明驱动通常另有系统缓存。应用缓存空与驱动缓存空不能混同。本轮未擦驱动缓存、未重启进行OS冷态对照，无法排除其对历史首轮的影响，也不代表新用户首次启动。

精确4.7.1-stable源码`servers/rendering/renderer_viewport.cpp`的`viewport_get_measured_render_time_cpu()`返回CPU起止timestamp之差；`handle_timestamp()`写入这两个时刻。该指标是视口渲染范围经过时间，不能当作线程实际执行CPU时间，更不能与本引擎整个应用关键路径直接相除。机制参考已从官方取得，未以本地4.8-dev源码冒充4.7.1实现。

官方材料及抓取SHA/摘录保存在证据`11-primary-sources.json`和`source-*`中：

- `https://docs.godotengine.org/en/stable/tutorials/performance/pipeline_compilations.html`
- `https://raw.githubusercontent.com/godotengine/godot/4.7.1-stable/servers/rendering/renderer_viewport.cpp`
- `https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getpriorityclass`

## 对下一步的影响与收尾

可以进入本引擎自己的CPU准备、编码、提交与同步等待拆分分析；不能因为Godot首轮也慢就放宽13.3 ms目标。原AIFE Trace/Summary的31.1879/29.7849 ms保持原DLL身份，本次未复测；Godot视口CPU范围较窄，不授“慢固定几倍”。下一步应先找本引擎实际等待与工作归属，再选择优化，不能预先把全部renderSubmit判成纯CPU计算。

本轮七次Godot进程总wall为73.438秒，包含启动/加载/截图/退出，非稳态帧成本；没有新增构建或导出。七个Player均exit0并已结束，分析复核通过。准备阶段两次路径错误发生在启动Player前，原失败资料保留；无运行结果被删除筛选。完整结构化结果见`09-variance-analysis.json`、缓存前后见`08-original-cache-after.json`、进程收尾见`10-closeout.json`；脚本为`analyze_variance.py`。

短路径专属缓存`D:/gv241024`与初次深路径复制残留保留作证据，未递归清理、未改父进程APPDATA或系统长路径设置。没有结束其他任务、修改驱动设置或进行本引擎优化；预算、安装、游戏V0.0.5与其他施工状态保持。
