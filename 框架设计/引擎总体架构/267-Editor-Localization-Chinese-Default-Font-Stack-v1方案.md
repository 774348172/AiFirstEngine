# 267 Editor Localization / Chinese Default / Font Stack v1 方案

## 1. 状态与结论

```text
系统编号：267
方案：B+
状态：用户已确认，正式方案已生成并自审
日期：2026-08-03
上游：257 Native Editor Dark Theme；258 Editor Workspace/Typography；266 Ordered Painter Batches
相关但独立：261 Project Font Asset / FontBundle（只负责项目 Runtime 字体）
施工状态：未生成施工文档，不可施工
```

采用 B+：

```text
EditorPreferenceStore
  -> EditorLocalizationModule
       -> trusted zh-CN / en-US catalogs
       -> stable message key + typed named arguments
       -> locale revision + immutable snapshot
  -> EditorUiModel / Editor renderer presentation boundary
  -> EditorFontStack per-glyph fallback
  -> Editor glyph atlas / WGPU present
```

默认 locale 固定为 `zh-CN`，缺失消息回退到 `en-US`。用户可以显式切换中文或英文，
切换在下一次 Editor UI publication 生效，不要求重启 Editor。locale 偏好属于 Editor application，
不属于项目、工作区布局、最近项目或 RuntimePackage。

本方案只负责 Native Editor chrome 和工作流文本，不负责项目游戏内容、AUI 文案或项目
Localization Asset。中文语言包与 Editor CJK 字体栈必须同阶段交付；只翻译字符串而继续使用
当前单字体系统不构成完成。

## 2. 系统用途

Editor Localization 是 Native Editor 的展示层基础能力。它负责：

1. 把稳定的用户可见消息身份解析为当前语言文本。
2. 保存和应用用户的 Editor 语言偏好。
3. 在中文资源缺失或损坏时提供确定性英文回退和诊断。
4. 保证中文 glyph 能通过正式 Editor 字体栈和 WGPU glyph atlas 真实显示。
5. 为 AI 提供可扫描、可补齐、可验证的语言包数据合同。

它不改变 command、diagnostic、schema、module 或 project object 的机器身份。翻译只发生在
presentation boundary；稳定 ID、路径、用户输入和原始外部错误仍保持原值。

## 3. 当前代码基线与缺口

### 3.1 文案散落

当前 Editor 没有 localization/i18n seam。用户可见英文字符串分布在：

```text
rust/crates/editor_ui_model/src/
rust/crates/editor_ui_renderer/src/renderer.rs
rust/crates/editor_ui_renderer/src/panels/
rust/crates/editor_window_winit/src/command_system.rs
rust/crates/editor_core/src/ui_model_composer.rs
```

典型文本包括 `Hierarchy`、`Game`、`Project`、`Console`、`Open Project`、
`Create Project`、`Trust Project Runtime`、`Approve`、`Cancel` 和 Build/Export 工作流文案。
当前 command ID 与 display label 也有混用，不能用直接字符串替换安全迁移。

### 3.2 没有独立 Editor preferences owner

当前存在 recent-project store 和 workspace layout store，但它们分别拥有项目历史和窗口布局，
都不是 locale 的合法 owner。把语言写入项目设置会导致不同项目改变整个 Editor 语言；写入布局
会把语义设置和窗口几何耦合；写入 recent-project store 则没有领域关系。

267 必须建立窄的 application-owned preferences seam，并允许 production、测试和 qualification
注入独立 state root。测试不得修改真实用户配置。

### 3.3 当前 Editor 字体不能可靠显示中文

`rust/crates/editor_wgpu_renderer/src/font_system.rs` 当前：

- 只持有一个 `FontArc`。
- Windows 只尝试 Segoe UI、Arial 和 Consolas。
- 缺失 glyph 会改用 `?`。
- cache key 不包含 font face。
- builtin debug font 只覆盖有限 ASCII。

因此仅增加中文 Catalog 会产生问号、错误 advance 和不可验证的布局。267 必须把单字体改成
逐字符 fallback 的 EditorFontStack，并给中文提供引擎打包的可再分发字体。

### 3.4 与 261 的边界

261 的 `Project Font Asset / FontBundle / AutoHybrid Rendering` 服务于 Project Asset、
RuntimePackage、AUI 和导出 Player。Editor chrome 在项目尚未打开、项目字体损坏或项目不受信时
也必须正常显示，因此不得依赖任何项目 FontBundle。

```text
267 Editor font assets       engine package owned
261 Project FontBundle       project/runtime package owned
```

二者可以复用底层字体解析或 metrics 知识，但不能共享资源所有权、locale 设置或 glyph atlas 状态。

## 4. 目标

1. 首次启动和没有显式 locale 设置时，Native Editor 默认显示简体中文。
2. 正式打包 `zh-CN` 和 `en-US` 两个 trusted Catalog。
3. 用户可以切换中文/英文，下一次 UI publication 生效并持久化。
4. 所有用户可见 Editor 文本使用稳定 message key，不用英文原文充当身份。
5. 动态消息使用 typed named arguments，不对翻译结果做脆弱字符串拼接。
6. 缺 key、参数错误、Catalog 损坏、locale 不支持和字体缺 glyph 均有 typed diagnostics。
7. 中文 glyph 使用引擎打包 CJK 字体或显式 fallback face 真实绘制，不静默替换成 `?`。
8. renderer 不读取磁盘、不访问全局可变 locale，也不拥有偏好持久化。
9. command ID、diagnostic code、schema、module ID、路径和用户数据不因语言变化。
10. Catalog completeness、参数 parity、硬编码用户文案和视觉布局可以自动验证。
11. Off/Summary/Trace 符合既有报告分档，不给热路径增加常驻长报告。
12. 方案保持窄型，未来可适配 Fluent/ICU adapter，但 v1 不引入其复杂度。

## 5. 非目标

- 项目 Runtime/AUI 本地化、项目 Localization Asset 或导出 Player 语言设置。
- 自动翻译用户项目、资源名、Prefab、Scene、规则或脚本。
- 翻译 Rust 编译器、Cargo、驱动、操作系统或第三方工具的原始输出。
- v1 支持任意第三方未受信语言包目录扫描、下载、签名或插件市场。
- 完整 ICU MessageFormat、CLDR plural/select、日期、货币和复杂文化格式。
- RTL、BiDi、阿拉伯文 shaping、竖排或完整 accessibility 重做。
- 把所有业务诊断内容一次性重构为新的诊断系统。
- 修改 261 Project FontBundle 或让 Editor 读取项目字体。
- 修改真实用户配置、替换 production/安装态二进制、运行 Local CI。

## 6. 成熟引擎源码参考

### 6.1 Godot

Godot 源码：

```text
editor/translations/editor_translation.cpp
editor/translations/editor/zh_Hans.po
core/string/translation_server.cpp
editor/settings/editor_settings.cpp
```

`get_editor_locales()` 从编译进 Editor 的翻译表列出 locale；`_load()` 解压并通过
`TranslationLoaderPO` 装载 PO；`load_editor_translations()` 清理并重新填充
`TranslationServer::get_editor_domain()`、property domain 和 doc domain。Editor 文本拥有独立
domain，不与项目运行时翻译混在一起。

学习：Editor 独立 domain、打包可信 Catalog、locale 设置和翻译资源分层。不照搬 Godot 的
Object notification、PO 构建生成器或全部 domain 数量；本引擎首版只需要一个 Editor domain。

### 6.2 Unity

UnityCsReference 源码：

```text
Modules/LocalizationEditor/LocalizationDatabase.cs
Modules/LocalizationEditor/LocalizationDatabase.bindings.cs
Modules/LocalizationEditor/LocalizedEditorFontManager.cs
Editor/Mono/PreferencesWindow/PreferencesSettingsProviders.cs
```

`L10n.Tr()` 通过 Editor localization database 解析文本；`currentEditorLanguage`、
`GetAvailableEditorLanguages()` 和 `GetDefaultEditorLanguage()` 形成语言选择 seam。
Preferences 把 locale 存入 `Editor.kEditorLocale`，语言变化通知 Editor 并触发 UI/脚本刷新。
`LocalizedEditorFontManager` 按语言读取 font settings，为基础 Editor font 设置 fallback font names。

学习：locale 是 Editor preference，语言与字体必须一起处理，切换需要统一 revision/refresh。
不照搬以英文 source string 作为主要 lookup key、C# Assembly group 或脚本 reload；本引擎使用
稳定 message key，并可在下一次 Native UI publication 生效。

### 6.3 Unreal

Unreal 官方 Text Localization 与源码体系使用：

```text
FText
FTextLocalizationManager
FInternationalization
Namespace + Key + Native String
.manifest / .archive / .locres
```

`FText` 保留 text history 和文化相关身份，支持 live culture switching；`FText::FormatNamed`
保留命名参数；`INVTEXT` / `AsCultureInvariant` 明确表示不可翻译文本。官方资料也明确指出 raw
string 反复导入会产生不稳定 localization key，应使用确定性 key。

学习：稳定 Key、invariant 文本、命名参数、culture change revision 和 native fallback。
不照搬完整 FText history、manifest/archive/locres toolchain 或 CLDR 格式系统；本项目当前只有
两个 Editor locale，用窄 Catalog 即可。

### 6.4 Bevy

Bevy 是模块化 Rust game framework，没有与 Unity/Godot/Unreal 同等的一体化官方 Editor 和
Editor localization owner。可学习的是把 locale/catalog 作为应用 Resource，由系统消费而不
使用 renderer 全局隐式状态。

不照搬生态 crate 作为 Editor 核心真相。Native Editor 的启动、偏好、Catalog 校验、字体和
presentation 生命周期必须由引擎自己拥有。

### 6.5 综合结论

```text
Editor locale 是 application preference，不是 project data
Editor translation 是独立 domain，不是 runtime localization
用户可见文本需要稳定身份，原始字符串需要显式 invariant
语言切换需要统一 revision，不应由各 panel 自己刷新
中文语言交付必须包含字体 fallback，而不只是 Catalog
```

## 7. 方案对比与选定理由

### 7.1 方案 A：直接把英文硬编码替换成中文

初始改动最少，但失去英文切换、语言包、Key 校验和 fallback。后续新增文案仍会散落，AI 无法
判断哪些字符串遗漏，也会把 command ID、路径或诊断码误翻译。拒绝。

### 7.2 方案 B+：稳定 Key + typed arguments + trusted Catalog + EditorFontStack

数据合同清晰，翻译大部分位于可热修改的结构化 Catalog；Rust 只实现通用 resolver、偏好、
snapshot、字体选择和校验 seam。运行时成本是有界 lookup/cache，施工复杂度与当前双语需求匹配。
选择 B+。

### 7.3 方案 C：直接引入 Fluent/ICU MessageFormat

Plural/select、文化格式和 translator tooling 最完整，但会引入新的 parser/runtime dependency、
CLDR 数据、错误恢复和更复杂 schema。当前 Editor 文案以 label、command 和简单命名参数为主，
收益不足以抵消长期维护成本。v1 拒绝，但保留 Catalog adapter seam。

## 8. 总体架构与深 Module

```text
Editor startup
  -> EditorPreferenceStore.load()
  -> select effective locale
  -> EditorLocalizationModule.load_trusted_catalogs()
  -> EditorFontStack.load_engine_faces()
  -> publish EditorLocalizationSnapshot(revision)

Editor UI composition
  -> EditorTextRef(Message | Invariant)
  -> snapshot.resolve(key, typed args)
  -> localized draw text
  -> per-glyph face selection
  -> glyph atlas
  -> WGPU present
```

`EditorLocalizationModule` 是深 Module，外部 Interface 只暴露 locale、可用 locale、revision、
snapshot、切换请求和 compact diagnostics。Catalog 装载、fallback chain、参数验证、缓存、偏好
持久化协调和失败策略都留在 Module 内部。

概念 Interface：

```rust
pub trait EditorLocalizationModule {
    fn snapshot(&self) -> EditorLocalizationSnapshot;
    fn available_locales(&self) -> &[EditorLocaleDescriptor];
    fn request_locale(&mut self, locale: EditorLocaleId)
        -> EditorLocaleChangeResult;
}
```

renderer 只接收 immutable snapshot/revision 或已经解析的 presentation text，不从 global singleton
读取 locale，不打开 Catalog 文件，也不写 preferences。

## 9. Locale 身份与默认优先级

### 9.1 v1 支持集

```text
zh-CN   简体中文，packaged default
en-US   英文，fallback/native recovery locale
```

内部统一使用规范 BCP 47 tag。v1 对 `zh_Hans`、`zh_CN` 等别名只允许在受控 migration/parser
入口规范化为 `zh-CN`，Catalog identity 和持久化结果只写规范 tag。

### 9.2 选择优先级

```text
valid explicit user preference
  > packaged default zh-CN
  > en-US recovery fallback
```

- 当前产品此前没有 locale preference，因此升级后首次启动使用 `zh-CN`。
- 用户未来显式选择 `en-US` 后，升级不得强制改回中文。
- preference 文件缺失、字段缺失或 locale 不受支持时使用 `zh-CN` 并输出 compact diagnostic。
- `zh-CN` Catalog 无法装载时才使用 `en-US` recovery；不能因为操作系统语言是英文而跳过产品默认。
- v1 不自动跟随 OS locale，避免不同机器产生不可审查的默认行为。

## 10. EditorPreferenceStore

新增 application-owned、版本化、原子写入的窄设置文档，概念结构：

```json
{
  "schemaVersion": "editor-preferences.v1",
  "locale": "zh-CN"
}
```

规则：

1. production 默认路径位于 Editor application state root。
2. 测试、qualification、production authority 可以显式注入独立 root。
3. 写入使用同目录临时文件、flush/replace 的原子策略；失败不改变当前有效 locale。
4. malformed preference 不阻止 Editor 启动，使用 `zh-CN` 并给出可恢复诊断。
5. store 只保存显式用户偏好，不复制整个 Catalog、字体路径或 derived revision。
6. 不复用 workspace layout、recent projects、project settings 或真实用户项目目录。

若未来 application preference 增加主题、缩放等字段，应扩展同一个深 store schema，不再为每个
setting 新造文件；267 首版只施工 locale 所需最小 Interface。

## 11. Catalog Schema

### 11.1 结构

建议引擎包内路径：

```text
Resources/Editor/Localization/zh-CN.editor.json
Resources/Editor/Localization/en-US.editor.json
```

概念 schema：

```json
{
  "schemaVersion": "editor-localization-catalog.v1",
  "domain": "editor",
  "locale": "zh-CN",
  "messages": {
    "editor.launcher.open_project": {
      "text": "打开项目",
      "arguments": {}
    },
    "editor.assets.selected_count": {
      "text": "已选择 {count} 个资源",
      "arguments": {
        "count": "u64"
      }
    }
  }
}
```

### 11.2 解析规则

- UTF-8 only；duplicate JSON key 必须由严格 loader 拒绝，不能 last-write-wins。
- `schemaVersion`、`domain`、`locale` 必须精确匹配。
- message key 必须符合稳定小写 dotted namespace 规则。
- `text` 不能为空；纯图标控件的 tooltip 仍必须有 message key。
- placeholder 只允许 `{name}`，不允许位置参数、嵌套表达式、控制流或任意函数。
- `arguments` 与模板 placeholder 必须一一对应。
- `zh-CN` 与 `en-US` 的 key 集和 argument contract 必须完全相同。
- Catalog 装载后形成 immutable compiled representation，draw 热路径不重复 parse JSON。

### 11.3 参数类型

v1 只支持有限 typed value：

```text
stringInvariant
i64
u64
f64
bool
path
stableId
```

`path` 和 `stableId` 只决定 escaping/展示策略，不翻译值。数字 v1 使用确定性基础格式，不声称
完整文化格式。需要日期、货币、plural/select 时必须独立评估 Fluent/ICU adapter，不能把复杂
语法偷偷塞入 v1 formatter。

## 12. Message Key 与 EditorTextRef

### 12.1 稳定身份

message key 不是英文原文，也不由中文内容计算。推荐命名：

```text
editor.launcher.open_project
editor.workspace.reset_layout
editor.panel.hierarchy.title
editor.runtime_trust.approve
editor.build_export.start
editor.diagnostic.catalog_missing.title
```

改写翻译不改变 key；移动 panel 时只有语义 ownership 真正变化才重命名 key。Key rename 需要
显式 migration/impact report，不能由 formatter 或 AI 隐式猜测。

### 12.2 文本引用

概念模型：

```rust
pub enum EditorTextRef {
    Message {
        key: EditorMessageKey,
        args: EditorMessageArgs,
    },
    Invariant(EditorInvariantText),
}
```

代码常量、schema-backed UI 数据和 command registry 都可以持有 `EditorMessageKey`。不要求为每个
key 生成巨型 Rust enum；使用受 validator 管理的 newtype/const key，保持新增翻译主要是结构化
Catalog patch，同时让拼写和完整性可检查。

`Invariant` 必须显式使用，不能把任意 `String` 自动当作不可翻译文本。用户输入、路径、项目名、
资源名和第三方原始错误通过 invariant wrapper 进入 presentation。

## 13. 文本所有权与翻译边界

| 文本类别 | 处理方式 |
|---|---|
| panel/menu/tab/button/field label | Message key |
| tooltip、empty state、modal、工作流说明 | Message key |
| command display name | Message key，command ID 保持稳定 |
| 面向用户的诊断标题、摘要、next action | Message key |
| diagnostic code、stage、schema、module ID | Invariant |
| 项目名、资源名、路径、GUID | Invariant |
| 用户输入和外部 API 返回文本 | Invariant |
| Rust/Cargo/compiler/driver 原始错误 | Invariant，可加本地化外层说明 |
| Runtime/AUI 游戏内容 | 不属于 267，由项目 Localization 拥有 |

domain/core service 不应提前生成中文或英文句子。它们优先输出 typed facts、stable codes 和参数；
Editor presentation owner 再选择 message key。对当前无法立即结构化的 legacy 原始诊断，v1 保留
invariant detail，并在外层增加本地化标题，而不是机器翻译整段日志。

## 14. 解析、Snapshot 与即时切换

### 14.1 Snapshot

成功装载后发布 immutable `EditorLocalizationSnapshot`：

```text
effectiveLocale
fallbackLocale
localeRevision
compiled active messages
compiled fallback messages
compact catalog status
```

UI composition/present 在一次 publication 内使用同一 snapshot。语言切换不能让同一 draw list
一部分中文、一部分英文。

### 14.2 切换事务

```text
validate requested locale
  -> load/validate active + fallback Catalog
  -> validate required font stack
  -> persist explicit preference atomically
  -> publish new snapshot and localeRevision +1
  -> invalidate localized UI/model text cache
  -> rebuild next Editor UI publication
```

任何一步失败都保留 last-good snapshot 和字体栈，不发布半切换状态。成功切换不要求 Editor
restart、Project reload、Runtime session restart 或 specialized Editor recomposition。

### 14.3 热修改边界

语言选择本身即时生效。v1 production 不扫描任意目录、不常驻 filesystem watcher；打包 Catalog
随 Editor release 更新。开发/测试可以通过显式 reload request 验证 Catalog 改动，reload 仍走同一
事务和 revision，不直接替换 renderer 内存。

## 15. Fallback 与失败语义

单条消息解析：

```text
active locale exact key
  -> en-US exact key
  -> compact unavailable marker + typed diagnostic
```

production 不把 `[[editor.some.key]]` 作为正常用户界面；若 active 和 fallback 都缺失，使用受控的
短 recovery 文本，并在 Summary/Trace 暴露 key。测试模式可以显示 key marker 便于定位。

typed diagnostics 至少包括：

```text
editor.localization.preference_malformed
editor.localization.locale_unsupported
editor.localization.catalog_missing
editor.localization.catalog_schema_invalid
editor.localization.catalog_duplicate_key
editor.localization.message_missing
editor.localization.argument_contract_mismatch
editor.localization.argument_value_missing
editor.localization.font_face_unavailable
editor.localization.glyph_missing
editor.localization.glyph_atlas_capacity_exceeded
editor.localization.preference_write_failed
editor.localization.switch_rejected
```

Catalog/字体错误不能使 Launcher 无法启动。只有 qualification/build gate 对 packaged resources
fail closed；production 启动使用 last-good 或 `en-US` recovery 并给出可操作诊断。

## 16. EditorFontStack

### 16.1 字体资源所有权

Editor 包必须携带可再分发的简体中文字体候选，推荐 Noto Sans SC TTF，并同时交付许可证、来源、
版本和 SHA-256。施工前必须用当前 `ab_glyph`/parser 对锁定的具体 static/variable TTF 做真实 glyph
和 metrics qualification；若不兼容，必须选择同许可证、可解析的 TTF 或回填方案，不能退回只依赖
Windows 系统字体。

系统字体只作为额外 fallback，不是默认中文资格的唯一依据。Editor 即使在干净 Windows 环境、
项目未打开或项目字体损坏时也必须显示中文。

### 16.2 逐字符选择

```text
primary Editor UI face
  -> packaged Simplified Chinese face
  -> supported platform fallback face
  -> visible missing-glyph recovery + diagnostic
```

每个 Unicode scalar 选择第一个具有真实 glyph 的 face。控制字符和无效输入按明确 normalization
规则处理；缺 glyph 不得静默用 `?` 并继续报告成功。

### 16.3 Cache identity 与 metrics

Glyph cache key 至少包含：

```text
fontFaceId + glyphId/character + pxSize + rasterization mode
```

layout 的 advance、bearing、bounds 和 rasterization 必须来自同一个 selected face。不能先用主字体
计算 advance，再用 CJK fallback 绘制。locale 切换不必清除共享 glyph；font stack revision 或 face
资产变化必须使不兼容 cache 失效。

### 16.4 Atlas 容量

v1 只要求 bounded、可诊断的 Editor atlas，不要求预烘焙全部 CJK Unicode。Catalog corpus 可以在
启动/切换时预热；项目名、路径和用户输入按当前可见文本按需进入 atlas。施工必须用完整双语 Editor
视觉矩阵证明选定 atlas 尺寸/重建策略无容量溢出。容量不足时输出 typed failure，不能把剩余中文
静默变成问号。

若当前单页 atlas 无法满足资格，施工文档必须选择有界扩容、重建或多页方案并重新核对 renderer
影响面；不能在正式方案外临时制造无上限缓存。

## 17. Language Settings UI

Native Editor 必须提供一个真实语言入口，放在 application-level Editor Preferences/Settings 的
General/Language 区域。v1 选项：

```text
简体中文（zh-CN）
English (en-US)
```

选项显示使用各语言自称，避免用户切到不熟悉语言后无法恢复。设置控件提交 typed
`ChangeEditorLocale` command；panel 不直接写文件或修改 renderer global。

切换失败时保持当前选择并显示本地化错误摘要和 invariant diagnostic code。成功后当前设置面板、
菜单、launcher、workspace 和 modal 在下一 publication 一致切换。

## 18. 打包与受信资源

`zh-CN`、`en-US` Catalog 和 Editor CJK font 属于 Engine release resources：

```text
source asset
  -> build-time schema/license/hash validation
  -> release package manifest
  -> trusted Editor resource locator
  -> localization/font module load
```

普通项目不能覆盖 Editor Catalog 或字体。Project RuntimeModule trust/composition 也不能注入 Editor
translation。未来第三方语言包若需要扩展，必须单独设计签名、版本、覆盖优先级、sandbox 和卸载
语义；267 v1 不提前开放路径扫描。

Source checkout、generated specialized Editor、production package 和测试 fixture 必须通过同一个
resource locator contract 获取 Catalog/font，不能各自硬编码不同绝对路径。

## 19. 迁移策略

现有用户可见文本按 owner 分批迁移，但 267 完成资格要求 production 可达 Editor UI 全部闭环：

1. 建立 message inventory，区分 Message、Invariant 和 machine ID。
2. 为 command registry 分离 stable command ID 与 display message key。
3. 迁移 launcher、shell/menu/toolbar、workspace/panels。
4. 迁移 trust、build/export、project browser、AI panel 和 modal。
5. 给 legacy raw diagnostics 增加本地化 wrapper，保留原始 detail。
6. 扫描并分类剩余英文 literal；测试 fixture、内部日志和 machine value 可显式 allowlist。
7. 完成 `zh-CN` 与 `en-US` Catalog parity 和视觉矩阵。

不得用一次全局字符串替换实现迁移，也不得翻译测试期望中的 stable ID。每个迁移 patch 应以 key
inventory/report 证明范围。

## 20. AI 适配与验证合同

267 对 AI 的主要价值不是“AI 能翻译”，而是 AI 能确定性判断改动是否完整：

```text
EditorMessageInventory
  -> message key / owner / source callsite / category
  -> zh-CN status / en-US status
  -> argument contract
  -> invariant justification
  -> validation diagnostics
```

AI patch 默认修改 Catalog entry 和显式 message reference，不直接改 renderer 字符串。新增 UI 文本
必须同时增加双语 key 或让 completeness gate 失败。报告至少能回答：

- 哪个 key 缺失或参数不一致。
- 哪个 callsite 仍含未分类用户可见 literal。
- 当前 effective/fallback locale 和 revision。
- 哪个 glyph、face 或 atlas stage 失败。
- 应修改 Catalog、presentation mapping、preference 还是字体资源。

不引入运行时“AI 自动翻译”或网络调用；工程真相仍是版本化 Catalog。

## 21. 性能与报告分档

### 21.1 热路径

- JSON 只在启动、显式 reload 或语言切换时解析。
- compiled Catalog 和 snapshot immutable，可通过 `Arc`/revision 共享。
- 无参数消息按 `(localeRevision, key)` 缓存解析结果。
- 有参数消息验证已编译 placeholder plan，只格式化当前值。
- glyph fallback 查询缓存 character-to-face 结果，font stack revision 变化时失效。
- draw 不访问磁盘、不持有 preference lock、不生成 JSON report。

### 21.2 报告

```text
Off      只保留功能必需 locale/revision 和错误结果
Summary  locale、fallback、Catalog/font 状态、missing counts
Trace    key resolution chain、argument validation、face/glyph/atlas detail
```

production 默认 Off 或轻量 Summary。Trace 只用于测试、qualification、显式诊断；不得每帧记录每个
key 或 glyph 的长字符串历史。

## 22. 兼容性与失败恢复

- 无 preference 文件：默认 `zh-CN`。
- 旧版本产生未知字段：v1 parser 按 schema policy 处理，不能静默改变 locale。
- malformed preference：默认中文并诊断，不覆盖原文件直到用户成功选择。
- active Catalog 单条缺 key：回退 `en-US`，qualification fail。
- active Catalog 整体损坏：使用 `en-US` recovery，qualification fail。
- CJK packaged face 缺失/损坏：尝试显式平台 fallback，但中文 release qualification fail。
- 切换写盘失败：保留 last-good locale，不产生仅本进程生效的假成功。
- font/glyph atlas 更新失败：保留 last-good font snapshot/atlas，不发布半渲染状态。

## 23. 验收矩阵

### 23.1 合同测试

| 场景 | 必须证明 |
|---|---|
| no preference | effective locale 为 zh-CN |
| explicit en-US | 重启后保持 en-US |
| malformed preference | zh-CN recovery + typed diagnostic |
| unsupported locale | 不发布变化，last-good 保持 |
| Catalog parity | zh-CN/en-US key 和 argument contract 完全一致 |
| missing active key | en-US fallback + diagnostic |
| missing both keys | recovery text + fail-closed qualification |
| argument mismatch | 不格式化错误字符串，typed failure |
| atomic persistence failure | locale/revision 不推进 |
| switch success | revision 只增加一次，下一 publication 全量一致 |

### 23.2 字体测试

| 场景 | 必须证明 |
|---|---|
| packaged font load | 来源、hash、license 和 parser qualification 有证据 |
| Latin + CJK mixed text | 每个 glyph 选择正确 face，advance/bearing 一致 |
| missing glyph | 不静默变 `?`，diagnostic 准确 |
| cache identity | 相同字符不同 face/size 不错误复用 |
| atlas corpus | 完整 Catalog 和真实可见动态文本无容量溢出 |
| font unavailable | last-good/recovery 语义确定 |

### 23.3 Editor workflow

至少覆盖：

```text
Launcher / Open Project / Create Project
Main shell / menus / toolbar / tabs / panel titles
Hierarchy / Game / Project / Console
Project Browser / AI Panel
Trust Project Runtime modal
Build / Export workflow
Preferences / Language switching
common dialogs / errors / empty states / tooltips
```

### 23.4 真实视觉矩阵

```text
1280x720 @ 100%
1600x900 @ 100%
1280x720 @ 125% or equivalent qualified DPI
1600x900 @ 150% or equivalent qualified DPI
zh-CN launcher
zh-CN workspace
zh-CN trust modal
zh-CN build/export
en-US switch-back matrix
```

每张视觉证据必须检查真实中文 glyph、无问号、无裁切、无重叠、无遮挡错误、按钮/字段文本不越界。
文本变长不能通过缩小到不可读字体过验收；应调整稳定几何、换行或布局。

### 23.5 扫描与受影响回归

- production reachable UI user-facing literal inventory。
- Catalog schema/completeness/argument validator。
- Editor UI Model/Renderer/Window 定向测试。
- `editor_wgpu_renderer` default 与 real-wgpu 字体路径。
- generated specialized Editor 和普通 Editor 使用同一 locale/font contract。
- 第二项目/空项目不依赖 Tower Defense 或项目 FontBundle。

具体命令、Gate、production replacement 和 Local CI 范围由后续施工文档基于当时代码重新核对。

## 24. Ownership 分类

| 范围 | Ownership | 267 处理 |
|---|---|---|
| EditorPreferenceStore | engine/editor application | 新增窄 seam |
| EditorLocalizationModule/Catalog validator | engine/editor | 新增深 Module |
| EditorUiModel/presentation text refs | engine/editor | 迁移用户可见文本 |
| Editor renderer | engine/editor presentation | 消费 snapshot，不读磁盘 |
| EditorFontStack/glyph atlas | engine/editor renderer | 新增逐 glyph fallback |
| zh-CN/en-US Catalog | engine release resource | 打包受信资源 |
| Editor CJK font/license/hash | engine release resource | 打包受信资源 |
| Project FontBundle/RuntimePackage | project/runtime | 不修改 |
| Tower Defense AUI/gameplay | project-owned | 不修改 |
| production binary/真实配置 | external operational state | 未授权 |

## 25. 风险与缓解

1. **迁移面大。** 先生成 inventory，以 owner 分区迁移；completion gate 扫描 production reachable
   literal，不用全局替换。
2. **误翻译机器值。** 强制 `EditorTextRef::Message/Invariant` 分类，command ID、diagnostic code、
   path 和 schema 保持 typed invariant。
3. **中文导致布局溢出。** Catalog 迁移与 1280/1600、多 DPI 视觉矩阵同行；布局修复按真正 owner
   处理，不通过截断关键信息或缩小字体规避。
4. **字体资产增大安装包。** 只打包经许可和 parser qualification 的 Editor CJK face；不依赖全
   Unicode 预烘焙，不用项目 FontBundle 复制多份。
5. **glyph atlas 溢出。** 对完整 Catalog corpus 和动态可见文本做容量测试；需要时使用有界扩容/
   重建/多页策略，不使用无上限缓存。
6. **即时切换形成半中文半英文。** locale revision 与 immutable snapshot 绑定一次 UI publication，
   所有 cache 按 revision 失效。
7. **fallback 隐藏翻译遗漏。** production 可回退保证可用，qualification 对任何 active missing key
   fail closed。
8. **偏好写入污染用户状态。** store root 可注入，测试和 qualification 强制 run-owned root。
9. **方案逐渐演变成 ICU。** v1 formatter 只允许有限 named placeholders；复杂语法必须独立方案。
10. **与项目本地化混淆。** Editor domain 和 Project Localization 在资源、locale、字体、cache、
    package 和设置 ownership 上完全分离。

## 26. 方案自审

### 26.1 范围

- [x] 只负责 Editor UI，不修改项目 Runtime/AUI 本地化。
- [x] 没有把塔防或其它项目概念写入 Engine Core。
- [x] 没有复用项目 FontBundle 作为 Editor 启动依赖。
- [x] v1 只打包受信 zh-CN/en-US，不开放任意第三方扫描。
- [x] 本轮未修改代码、项目、真实配置或 production binary。

### 26.2 合同

- [x] 默认 zh-CN、fallback en-US 和用户显式设置优先级唯一。
- [x] preference 有独立 application owner 和原子失败语义。
- [x] message key 不使用英文原文作为身份。
- [x] typed named arguments、Message/Invariant 边界明确。
- [x] Catalog schema、parity、duplicate、placeholder 校验明确。
- [x] snapshot/revision 保证一次 publication 语言一致。
- [x] locale 切换事务失败保留 last-good。
- [x] renderer 不读取磁盘或全局可变 locale。

### 26.3 字体与视觉

- [x] 中文 Catalog 与 Editor CJK 字体同阶段交付。
- [x] 字体是 engine release resource，不依赖项目或用户系统字体。
- [x] per-glyph fallback、metrics 和 cache identity 明确。
- [x] missing glyph/atlas capacity 不再静默替换问号。
- [x] 1280/1600、多 DPI、中文/英文视觉矩阵明确。

### 26.4 AI 与复杂度

- [x] schema-first Catalog 和 inventory 可由 AI 生成、补齐和审查。
- [x] typed diagnostics 能定位 key、argument、preference、font、glyph 和 atlas owner。
- [x] 没有引入运行时 AI 翻译或网络依赖。
- [x] 没有引入 Fluent/ICU 的过度复杂度，但保留未来 adapter seam。
- [x] Off/Summary/Trace 不把热路径变成长报告系统。

### 26.5 自审结论

B+ 用一个深 EditorLocalizationModule、一个 application-owned preference store、结构化双语 Catalog
和 EditorFontStack 完成真实中文默认链路。它比直接中文硬编码多出必要的身份、fallback、切换和
字体合同，但没有引入当前不需要的 ICU/Fluent、第三方语言包或项目 Runtime 本地化复杂度。

方案与 257/258/266 的 Native Editor 主线兼容，与 261 Project FontBundle ownership 分离，满足
默认中文、英文可恢复、即时切换、真实 glyph 和 AI 可验证要求。正式方案自审通过。

## 27. 后续流程与授权边界

当前下一步只能是：

```text
根据 267 生成并自审独立引擎施工文档
```

仍需用户明确要求。施工文档必须重新核对 dirty worktree、当前唯一施工槽、现有 UI 文本 inventory、
preferences state root、release resource locator、字体 parser/atlas 基线、分 Gate 测试和 production
授权窗口。

获得进一步授权前，不修改 Rust、Catalog、字体资产、项目文件或真实用户设置；不运行测试、Local CI
或视觉 Gate；不重建或替换 production/安装态 Editor。
