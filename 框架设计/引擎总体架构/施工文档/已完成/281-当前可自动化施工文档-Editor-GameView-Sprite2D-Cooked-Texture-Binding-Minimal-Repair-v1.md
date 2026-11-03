# 281 Editor GameView Sprite2D Cooked Texture Binding Minimal Repair v1 施工文档

状态：当前可施工。

用户授权：引擎侧普通 Editor GameView Sprite2D cooked texture resolve、binding 与缺失纹理诊断
最小修复；完成后更新普通 Editor，只重建 Tower Preview 缓存。

正式方案：`281-Editor-GameView-Sprite2D-Cooked-Texture-Binding-Minimal-Repair-v1方案.md`。

## 范围复核

- 当前施工槽原为空，281 编号未占用。
- 工作区包含既有未提交施工基线；所有修改必须增量保留，不回退用户或历史施工改动。
- engine-owned：`editor_core/editor_gameview_play.rs`、必要的 `engine_runtime` texture/render submit
  接线与 owner tests；本次已有明确引擎授权。
- project-owned：Tower 只消费修复并重建 Preview，不修改 gameplay、RuntimeModule、AUI layout 或配置。
- 禁止：Local CI、完整视觉矩阵、真实配置修改、Player/MCP/其它 production 二进制替换。

## Gate A：红灯与 owner contract

任务：增加最小 RuntimePackage fixture，证明 Scene-only Sprite texture 在普通 Editor Play 中必须进入
upload registry，且最终 `DrawSpriteTextured` command 必须携带 texture handle；增加缺失 Sprite texture
诊断断言。

验证：

```powershell
cd G:\gameEngin\rust
cargo test -p editor_core editor_gameview_sprite_texture -- --nocapture
```

## Gate B：最小接线

任务：Editor 收集 AUI/Sprite texture 并集；EngineHostLoop 从共享 runtime texture context 生成
Sprite2D binding；按 consumer 分类 resolve diagnostic，不新增资源系统。

验证：复跑 Gate A，并运行直接受影响的 EngineHost/Editor GameView 定向测试。

## Gate C：受影响回归

```powershell
cd G:\gameEngin\rust
cargo fmt --check
cargo test -p engine_runtime engine_host_loop -- --nocapture
cargo test -p editor_core editor_gameview_play -- --nocapture
```

若上层过滤已包含相同测试，不机械重复。不得运行 Local CI。

## Gate D：production Editor 与 Tower Preview

1. 在 repository-external fresh run root 构建 production Editor candidate。
2. 对旧 `rust/target/debug/editor_host.exe` 做 byte-exact backup 后事务替换。
3. 对 Tower `samples/tower_defense_project/.aife/editor-preview/windows-dev/scene-main` 做备份。
4. 只重建该 Preview cache，不清理其它项目或场景缓存。
5. 打开普通 Editor 并完成最小 Play present 检查；保留 Editor 供用户查看。

## 完成闭环

生成 281 阶段完成记录；同步 `49`、`54`、施工文档 README、阶段完成记录 README；将本文归档到
`施工文档/已完成/`。报告 source/binary/cache identity、备份路径、实际测试和未执行项。

## 自审

- 范围与用户授权一致，不修改 Tower 玩法或真实配置。
- 红测覆盖用户实际缺口，不再以 renderable count 代替 texture binding/pixel present。
- 复用 275 runtime texture owner，不引入 Editor 私有 texture loader。
- production 替换和 cache 重建均先备份；只处理明确命名目标。
- 预计可在三小时窗口内完成；2h45m 后不启动新阶段。

