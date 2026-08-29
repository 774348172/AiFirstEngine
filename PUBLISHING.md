# 发布到 GitHub 和 Gitee

本目录是独立的 V0.0.3 源码发布仓库，并继承 V0.0.1/V0.0.2 Git 历史。不要把上级主仓库的 `samples`、`target`、`evidence`、施工资料或其它 `release` 内容带入。

## 发布身份

```text
branch: main
tag: v0.0.3
commit message: release: AI First Game Engine v0.0.3
```

发布 commit 和 annotated tag 由成包流程创建。推送前应确认：

```powershell
git status --short
git cat-file -t v0.0.3
git show --stat --oneline v0.0.3
```

## 双远端

```powershell
git remote set-url origin https://gitee.com/brother-b/ai-game-engine.git
git remote add github https://github.com/774348172/AGEngine.git
git remote -v
```

如果 `github` 已存在，使用 `git remote set-url github ...`。

## 推送

```powershell
git push -u github main
git push github v0.0.3
git push -u origin main
git push origin v0.0.3
```

推送会改变远端状态，不属于本地成包步骤，必须单独确认后执行。本地成包不会自动推送。

推荐在两个平台的 Release 页面使用 [RELEASE_NOTES.md](RELEASE_NOTES.md) 作为版本介绍，并附上成包流程生成的 `AiFirstGameEngine-v0.0.3-source.zip` 及其 SHA-256。
