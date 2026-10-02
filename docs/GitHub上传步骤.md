# 把 Lexideck 传到 GitHub · 手把手

> 前提（已经和你确认过的）：仓库建**私有**，走 **RustRover 的图形界面**，你只需要在 Terminal 里敲两条命令。
> 全程约 15 分钟。卡在哪一步，就把那一步的原文发我，别自己瞎试。

---

## 0. 一次性：告诉 git「你是谁」

打开 RustRover 底部的 **Terminal**（`Alt+F12`），敲这两条 —— 换成你自己的：

```bash
git config --global user.name "你的GitHub用户名"
git config --global user.email "你注册GitHub用的邮箱"
```

- 用户名写 **GitHub 用户名**，这样网页上的提交记录能挂上你的头像。
- 邮箱不想暴露真地址的话，可以填 GitHub 给你的 `<用户名>@users.noreply.github.com`。
- **只有第一次要敲**，以后所有项目、所有编辑器都认这个身份。
- 检查：`git config --global user.name` 能打印出你填的东西就对了。

（现在的状态：本机还**没设过**这个身份，所以这一步必须先做，否则提交时 RustRover 会报
`Please tell me who you are`。）

> **本机的 git 是哪一个**：`C:\Users\muyan\AppData\Local\hermes\git\cmd\git.exe` —— 它是 Hermes
> 自带的便携版，已经加进了你的用户 PATH，RustRover 能自动找到，**不用额外装东西**。
> 万一 RustRover 提示找不到 git：`Settings` → `Version Control` → `Git` → 把上面这个路径填进
> `Path to Git executable`。
> 顺带一提：长远看值得装一个独立的 Git for Windows（Hermes 若重装，这份便携版可能一起没掉），
> 但**现在不用**，不影响今天的事。

## 1. 一次性：在 RustRover 里登录 GitHub

`File` → `Settings` → `Version Control` → `GitHub` → `Add account` → `Log In via GitHub…`

浏览器会弹出来 → 点授权 → 回到 RustRover，能看到你的头像就成功了。

**这一步直接替代了「去网页上生成令牌、再粘到配置里」那条麻烦路**，以后 push 不会再问你要密码。（GitHub 早就禁用了密码推送，弹窗要密码时别输，回这一步。）

## 2. 提交前先核对文件清单（最关键的一步）

`.gitignore` 我已经放好了，但**第一次提交照样要亲眼看一遍**。

在 RustRover 里按 `Alt+0` 打开 **Commit** 面板，看列出来的文件：

✅ **应该有**：`src/`、`Cargo.toml`、`Cargo.lock`、`README.md`、`AGENT.md`、`CONTRIBUTING.md`、`CHANGELOG.md`、
`docs/`、`design/`、`assets/`、`default_words.txt`、`skill-词表生成.md`、`.cargo/config.toml`、
`.gitignore`、`.gitattributes`、`.editorconfig`、`.github/`

❌ **绝对不该出现**：`target/`、`lexideck.exe`、`build_check.log`、`.bak/`、`.idea/`

> `target/` 有 **1.4 GB**。它一旦进了提交历史，就**再也删不干净**（删掉文件只是多一条记录，体积永远留在仓库里），只能重建仓库。这是本项目唯一一个「错一次就要推倒重来」的操作，所以第 2 步不能省。

看到不该出现的，**停下来告诉我**，别自己按提交。

## 3. 让 RustRover 建远程仓库并推上去

菜单栏 `Git` → `GitHub` → `Share Project on GitHub…`
（不同版本的菜单位置可能略有差别，也可能在 `VCS` 菜单下，或者在欢迎页/工具栏的 GitHub 图标里。）

弹窗里填：

| 字段 | 填什么 |
|---|---|
| Repository name | `Lexideck` |
| Description | `教室一体机上用的英语词卡看板（Rust + egui）` |
| Private | **勾上**（弹窗里若没有这一项：先建，建完到仓库 `Settings` → 最下面 `Change visibility` 改成 Private） |

然后点 `Share`。RustRover 会一口气做完三件事：

1. 把当前目录变成 git 仓库（**我已经帮你初始化过了，它只是认出来**）
2. 把上面那批文件做成第一次提交 —— 它可能问你要 commit 信息，填 `chore: 首次提交`
3. 在 GitHub 上建好仓库，并推送上去

## 4. 确认成功

三处对一下：

- RustRover 右下角状态栏显示当前分支 `master`，旁边的 ↑ 箭头**已经消失**（说明推完了）
- 打开 `https://github.com/你的用户名/Lexideck` —— 能看见文件列表和一条提交记录
- 仓库名旁边带 **Private** 标记

## 5. 以后每天怎么用

只有一个循环：

```
改代码  →  Alt+0 写提交信息并提交  →  右上角 ↑ 推送
```

- **改之前先拉一下**：右上角 ↓ 是 `Pull`。现在只有你一台机器，基本用不上；等哪天在另一台电脑上改了，它就是救命的。
- **推之前**：`cargo fmt` + `cargo test --release` + 界面改了要跑一眼 —— 详见 `CONTRIBUTING.md`。
- **要开分支**：右下角点 `master` → `New Branch`，名字 `feat/xxx`。分支上做完、CI 绿了，再合并回 `master`。

## 6. 出一体机用的 exe：打 tag

1. RustRover：`Git` → `New Tag…` → Tag name 填 `v0.1.0` → 勾上「推到远程」→ 确定
2. 推送后 GitHub 自动开始构建（仓库页 `Actions` 标签能看到进度），大概 5~10 分钟，变绿勾就好了
3. 进 `Releases` 页面 —— 可以下载到挂着 `lexideck.exe` 的发布包，直接拷到教室一体机上用

> 普通提交也会构建一次，产物在 `Actions` → 点进某次运行 → 页面最下面 `Artifacts` 里能下载。平时不用管，当备用。

## 7. 卡住了怎么办

| 现象 | 原因 / 怎么办 |
|---|---|
| 提交时提示 `Please tell me who you are` | 第 0 步没做，回 Terminal 敲那两条 |
| push 时弹窗要账号密码 | **别输密码**（GitHub 早就不支持了）。回第 1 步，在 RustRover 里把账号登进去 |
| push 被拒，提示 `rejected` / `non-fast-forward` | 本地和远程各有对方没有的提交。右上角 ↓ `Pull` 一次合并，再推 |
| 提示文件超过 100 MB | 十有八九是 `target/` 混进来了 —— 停下告诉我，这个要专门清理 |
| Commit 面板里看不到 `.gitignore`、`.github/` | RustRover 默认隐藏以点开头的文件，**不影响提交**，别为它折腾 |
| `Actions` 页面全红 | 点进去看是哪一步失败，把失败那步的日志发我 |
| 花不花钱 | 私有仓库每月免费 2000 分钟 Actions；**Windows 构建按 2 倍计费**。本项目一次约 3~5 分钟 → 按 10 分钟算，一个月跑 200 次也用不完 |

## 附：为什么先私有、暂不加 License

- **私有**：里面有你自己的设计取舍和备注，先自己看着；想公开随时能在 Settings 里改。
- **License（授权协议）**：私有仓库**不需要**。哪天想开源了再加一个 `LICENSE` —— 个人小工具一般选 MIT；同时要顺手清一遍 README 里不适合公开的内容。这件事不急，但别忘。
