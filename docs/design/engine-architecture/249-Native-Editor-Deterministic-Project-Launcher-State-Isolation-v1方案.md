# 249-Native Editor Deterministic Project Launcher State Isolation v1 方案

> 状态：已完成施工并归档。本文是 `125-Native-Editor-Project-OpenCreate-Persistence-C-min方案.md` 的增量收敛，不取代 125，也不修改 248 的 UI 架构结论。
> 建立日期：2026-07-13。
> 触发证据：P0-0.5 v2 Run 01 在 Manifest 前发现 Windows 文件夹选择器恢复并枚举历史目录 `<LOCAL_TEST_ROOT>\test2`，按协议终止为 `Invalidated`。

## 1. 问题与结论

当前 Native Project Launcher 的真实文件夹选择只设置标题：

```text
rfd::FileDialog::new()
  -> set_title(...)
  -> pick_folder()
```

它没有设置初始目录。Windows 会恢复系统最近使用目录，导致本次启动的行为依赖机器历史状态。与此同时，真实窗口默认从 `%APPDATA%/AI First Engine/editor_recent_projects.json` 加载最近项目；测量、自动化或隔离运行若不显式改写这一状态根，也可能在 Manifest 前读取和探测历史项目路径。

正式结论：

```text
不把历史目录加入白名单。
不恢复或改写 v2 Run 01 / Run 02 的终态证据。
修复 Project Launcher 对隐式 OS 历史状态和默认用户状态的依赖。
修复完成后用全新的 v3 authority、run root、evidence 和任务重新运行。
```

## 2. 目标

1. Open Project、Create Project、Open Runtime Package 三个真实文件夹选择入口都携带明确初始目录。
2. Native backend 只接受绝对、存在且为目录的初始目录；非法配置或平台调用失败返回结构化 `Unavailable`，禁止静默退回 Windows MRU。
3. 普通用户模式继续使用持久 recent-project store，但文件夹选择器也有确定性的默认起点。
4. 隔离运行通过一个原子启动配置同时指定 run-local picker 起点和 run-local recent store，不能漏配其中一项。
5. 默认 `%APPDATA%`、Windows 历史目录和任意其它 run 的状态均不能进入隔离运行的启动读取面。

## 3. 选定结构

### 3.1 显式请求合同

`ProjectFolderDialogRequest` 增加必填字段：

```text
initial_directory: PathBuf
```

这是调用者和 backend 之间的正式合同，不使用全局变量，也不让 backend 自行猜测路径。

`NativeEditorApplication` 持有本次应用实例的 `project_dialog_initial_directory`。三个 picker 请求统一从这一字段生成，避免 Open/Create/RuntimePackage 漏配或出现不同隐式行为。

### 3.2 Native backend 失败关闭

调用平台 folder-dialog adapter 前统一验证：

```text
非空
绝对路径
路径存在
路径是目录
路径可以由当前平台 backend 表达
```

验证失败时返回稳定 diagnostic，不调用原生对话框；不提供“验证失败后继续打开系统默认位置”的降级路径。

Windows 不继续使用会把 `SHCreateItemFromParsingName` / `SetFolder` 失败压成 `None` 的 `rfd 0.15.4` folder picker。Windows adapter 直接调用 `IFileOpenDialog`：

```text
CoInitializeEx
  -> CoCreateInstance(FileOpenDialog)
  -> GetOptions / SetOptions(FOS_PICKFOLDERS ...)
  -> SHCreateItemFromParsingName(initial_directory)
  -> SetFolder(exact shell item)
  -> Show
  -> GetResult / GetDisplayName(SIGDN_FILESYSPATH)
```

每一步保留 HRESULT。只有系统明确返回 `ERROR_CANCELLED` 才映射为 `Cancelled`；其它错误全部映射为携带阶段与 HRESULT 的 `Unavailable`。非 Windows 平台仍可使用 `rfd::set_directory`，但也必须经过同一请求校验和 `Result<Option<PathBuf>, diagnostic>` adapter 合同。

### 3.3 取消与失败的用户反馈

`Cancelled` 表示用户主动取消，不生成错误。`Unavailable` 表示引擎或平台无法满足请求，三个入口必须统一：

```text
last_command_status = Rejected
EditorCommandFeedback.status = Rejected
message / reason 保留原始 project.dialog.* diagnostic
source / command_id 保留触发命令
```

Self UI Renderer 在 Project Launcher 与 Authoring Workspace 都绘制一条最小错误反馈横幅，避免真实用户只看到“点击没反应”。结构化完整 diagnostic 保留在 model/report；横幅可按当前宽度截断显示，但不能丢失错误码前缀。

### 3.4 两种启动模式

普通模式：

```text
RealNativeEditorLaunchOptions::default()
  project_dialog_initial_directory = 确定性的用户项目起点；无可用用户目录时才退到现有 cwd/temp
  recent_store_path = 默认平台用户配置路径
```

隔离模式：

```text
RealNativeEditorLaunchOptions::isolated_project_launch_root(own_run_root)
  project_dialog_initial_directory = <own_run_root>/picker-start
  recent_store_path = <own_run_root>/state/editor_recent_projects.json
```

隔离构造必须验证 `own_run_root` 和 `picker-start` 为绝对、存在的目录；`own_run_root` 的全部既存祖先组件都不得是 symlink/junction/reparse point。recent store 可以在启动时不存在，加载结果必须为空，首次成功创建/打开后只写入 run-local `state/`。private isolated profile 在真实 app 构造、recent store 加载前必须完整复验一次，防止 options 创建后状态被替换。

`editor_host` 提供单一入口：

```text
--real-window --isolated-project-launch-root <ABS_EXISTING_RUN_ROOT>
```

禁止提供两个相互独立、可能只设置一半的测量参数。

## 4. 调用链

```text
editor_host CLI
  -> RealNativeEditorLaunchOptions
  -> RealNativeEditorApp
      -> ProjectManagerController(run-local/default recent store)
      -> NativeEditorApplication(explicit dialog initial directory)
          -> ProjectFolderDialogRequest
          -> NativeFolderDialogBackend.validate
          -> platform folder-dialog adapter
          -> Windows IFileOpenDialog.SetFolder / non-Windows rfd.set_directory
          -> Cancelled | Unavailable | Selected
          -> EditorCommandFeedback + renderer error banner (Unavailable only)
```

`ProjectManagerController` 继续只负责 recent-store 生命周期，不把 UI/平台对话框配置塞进其可序列化状态。

## 5. 结构化诊断

至少固定以下错误族：

```text
project.dialog.initial_directory_empty
project.dialog.initial_directory_not_absolute
project.dialog.initial_directory_missing
project.dialog.initial_directory_not_directory
project.dialog.initial_directory_not_representable
project.dialog.windows_<stage>_failed
editor_host.isolated_project_launch_root_missing
editor_host.isolated_project_launch_root_invalid
editor_host.isolated_picker_start_invalid
```

错误必须带路径上下文，但不得触发默认目录 fallback。

## 6. P0-0.5 v3 约束

249 只修引擎启动状态隔离，不直接修改 v1/v2 证据。v3 另行冻结：

1. 新根 `<run-root>\gameEngin-p0-0-5-v3`，新 authority、plan/index、context、run/evidence ID。
2. 每个 run 都有独立空的 `<own_run_root>/picker-start` 与 run-local recent store。
3. 三套引擎统一约束“正式创建项目入口从 run-local 空目录开始”；实现手段可以不同。
4. `<LOCAL_TEST_ROOT>\test2` 和任何历史目录不得进入读取白名单。
5. v2 Run 01、Run 02 的 `Invalidated` 终态只读封存，不允许补写回执或候选产物。
6. controller 派发并启动 app 后不得再修改本 run 的 root、`picker-start` 或 `state`；本轮合同是非对抗测量隔离，不宣称抵抗恶意并发路径替换。

## 7. 测试与证据

定向测试：

```text
cargo test --locked -p editor_window_winit project_dialog_request_roundtrips_explicit_initial_directory
cargo test --locked -p editor_window_winit configured_initial_directory_reaches_all_project_folder_requests
cargo test --locked -p editor_window_winit --features real-window native_dialog_rejects_invalid_initial_directory_without_fallback
cargo test --locked -p editor_window_winit native_editor_application_distinguishes_cancelled_and_unavailable_project_dialog
cargo test --locked -p editor_ui_renderer renderer_draws_interaction_feedback_in_launcher_and_workspace
cargo test --locked -p editor_window_winit --features real-window isolated_launch_options_use_run_local_dialog_and_recent_state
cargo test --locked -p editor_window_winit --features real-window dialog::tests::windows_dialog_show_starts_in_exact_initial_directory -- --ignored --exact --nocapture
cargo test --locked -p editor_host --features real-window isolated_project_launch_root
```

受影响域：

```text
cargo test --locked -p editor_window_winit --all-features
cargo test --locked -p editor_host --all-features
```

最终权威回归必须覆盖 workspace default/all-features、clean exact candidate、受控 target/output 根和 cleanup；不能用定向测试代替。

### 7.1 Gate 4 Provider artifact 合同恢复

249 首个冻结候选 `971670f73da2c3dc2152f5faea73d76e4a13981c` 已通过 Fast Gate 12/12 与 exact-commit Local CI 12/12。随后真实 `gpt-5.6-sol` 单 subject 审查证明 streaming Adapter 可以消除非流式 504，但也暴露了 246 遗留的 Provider response schema / artifact verifier 合同错位：

```text
response schema 允许 rule_ids=[]
response schema 把 evidence_digest 交给 Provider 自行填写
response schema 不能把普通 symbol 名绑定为 inventory canonical symbol
local verifier 却要求 rule_ids 非空、evidence_digest 为 sha256、symbol 与 inventory 精确一致
```

该缺口会让 strict JSON 在 Provider 侧合法、在 EngineStrict 本地必然成为 `provider_invalid_evidence`。这不是 Project Launcher 功能失败，也不能通过手工修改 trusted artifact、伪造 finding disposition 或反复调用 Provider 规避。

249 的最终 Gate 增加一个阻断恢复项，范围只限 246 已有质量工具：

1. Provider response 只携带语义 finding 内容，不再承担本地证据 digest 计算。
2. `architecture_review` 在生成 artifact 前解析 canonical context，把唯一可判定的普通 symbol 规范化为 inventory symbol；歧义、缺失或越界 symbol 失败关闭。
3. 本地共享接口根据规范化 finding 内容生成稳定 SHA-256；artifact verifier 重算并核对，禁止调用点各自实现 digest。
4. 不依赖 committed dependency rule 的语义 finding 允许 `rule_ids=[]`；只要 Provider 声明 rule ID，就必须存在于 canonical policy。
5. 不改变模型、subject、canonical source context 或 trusted provenance；修复后必须形成新的 exact candidate、重新跑 Fast Gate / Local CI、重新生成 bundle，并重新取得用户对源码发送和费用的明确授权。
6. 首次无效 artifact 与 Adapter audit 保留在 ignored `target/quality-gate` 作为失败收敛证据，不得改写成通过证据。

恢复项不是为 249 新增 AUI 拆分施工。`aui.rs` 的 high / medium 语义 finding 是否确认、拒绝或例外，必须等新 artifact 通过本地 evidence contract 后再由 `local-maintainer` 独立决定。

最终执行结果（2026-07-14）：恢复候选 `a09529898d08f6dfe8961dc35be7c72f4c51ea46` 的新 canonical streaming 请求成功，trusted artifact 为 `outcome=complete`，返回 4 high / 3 medium 既有 AUI finding。用户以 `local-maintainer` 身份授权 4 个 high 为仅限 249 gate 的 `approved_exception`；AUI 文件在 base/head 之间未变化，7 项均进入独立 AUI 整改跟踪，不视为已修复。EngineStrict 12/12 passed，`artifact_status=verified`，architecture/general diagnostics 均为 0，最终 candidate worktree clean。

## 8. 非目标与残余边界

本方案不把 Windows 系统文件夹选择器改造成安全沙箱。显式 `set_directory` 能消除本次 MRU 恢复，但用户仍可在系统对话框中主动浏览其它目录。

非 Windows 当前仍通过 `rfd` 的 `Option<PathBuf>` 合同接入；该 API 无法可靠区分用户取消与平台 backend 失败。P0-0.5 v3 只在 Windows 计分，此限制作为跨平台后续缺口，不得据此宣称其它平台已有同等级 HRESULT 证据。

如果未来协议要求“OS 层绝不枚举或浏览白名单外路径”，必须新增引擎内受限路径选择器；不能把 `rfd` 的普通系统对话框描述为强隔离浏览器。v3 当前只要求确定性起点、隔离启动状态和越界即失效。

## 9. 施工入口

```text
施工文档/已完成/249-当前可自动化施工文档-Native-Editor-Deterministic-Project-Launcher-State-Isolation-v1.md
阶段完成记录/2026-07-13-Native-Editor-Deterministic-Project-Launcher-State-Isolation-v1/00-总览.md
```
