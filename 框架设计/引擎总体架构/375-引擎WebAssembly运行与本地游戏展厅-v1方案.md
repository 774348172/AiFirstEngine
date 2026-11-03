# 375 引擎 WebAssembly 运行与本地游戏展厅 v1

状态：用户已选定真实浏览器运行方向并授权文档、自审及十小时施工；2026-10-25 建立并完成 G1–G6 本地 Web 验证交付，见[完成记录](阶段完成记录/2026-10-25-375-WebAssembly-Local-Showcase/00-本地Web试玩交付.md)。属于引擎开发源码，不升级任何 V0.0.5 游戏。

## 1. 产品结果与资源归属

提供本地 HTTP 展厅，访客点游戏卡片后在网页内试玩。Rust 引擎、项目 Rust SDK 逻辑编译为 WebAssembly，在访客机器执行。游戏 RuntimePackage、图片、字体、声音放在我方托管的静态目录；浏览器经 HTTP 下载到内存及浏览器缓存。HTML 是入口，不是资源仓库。首版同一目录、同一源部署；未来可将相同版本目录放对象存储/CDN。

本地 localhost 仅供本机访问，局域网/公网可达性另由部署决定。首版不改路由器、云账户或公共服务。

## 2. 已确认方案与架构关系

普通项目文件和 Rust Project Module → 现有 ProjectRuntimePackageAssembler → RuntimePackage 与编译器生成的静态链接 glue → WebAssembly Player → 现有 engine_runtime_core、engine_render_native 中可复用 wgpu backend、AUI → 浏览器 Canvas。

Native Runtime 仍是正式主线；Web 是新增可选平台消费端，复用同一逻辑与包，不成为第二套 JS 引擎。JS 只承担下载、浏览器生命周期、DOM 容器及 wasm 启动。不得在 JS 重写项目状态机、物理或渲染。

## 3. v1 能力与边界

首个实际验收目标是桌面 Chromium/Edge 的 WebGPU、单线程 2D 项目、Rust SDK 逻辑、AUI 可交互文字、HTTP 独立资源。WebGL2、手机浏览器、线程、复杂 3D/Spine、热更新及任意既有游戏一键兼容不在本版完成声明内。浏览器或适配器不支持时明确报错，不静默切换到假游戏。以独立开发验证项目验证，不修改冻结游戏。

## 4. 平台适配

新增 browser player，异步创建 wgpu Instance/Surface/Adapter/Device，再使用现有 RealWgpuBackend::from_device_queue。使用 Inline render dispatcher，requestAnimationFrame 驱动，固定步进有上限并丢弃离开页面的长间隔。键盘、指针、焦点、缩放、暂停和重启进入现有 EngineFrameInput。Web 运行路径不调用 std::thread、同步 GPU 等待或原生文件系统。

原生 DLL 加载改为 Web 构建时静态链接编译器生成的 ProjectRuntime ABI glue；ABI 校验、会话及 SDK 语义复用既有 owner，不手写替代派生产物。

## 5. 包与资源读取

RuntimePackage 保持运行输入权威。为平台 I/O 注入只读 package source：native source 保留路径 canonicalize 与符号链接逃逸防护；memory source 使用经过逻辑路径检查的相对键读取下载字节。键按既有 Windows collision key 作唯一性和读取匹配，兼容原包内 AUI/aui 混用而拒绝大小写重名；HTTP 仍按实际导出清单路径取文件。相同 manifest/schema、依赖、规则、字体、场景及哈希校验逻辑复用，不能另造跳过验证的简化包解析器。

首版导出生成文件清单，记录相对路径、长度和 SHA256；网页异步下载、校验，准备完成后交给 Rust source。目录穿越、绝对路径、重复路径、缺失或损坏资源必须拒绝并指出文件。预取首包是 v1 策略；分块按需下载以后由相同 source 接口扩展。原生安全边界不弱化。

## 6. 导出与展厅

导出结果是可移动静态目录：index.html、播放器 JS/WASM、version manifest、package 文件；不含源码绝对路径。wasm MIME 为 application/wasm，资源普通 HTTP GET；首版不需要跨源隔离和 SharedArrayBuffer。版本目录稳定绑定包、项目逻辑和播放器，避免混用缓存。

展厅提供卡片、分类/搜索、详情、内嵌试玩、加载进度、失败重试、全屏与退出。平台外观借鉴 Craft 的作品展厅概念，品牌和素材使用本项目内容。AI 生成聊天不做虚假可用入口。iframe 消息检查 origin、source、type 和会话标识；初版同源。游戏 HUD 必须走 AUI，网页 DOM 只显示平台和诊断状态。

音频如验证样例使用，必须用户操作后初始化并验证实际输出；未验音频不得宣称具备。暂停、重启必须销毁上一会话的事件/RAF/资源，避免重复循环。

## 7. 性能与报告

正常循环默认 Off；Summary 仅紧凑加载/状态指标，Trace 仅测试显式启用。不每帧 JSON 序列化整个世界。记录包字节量、启动阶段耗时与失败文件；首版数据是本机开发资格，不给未测设备性能承诺。

## 8. 源码参考及采用范围

本地 Godot platform/web/export/export_plugin.cpp 的 export_project 使用游戏包加 HTML/JS/WASM 模板；platform/web/js/engine/engine.js 的 init/startGame 负责异步获取和启动。采用资源与运行器分离、异步初始化，不能直接照搬其 Emscripten FS 或引擎 API。

本地 Bevy examples/README.md 的 wasm32-unknown-unknown、wasm-bindgen --target web 和 WebGPU/WebGL2 区分用于构建参考。当前 vendored wgpu 26 已提供 webgpu feature；本引擎已有 from_device_queue 是复用接入点，不新造渲染器。

Craft 已观察到平台 iframe 加载 file.craft.qq.com 独立游戏页，图片/声音来自文件托管。只能证明前台分离，不能由此前端现象推断其私有 AI 后台。原始观察证据保存在本聊天 craft-research 目录。

## 9. 验收与失败保留

必须证明：普通项目走中立 assembler；同一 Rust 项目逻辑 native 与 wasm 消费；浏览器 HTTP 加载真实包；现有 wgpu 和 AUI 输出真实 glyph；输入改变项目状态；刷新/暂停/重启/resize 有效；移出源码目录的静态副本可玩；缺失/损坏资源明确失败。记录源码清单哈希、编译版本、浏览器和截图/交互结果。target check 不等于浏览器可用，页面可见也不等于全平台完成。

未完成项逐项标 partial，不用测试数量替代链路证明。10 小时内优先打通真实链路，若发现架构前提错误，先修改本文再改实现。

## 10. 施工隔离

用户明确同意网页版接替 B 槽并保留 374 待续。375 在独立 01-Engine/worktrees/375-web 工作目录及 05-Build/375-web 输出施工，复制当前脏源码作为可追溯快照，不能只取落后 HEAD。371 的几何/性能优化不归本路；共享文件仅做 Web 平台必需改动，最终接入逐文件核对基线，不覆盖另一条路。

十小时窗口自 2026-10-25 06:10:20 UTC 至 16:10:20 UTC（北京时间 14:10:20 至次日 00:10:20），最后 15 分钟收尾。此次明确时长覆盖技能默认三小时限制，不修改公共技能。
