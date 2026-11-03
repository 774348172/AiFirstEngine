# 361 Gate D 收口复核

日期：2026-09-29

状态：`gate_d_closed`

源码基线：`acaffa8cb95bbb54d093bbf45835a59315c82da5`；工作区保留既有未提交/未跟踪施工修改。本记录关闭普通项目、正确性与生产消费范围的 Gate D，不声明 clean commit、默认安装或 Gate E 性能资格。

## 1. 收口结论

D1–D3 已用三组普通项目链证明设置面板、塔防拖放/合成及扩展/10k 虚拟列表通过真实 Project Module、Generated Glue、Engine Runtime DLL 和 Windows Player 消费。D3 解除 Slider typed value、Custom Layout measured-size 和 multi-face Runtime glyph 三项公共 owner 阻塞，并封锁 equal-value ownership 接管错误。

收口审计发现唯一失效证据是塔防 `RuntimeModule/src/sdk.rs`：D3 接入 `UiSliderRef` 后，其 SHA-256 从 D1 的 `2446E0F7...` 变为 `9EBA4BC3...`。因此本轮只刷新当前 RuntimeModule 的完整项目测试、格式和 Clippy；未重跑身份未变的引擎、Player、virtual-list 或真实窗口候选。

## 2. 闭合矩阵

| Gate D 要求 | 有效证据 | 消费身份 | 结论 |
| --- | --- | --- | --- |
| 普通项目 SDK/ABI/Glue，而非测试专用入口 | A2.2/A2.3、C2–C6；D3 两个生产 check/build 与 Project Module bind receipt | Project Runtime ABI 1.6；D3 Provider `5A9833F2...`；两个候选均使用 `engine_runtime.dll` `E0A9EFF0...` | 通过 |
| 设置面板 Button/Text/Slider typed refs、订阅/取消、通知/无通知、启停 | C1/C2/C5/C6 owner 证据；D3 equal-value takeover 回归；塔防候选 26 帧执行“打开→3倍→关闭→重开→2倍”且无 session fault | 当前 `sdk.rs` `9EBA4BC3...`；D3 tower delivery `sha256:5179d408...` | 通过 |
| 设置重复启用不叠加监听、绑定回填无循环、同值无失效 | 项目以单 token/`is_none` 注册并在关闭时取消；C2 真实 Host callback/cancel、C5 ownership、C6同值 revision、D3 equal-value takeover owner测试与重复开关生产回放 | 当前 RuntimeModule + D3 engine owner身份 | 通过 |
| 塔防部署、合法/非法合成、拖放取消/拒绝、暂停与三关流程保持 | D1 拖动中间态、阵位合成和连续战斗真实窗口；D4 当前48项项目测试含拖放/合成/拒绝、暂停及8/10/12波通关 | `sdk.rs` `9EBA4BC3...`；`lib.rs` `3B0DA50D...`；AUI `59A473FB...` | 通过 |
| 模板多实例、坐标转换、父子顺序、KeepScreen拒绝、旧句柄/回调回收 | C1–C3 生命周期、结构事务、两个模板实例、坐标往返、删除generation与卸载清理；相关结构owner未被D3修改 | C1–C3记录中的SDK/ABI/结构owner；D3不改变结构协议 | 通过 |
| 捕获、同帧输入、失焦/取消/删除不遗留状态，拖动不重排无关布局 | C7 109项捕获/Cancel/FocusLost/真实CursorLeft；B1/B11局部布局；D1真实连续拖动 | C7 input owner、B retained layout owner及D1窗口输入身份未被D3对应修改 | 通过 |
| 普通项目圆形进度、双径向布局和10k虚拟列表，不改Core写入玩法 | D2项目4项测试；D3 engine 705项；新virtual-list check/build/semantic/120帧窗口 | RuntimeModule `38E93D59...`；D3 vlist delivery `sha256:38fa00ca...`；Engine DLL `E0A9EFF0...` | 通过 |
| 自定义绘制按需、错误measure/paint隔离、列表回收不串身份 | C4 clean/repaint-only/last-good及错误隔离；D3完整engine lib回归；D3语义保留`item-00012`且两表live 9/6 | D3 engine owner源码与vlist生产候选 | 通过 |
| 双扩展实例、真实child measured-size、多face glyph共同呈现 | D3新候选中9个径向子项均为`46x34`，中文/Arabic/Latin glyph和圆形mesh可见 | D3 vlist候选、同一Engine DLL `E0A9EFF0...` | 通过 |
| release P95、400/1k/10k静态树、内存高水位与缓存预算 | 正式方案第13/13.1节及当前施工文档明确归入E | 尚未产生Gate E冻结身份 | 不属于D；Gate E保持未启动 |

Gate D 没有剩余正确性、普通项目接口或生产消费缺口，正式关闭。D2 的 `blocked_by_engine_owner` 是历史中间态，其三个阻塞均已被 D3 的新证据替代。

## 3. 本轮验证

工作目录：`samples/tower_defense_project/Projects/hezi_shoucheng/RuntimeModule`

| 命令 | 结果 |
| --- | --- |
| `cargo test --locked` | 48/48 passed，doc-test 0/0 |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --locked -- -D warnings` | exit 0，当前项目0 warning |
| `rg -n "DEBUG-d3|d3_diagnostic_tower|failure_for_debug" rust` | 无匹配 |

本轮未修改引擎或项目产品源码，未运行全 workspace、Local CI、Gate E 性能矩阵、Provider安装、候选重建或旧塔防包覆盖。D3 的705项engine、15项Player AUI、real-window编译和两个生产候选继续复用；其消费源码与二进制身份未因本轮文档及项目验证改变。

## 4. 下一步

Gate E 是 361 唯一剩余阶段。下一施工块必须先冻结 Windows release 基准机器、1280x720场景、400/1k/10k静态树与10k逻辑虚拟列表的独立样本、120帧预热/至少600帧采样、分段计时与内存/扫描/分配/上传指标；未冻结前不启动昂贵测量。361在Gate E完成前继续位于当前施工槽，不归档；默认安装继续保持360，旧塔防包不变。
