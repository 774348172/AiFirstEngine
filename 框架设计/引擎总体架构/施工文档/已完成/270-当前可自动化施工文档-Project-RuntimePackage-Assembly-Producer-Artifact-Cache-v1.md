# 270 Project RuntimePackage Assembly Producer Artifact Cache v1 施工文档

状态：Gate A-G 已完成，2026-08-04 待归档。用户于 2026-08-04 授权生成施工文档并直接施工。

正式方案：`270-Project-RuntimePackage-Assembly-Producer-Artifact-Cache-v1方案.md`。

## 1. 施工目标

在不改变 RuntimePackage 真相、不修改任何样例项目内容的前提下，将
`ProjectRuntimePackageAssembler` 的高成本字体派生过程改造成可计时、可解释、内容寻址、可删除并
确定性重建的 `FontCookProducer` artifact cache。真实 Tower 样本中，整包 Preview Cache stale 而字体
recipe 未变化时，必须跳过 Bitmap/MSDF raster。

269 基线证据：

```text
.artifacts/269/tower-preview-copy/editor-play-preview-package-report.json
assemble_project_runtime_package_input = 89,172 ms
```

## 2. Owner、Consumer 与预计文件

Owner：

```text
rust/crates/editor_core/src/project_assembly_artifact_cache.rs（新增）
rust/crates/editor_core/src/project_font_cook.rs
rust/crates/editor_core/src/project_runtime_package_assembler.rs
```

直接 consumer / contract：

```text
rust/crates/editor_core/src/editor_preview_package.rs
rust/crates/editor_core/src/desktop_export.rs（仅确认默认/显式 cache 行为，不扩大导出流程）
rust/crates/editor_core/src/release_package.rs（同上）
rust/crates/editor_core/src/lib.rs
rust/crates/engine_runtime/src/font_bundle.rs
rust/crates/engine_runtime/src/runtime_package_builder.rs（仅在 typed source 反序列化确有需要时）
```

测试与真实样本：

```text
editor_core owner tests
engine_runtime FontBundle/RuntimePackage tests
samples/tower_defense_project/Tests/FontBundleV2
第二项目 fixture 或既有 project_e2e_gate font compatibility fixture
```

允许根据实际 owner 边界调整具体文件，但不得新增通用动态插件系统、远程 cache、网络服务或项目语义。

## 3. 禁止项与授权边界

```text
禁止修改 Tower Defense / complex shooter / switch puzzle 的玩法、资产、字体 profile、RuntimeModule。
禁止减少 glyph、字号或关闭 MSDF 冒充性能优化。
禁止 Runtime 读取项目源目录或 producer cache。
禁止 mtime 正确性身份、绝对项目路径身份和未经验证的缓存 payload。
禁止删除或重置工作树既有改动。
禁止替换 production/安装态二进制、修改真实用户配置、运行 Local CI。
禁止自动恢复历史 Tower P0-5 Gate G/H。
```

本轮只允许源码级开发测试、run-owned cache/evidence root 和只读样例资格。若真实资格需要复制项目，
副本必须位于显式 run-owned root，不能写回样例源项目。

## 4. Gate A - 真实计时与根因资格

### 实现

1. 为 assembly 增加 producer report 基础 schema，并保持 v1 report 旧 reader 可反序列化。
2. 为现有收集阶段加入低成本计时：Project/Scene/Prefab/Input/Rule/AUI/Font/Asset/Texture。
3. 为 Font Cook 加入：load request、collect requirements、resolve assets/glyphs、metrics、Bitmap raster、
   MSDF raster、kerning、atlas pack、bundle build 的持续时间。
4. 计时只记录阶段、计数与耗时，不记录原始私密文本。
5. 在 run-owned Tower 项目副本或只读源 + 外部 cache/evidence root 复现一次冷 assembly。

### 停线条件

若字体 Bitmap/MSDF/bundle 路径不是 89 秒主因，保留报告并暂停 Gate B-C，先修订 270 正式方案；不得仍以
Font cache 名义继续扩大实现。

### 开发验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_runtime_package_assembly_report -- --nocapture
cargo test -p editor_core project_font_cook -- --nocapture
```

过滤器以施工后真实测试名为准；不得用零测试匹配冒充通过。

## 5. Gate B - Artifact Envelope、Recipe 与 Cache Service

### 实现

1. 新增深模块 `ProjectAssemblyArtifactCache`，隐藏路径布局、lookup、验证、temp、原子 publish 和恢复。
2. 冻结 schema：

```text
project-assembly-artifact-envelope.v1
project-assembly-producer-report.v1（或作为 assembly report v2 的内嵌稳定结构）
```

3. `ProjectRuntimePackageAssemblyRequest` 增加可选显式 `artifact_cache_root` builder；默认 `None` 表示
   Disabled，不在深模块猜真实用户路径。
4. recipe key 使用 canonical content/dependency digest，覆盖 producer/recipe/schema、字体资产声明、源字体
   bytes、codepoint/kerning requirements、raster/packing profile 和相关工具链身份。
5. 目录严格在 caller 提供的 cache root 下；key 必须验证为固定 digest 格式，不能形成路径穿越。
6. lookup 验证 envelope schema、producer、recipe key、payload digest；损坏返回 typed Corrupt，不 panic。

### 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_assembly_artifact_cache -- --nocapture
```

必须覆盖 missing/hit/invalid/corrupt、路径 containment、项目复制路径无关、JSON 无语义重排和 recipe version
失效。

## 6. Gate C - FontCookProducer 真实复用

### 实现

1. 把 `load_project_request` 与昂贵 `cook` 之间拆出 `prepare_recipe`；recipe 阶段不得 raster/pack。
2. `ProjectFontCookModule` 保持一个深调用入口，但内部流程变为 prepare -> lookup -> produce -> publish。
3. cache payload 使用可反序列化的 typed `ProjectFontRuntimePackageCook` 或等价 sealed typed payload；为
   `RuntimePackageSourceFontBundle`（以及必要的 legacy atlas）补齐对称 `Deserialize` 合同。
4. hit 后验证 payload digest、typed schema、FontBundle metadata/bundle digest，直接返回 typed artifact。
5. miss 后执行现有确定性 Cook，验证后原子 publish；cache publish 失败不得把未验证产物报告为 hit。
6. 没有项目 FontBundle、使用 legacy C-min atlas 时 v1 可以明确 Disabled/Bypassed，不得谎报 Font hit。

### 失效矩阵测试

```text
相同 recipe -> hit，Bitmap/MSDF produceDurationMs = 0
Scene/Rule/Input only -> hit
AUI layout only -> hit
文本重排但 codepoint/kerning requirements 相同 -> hit
新增 glyph -> miss
字体 bytes/profile/recipe version -> miss
复制项目绝对路径变化 -> hit
```

### 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_font_artifact_cache -- --nocapture
cargo test -p engine_runtime font_bundle -- --nocapture
```

## 7. Gate D - 原子发布、并发与损坏恢复

### 实现

```text
同 root 临时文件/目录写入；flush/验证完成后原子 rename/commit。
reader 永远看不到半写 artifact。
同 key winner 已存在且 output digest 相同 -> PublishRaceReused。
同 key 不同 output digest -> deterministic violation，禁止覆盖。
残缺 envelope、截断 payload、digest mismatch -> Corrupt 后重建。
cache root 不可用 -> 明确 Disabled/Failed diagnostic，并按合同执行 uncached produce 或失败。
```

v1 不要求跨主机共享锁。若实现 per-key lock，必须有 stale/process-crash 恢复，锁不能成为正确性真相。

### 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core project_assembly_artifact_cache_recovery -- --nocapture
cargo test -p editor_core project_assembly_artifact_cache_concurrency -- --nocapture
```

## 8. Gate E - Assembly/Preview Report Handoff

### 实现

1. Assembly report 增加 `producerReports[]`，包含 recipe/lookup/produce/validate/publish duration、cache status、
   recipe key、output digest、miss reason 和 diagnostics。
2. 保持旧 JSON 缺少新字段时可读；新 schema version 与 compatibility 策略必须明确，不能在常量仍为 v1 时
   写入声称 v2 的结构。
3. Preview caller 为 assembly 显式传入 engine-managed generated cache root。优先使用项目 `.aife` 内现有
   generated area 的稳定 sibling，但必须由 caller 解析并通过 `ProjectWriteScope` containment；测试可传
   run-owned external root。
4. Preview report 的 assembly stage diagnostics/summary 能指出 Font producer hit/miss 和耗时；不复制巨大
   payload 或私密文本。
5. Export/Release caller 的 cache 行为保持显式：若未授权共享 root，可以 Disabled 并正确报告；不得读取
   Preview RuntimePackage 作为发布真相。

### 测试

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core editor_preview_package_ -- --nocapture
cargo test -p editor_core project_runtime_package_assembler -- --nocapture
```

## 9. Gate F - 项目无关、确定性与 consumer 矩阵

### 验证

```powershell
Set-Location G:\gameEngin\rust
cargo test -p editor_core
cargo test -p engine_runtime
cargo test -p project_e2e_gate font_bundle -- --nocapture
```

若 project_e2e_gate 过滤器没有覆盖第二项目，新增/复用一个最小第二项目 owner fixture，只验证项目无关
合同，不把塔防内容复制进引擎 fixture。

Tower `FontBundleV2` 是外部真实 consumer；只在其现有构建方式可控且不会写源项目 generated state 时运行。
否则复制到 run-owned root 后运行或记录明确豁免。

## 10. Gate G - Fresh 性能资格、受影响回归与闭环

### Fresh 资格

在 fresh run-owned cache/evidence root 中至少记录：

```text
run 1：empty producer cache，Font Produced，完整 substage 时间。
run 2：使 Preview 整包 stale 但 Font recipe 不变，Font Hit，raster 不执行。
run 3：新增测试副本 glyph requirement，Font Miss，正确重建。
```

不得修改源样例。性能资格比较 assembly/Font producer 时间，但不设跨机器硬墙钟 CI 断言。目标是 run 2
从 89 秒级收敛到数秒级，并以 `cacheStatus=Hit` + `produceDurationMs=0` 证明真实跳过。

### 受影响质量

```powershell
Set-Location G:\gameEngin\rust
cargo fmt --all -- --check
cargo test -p editor_core
cargo test -p engine_runtime
```

不运行 Local CI、全 workspace、真实 Windows production Editor 或安装态资格，因为本次 claim 不包含这些
环境。若 owner 变化实际触及额外 crate consumer，只补相应受影响测试，不机械扩大到全部平台。

### 闭环

```text
生成阶段完成记录。
更新 00-文档地图、49、54、施工文档/README、阶段完成记录/README。
将本施工文档从 当前/ 移到 已完成/。
记录真实测试、fresh evidence path、冷/热时间、遗漏测试和残余风险。
```

## 11. 自审

```text
与 270 正式方案一致：是。
是否扩大为完整 Build/Asset Graph：否。
是否把 Font 做成无法扩展的孤立缓存：否；共享 envelope/cache/report，v1 仅 Font 真复用。
是否在昂贵 raster 前 lookup：是，Gate C 强制。
是否保留 RuntimePackage 真相：是。
是否使用 mtime/绝对路径作正确性 identity：否。
是否要求修改项目内容：否。
是否覆盖 shared Deserialize consumer：是，Gate C + engine_runtime 测试。
是否覆盖损坏、并发、原子发布：是，Gate D。
是否用墙钟替代正确性测试：否。
是否错误要求 Local CI/production replacement/真实配置：否。
是否存在其它 AI 审查文档：未发现针对 270 的审查文档；无需修改正式方案。
```

激活前复核结果：`当前/` 原无其它施工文档，270 是 54 指定的下一项，工作树既有改动可保留，用户授权
仍在连续施工窗口内。2026-08-04 已移动到 `当前/` 并允许开始 Gate A。

## 12. 完成结果

```text
Gate A：完成。Tower 冷 Font producer 84,134ms；MSDF raster 79,655ms，确认首因。
Gate B：完成。内容寻址 envelope/cache service、显式 root、digest/path containment 已落地。
Gate C：完成。Font recipe 在 raster 前计算；sealed typed FontBundle miss/publish/hit 已落地。
Gate D：完成。atomic publish、同 key race reuse、corrupt/invalid quarantine/rebuild 已验证。
Gate E：完成。Assembly report v2 producerReports 已进入 Preview assembly stage handoff。
Gate F：完成。第二项目 first Produced / second Hit / output digest 一致。
Gate G：完成。Tower cold 85,270ms -> hot 1,550ms；新增 glyph miss；跨绝对路径 hit。
```

权威验证：

```text
cargo test -p editor_core project_assembly_artifact_cache：4 passed
cargo test -p editor_core project_font_cook：9 passed
cargo test -p editor_core project_runtime_package_assembler：12 passed
cargo test -p editor_core editor_preview_package_：4 passed
cargo test -p project_e2e_gate non_game_specific_second_project_reuses_sealed_font_producer_artifact：1 passed
cargo test -p engine_runtime：504 passed
cargo test -p editor_core：625 passed / 2 ignored
cargo fmt --all -- --check：passed
```

资格证据：

```text
.artifacts/270/tower-assembly-cache-qualification.json
.artifacts/270/tower-glyph-mutation-qualification.json
.artifacts/270/tower-path-independent-qualification.json
.artifacts/270/editor-core-full-final.log
```

未运行 Local CI；未替换 production/安装态二进制；未修改真实用户配置；未修改 Tower 源项目内容；
未执行或恢复 Tower P0-5 Gate G/H。
