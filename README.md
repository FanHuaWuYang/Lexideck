# Lexideck

装在班级电子一体机上的**英语词卡看板**：一个控制面板统一管理多个无边框悬浮窗，上课时把英语老师准备好的内容显示在屏幕上。

> **架构原则**：程序只负责显示，内容全在外部文件里。
> 老师给一份词表文件，它就照着显示；换内容 = 换文件，不用重新编译、不用改程序。

- 设计与调研记录：`Obsidian/Novawiki/concepts/教室单词悬浮窗-设计记录.md`
- 技术路线调研报告：`Nova/07_报告/reports/教室一体机英语单词悬浮窗-技术路线调研-2026.09.17.md`
- 产品目标与形态（要做什么）：`AGENT.md`
- 开发规范（分支 / 提交 / 发布）：`CONTRIBUTING.md` · 版本变化：`CHANGELOG.md`
- 上传 GitHub 的操作步骤：`docs/GitHub上传步骤.md`

## 当前状态（2026-10-01 更新：P1 完成）

**P1 已完成** —— 基础功能做扎实（词库 / 导入 / 词表管理 / 卡片区块），面板照 `design/控制面板-预演-v2.html` 实现。
逐项记录（做了哪些、子 agent 审查发现的问题与处置、验证方式、遗留项）见 `docs/开发记录-P1.md`。

- **数据**：`lexideck.json`（exe 同目录）是唯一主库；内容从面板的「导入词表…」并入主库，导入源之后与程序无关
- **控制面板**（主窗口）= 四个板块：词表 / 卡片 / 策略 / 设置；随窗口宽度自适应，控件按触摸加大
- **悬浮窗**（可开多个）= 纯展示：无边框、置顶、可拖拽；**一个窗固定显示一条内容，窗上没有任何按钮**
- 版式与入场动画自 9-25 冻结（直角控件 · 白底卡片 · 右侧 2.2% 色条 · 闪烁 + 色块滑出）

| 文件 | 作用 |
|---|---|
| `src/deck.rs` | 词库数据模型 + 格式校验 + 合并计划 |
| `src/library.rs` | 主库读写、旧 `词表.json` 一次性升级、主库不可信标记 |
| `src/import.rs` | 导入会话（解析 → 校验 → 分类 → 问冲突 → 提交）与结果清单 |
| `src/card.rs` | 卡片内容与版式（六个区块，可关闭默认展示） |
| `src/control.rs` | 控制面板 UI（四个板块 + 两种弹窗 + 离屏渲染测试） |
| `src/app.rs` | 应用状态机 + 悬浮窗 viewport 管理 + 平铺 |
| `src/float.rs` | 悬浮窗渲染（内容 + 入场动画覆盖层） |
| `src/anim.rs` | 入场动画（flashing 节奏 / easeOutExpo / 状态机） |
| `src/icons.rs` | 面板图标的矢量绘制（不依赖图片素材） |
| `src/theme.rs` | 三套主题：纯白（默认）/ 浅青 / 黄黑 |
| `src/config.rs` | 设置读写（exe 同目录 `设置.txt`，写失败静默） |
| `src/util.rs` | 宽容读文本（BOM / GBK）与文件时间戳 |
| `src/window.rs` | 主窗口 / 悬浮窗 viewport 定义 |
| `src/main.rs` | 入口 |

## 怎么跑

```bash
cargo run --release     # 控制面板 + 已启用的悬浮窗
cargo test              # 33 个测试（含面板的离屏渲染测试）
```

数据都写在 exe 同目录：主库 `lexideck.json` + 设置 `设置.txt`。首次运行程序会自己建主库；
换内容走面板导入，不需要手动改文件。

## 技术选型（已定）

Rust + **eframe / egui 0.36**（glow / OpenGL 后端），绿色单 exe。理由：只有 egui 把「无边框 + 逐像素透明 + 置顶 + 拖拽 + 运行时可切换鼠标穿透」全做成了现成 API，其余框架要么缺、要么得自己写 Win32。

## 已经踩过并解决的坑

1. **eframe 0.36 的 API 变了**：`App` trait 的必需方法是
   `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)`；
   多窗口的 viewport 回调同样直接给 `&mut Ui`（`|ui, class|`，不再是 `&Context`）。
   照网上教程写会编译失败。

2. **默认编译会依赖 `VCRUNTIME140.dll`**：教室机器没装 VC++ 运行库就直接打不开。
   `.cargo/config.toml` 里的 `+crt-static` 解决 —— 要在项目一开始就加。

3. **中文字体**从 `C:\Windows\Fonts\msyh.ttc` 读；**雅黑不含国际音标字符**
   （`ə` `ˈ` 会变方块）→ 字体回退链：雅黑 → Segoe UI → Arial。

4. **多个悬浮窗**用 `show_viewport_immediate` + 每个窗独立的 `ViewportBuilder`；
   置顶切换用 `send_viewport_cmd_to` 对每个窗单独发送。

5. **`App::ui` 只对根 viewport 调用，绝不能用「画布尺寸」判断当前在画哪个窗口**
   （2026-09-25 排查后修正）。曾经在 `ui()` 里用 `w≈600 && h≈560` 猜「这一 pass 是控制面板」，
   想拦住「面板被画进悬浮窗」——实测那段判断是**死代码**：`App::ui` 每帧只被调用一次，
   且只带根窗口的 Ui（实测日志连续 249 帧全是 `viewport_id=ROOT / 600×560`）。
   而它一旦因面板被拖动改过尺寸而失配，后果正是它想防的那件事：整幅控制面板按面板版式
   被压进悬浮窗的小画布（字挤成一团、词卡正文被盖掉）。**正确写法**：

   ```rust
   if ui.ctx().viewport_id() != egui::ViewportId::ROOT { return; }
   ```

   `ctx.viewport_id()` 是 egui 给的权威身份，任何窗口尺寸下都不会误判。
   悬浮窗的画面只由 `show_viewport_immediate` 的回调闭包（`float::draw`）负责。

6. **`Painter::galley` 的 `fallback_color` 不是「文字颜色」**：排版时
   `Painter::layout(text, font, color, wrap)` 已经把颜色烘进 galley 了，
   `galley(pos, g, fallback)` 的第三个参数只在字形颜色是占位符时才生效。
   把回退色临时改成 `Color32::RED` 调试，正文**不会**变红，只有占位字形会——
   想改文字颜色要改排版那一侧。

7. **⭐ 词卡排版必须在悬浮窗自己的 pass 里做**（2026-09-25 真正的主 bug，冷启动"乱字"的根因）。
   `card::plan` 早期是在主窗口（控制面板）那一 pass 里用 `ctx.layer_painter(...)` 预先排版，
   把排好的 `Galley` 存起来，再拿到悬浮窗的 pass 里绘制。**这是错的**：galley 里的字形
   UV 绑定的是「排版那一刻」的字体图集状态，隔一个 pass 才真正画出来，字形就取错了位置。
   症状非常有辨识度：

   - **行位置、行高、字号、颜色全对**，但**每个字都是错的形状**（用户描述"乱字"）
   - 文字的**抗锯齿过渡像素几乎消失**（实测 712 个纯色 vs 1493 个过渡，比值接近 0）
   - **随便点一下控制面板就正常**——因为交互触发了重绘，galley 在正确的 pass 里重排
   - 冷启动必现、稳定复现（不是偶发）

   **修法**（`app.rs`）：计划改为**惰性重建** —— `FloatWin::plan_dirty` 置脏后，
   在 `show_viewport_immediate` 的回调闭包里调 `card::plan(ui.ctx(), ...)`，
   排完顺便 `ViewportCommand::InnerSize` 调窗口大小。绝不在别的 pass 里预排。
   修复后实测：词头清晰可辨、过渡/纯色比 = 4.04。

8. **悬浮窗的入场动画必须由悬浮窗自己驱动**（2026-09-25 冷启动排查结论）。
   两个坑叠在一起：

   - **计时起点错了**：在 `spawn_float` 里 `anim.play()` 就开始计时，但窗口从创建到
     真正出现在屏幕上还有几百毫秒延迟 —— 等画面出来，0.70s 的闪烁段已经跑掉三分之一。
     改成窗口**第一帧真正画出来**时才启动计时（`FloatWin::replay_anim` /
     `start_anim_if_pending`）。
   - **帧率靠别人施舍**：`float::draw` 里只判断动画在播，重绘却依赖根窗口（控制面板）
     的节奏 —— 而面板常态节流是 **300ms 一帧**。面板一慢下来，浮窗动画就一顿一顿地停在
     半路，**必须点一下面板触发重绘才继续**（这是"点一下就好了"的另一个来源）。
     改成在 `float::draw` 里直接
     `ctx.request_repaint_of(ctx.viewport_id())`，让浮窗自己给自己要下一帧。

   实测（冷启动逐帧抓屏）：窗口 0.92s 出现 → 闪烁 → 扫出 → **2.63s 静止**，全程自驱。

## 复现体积数字

```bash
cargo build --release
ls -l target/release/lexideck.exe
```

当前 exe **5830656 字节**，二进制里 `VCRUNTIME140` 出现 **0 次**（已复验）。用 dumpbin（VS BuildTools）可看完整依赖清单：只应出现系统 DLL。
