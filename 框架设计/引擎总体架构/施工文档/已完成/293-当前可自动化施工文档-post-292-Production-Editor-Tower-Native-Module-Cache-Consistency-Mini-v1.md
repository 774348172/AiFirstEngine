# 293 post-292 Production Editor + Tower Native Module Cache Consistency Mini v1 施工文档

状态：唯一 Gate 已完成并归档。

正式方案：`../../293-post-292-Production-Editor-Tower-Native-Module-Cache-Consistency-Mini-v1方案.md`

## 1. 授权

用户于 2026-08-16 明确选择方案 B，要求生成正式方案、极简施工文档并直接施工。授权包含：增量构建
`editor_host`、production Editor 备份/替换、Tower trust receipt 备份与一次显式 approve、已验证 exact
artifact 种入 production native-module cache、一次普通 Editor Open/Play/Stop 和失败回滚。

## 2. 唯一 Gate

1. 新建 `G:\AifeProductionRuns\293-post-292-<timestamp>`，记录 source/dirty、production/cache/receipt/process
   preflight。
2. 复用 `G:\Aife292Runs\20260816-143623\cargo-target`，运行：
   `cargo build -p editor_host --locked --features real-window,real-wgpu-surface`。
3. 将正确的 `editor_host.exe` 复制到 run root，记录 hash；确认相关进程为 0，byte-exact 备份 production
   Editor 和 Tower trust receipt，以 same-directory pending + hash 完成替换。
4. 将 Gate H exact artifact 复制到 production state root 的 run-owned staging，复核 identity、descriptor、seal、
   DLL/build-report hashes 后发布唯一 exact cache entry。
5. 启动普通 production Editor，完成 Tower Open / approve / Play / Stop；10 ms 采样 Cargo/rustc，要求
   Cargo spawn=0，结束后 owned processes=0。不得把 authority-only binary 冒充普通 Editor。
6. 写 receipt、完成记录、入口同步并归档本文。

## 3. 退出与回滚

```text
通过：candidate=pending=production hash；旧 Editor/receipt 备份存在；cache exact key/hash 正确；
      普通 Editor Open/Approve/Play/Stop 成功；Cargo spawn=0；owned processes=0。

失败：关闭本轮 owned Editor/Cargo/rustc；恢复旧 Editor 与旧 receipt；移除本轮新增 exact cache entry；
      验证 production before hash 与原 inventory；保留 evidence 后停止。
```

## 4. 禁止范围

不修改产品/Tower 源码或真实项目配置；不重建 Tower DLL/Preview cache；不运行完整 E2E、Local CI、视觉
矩阵、292 suites；不替换 Player/MCP/其它安装态二进制；不删除历史备份。

## 5. 必要性与验证经济审计

```text
claim: 292 在普通 installed Editor 中可用，Tower 首次使用 exact hit 且 Cargo spawn=0。
minimum change: editor_host replacement + one exact cache entry + one trust receipt + one smoke。
Gate risk: binary mismatch / cache corruption / stale trust / normal loader miss。
cheapest red-capable evidence: hash transaction + descriptor/seal validation + ordinary Editor smoke/process sample。
reused evidence: 292 Gate H source suites、retained artifact、source-built scenario；产品源码未变即有效。
estimated cost: 增量 build 5 分钟内，外部事务与 smoke 10 分钟内。
stop condition: build 超 15 分钟、identity/hash 不一致、出现 Cargo spawn、普通 Editor 任一步失败。
```

自审结论：与正式方案一致；单 Gate 足够；没有 schema、runner、冷构建、重复 suite 或发布级矩阵；允许施工。

## 6. 激活前复核

```text
当前施工文档冲突：无
待执行队列：空
production before：sha256:F8856CE9A507E7A2E6D4DCA6E8D20B2957ECB507AACB414917281240D7420EA8
Editor/Cargo/rustc：0
exact source artifact：存在
production native-module cache：不存在
授权与文档：一致
结论：通过，已激活
```

## 7. Gate 结果

```text
增量 editor_host build：passed，230.3s
production before：F8856CE9A507E7A2E6D4DCA6E8D20B2957ECB507AACB414917281240D7420EA8
candidate / production after：84729797707C5948D8F0D306D6ED2D42A6BB56F8CD1DED00A4282D6BDF441682
exact module identity：sha256:65029b9650706621ebc15ba5c02f4245e040a5bf617df01feb3438abecf81303
ordinary Editor Open / Approve / Play / Stop：passed
GameView terminal：stopped，frameCount=1131
Preview cache：Hit；未重建
Cargo spawn：0
remaining Editor/Cargo/rustc processes：0
rollback：not required
```

完成记录：`../../阶段完成记录/2026-08-16-post-292-Production-Editor-Tower-Native-Module-Cache-Consistency-Mini-v1/00-总览.md`。
未运行禁止范围内的 Local CI、完整 E2E、视觉矩阵或 292 suites；未重建 Tower DLL，未替换其它二进制。
