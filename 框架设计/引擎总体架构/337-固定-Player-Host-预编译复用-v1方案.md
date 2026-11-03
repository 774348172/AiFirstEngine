# 337 固定 Player Host 预编译复用 v1

状态：2026-09-13 用户选择方案B；两次文档自审及Gate A/B/C全部通过，已[归档施工](施工文档/已完成/337-固定-Player-Host-预编译复用-v1施工文档.md)。见[完成记录](阶段完成记录/2026-09-13-337-Fixed-Player-Host-Reuse-v1/00-总览.md)。本文是327-R3/335/336的构建边界收敛，不替代顶层架构v1.1；源码Provider验收通过，安装态未更新。

## 1. 要解决的问题

施工前真实项目 DLL 已由 Engine DLL 加载执行，但 `project_player_artifact::write_generated_host` 仍为每项目生成静态依赖 RuntimeGlue 的 Host，并引入 runtime_cli(real-window)、engine_runtime 及图形依赖。编译工作区键包含项目绝对路径，所以新目录的首次项目构建仍编译 Host 与重型引擎依赖。

普通源码修改已经复用该项目工作区。336 的 137 fresh / 3 rebuilt、Host 3.498 秒是同项目增量证据；先前 337 只读测量约 53.731 秒属于新项目目录的 Cargo 阶段，不能解释为每次修改都全量重编，也不能承诺全部省掉。SDK 本身不引入图形引擎，重型依赖来自 Host。

目标：固定 Host 和 Engine DLL 由引擎版本承担构建成本，普通项目只编译项目模块、必要 Generated Glue 与其 SDK/第三方依赖，再使用现有布局装配交付。

## 2. 成熟引擎依据

- Unity 6 Windows 构建文档明确 EXE 为启动入口，UnityPlayer.dll 包含原生引擎；Mono 编译 C#，IL2CPP 将 IL 转 C++ 并编译项目原生代码。学习引擎成品复用，不推断其未公开内部 ABI，也不承诺 Rust 达到脚本导出时间。
  - https://docs.unity3d.com/6000.0/Documentation/Manual/WindowsStandaloneBinaries.html
  - https://docs.unity3d.com/6000.0/Documentation/Manual/scripting-backends.html
- Godot 本地源码 `I:/godotcode/godot-master/godot-master/editor/export/editor_export_platform_pc.cpp`：`export_project` 调用 prepare_template / modify_template / export_project_data；190 行复制模板，220 行 save_pack。原生扩展由 `core/extension/gdextension_library_loader.cpp` 的 open_dynamic_library / initialization_function 加载。
  - https://docs.godotengine.org/en/stable/tutorials/export/exporting_projects.html
- 比较结论：A 共享 Cargo target 可减少重复依赖编译，但仍保留每项目 Host；C 薄壳仍须每项目生成/编译；选定 B 直接复用项目无关 Host。共享缓存不是天然不安全，但不是本轮主方案。

## 3. 最小设计

### 引擎产物

在现有 runtime_cli crate 提供固定 `ai_project_runtime_player` binary，复用 `run_from_env()` 与 336 正式包普通/semantic DLL 分流。不强制移除 Host 内全部 Rust 引擎类型依赖；它们只在引擎准备时编译。

保留 `--describe-project-runtime-module` 合同，改为加载真实项目 DLL 并读取其 descriptor。显式 `--project-runtime-dll <path>` 仅属内部 CLI；默认复用正式 `data/runtime_package` 定位 `data/bin/<module-id>.dll`，并兼容同级项目 DLL。产物构建者为当前嵌套 EXE 布局提供明确 DLL 路径。不使用外部 manifest 的自述代替 DLL 内身份，不回退静态项目模块。

构建 owner 在现有 project_player_artifact 内准备项目无关 Host + engine_runtime.dll。引擎工作区按 SDK 位置、toolchain、Windows target、固定 dev profile/real-window features、有效构建环境复用；产物身份再绑定引擎构建输入摘要。项目路径、项目代码、AOT/glue 摘要不进入引擎身份。

复用现有 CompileWorkspace 锁、完整键校验与 bounded Cargo 执行机制。引擎配对收据保存输入身份与两个文件摘要；命中必须核对文件与 DLL 能力。未命中才在引擎独占 target 中用 SDK trusted Cargo.lock 构建两个产物；源码变化允许 Cargo 复用旧依赖。只有成功并验证完整后写收据；失败不产生可复用状态。不能仅凭 SDK target 中 DLL 已存在来判定有效。

跨项目复用以同一个 configured build_root 为范围，默认根是应用共享缓存；显式指定不同根则隔离。引擎键与项目键独立，复用既有短 `c-<key>` 路径，不增加目录嵌套，避免MSVC对象路径超长。收据失效时删除的仅为该锁内两项构建输出，避免Cargo把被改写的最终二进制判断为fresh；有效依赖对象保留。

引擎输入覆盖现有运行依赖和 engine_runtime_host 本身、Cargo 清单/锁、toolchain、相关 Cargo 配置。无需新增公共 Registry、缓存服务或通用分发系统；源码开发环境首次按需准备，后续项目消费同一对产物。安装型 SDK 预置分发渠道延后。

### 项目构建与交付

保留现有项目 compile workspace、FrozenSource、tests compile 开关、334 内部准备语义。删除每项目 Host 源码生成与 Host Cargo build，直接 build RuntimeGlue（legacy ABI 项目使用已有模块构建产物）。从引擎产物拷贝 Host/DLL，从项目 target 拷贝项目 DLL，沿用 335 staging/publish、descriptor、manifest、delivery digest 与 verify。

legacy ABI 的内部派生manifest补rlib/cdylib并保留自定义lib.name，项目源不变。成功artifact封存Host、Engine DLL、项目DLL三个摘要，命中在probe/descriptor之前验证，避免旧整包缓存分支绕过新检查。公开inspect只在识别自身封存产物布局后显式定位DLL，其他既有入口保持原调用方式。

内部产物版本标记变化使旧静态 Host 缓存失效；项目源码修改仍保留已有依赖缓存。报告继续使用现有 steps，明确 engine prepare 与 project module build；仅追加必要的可选引擎身份/复用状态，不升级外部 Tool schema。

## 4. 边界、失败与验收

- 九项独立 AI tools、轻量 Skill、公开 SDK/API 与 ABI 继续有效；不新增 prepare 工具、不形成工具前置等待。
- 本轮仅 Windows dev。Android、release profile 改造、Editor、安装 Provider/真实宿主配置、热更新、整体 SDK 迁移、API 白名单、玩法、美术、1800 tick 性能优化均不在范围。
- 引擎输入变化、收据丢失/坏摘要或 DLL 不兼容必须拒绝复用并正常重建/报告失败。错误项目 DLL / descriptor 不匹配仍失败。
- 必须证明同一引擎身份下两个独立新项目目录复用同一 Host/DLL；项目 Cargo 产物不包含 Host、engine_runtime、runtime_player_winit、wgpu、winit。项目自身原生依赖不受此列表限制，本轮使用原样 shooter 作测量夹具。
- 同一次成功交付完整复制后通过现有窗口与 semantic / capture 验证，保持 DLL execution owner。已有 335/336 不变 ABI/报告合同证据继承，不机械重复全矩阵。
- 分开报告首次引擎准备、首次项目、第二项目复用与同项目增量成本；没有同条件对照时不声称精确节省秒数。

## 5. 正式方案自审

2026-09-13 自审通过。主审已核对权威 v1.1、327-R3/335/336、54 空槽与实际 producer；独立只读复核确认 `run_from_env()` 已具备正式包 DLL 分流，唯一项目静态绑定是 descriptor，不需新 ABI 或运行接口。

自审发现并已修正正式导出的项目 DLL 位置：真实路径是 `data/bin/<module-id>.dll`，不是假设 EXE 同级。保留显式 descriptor 参数以兼容内部 target/debug 布局。裸包 semantic 不在资格声明，不为其新增 fallback。验证复用现有 desktop_export_owner 集成入口及 336 MCP 原样场景；不复用 337 旧脚本错误的 elapsedMs 或超时窗口1800帧作为成功计时。

没有用户指定的适用外部审查文件；已完成方案接缝与最小验证两项独立复核，无需扩大正式设计。可生成施工文档。
