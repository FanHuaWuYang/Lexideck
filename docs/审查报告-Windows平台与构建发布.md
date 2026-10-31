> （2026-10-17 归档注记：本文作于 0.1.1 时期，文中引用的 `docs/开发记录-P3.md` 已合并进 `docs/开发记录.md`；
> 本文各项结论的逐条复核与修复情况见 `docs/开发记录.md` §5.4。）

# Lexideck 审查报告 · Windows 平台集成 + 构建/发布

审查对象：`C:\Users\muyan\RustroverProjects\Lexideck`（分支 `master`，HEAD `c795d06`，版本 0.1.1，
工作区干净）。本报告为**只读审查**产出，未修改任何被审文件。

证据一律写成 `文件:行号`。凡是引用上游 crate 源码的结论，都标注了实际的 crate 版本与文件
（`Cargo.lock` 实测：`eframe 0.36.2` / `egui 0.36.2` / `egui-winit 0.36.2` / `winit 0.30.13` /
`tray-icon 0.26.0` / `windows-sys 0.61.2` / `raw-window-handle 0.6.2`）。

---

## 0 先给结论（TL;DR）

| # | 结论 | 级别 |
|---|---|---|
| 1 | 自绘菜单窗口 `.with_taskbar(false)` 是**空操作**、且没有像卡片那样每帧自查 → 右键菜单会在任务栏留一个按钮 | 高 |
| 2 | 菜单定位把"物理像素"硬按 `pixels_per_point` 换算，且**不做屏幕边界钳制** → 高缩放/多屏/图标靠边时菜单可能跑到屏幕外 | 高 |
| 3 | 菜单"失焦即关"依赖一次性 `Focus`；系统拒绝给焦点时菜单**永不自动关闭** | 中高 |
| 4 | 代码与文档反复断言"eframe 0.36 没有实现 `ViewportBuilder::taskbar`" —— **与上游源码不符**（`egui-winit-0.36.2/src/lib.rs:2052,2191-2193` 明确有 `with_skip_taskbar(!show)`）。最终修法（自己改样式位）是对的，但理由错了，属于会误导后人的事实性错误 | 中高（认知风险） |
| 5 | 托盘自检**只做一次并永久缓存**，Explorer 重启后 crate 静默重注册失败也察觉不到；状态栏却一直写着"已就绪" | 中 |
| 6 | 托盘"暂时不可用"会**顺带改掉关闭语义**（`use_tray()` 依赖 `tray.is_some()`） | 中 |
| 7 | 唯一活着的托盘图标创建路径 `NIM_ADD` 失败被 crate **静默吞掉**（`tray-icon-0.26.0/.../mod.rs:164-167,640` 返回值被丢弃）→ 项目必须靠自检兜底，这个兜底又是"一次性"的（见 #5） | 中 |
| 8 | 一份 Windows 专属 unsafe 代码里，`SetWindowLongPtrW` / `SetWindowPos` / `PostMessageW` / `AllowSetForegroundWindow` / `EnumWindows` 的返回值**全部未检查** | 中 |
| 9 | 文档/CI 与实际产物脱节：README 写 exe 5,830,656 字节并声称"已复验"，实测 6,411,776 字节；release.yml **完全不跑测试/格式检查**；tag 与 `Cargo.toml` 版本号无一致性校验 | 中 |
| 10 | 平台模块的可自动化部分**几乎没被测试覆盖**：注册表真实读写 0 测试、`verify`/`msg_hwnd`/`set_taskbar_visible`/`ensure_no_taskbar`/菜单视口 0 测试；`window.rs` 的 4 个单测全是纯位运算 | 中 |

---

## 1 各平台相关模块的实现机制

### 1.1 托盘图标（`src/tray.rs`）

**创建时机与持有**
- 不在 `LexideckApp::new` 里建，而是在 `ui()` 第一帧懒创建：`src/app.rs:1424` → `tray_tick`
  → `src/app.rs:965` `tray::Tray::create(ctx, panel_hwnd(frame))`。文件头三条硬约束写在
  `src/tray.rs:3-10`：主线程 + 事件循环跑起来之后、句柄必须长期持有、事件走 crate 自带 channel。
- 句柄存在 `LexideckApp::tray: Option<tray::Tray>`（`src/app.rs:142`），`Tray` 里 `icon` 字段
  只为"持有"而存在（`src/tray.rs:79-80`），drop 即从托盘消失。

**图标本体**
- 逐像素画 32×32 RGBA：雾青 `#668D82` 圆角方块 + 白色「L」（`src/tray.rs:329-370`），
  `Icon::from_rgba`（`src/tray.rs:332`），tooltip「Lexideck — 词卡看板控制面板」（`src/tray.rs:123`）。
  不引图片素材，`icon_rgba`/`inside_round_rect` 是纯函数因此可单测（`src/tray.rs:410-424`）。

**事件通道**
- 不挂 `set_event_handler`，走 crate 的全局 channel：`TrayIconEvent::receiver().try_recv()`
  （`src/tray.rs:238`，上游 `tray-icon-0.26.0/src/lib.rs:852` 是 `static TRAY_CHANNEL: Lazy<(Sender, Receiver)>`，
  即**进程级无界队列**）。每帧 `poll()` 收干（`src/app.rs:1016`），翻译动作见 `src/tray.rs:286-297`：
  只认 `MouseButtonState::Up`（否则一次点击触发两次，`src/tray.rs:387-395` 有单测钉住）。
- 代价：事件最多等一帧才落地（文件头 `src/tray.rs:10` 自己承认 ≤250ms）。

**自检 `verify()`（2026-10-02 加）**
- 目的（`src/tray.rs:14-24`）：`build()` 返回 `Ok` 只代表 `NIM_ADD` 报 TRUE，**不代表图标真的进了通知区**。
- 做法（`src/tray.rs:171-202`）：
  1. `msg_hwnd()`（`src/tray.rs:205-222`）用 `EnumWindows` + `GetWindowThreadProcessId` +
     `GetClassNameW` 找"本进程 + 类名 `tray_icon_app`"的顶层窗口（常量 `src/tray.rs:108`，
     回调 `src/tray.rs:266-280`），懒查找 + 缓存在 `Cell<isize>`（`src/tray.rs:86,220`）；
  2. 对该 `hWnd` 逐个 `uID`（0..=32，`src/tray.rs:112`）构造 `NOTIFYICONIDENTIFIER`
     调 `Shell_NotifyIconGetRect`（`src/tray.rs:180-197`），`== 0`（S_OK）即认定"外壳认得这个图标"；
  3. 通过一次就 `verified = true` 永久缓存（`src/tray.rs:172-174,190`）。
- 上游事实核对：`tray-icon-0.26.0/src/counter.rs:11` 计数器**从 1 开始**（`AtomicU32::new(1)`），
  所以 uID 是 1、2、3……；"实测命中 2"（`src/tray.rs:110`）说明那次会话前一个图标已建过。
  扫 0..=32 能覆盖，`< 5` 次实例不会越界。

**重试策略（在 app 侧，不在 tray 侧）**
- `tray_retry_gap`：前 5 次每 2 秒，之后每 60 秒（`src/app.rs:934-940`，单测 `src/app.rs:1734-1741`）。
- `tray_due`：到点才试，并预约下一次（`src/app.rs:943-952`）。
- 自检节奏：建好后隔 `TRAY_CHECK_GAP = 700ms` 查第一次，最多 `TRAY_CHECK_MAX = 3` 次
  （`src/app.rs:921-930`）。判定是纯函数 `tray_check_again`（`src/app.rs:928-930`，单测 `src/app.rs:1719-1731`）。
- 三次都没确认 → **丢掉图标**（`self.tray = None`，`src/app.rs:1001`）→ 排队重试（`src/app.rs:1002-1003`）。

**失败时的用户可见反馈**
- 只有一条状态栏文字，没有弹窗、没有通知：
  - 创建失败：`src/app.rs:977-980`（含 crate 原始错误 + "稍后自动重试"）；
  - 自检未通过：`src/app.rs:1004-1009`（如实说"没能进入系统通知区"，并给出"换个文件夹"的经验提示）；
  - 确认通过：`src/app.rs:992-995`（"托盘图标已就绪…"）。
- 诊断用 `eprintln!("[tray] …")`：`src/tray.rs:230-232`，调用点 `src/tray.rs:134,148,177,191,198,315,325`。
  ⚠️ release 版是 `windows_subsystem = "windows"`（`src/main.rs:6`），**这些输出在发布产物里没有出口**。

**心跳 / poke（通用保险）**
- 250ms 心跳线程（`src/tray.rs:101,132-153`）：每轮先 `ctx.request_repaint()`，若
  `now - last_frame_ms > FROZEN_MS(800ms)`（`src/tray.rs:104`）再 `poke(hwnd)`。
- `poke`（`src/tray.rs:313-327`）：`GetCursorPos` + `ScreenToClient` 取光标真实位置，
  `PostMessageW(hwnd, WM_MOUSEMOVE, 0, lp)`。动机写在 `src/tray.rs:26-43`：原生菜单 `TrackPopupMenu`
  的模态循环会让 winit 主循环"睡死"，只有真实鼠标消息能叫醒（winit 自己也用这一招）。
- `touch()` 每帧更新时间戳（`src/tray.rs:225-227`，调用点 `src/app.rs:1015`）。
- `Drop for Tray` 只置 `alive=false`（`src/tray.rs:91-95`），线程最迟 250ms 后自行退出。

**原生菜单已整个弃用**
- `TrayIconBuilder` 不挂 menu（`src/tray.rs:125-127` 注释），右键改成自绘菜单。

### 1.2 自绘菜单（`src/menu.rs` + `src/app.rs::draw_menu`）

- 动机（`src/menu.rs:3-9`）：原生菜单①冻主循环、②心跳唤醒又会打断它自己的模态循环（约 1/2 概率
  变"点不动的幽灵"）——两难，所以整条路弃用。实测记录在 `docs/开发记录-P3.md:30-43`。
- 形态：**即时视口**（`ctx.show_viewport_immediate`，`src/app.rs:1088`），
  视口 id = `ViewportId::from_hash_of(("lexideck-menu", 0u8))`（`src/app.rs:1084`）。
- 排版：`menu::draw` 手画 4 个条目 + 2 条分隔线（`src/menu.rs:51-111`），
  高度是**编译期常量** `H = PAD*2 + ITEM_H*4 + SEP*2`（`src/menu.rs:36`），
  条目中心由 `item_center_y(n)` 给出供自动化脚本按固定偏移点击（`src/menu.rs:40-48`）。
  `H` 与排布的一致性有单测钉住（`src/menu.rs:183-187`）。
- 窗口（`src/window.rs:166-176`）：无边框、`AlwaysOnTop`、`with_active(true)`、`with_taskbar(false)`、
  不可缩放、按传入 `pos` 定位。
- 位置（`src/app.rs:1080-1083`）：`pos = (x/ppp - W, y/ppp - H)`，即"菜单右下角贴住点击处"；
  `ppp = ctx.pixels_per_point()`。
- 交互（`src/app.rs:1089-1109`）：点条目 → 执行并关闭；`Escape` → 关闭；
  失焦 → 关闭，但**必须这一辈子拿到过一次焦点**（`menu_got_focus` 门闩，`src/app.rs:1092-1096`，
  意图写在 `src/app.rs:1071-1072`）。
- 路由与图标点击共用 `do_tray_action`（`src/app.rs:1026-1064`），保证两边行为一致。

### 1.3 开机自启（`src/autostart.rs`）

- 位置与权限：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`（`src/autostart.rs:66`），
  值名 `"Lexideck"`（`src/autostart.rs:17`），只碰 HKCU、免管理员（文件头 `src/autostart.rs:8`）。
- **"以注册表为准"的核对逻辑**：
  - `query()` 每次现读（`src/autostart.rs:86-151`）：`RegOpenKeyExW(KEY_READ)`
    → `RegQueryValueExW` 先问长度、再读内容（`src/autostart.rs:106-131`）
    → UTF-16LE 解码 + `parse_target`（`src/autostart.rs:39-41`，去引号/空白）
    → 与 `current_exe()` 走 `same_path`（`src/autostart.rs:45-48`，忽略大小写与尾斜杠）比较。
  - 四态：`Off` / `On` / `Stale(旧路径)` / `Unknown(原因)`（`src/autostart.rs:20-30`）。
    空值被**特意**判成 `Stale("（值内容为空）")`，而不是"一致"（`src/autostart.rs:143-144`，单测 `src/autostart.rs:263`）。
  - 面板侧：设置页 `panel.autostart` 缓存为 `None` 时现读一次（`src/control.rs:1114-1116`），
    拨开关后**作废缓存**（`src/control.rs:1130`、`1137`）→ 下一帧重读，显示真结果。
    `Off` 且 `设置.txt` 记的是开启 → 明确提示"多半被安全软件清过"（`src/control.rs:1121-1123`）；
    `Stale` → 多显示一行旧路径 + 「重写为当前位置」按钮（`src/control.rs:1125,1132-1147`）。
  - `设置.txt` 里那份只是"人的意图"（`src/config.rs:192-195,252`；`src/autostart.rs:5-7`）。
- 写 / 删：
  - `enable(exe)`（`src/autostart.rs:154-183`）：值 = 带引号全路径（`value_data`，`src/autostart.rs:51-53`），
    `REG_SZ` + UTF-16LE + 结尾 0（`sz_bytes`，`src/autostart.rs:74-79`），`cbData` 含结尾 0。
    **只 open 不 create**（注释 `src/autostart.rs:153`，为了少一个 `Win32_Security` feature）。
  - `disable()`（`src/autostart.rs:186-210`）：`RegDeleteValueW`，键不存在或值不存在都算成功（幂等）。
- 错误处理：全部返回 `Result<(), String>`，错误串是 `注册表错误码 {code}`（`src/autostart.rs:81-83`），
  失败只改状态栏（`src/app.rs:1277-1282`），不 panic。
- 非 Windows：`query()` 恒 `Off`、读写成空操作（`src/autostart.rs:213-228`）。

### 1.4 单实例（`src/single.rs`）

- 名字：`Local\Lexideck.SingleInstance` / `Local\Lexideck.Wake`
  （`src/single.rs:23,29`），测试用名带 `.test` 后缀避免被真跑着的 app 影响
  （`src/single.rs:20-31`，理由写在 `src/single.rs:20-21`）。
- `install()`（`src/single.rs:75-96`）：
  1. `CreateMutexW` → 失败**当没有单实例保护，照常启动**（"别把用户挡在外面"，`src/single.rs:79-81`）；
  2. `GetLastError() == ERROR_ALREADY_EXISTS` → `CloseHandle` 并返回 `false`（`src/single.rs:82-85`）；
  3. 否则句柄存进 `static MUTEX: AtomicIsize`，**故意不关**（`src/single.rs:74`，随进程结束由系统回收）；
  4. `CreateEventW` 自动重置（`bManualReset=0`，`src/single.rs:90`）→ 存 `static EVENT`；建不出来只是少一条唤回通道。
- 二次启动（`src/main.rs:30-33`）：`!install()` → `wake_existing()` → `return Ok(())`，**不开新窗口**。
- `wake_existing()`（`src/single.rs:100-109`）：先 `AllowSetForegroundWindow(ASFW_ANY)` 把前台许可让出去，
  再 `OpenEventW(EVENT_MODIFY_STATE)` + `SetEvent` + `CloseHandle`。
  为什么不自己抢前台：Windows 默认禁止后台进程抢前台，硬抢只会闪任务栏（文件头 `src/single.rs:3-6`）。
- 已有实例（`src/single.rs:112-130`）：`watch(&ctx)` 起一个**阻塞等待**线程
  `WaitForSingleObject(h, 500)`，事件到了就 `WAKE.store(true)` + `ctx2.request_repaint()`；
  500ms 只是回头看一眼 `alive`。
- 主循环消费：`if single::take_wake() { self.show_panel(&ctx) … }`（`src/app.rs:1426-1429`），
  `take_wake` 是 `WAKE.swap(false)`（`src/single.rs:53-55`）。
- 句柄只有主线程在 `install()` 里写、`watch()` 里读一次（`src/single.rs:114`），竞态靠"install 先于 watch"这一调用顺序保证。

### 1.5 窗口样式修补（`src/window.rs`）

- 样式位：`WS_EX_TOOLWINDOW = 0x80`、`WS_EX_APPWINDOW = 0x40000`
  （`src/window.rs:13-14`）。纯函数 `ex_style_for_taskbar(ex, visible)`
  （`src/window.rs:17-23`）：显示 = 清 TOOLWINDOW 置 APPWINDOW；隐藏 = 置 TOOLWINDOW 清 APPWINDOW；
  其它位一个都不动（单测 `src/window.rs:218-241` 覆盖"两位互斥 + 幂等 + 其它位不变"）。
- `set_taskbar_visible(hwnd, visible)`（`src/window.rs:35-61`）：
  `GetWindowLongPtrW(GWL_EXSTYLE)` → 若 `new == ex` 直接 return（避免每次都 `SetWindowPos` 让任务栏闪，
  `src/window.rs:47-49`）→ `SetWindowLongPtrW` → `SetWindowPos(.., SWP_FRAMECHANGED | NOMOVE | NOSIZE | NOZORDER | NOACTIVATE)`。
  `SWP_FRAMECHANGED` 是必须的（"否则 Explorer 不重新评估、按钮赖着不走"，`src/window.rs:33`；
  真机验证表见 `docs/开发记录-P3.md:151-159`）。
- 面板侧：收进托盘时 `set_taskbar_visible(panel_hwnd, false)`
  （`src/app.rs:860`，`hide_panel`）→ 唤回时 `true`（`src/app.rs:868`，`show_panel`）。
  面板**不用** `Visible(false)`，而是挪到 `PARK_POS = (-32000,-32000)`
  （`src/app.rs:180,856`），理由写在 `src/app.rs:843-850`：eframe 0.36 对不可见窗口走"直接补画"旁路，
  没登记事件循环上下文，建新浮窗会断言崩（实测记录 `docs/开发记录-P3.md:16-28`）。
- 卡片侧：eframe 动态建的窗口拿不到句柄，于是**按标题找回来**：
  `find_window_by_title`（`src/window.rs:70-75`，`FindWindowW(null, "词卡 #n")`）
  → `taskbar_visible` 读位（`src/window.rs:78-87`）→ 不对才写（`ensure_no_taskbar`，`src/window.rs:96-105`）。
- **每帧自查**：卡片在 `show_viewport_immediate` 回调里每帧调一次
  （`src/app.rs:749-753`）。不记"改过没有"的原因写在 `src/window.rs:89-95`：卡片隐藏后重新显示
  窗口会被**重建**，记标记会漏掉新窗口。成本 = 每卡每帧两次 Win32 调用（`docs/开发记录-P3.md:174-175`）。
- `panel_hwnd` 的获取：每个根视口帧从 `eframe::Frame` 用 raw-window-handle 取一次
  （`src/app.rs:1420` → `src/app.rs:1560-1566`：`frame.window_handle()` → `RawWindowHandle::Win32(h) => h.hwnd.get()`，失败回 0）。

---

## 2 unsafe / Win32 调用清单

### 2.1 用到的 API（按模块）

| 文件 | API | 返回类型 | 返回值是否检查 |
|---|---|---|---|
| `src/tray.rs:215-219` | `EnumWindows` | `BOOL` | ❌ 未检查 |
| `src/tray.rs:269` | `GetWindowThreadProcessId` | `u32`（线程 id） | ❌ 未检查 |
| `src/tray.rs:274` | `GetClassNameW` | `i32` | ✅ `n > 0` 才用缓冲 |
| `src/tray.rs:189` | `Shell_NotifyIconGetRect` | `HRESULT` | ✅ `== 0`（S_OK）判成功；其余一律当"认不出" |
| `src/tray.rs:321` | `GetCursorPos` / `ScreenToClient` | `BOOL` | ✅ 两个都查 |
| `src/tray.rs:324` | `PostMessageW` | `BOOL` | ❌ 未检查 |
| `src/window.rs:45,79` | `GetWindowLongPtrW` | `isize` | ⚠️ 无法区分"样式就是 0"与"失败"，代码未尝试区分 |
| `src/window.rs:50` | `SetWindowLongPtrW` | `isize`（旧值，0 = 失败） | ❌ 未检查 |
| `src/window.rs:51-59` | `SetWindowPos` | `BOOL` | ❌ 未检查 |
| `src/window.rs:74` | `FindWindowW` | `HWND` | ✅ `h == 0` 时跳过 |
| `src/autostart.rs:89,106,124,158,169,189,203` | `RegOpenKeyExW` / `RegQueryValueExW` / `RegSetValueExW` / `RegDeleteValueW` / `RegCloseKey` | `WIN32_ERROR` | ✅ 逐个比对 `ERROR_SUCCESS`/`ERROR_FILE_NOT_FOUND` |
| `src/single.rs:77,82,83,90,102,103,105,106,123` | `CreateMutexW` / `GetLastError` / `CloseHandle` / `CreateEventW` / `AllowSetForegroundWindow` / `OpenEventW` / `SetEvent` / `WaitForSingleObject` | 混合 | 部分：`CreateMutexW` ✅、`OpenEventW` ✅、`WaitForSingleObject` ✅；`AllowSetForegroundWindow` ❌、`SetEvent` ❌、`CloseHandle` ❌ |

**句柄获取途径（raw-window-handle）**：唯一一处 —— `src/app.rs:1560-1566`，
`eframe::Frame::window_handle()` → `HasWindowHandle::as_raw()` → 只接受 `RawWindowHandle::Win32`，
其余（含 `Err`）一律回 `0`，由各调用点自己"静默跳过"（`src/window.rs:40-42`、`src/tray.rs:314-317`）。
版本一致性上没问题：`Cargo.toml:20` 锁 `raw-window-handle = "0.6"`，`Cargo.lock` 解析为 0.6.2，
与 winit 0.30 上游一致（同一份 crate，不会因 trait 版本不同而误判）。

### 2.2 内存 / 句柄泄漏

- **有意的**：`static MUTEX` / `static EVENT`（`src/single.rs:35-36`）拿到后永不关闭
  （`src/single.rs:74` 明说"句柄故意拿了不放"），互斥体活到进程结束由系统回收。可接受，但见 D-17。
- **无泄漏**：托盘图标由 `TrayIcon` 的 `Drop` 收尾（上游 `tray-icon-0.26.0/src/platform_impl/windows/mod.rs:320-332`：
  `remove_tray_icon` + `DestroyWindow`，`WM_DESTROY` 里 `Box::from_raw` 释放 `TrayUserData`）。
- **无泄漏**：`autostart` 每条路径都 `RegCloseKey`（`src/autostart.rs:115,119,132,177,204`），
  包括两次 `RegQueryValueExW` 之间的提前返回分支 —— 这一块做得比一般代码严谨。
- **线程**：`tray.rs` 与 `single.rs` 各起一个 `std::thread`，都不 join，随 `alive` 原子标记退出
  （`src/tray.rs:137-152`、`src/single.rs:119-128`）。最迟 250ms / 500ms 退出，不阻塞进程退出。
- **重复建对象**：`single.rs` 的单测里 `install()` 被调用两次（`src/single.rs:159-161`），
  第二次走 `CloseHandle`，不额外泄漏。

### 2.3 线程安全与消息循环的相互作用

- 三个线程在跑：winit 主线程（唯一拥有 HWND/egui 的那条）、托盘心跳线程、单实例等待线程。
- 两个后台线程只做两件事：置/读 `Atomic*`、调 `egui::Context::request_repaint()`
  （`src/tray.rs:143`、`src/single.rs:125`）。`egui::Context` 是 `Clone` 的 Arc 句柄，
  `request_repaint` 内部是原子操作 —— 这是本项目最正确的一处线程协作设计。
- **HWND 的跨线程使用**：`poke()` 在心跳线程里用主线程捕获的 `hwnd` 发 `PostMessageW`
  （`src/tray.rs:149` → `src/tray.rs:324`）。Win32 允许跨线程 `PostMessage`，但窗口若已销毁，
  `PostMessageW` 返回 FALSE 且被忽略 → 唤醒保险静默失效，无任何日志（`[tray] 已发唤醒消息` 这一行
  在 `PostMessageW` 之后无条件打印，`src/tray.rs:325`，所以日志会**谎报成功**）。
- **消息循环**：托盘图标的宿主窗口由 crate 在**创建它的线程**上用 `CreateWindowExW` 建
  （上游 `.../windows/mod.rs:133-154`），因此其消息泵就是 winit 主线程的消息泵 —— 这也是
  `tray.rs:3-5` 要求"主线程 + 事件循环起来之后"创建的真正原因。
- **自检会阻塞主线程**：`verify()` 最坏 33 次 `Shell_NotifyIconGetRect`（`src/tray.rs:180-197`）+
  一次 `EnumWindows` 全桌面枚举（`src/tray.rs:215`），发生在 UI 线程上、每 700ms 一次、最多 3 轮
  （`src/app.rs:921-923,985-997`）。`Shell_NotifyIconGetRect` 是同步跨进程调用，Explorer 卡顿时会带累主线程。
- **crate 内部的 Channel 是全局的**：`TrayIconEvent::receiver()` 是进程级单例
  （上游 `tray-icon-0.26.0/src/lib.rs:852`），`poll()` 不分图标（`src/tray.rs:236-255`）——
  与项目"一个进程只有一个托盘图标"的设定（单实例）自洽。
- **crate 自己的消息处理循环**：`WM_USER_TRAYICON` 里取 `GetCursorPos` 并查托盘矩形
  （上游 `.../windows/mod.rs:411-419`），事件推送是 **crate 在 UI 线程回调里同步 send 到 channel**，
  所以 `poll()` 实际上是在"同线程、下一帧"取，不存在顺序错乱。

---

## 3 构建与发布

### 3.1 crt-static 静态链接

- `.cargo/config.toml:3-4`：
  ```toml
  [target.'cfg(all(windows, target_env = "msvc"))']
  rustflags = ["-C", "target-feature=+crt-static"]
  ```
- **实测复验（本次审查）**：对 `target/release/lexideck.exe`（6,411,776 字节，mtime 2026-10-02 18:07）
  全文扫描 ASCII：
  `VCRUNTIME140` 0 次、`MSVCP140` 0 次、`api-ms-win-crt` 0 次、`ucrtbase` 0 次。
  ⇒ 静态链接**当前是生效的**，"拷到教室机器双击就能跑"这条成立。
- 局限：这是通过 `rustflags` 而非 `[target.*]` + `target-feature` 配置项实现的，
  会作用于该目录树内的**所有**构建（含 `cargo test` 的测试 exe），不能按 profile 覆盖；
  也没法用 `RUSTFLAGS` 之外的开关关掉。属于可接受的设计，但写在 README 的"坑 2"
  （`README.md:78-79`）里作为"一开始就要加"的经验，是合适的。

### 3.2 Cargo profile

- `Cargo.toml:31-36`（release）：`opt-level=3`、`lto=true`、`codegen-units=1`、`panic="abort"`、`strip=true`。
- `Cargo.toml:41-45`（dev）：自身 `opt-level=1` + 依赖 `opt-level=3`，理由（实测面板 34.5ms/帧 → 动画 29fps）
  写在 `Cargo.toml:38-40`。
- `panic="abort"` 的两个后果需要注意：
  1. `cargo test` 不受影响（Cargo 对 test profile 强制 `unwind`），CI 不会因此挂；
  2. 发布版**任何 panic 都会让进程直接消失**（没有 unwind、没有错误框、
     `windows_subsystem="windows"` 又没有控制台，`src/main.rs:6`）——
     教室场景下表现为"程序自己没了，什么都没留下"。见 D-13。
- `strip=true` + `panic="abort"` 也让"用户报错时抓 backtrace"这条路彻底断掉。

### 3.3 二进制体积与 VCRUNTIME 依赖

- README 声明：**当前 exe 5830656 字节**、`VCRUNTIME140` 出现 **0 次**、"已复验"（`README.md:186`）。
- 实测：**6,411,776 字节**（比文档多 581,120 字节 ≈ +10%），`VCRUNTIME140` 0 次。
  ⇒ 体积数字**已过期**，且 README 用了"已复验"这种强表述（D-9）。
- CI 的体积/依赖自检：`ci.yml:38-50`。逻辑是"把 exe 整个读成 ASCII 字符串，匹配 `VCRUNTIME140`"。
  注释说"与"但**只打印不设阈值**（`ci.yml:44`），实际卡的是字符串匹配这一条。
- 第三方 crate 事实：`tray-icon 0.26.0` 传递依赖 `muda 0.21.0`
  （`Cargo.lock`，`tray-icon` 的 `dependencies` 里有 `"muda"`），即使本项目从不用原生菜单，
  这份代码仍会被编译进产物。`Cargo.toml:16-18` 关于"windows-sys 选 0.61 是因为 tray-icon 已带同一版"
  的注释**属实**（`tray-icon` 依赖 `windows-sys 0.61.2`，项目也用 0.61.2，没有重复版本）；
  但依赖树里仍有 `windows-sys 0.52.0 / 0.59.0 / 0.60.2` 三份（`Cargo.lock`），
  注释想表达的"不额外多编一份"只对 windows-sys 本身成立。

### 3.4 CI workflow 的逻辑

**`.github/workflows/ci.yml`**
- 触发（`ci.yml:4-9`）：`push` 到 `master`、`pull_request` 到 `master`、`workflow_dispatch`。
  ⚠️ **其它分支的 push 不触发**（D-10）。
- runner（`ci.yml:15-17`）：`windows-latest`，注释说明"必须 Windows：glow/OpenGL + 静态运行库"。
- 步骤：`actions/checkout@v4`（`ci.yml:20`）→ `Swatinem/rust-cache@v2`（`ci.yml:22-23`）
  → `cargo fmt --all -- --check`（`ci.yml:25-26`，**会卡**）
  → `cargo clippy --all-targets`（`ci.yml:28-30`，**`continue-on-error: true`，不卡**）
  → `cargo test --release --all-targets`（`ci.yml:32-33`，**会卡**）
  → `cargo build --release`（`ci.yml:35-36`）
  → pwsh 自检产物体积 + VCRUNTIME（`ci.yml:38-50`）
  → `actions/upload-artifact@v4`，`if-no-files-found: error`（`ci.yml:52-57`）。
- **幂等性**：整体是幂等的（纯构建 + 上传），没有写外部状态的步骤。
  唯一非幂等点是 artifact 名固定为 `lexideck-exe`（`ci.yml:55`），多次运行会各留一份同名 artifact。
- **是否真的是"同一个产物"**：是。`cargo test --release` 与 `cargo build --release` 共用
  同一个 `target/release` 与同一套 rustflags，被检查的 exe 就是紧接着构建出来的那个。
- ⚠️ `--all-targets` 隐含"编译 benches/examples"，对当前仓库无实际影响。

**`.github/workflows/release.yml`**
- 触发（`release.yml:4-7`）：`push` tag `v*`。权限 `contents: write`（`release.yml:9-10`）。
- 步骤：checkout → rust-cache → `cargo build --release`（`release.yml:22-23`）
  → pwsh 建/复用 Release 并上传（`release.yml:25-36`）。
- **幂等性做得对**：先 `gh release view`，非 0 才 `gh release create`（`release.yml:32-35`），
  再 `gh release upload … --clobber`（`release.yml:36`）。重复打同一 tag 不会整 job 变红。
- ⚠️ **发布链路完全不跑 `cargo test` / `cargo fmt`**（D-9②）：打 tag 就能发布未过测试的代码。
- ⚠️ 资产名固定 `lexideck.exe`，不体现版本；下载到本地多个版本会互相覆盖。
- ⚠️ 不产出校验和（sha256），也不签名。

### 3.5 版本号一致性检查：**没有**

三处版本号，全部靠人手同步：

| 位置 | 当前值 | 谁在用 |
|---|---|---|
| `Cargo.toml:3` | `version = "0.1.1"` | 编译期 `env!("CARGO_PKG_VERSION")` |
| `Cargo.lock`（`lexideck` 包块） | `0.1.1` | 构建 |
| `CHANGELOG.md:11` | `## [0.1.1] - 2026-10-02` | 人看 |
| git tag | `v0.1.0`、`v0.1.1`（实测 `git tag`） | 触发 release |

- 版本号进产物的方式是好的：`src/control.rs:2759-2762` 用 `env!("CARGO_PKG_VERSION")` 显示，
  跟着 `Cargo.toml` 走（`docs/开发记录-P3.md:129` 也这么记）。
- **缺**：没有脚本/CI 校验 `tag == Cargo.toml.version`，也没有校验 CHANGELOG 有对应小节。
  打一个 `v0.2.0` 但忘了改 `Cargo.toml`，CI/release **都会全绿**，而 Release 页面挂的 exe
  自己报的是 0.1.1。见 D-9①。
- 另外 `CHANGELOG.md:36` 与 `CHANGELOG.md:85` 存在**两个 `## [0.1.0]`**（2026-10-02 与 2026-09-25），
  不符合 Keep a Changelog（它自己在 `CHANGELOG.md:3-4` 声明遵循该格式），会让任何自动解析吃不准。
- `README.md:60` 写"72 个测试"，`docs/开发记录-P3.md:270` 写"`cargo test` 78 通过" —— 数量说明也各说各话。

---

## 4 兼容性风险

### 4.1 Win10 / Win11
- 托盘图标：`Shell_NotifyIconGetRect` 需要 Win7+（本项目的下限满足）。Win11 默认把新图标放进
  `^` 折叠区，**应用抢不到常驻位**，只能引导用户去设置（面板给了直达按钮：`src/app.rs:1125-1129`
  → `open_taskbar_settings`，`src/app.rs:1552-1556`，`cmd /C start "" ms-settings:taskbar`）。
  这条经验写在 `README.md:159-161`。
- Win11 的"每个不同路径的 exe 算一个单独图标"（`README.md:150-151`）与上游实现有关：
  crate 用 `uID`（不用 GUID）标识图标（上游 `.../windows/mod.rs:629`，`uFlags` 里没有 `NIF_GUID`），
  而 `register_tray_icon` 每次启动都新建宿主窗口、uID 从 1 重新开始
  （`.../windows/mod.rs:97` + `counter.rs:11`），所以 `(hWnd, uID)` 这个"身份"每次启动都变。
  上游注释自己指出这是 Windows 用来记"是否常驻"的键（`.../windows/mod.rs:659-664`）——
  也就是说**用户的"固定到任务栏"偏好在每次重启后都会丢**。项目文档没有提到这一层（D-7 备注）。
- 有个现成的解：给图标传一个固定 `guid`。但 `tray-icon` 的 GUID 支持要走 `TrayIconBuilder` 的
  guid 接口，且项目现在完全没用 —— 属于"可选改进"，不是缺陷。

### 4.2 DPI 缩放
- 菜单定位：`src/app.rs:1080-1083` `x/ppp - menu::W`。`x` 来自
  `TrayIconEvent::Click.position`（上游是 `PhysicalPosition`，`.../windows/mod.rs:417`），
  即物理像素；`ppp = ctx.pixels_per_point()` 是**主窗口所在视口**的缩放。
  在 125%/150% 缩放、或点击发生在与主窗口不同 DPI 的显示器上时，这个换算不成立（D-2）。
  `docs/开发记录-P3.md:78` 自己把这条列为"没验到的"：**"多显示器 + 屏缩放 ≠ 100% 下菜单定位
  只按主屏像素比换算，等一体机实机验"** —— 本文确认这是真风险。
- 没有做屏幕边界钳制：菜单恒为"右下角贴住点击处"（`src/app.rs:1083`）。
  点击发生在屏幕右下角（正是托盘图标的位置！）时，`pos = (x/ppp - 196, y/ppp - H)`；
  若托盘在屏幕右侧，`x/ppp ≈ 屏宽`，则菜单左边缘 ≈ `屏宽 - 196`，一般还在屏内；
  但**任务栏在左侧/顶部**（Windows 允许）或托盘图标被拖到非右下角时，菜单会跑到屏幕外（D-2）。
- 卡片/面板：`screen_scale` 用 `monitor_size.y / 1080`（`src/app.rs:542-546`），
  `monitor_size` 取的是 `viewport().monitor_size`（逻辑点，`src/app.rs:531`）。
  DPI 150% 的 1920×1080 屏会拿到 ~1280×720 点 → `scale ≈ 0.667` → 卡片比 100% 缩放时明显小。
  这是"按分辨率等比"与"按 DPI"两种语义混在一起的老问题：`README.md:94`（"字号随屏幕分辨率等比适配"）
  声称的是分辨率适配，但实现里 DPI 会顺带改变结果。

### 4.3 多显示器
- `layout_cards`（`src/app.rs:567-602`）只用 `monitor.x / monitor.y`（**尺寸**），
  从 `(0,0)` 开始算绝对坐标（`src/app.rs:593` `x = monitor.x - margin - w - col*(w+gap)`）。
  完全不考虑显示器的**原点**：主屏不在 (0,0)、或副屏在负坐标时，卡片会被放到主屏的绝对坐标上，
  而面板可能根本在另一块屏（D-11）。
- `hide_panel` 把窗口挪到 `(-32000,-32000)`（`src/app.rs:180`）。多屏且某块屏覆盖该区域时，
  这个位置会真的落在某块屏上。`docs/开发记录-P3.md:50-53` 记录了另一个真实后果：
  挪到屏外后系统把面板算成"最近的显示器"，屏幕系数从 1.0 掉到 0.75、锚点换屏 ——
  修法是"面板可见时读实时值并记住"（`src/app.rs:528-540`），这是对的，但只是把症状压住了。
- `monitor_size` 的兜底是硬编码 `1920×1080`（`src/app.rs:539`），在拿不到信息的早期帧会按 1080p 排一版卡片。

### 4.4 Explorer 重启
- crate 侧**有**处理：注册了 `TaskbarCreated` 广播消息并加白 UIPI
  （上游 `.../windows/mod.rs:53-54,160`），收到就 `remove_tray_icon` + `register_tray_icon`
  （上游 `.../windows/mod.rs:378-388`）。
- ⚠️ **但重新注册的返回值被丢弃**（上游 `.../windows/mod.rs:380-387` 没接 `bool`），
  失败静默。而项目的 `verify()` 已经"通过一次就永久缓存"（`src/tray.rs:172-174`），
  **没有任何机制在 Explorer 重启后复查**（D-5）。最坏情况：Explorer 重启 → 图标消失 →
  项目仍显示"托盘图标已就绪" → 用户关掉面板后发现没有唤回入口。
  兜底只剩两条：再双击一次 exe（单实例唤回，`src/app.rs:1426-1429`）、
  或`use_tray()` 因 `tray.is_some()` 仍为真而继续"关闭=收进托盘" → 面板藏进屏外且没有图标。
- winit 侧也会在收到 `TaskbarCreated` 时重放 `skip_taskbar`
  （`winit-0.30.13/src/platform_impl/windows/event_loop.rs:2454-2457`），
  但它是 `ITaskbarList::DeleteTab` 路线，见 D-1。

### 4.5 注册表权限
- 只碰 `HKCU`（`src/autostart.rs:8,90,159,190`），普通用户可写，`免管理员` 这条成立。
- `enable()` **只 open 不 create**（`src/autostart.rs:153-167`）。若某台机器上
  `HKCU\...\Run` 键不存在（组策略清理 / 全新精简镜像 / 被安全软件删过），
  `RegOpenKeyExW` 回 `ERROR_FILE_NOT_FOUND` → 界面报"注册表错误码 2"，
  而**代码本来就知道这个错误码的含义**（`query()`/`disable()` 都专门处理了它，
  `src/autostart.rs:96,196`）—— 唯独写入路径不处理，也不会去建键（D-14）。
- 组策略 `DisableRegistryTools` / 受限账户：`RegSetValueExW` 回错误码 → 面板照实报错
  （`src/app.rs:1277-1282`），不假装成功。设计正确。
- 错误文案只有数字（`src/autostart.rs:81-83`），没有 `FormatMessage` → 用户看到"注册表错误码 5"
  需要自己上网查（D-14 附带）。

### 4.6 杀软误报
- 高风险特征清单（客观存在，不等于"会被报"）：
  1. `+crt-static` 静态链接 VC 运行库 —— 大量恶意样本的常见特征（`README.md:78-79` 记录的坑 2 本身就是它的好处）；
  2. 往 `HKCU\...\Run` 写启动项（`src/autostart.rs:154-183`）—— 自启是经典敏感行为；
  3. 单实例命名互斥体 + 命名事件（`src/single.rs:23,29`）；
  4. 无签名、无版本资源、`strip=true`、`panic="abort"`（`Cargo.toml:35-36`）——
     没有公司/产品元数据，云查杀拿不到信誉分。
- 项目里**没有任何缓解**：没有签名步骤、没有 SmartScreen 说明、没有在 README 里提"可能被拦"。
  已被验证过的间接影响是自启项被安全软件清掉 —— 代码把这种情况当正常态如实显示
  （`src/control.rs:1121-1123`"多半被安全软件清过"），这点处理得好。
- 建议（非本次改动范围）：买/用一张代码签名证书给 release 资产签名；或至少在 README
  写清楚"首次运行若被拦怎么放行"。

### 4.7 非中文系统 / 区域设置下的编码

先说结论：**GBK/BOM 这条线做得是对的，问题集中在路径编码与硬编码中文字面量上。**

- BOM 处理：
  - `设置.txt` 读取先剥 UTF-8 BOM：`src/config.rs:128` `text.trim_start_matches('\u{feff}')`；
  - 通用文本读取（词表/主库 JSON 走这里）：先认 `EF BB BF`，再试 UTF-8，失败退 GB18030
    （`src/util.rs:8-19`，用 `encoding_rs`，`Cargo.toml:10`）。GBK 写成的词表也能读。
  - 写盘：`config::save` 用 `String` + `fs::write`（UTF-8 无 BOM）。
    ⚠️ **不对称**：读的时候认 BOM，写的时候不写 BOM —— 自身闭环没问题，但用户手动改成
    "UTF-8 with BOM" 也能读，改成 GBK 就读不出来了（`from_str` 失败即回默认值，
    `src/config.rs:125-127` 直接 `return cfg`）。属于宽容度缺口，不是崩溃点。
- Win32 侧**全程用 W 系列 API**：`autostart.rs:69-71` `wide()` 只做 UTF-16 + 结尾 0，
  `single.rs:68-70` 同；`RegOpenKeyExW/RegQueryValueExW/RegSetValueExW/RegDeleteValueW`
  （`src/autostart.rs:60-63`）与 `CreateMutexW/CreateEventW/OpenEventW`（`src/single.rs:64`）
  都是宽字符版 —— 这是避免 ANSI 代码页问题（中文/日文/西欧区域）的**正确做法**。
- 路径编码的**真实缺口**：
  - `value_data()` 用 `exe.display()` 拼进 `REG_SZ`（`src/autostart.rs:51-53`），
    比较时 `query()` 用 `current_exe().to_string_lossy()`（`src/autostart.rs:145`）。
    路径若含无法用 UTF-8 表示的部分，`display()`/`to_string_lossy()` 会产生 U+FFFD，
    两边替换结果不一致 → 永久 `Stale`。概率极低（Windows 路径是 UTF-16，绝大多数可映射），
    但这是"用格式化字符串当路径"的通病。
  - `parse_target` 只做去引号+trim（`src/autostart.rs:39-41`），**不解析命令行参数**。
    所以 `"C:\a b\l.exe"` 能对上，但 `"C:\a b\l.exe" --silent`（或未加引号的
    `C:\Program Files\Lexideck\lexideck.exe`）会被当成"指向旧位置"，且"重写为当前位置"一点
    就会把用户手写的参数**抹掉**（`src/app.rs:1260-1266` → `enable` 直接覆盖值）。
    这个覆盖行为是破坏性的（D-15）。
- 硬编码中文/字体路径：
  - `C:\Windows\Fonts\msyh.ttc` 等四个候选（`src/app.rs:1614-1619`）—— 非中文 Windows
    （英文/日文系统）通常没有 `msyh.ttc`/`simhei.ttf`，会退到 `simsun.ttc`；
    全都没有时 egui 自带字体不含 CJK，**界面与词卡会全变成方块/空白**（不是崩溃，但是不可用）。
  - 托盘 tooltip（`src/tray.rs:123`）、状态栏文案、窗口标题（`src/window.rs:64-66,111,168`）
    全是中文字面量 —— 这些本身没问题（源码是 UTF-8），只是没有本地化能力。
  - `windows_subsystem="windows"` + `eprintln!` 的组合与代码页无关（release 下根本没控制台）。

---

## 5 可维护性问题

### 5.1 unsafe 边界
- 优点：`unsafe` 块**粒度很小、贴着调用点**，没有把整段逻辑包进 `unsafe fn`；
  纯逻辑被刻意抽出来做单测（`ex_style_for_taskbar`（`src/window.rs:17`）、
  `click_action`（`src/tray.rs:286`）、`parse_target`/`same_path`/`value_data`（`src/autostart.rs:39,45,51`）、
  `tray_check_again`/`tray_retry_gap`（`src/app.rs:928,934`））。这是**本项目最值得保留的部分**。
- 问题：
  1. **win32 调用的类型错误处理是"部分检查"**：注册表那一组做得很好，窗口样式/消息那一组
     基本不查（见 §2.1、D-8）。同一份代码库里两种标准并存，会让后来者以为"这个项目的 unsafe 都查过"。
  2. `enum_msg_window` 是 `unsafe extern "system" fn`，靠 `lparam as *mut MsgWindowTarget`
     把 `&mut` 穿过 FFI（`src/tray.rs:266-280`）。当前调用点 `&mut target as *mut _ as isize`
     （`src/tray.rs:215-218`）是合法的，但这段"裸指针 + 回调"没有任何注释解释别名规则；
     若将来有人把它改成异步/跨线程枚举就会立刻 UB。
  3. `unsafe { std::mem::zeroed() }` 造 `guidItem`（`src/tray.rs:185`）没问题（GUID 全零合法），
     但同处 `cbSize` 手写（`src/tray.rs:182`）—— 将来 crate 升级到带 `NIF_GUID` 的用法就会错配。
- 边界之外的隐性 unsafe：`EpiIntegration`/`glow` 那一层不在本次范围内，但项目里所有
  Win32 入口都集中在 `tray.rs`/`window.rs`/`autostart.rs`/`single.rs` 四个文件，
  加上 `app.rs:1560-1566` 的句柄获取 —— **边界清晰，这是好设计**。

### 5.2 平台代码与 egui 生命周期的耦合（最需要担心的一块）
- **平台动作挂在 `eframe::App::ui` 上**：`tray_tick` / `take_wake` / `intercept_close`
  必须在每帧开头、`place_once` 之前跑（`src/app.rs:1420-1432`），注释解释了原因
  （CancelClose 必须与 close_requested 同帧，`src/app.rs:888-889`）。这类"顺序即正确性"
  的隐式契约完全靠注释维持，没有任何类型/测试能防回归。
- **`self.tray.is_some()` 被当成"UI 语义"**：`use_tray()`（`src/app.rs:839-841`）同时决定
  关闭按钮语义（`src/app.rs:901-907,1227-1234`）和 `hide_panel` 是否安全（`src/app.rs:838` 注释）。
  托盘因为**平台原因**暂时不可用（自检失败被丢弃，`src/app.rs:1001`），会连带**改变产品行为**
  （关闭 = 直接退出）。见 D-6。
- **平台状态存在 `Cell` 里**：`Tray { msg_hwnd: Cell<isize>, verified: Cell<bool> }`
  （`src/tray.rs:86-88`）。`Cell` 明确不是 `Sync`，等于用类型系统禁止 `Tray` 跨线程 ——
  意图是好的，但同时也让 `Tray` 无法被静态持有/测试并行。
- **新窗口创建必须避开 eframe 的两条旁路**：不可见窗口（`src/app.rs:843-850`）
  与最小化窗口（`src/app.rs:675-678`）—— 两个"时序敏感"的规避都靠调用顺序和注释。
  一旦 eframe 升级修掉上游问题，这些规避就成了负资产（没有任何测试能提示"可以删了"）。
- **同一语义有三套实现**：窗口几何同时被 eframe 的窗口持久化（`NativeOptions` 默认
  `persist_window: true`，`eframe-0.36.2/src/epi.rs:379,463`）、自己的 `设置.txt`
  （`src/app.rs:796-831`）以及 `place_once` 的兜底（`src/app.rs:778-794`）管。
  本次审查确认：因为 `Cargo.toml:8` 用 `default-features = false` 且没开 `persistence`
  （`eframe-0.36.2/Cargo.toml` 的 `persistence = ["egui-winit/serde", "egui/persistence", "ron", "serde"]`），
  `create_storage` 返回 `None`（`eframe-0.36.2/src/native/epi_integration.rs:130-136`），
  ⇒ **没有产生 `%APPDATA%` 里的 `app.ron`，"绿色版"这条成立**。
  但这是一个纯靠 feature 开关维持的巧合，值得写进注释（否则哪天有人手滑加回默认 features，
  就会偷偷在用户 AppData 里留文件）。见 D-18。

### 5.3 错误处理
- 好的部分：
  - `autostart` 全部 `Result` + 精确到错误码，面板如实显示（`src/app.rs:1277-1282`）；
  - `Tray::create` 返回 `Err(String)`、绝不 panic（`src/tray.rs:116`），调用方降级为"没有托盘照常用"；
  - 状态栏是唯一反馈面（`src/app.rs:977-980,1004-1009`）。
- 差的部分：
  - **错误信息会被下一条状态覆盖**，且没有任何历史/日志（`self.status` 是单个
    `Option<String>`，`src/app.rs:133`）。用户看到"图标没进通知区"后随便点一下面板就再也找不回来了
    —— 而这条信息恰是排查唯一线索（`docs/开发记录-P3.md:209-210` 记录了"那句 status 只在面板底部
    一闪而过"曾经导致误判）。见 D-5 附带。
  - `trace()` 写 `eprintln!`（`src/tray.rs:230-232`），release 下没有出口（`src/main.rs:6`）
    —— 等于**最重要的诊断输出在生产环境完全不可见**。唯一可用的日志通道是 opt-in 的
    `LEXIDECK_FPS_LOG`（`src/app.rs:1568-1600`），它只记帧间隔，不记托盘/注册表错误。
  - 注册表错误只用数字（`src/autostart.rs:81-83`）。

### 5.4 测试方式：这些平台模块到底被测了什么

`#[test]` 实测统计（按文件，`grep -c '#\[test\]'`）：`deck.rs` 15、`control.rs` 17、`schedule.rs` 12、
`config.rs` 7、`import.rs` 6、`app.rs` 4、`autostart.rs` 4、`tray.rs` 3、`window.rs` 4、`menu.rs` 2、
`single.rs` 2、`anim.rs` 3、`card.rs` 1、`library.rs` 1 ⇒ **合计 81 个 `#[test]`**
（README 写 72、P3 记录写 78 —— 都不准）。

**平台模块的覆盖情况：**

| 模块 | 被测的 | **没被测的** |
|---|---|---|
| `src/tray.rs` | `click_action` 映射（`:378-400`）、`MSG_WINDOW_CLASS`/`UID_SWEEP_MAX` 常量（`:403-407`）、`icon_rgba` 像素（`:410-424`） | `Tray::create`、`msg_hwnd`（`EnumWindows` 那套）、`verify`（`Shell_NotifyIconGetRect` 扫描）、`poke`、心跳线程、`Drop` 语义 |
| `src/window.rs` | `ex_style_for_taskbar` 位运算（`:218-241`）、`float_title`（`:204-208`）、`level_of`（`:211-215`）、`float_viewport` builder 字段（`:184-201`） | `set_taskbar_visible`、`taskbar_visible`、`find_window_by_title`、`ensure_no_taskbar`、`menu_viewport` |
| `src/menu.rs` | `H` 与排布一致（`:183-187`）、条目中心递增且在框内（`:169-180`） | `draw`/`item`/`sep` 的绘制与命中（没有任何"点第 n 项 → 得到哪个 TrayAction"的测试，尽管 `item_center_y` 注释明确说是为脚本点击准备的，`src/menu.rs:39`） |
| `src/autostart.rs` | **纯函数**：`parse_target`、`same_path`、`value_data`、`classify`（本地复刻的判定逻辑，`:275-284`） | `query()` / `enable()` / `disable()` —— **真实的注册表读写 0 覆盖** |
| `src/single.rs` | ✅ 真实内核对象：`second_install_sees_running_instance`（`:157-162`）、`wake_round_trip`（`:165-183`，真的起 `egui::Context` + 等待线程 + `SetEvent`） | `AllowSetForegroundWindow` 效果、事件创建失败分支、`watch()` 在 `EVENT==0` 时的退化 |

**这些测试在 CI 里真的会跑吗？—— 会，但有两处值得注意：**
1. CI 跑 `cargo test --release --all-targets`（`ci.yml:33`），
   `single.rs` 的两个测试标了 `#[cfg(windows)]`（`src/single.rs:157,165`），在 `windows-latest` 上**确实执行**，
   会在 runner 里创建真实的命名互斥体/事件（进程退出即回收，不污染 runner，可接受）。
2. `autostart.rs` **没有任何测试碰真实注册表**（这是好设计），所以"注册表读写"这条链
   在 CI 里**零覆盖** —— 它的正确性完全依赖真机手测（`docs/开发记录-P3.md:93-101` 那张实测表）。
   若将来想加真实注册表测试，必须放在 `HKCU` 的**测试专用值名**下并在测试末尾清理，否则会污染 runner。
3. 一个副产品问题：`src/main.rs:6` 的 `windows_subsystem="windows"` 让 release 版
   **没有 stdout/stderr**。本次审查尝试直接运行已有的 release 测试二进制 `--list`，
   **重定向到文件后仍 0 字节输出** —— 也就是说 CI 里 `cargo test --release` 的失败详情
   （实际只有 "test result: FAILED" 与 panic 文本）仍然可见，但任何 `println!`/`eprintln!` 诊断
   在 release 测试二进制里都拿不到。排查 CI 失败时会比预期更难。

**测试的真实价值分布**：项目的测试重心在"数据/排版/面板渲染"（`deck/control/schedule/config/import` 共 57 个），
平台部分只有"纯函数 + 位运算 + 常量"（13 个），真正的 Win32 交互（建图标、改样式、写注册表）
**全部靠真机脚本手测**（`docs/开发记录-P3.md:151-159,177-184,272-282` 记录了这些脚本与证据）。
这个分工是**合理但写下来的**：平台代码不可自动化，所以必须把逻辑尽量挤到纯函数里 ——
项目在这条上做得比多数 Windows Rust 项目好，剩下的缺口（§6 D-8/D-12）正是"没挤干净的"那部分。

---

## 6 缺陷 / 隐患清单（按严重程度分级）

> 分级标准：**高** = 影响可见行为/用户可能踩到且无自救路径；**中** = 有自救路径或影响面窄，但应在下个版本处理；**低** = 卫生问题/文档漂移/未来风险。

### 高

#### D-1 自绘菜单会出现在任务栏（`with_taskbar(false)` 没生效、也没有自查）
- 证据：`src/window.rs:174` `.with_taskbar(false)` 建菜单视口；卡片有每帧自查
  （`src/app.rs:751-753` → `src/window.rs:96-105`），**菜单没有任何等效调用**。
- 上游事实：`with_taskbar` **是被实现的** —— `egui-winit-0.36.2/src/lib.rs:2052`（`taskbar: _taskbar`）
  与 `:2191-2193`（`window_attributes.with_skip_taskbar(!show)`），`#[cfg(target_os = "windows")]` 分支内。
- 为什么仍然失效：winit 的 `skip_taskbar` 走 `ITaskbarList::DeleteTab`
  （`winit-0.30.13/src/platform_impl/windows/window.rs:1498-1535`），
  而它在窗口创建流程里被调用在 `set_visible(true)` **之前**
  （同文件 `:1249` vs `:1263`），且 `DeleteTab` 不改任何扩展样式位。
- 影响：右键托盘图标时，任务栏会多出一个按钮；菜单关闭后按钮仍在（Explorer 已为该窗口登记 tab）。
- 修法：在 `draw_menu` 里对菜单视口做一次"按标题找回句柄 + 清 APPWINDOW/置 TOOLWINDOW"
  （菜单标题在 `src/window.rs:168`，唯一），或直接读 `ITaskbarList::DeleteTab` 的时机问题绕过。

#### D-2 菜单定位不钳制到屏幕，高缩放/多屏下会跑到屏幕外
- 证据：`src/app.rs:1083` `let pos = egui::pos2(x / ppp - menu::W, y / ppp - menu::H);`
  —— 没有 `ctx.input(|i| i.viewport().monitor_size)` 参与的边界判断，也没有按点击处所在显示器取 `ppp`。
- `x, y` 来自 crate 的 `PhysicalPosition`（上游 `tray-icon-0.26.0/.../windows/mod.rs:417`），
  `ppp` 取自根视口（`src/app.rs:1080`）。
- 项目自己已把这条列为未验证风险：`docs/开发记录-P3.md:78`。
- 影响：150% 缩放下菜单位置偏 ~1/3 尺寸；任务栏在屏幕顶部/左侧、或点击处靠近屏幕边缘时菜单部分或全部在屏幕外。
- 修法：按 `TrayIconEvent` 里的 `rect`（crate 提供了托盘图标矩形）/点击处所在显示器换算，
  并把 `pos` 钳到该显示器的工作区内（`menu::W/H` 是常量，钳制很便宜）。

### 中高

#### D-3 菜单"失焦即关"依赖一次性的 Focus 请求，焦点拿不到就永不自动关
- 证据：`src/app.rs:166`（`menu_need_focus` 注释"不能每帧都请求"）、`src/app.rs:1078-1079`
  （每帧开头清掉）、`src/app.rs:1089-1091`（只在 `need_focus` 为真时发一次 `ViewportCommand::Focus`）、
  `src/app.rs:1092-1096`（`Some(false) if got_focus` 才关）。
- 影响：Windows 前台窗口锁（用户正在别的程序里操作时弹菜单）会拒绝这次 `Focus`。
  此时 `menu_got_focus` 永远为 false → **点菜单外面不会关**，只剩 ESC 或点条目两条路。
  同时菜单窗口在任务栏上（D-1），用户很难判断它还开着。
- 修法：`got_focus` 为 false 时在若干帧后重试一次 `Focus`，或退化为"点击别处（`ctx.input` 的
  全局 pointer down）也关"。

#### D-4 代码/文档关于"eframe 没实现 `ViewportBuilder::taskbar`"的断言与上游不符
- 证据（项目侧）：`src/window.rs:26-31`、`src/window.rs:91-93`、`src/app.rs:751`、
  `README.md:144-147`、`README.md:171-172`、`docs/开发记录-P3.md:166-170`
  （"在 `eframe-0.36.2/src/` 里搜 `taskbar` 一个都搜不到"）。
- 证据（上游侧）：`egui-winit-0.36.2/src/lib.rs:2052,2191-2193` 明确把 `taskbar` 翻译成
  `WindowAttributesExtWindows::with_skip_taskbar(!show)`；`egui-0.36.2/src/viewport.rs:322,519-521`
  定义并实现了 `with_taskbar`。eframe 自己不碰它是因为转交给了 `egui_winit::create_window`，
  在 `eframe/src/` 里搜不到**并不等于**没人实现。
- 为什么仍然修对了：winit 的 `skip_taskbar` 只在建窗口时调一次 `ITaskbarList::DeleteTab`
  （见 D-1），不设 `WS_EX_TOOLWINDOW`、不清 `WS_EX_APPWINDOW`，窗口可见后 Explorer 会把 tab 加回来。
  所以"自己改样式位 + `SWP_FRAMECHANGED`"确实是正确解法。
- 风险：结论正确但论据错误，会误导后人；也会让 `float_viewport` 里那句
  `.with_taskbar(false)`（`src/window.rs:155`）变成"看起来没用就删掉"的诱饵 ——
  实际删掉后连 `DeleteTab` 都没了。
- 修法（纯文档）：把注释/README/开发记录改成"`with_taskbar` 有实现，但经 `ITaskbarList::DeleteTab`
  且时机在置可见之前，实测不生效；须自行改扩展样式位"。PR 模板要求"改了行为就改 README"
  （`CONTRIBUTING.md:81`），这条正是该走的路。

#### D-5 托盘自检只做一次并永久缓存，Explorer 重启后无法察觉图标已丢
- 证据：`src/tray.rs:172-174`（`verified` 命中即 `return true`）、`src/tray.rs:190`（置 true 后不再复位）；
  `src/app.rs:985-1012`（自检只在"刚建好"这一幕跑，`tray_check_at` 一旦置 `None` 就不再复查）。
- 上游：Explorer 重启后 crate 会重注册，但**返回值被丢弃**（`tray-icon-0.26.0/.../windows/mod.rs:378-388`）。
- 影响：图标消失后，面板状态栏仍写着"托盘图标已就绪"（`src/app.rs:992-995`），
  而它正是收进托盘后**唯一**的唤回入口（D-6 会进一步恶化）。
- 附带问题：这条状态会被任何后续 `status` 覆盖（`src/app.rs:133` 只有一个 `Option<String>`），
  用户无法回看。
- 修法：把自检做成低频周期任务（例如每 30s 或只在"收进托盘"前查一次），
  或订阅/轮询 `TaskbarCreated` 后重新 `verify`（把 `verified` 清掉）。

#### D-6 托盘"暂时不可用"会顺带改变关闭语义
- 证据：`src/app.rs:839-841` `fn use_tray(&self) -> bool { self.cfg.close_action == CloseAction::Tray && self.tray.is_some() }`；
  被用于关闭拦截（`src/app.rs:901-907`）与面板 ✕（`src/app.rs:1227-1234`）。
  `self.tray = None` 发生在自检连败 3 次后（`src/app.rs:1001`）。
- 影响：自检失败后到下次重试成功之间（前 5 次间隔 2s，之后就固定 60s），
  **关闭按钮从"收进托盘"变成"直接退出"**，而设置页仍显示"关闭窗口时：驻留托盘"
  （`src/control.rs:1057-1071` 的开关状态只跟 `cfg.close_action` 走）。用户视角是"随机一次点 ✕ 就退出了"。
- 修法：把"用户意图"（`cfg.close_action`）与"此刻能不能真的藏"解耦 ——
  托盘不可用时仍然按"意图"处理，只是提示"现在没有图标，建议先明确退出"。

#### D-7 托盘图标身份每次启动都变 → 用户的"固定到托盘"偏好每次重启丢失
- 证据（上游）：`tray-icon-0.26.0` 不使用 `NIF_GUID`（`.../windows/mod.rs:626-640` 的 `uFlags` 无 `NIF_GUID`），
  宿主窗口每次 `TrayIcon::new` 重建（`:133-154`），`uID` 来自进程内计数器且从 1 开始
  （`:97` + `counter.rs:11`）。上游注释自己指出 `(hWnd, uID)` 是 Windows 记"是否常驻"的键
  （`.../windows/mod.rs:659-664`）。
- 与文档的关系：`README.md:150-151` 只记了现象（"每个不同路径的 exe 在系统里算一个单独的图标，
  换目录/换机器都要重新放一次"），没记"每次重启都要重新放"这一更严重的一层。
- 修法：给托盘图标一个**固定的 GUID**（crate 支持，`apply_guid`，`.../windows/mod.rs:665-671`）。
  这不是 bug 修复而是体验修复，值得单开一条 issue。

#### D-8 Win32 返回值普遍未检查（同一份代码库内标准不一致）
- 证据：
  - `SetWindowLongPtrW` 返回旧值、0 表示失败 —— `src/window.rs:50` 未检查；
  - `SetWindowPos` 返回 `BOOL` —— `src/window.rs:51-59` 未检查；
  - `PostMessageW` 返回 `BOOL` —— `src/tray.rs:324` 未检查，且 `src/tray.rs:325` 无条件打印
    "已发唤醒消息" ⇒ **日志会谎报成功**；
  - `AllowSetForegroundWindow` 返回 `BOOL` —— `src/single.rs:102` 未检查（注释承认"失败也不影响"，
    但没有告诉用户"这次不会置顶"）；
  - `SetEvent` 返回 `BOOL` —— `src/single.rs:105` 未检查；
  - `EnumWindows` 返回 `BOOL`（枚举中途失败会返回 FALSE）—— `src/tray.rs:215-219` 未检查，
    失败时 `target.hwnd` 保持 0，被当成"没找到"。
- 影响：这些失败路径全都"静默降级"。结合 release 无控制台（`src/main.rs:6`），
  排查时拿不到任何线索。注册表那一组（`src/autostart.rs`）已经证明"逐个查返回值 + 把错误码报到 UI"
  是这个项目做得到的标准。

### 中

#### D-9 版本/产物一致性完全没有自动校验
- ① tag 与 `Cargo.toml` 版本号无校验：`release.yml:4-7` 只按 `v*` 触发，
  之后没有任何 `grep` 版本号或比对 tag 的步骤（`release.yml:22-36`）。CI 同理（`ci.yml`）。
  打 `v0.2.0` 而忘记改 `Cargo.toml:3` → 全绿发布，exe 自报 0.1.1（`src/control.rs:2759-2762`）。
- ② `release.yml` **不跑 `cargo test` / `cargo fmt`**（只有 `cargo build --release`，`release.yml:22-23`），
  而 `CONTRIBUTING.md:70` 的发布流程要求"打 tag 触发构建"。等于"打 tag 就能发未过测试的代码"。
- ③ CHANGELOG 有两个 `## [0.1.0]`（`CHANGELOG.md:36,85`），与其声明的 Keep a Changelog 格式不符（`CHANGELOG.md:3`）。
- ④ 测试数量文档漂移：`README.md:60` "72 个测试"，`docs/开发记录-P3.md:270` "78 通过"，实测 81 个 `#[test]`。

#### D-10 CI 触发条件与团队约定不匹配 + Clippy 形同虚设
- `ci.yml:5-6` 只在 `push` 到 `master` 时触发，其它分支的 push **不跑 CI**；
  而 `CONTRIBUTING.md:16` 明确要求"交给 AI 干活时一律开分支 → 等 CI 跑绿 → 再 Merge"。
  实践上只有开 PR 才会跑（`ci.yml:7-8`），"开完分支先等 CI"这条做不到。
- `ci.yml:28-30` `continue-on-error: true` + 注释"还有历史遗留警告没清" ⇒ 永远不会有动力清；
  现在等于**没有 lint 门禁**。

#### D-11 多显示器坐标只算尺寸、不算原点
- 证据：`src/app.rs:528-540`（`monitor_size` 返回 `Vec2` 尺寸，无原点）、
  `src/app.rs:593`（`x = monitor.x - margin - w - col*(w+gap)`，从 0 开始算绝对坐标）、
  `src/app.rs:872`（`fallback = (60, ms.y - window_h - 80)`）。
- 影响：面板在副屏（或主屏原点不是 (0,0)）时，卡片按主屏绝对坐标摆放 → 卡片可能落在另一块屏上；
  唤回面板的兜底位置也恒在"主屏左下"。
- 修法：把 `monitor_size` 换成 `monitor_rect`（原点 + 尺寸），`layout_cards` 接受 `Rect`。

#### D-12 按标题找回窗口句柄（`FindWindowW`）是全局命名空间匹配
- 证据：`src/window.rs:70-75`（`FindWindowW(null, title)`，**class 传 null = 任何类**）、
  `src/window.rs:96-105`（每卡每帧调用）、`src/app.rs:1084`（菜单视口 id 走 hash，不走标题）。
- 风险：
  1. `FindWindowW` 不限定进程/线程，只在"同桌面唯一标题"这一约定下工作
     （`src/window.rs:68-69` 注释承认这个依赖）。同桌面**另一个进程**若有名为"词卡 #1"的顶层窗口，
     就可能被本程序**改写扩展样式**（清 `WS_EX_APPWINDOW` 并置 `WS_EX_TOOLWINDOW`）。
     这是真实的跨进程副作用，不只是性能问题。
  2. 找不到时静默返回（`src/window.rs:98-100`）—— 若哪天 eframe 改了子视口标题策略，
     唯一症状就是"卡片又回到任务栏"，而代码不会报任何错。
  3. 成本：每卡每帧 2 次 Win32 调用（`src/window.rs:97,101`），≤16 张卡（`control::MAX_FLOATS`）
     ⇒ 每帧最多 ~32 次全桌面窗口查找，在教室一体机（低端 CPU + 多窗口）上是可感知的固定开销。
- 修法：改用 `EnumWindows` + PID 过滤（项目里已有现成写法：`src/tray.rs:266-280`），
  或者只在"样式不对"时才查找（把"每帧查"改成"失效时重查 + 低频兜底"）。

#### D-13 发布版 panic = 进程静默消失，且没有日志通道
- 证据：`Cargo.toml:35` `panic = "abort"`、`Cargo.toml:36` `strip = true`、
  `src/main.rs:6` `windows_subsystem = "windows"`、`src/tray.rs:230-232`（唯一的 trace 出口是 `eprintln!`）。
- 影响：教室场景下任何 panic（例如 eframe 那条"新建窗口被静默跳过"的断言，
  `docs/开发记录-P3.md:19-21`）表现为"程序没了"，用户无法提供任何信息，也没有本地日志文件可带回来。
- 修法（低成本、高收益）：注册一个 `std::panic::set_hook`，把消息 + 版本 + 时间写到
  exe 同目录的 `lexideck-crash.log`（符合"数据都放 exe 同目录"的绿色版约定，
  `.gitignore:12` 已挡 `*.log`）。

#### D-14 写自启项不建键 + 错误只有数字码
- 证据：`src/autostart.rs:158-167`（`enable` 只 `RegOpenKeyExW`，非 `ERROR_SUCCESS` 直接 `Err`）、
  对比 `src/autostart.rs:96`（`query` 处理 `ERROR_FILE_NOT_FOUND`）、
  `src/autostart.rs:196`（`disable` 也处理）。`src/autostart.rs:153` 注释：**"Run 键一定在，只开不建"**。
- 问题：这个"一定在"的假设不总成立（组策略清理、精简镜像、安全软件删键）。
  届时用户点开关只会看到"注册表错误码 2"（`src/autostart.rs:81-83`），
  既不告知原因也不提供出路。
- 修法：键不存在时用 `RegCreateKeyExW` 创建（多一个 `KEY_CREATE_SUB_KEY` 权限位，
  不需要额外的 `Win32_Security` feature），或至少把错误码翻译成人话。

#### D-15 `enable()` 会覆盖用户手写的启动项参数
- 证据：`value_data(exe)` 只写 `"<path>"`（`src/autostart.rs:51-53`），`RegSetValueExW` 直接覆盖（`:169-176`）；
  而 `query()` 比较的是**整个字符串**（`parse_target` 不去参数，`src/autostart.rs:39-41`）。
- 影响：用户（或某个安装器）把启动项改成 `"C:\a\lexideck.exe" --autostart`，面板判定为 `Stale`
  → 用户点「重写为当前位置」（`src/control.rs:1135-1138`）→ 参数被无声抹掉。
  另外未加引号、路径含空格的健壮写法 `C:\Program Files\...\lexideck.exe`
  会被 Windows 解释成 `C:\Program` + 参数，而本程序会把它显示成"旧位置"，误导用户去"修"一个本来能用的项。
- 修法：`parse_target` 拆出"第一个 token（考虑引号）"用于比较；`enable()` 时若发现原值带额外参数，
  保留它们或明确询问。

### 低

#### D-16 README 的 exe 体积"已复验"与实测不符
- 证据：`README.md:186` "当前 exe **5830656 字节**…（已复验）"；
  实测 `target/release/lexideck.exe` = **6,411,776 字节**（mtime 2026-10-02 18:07，即 0.1.1 的那个产物）。
- 影响：读者无法用它判断"我的产物是否正常"；"已复验"这个措辞会让下一次审查直接跳过它。
- 顺带：`README.md:181-184` 用 `ls -l`（Linux 语法）给 Windows 项目举例。

#### D-17 单实例的内核对象永不释放 + 事件名可被同会话任意进程触发
- 证据：`src/single.rs:35-36`（进程级 `AtomicIsize` 持句柄）、`src/single.rs:74`（"故意拿了不放"）、
  `src/single.rs:103`（`OpenEventW(EVENT_MODIFY_STATE, …)`，任何同会话进程都能 `SetEvent`）。
- 影响：① 句柄泄漏（设计取舍，可接受，但 `install()` 在单测里被调用两次时会多一个，`src/single.rs:159-161`）；
  ② 同会话的其它进程能"敲一下门"把面板唤到前台（轻微的窗口骚扰，无需提权）——考虑到教室一体机多为共享账户，值得在注释里记一句。

#### D-18 "绿色版"依赖 eframe feature 开关的巧合，无注释保护
- 证据：`Cargo.toml:8`（`default-features = false`、只开 `glow`/`default_fonts`）+
  `eframe-0.36.2/Cargo.toml` 的 `persistence` feature +
  `eframe-0.36.2/src/native/epi_integration.rs:130-136`（无 `persistence` 则 `create_storage` 返回 `None`）
  + `eframe-0.36.2/src/epi.rs:379,463`（`persist_window` 默认 `true`）。
- 现状：**没有** `%APPDATA%\lexideck\app.ron`，绿色版成立（本次已核对）。
- 风险：任何人加回 `default-features`（或加 `"persistence"`），eframe 就会开始在用户 AppData
  写窗口几何，与 `设置.txt`（`src/app.rs:796-831`）抢同一个语义，且**不会有任何编译错误或测试失败**。
- 修法：在 `Cargo.toml:8` 上面加一行注释说明"必须保持 `default-features = false` 且不开 `persistence`，
  否则会往 AppData 写 app.ron，破坏绿色版"。

#### D-19 诊断输出与发布形态不匹配
- 证据：`src/tray.rs:230-232` + 7 个调用点，全部 `eprintln!`；`src/main.rs:6` 让 release 版没有控制台。
- 影响：`docs/开发记录-P3.md:272-282` 那套"看 `[tray] 自检通过 / 未通过` 判断"的排查手段
  **只对 debug 版或从有控制台的终端启动时有效**；用户拿到的 release exe 永远不产生这些行。
- 修法：见 D-13（写文件）。另外 `Cargo.toml:41-45` 的 dev profile 优化是为了让 debug 版能跑动画，
  而 debug 版是控制台程序 —— `docs/开发记录-P3.md:108-109` 已经记录过"用 Start-Process 起 debug exe
  会继承管道、终端工具卡死"的坑，说明这条链路本身也在制造摩擦。

#### D-20 菜单打开位置用的是"过期坐标"
- 证据：`src/tray.rs:239-251`（坐标来自 crate 事件，`src/app.rs:1031` 存进 `menu_pos`）→
  `src/app.rs:1081-1083`（同一帧或后续帧才用）。事件最多滞后一帧（`src/tray.rs:10` 自述 ≤250ms）。
- 影响：主循环卡顿时菜单可能出现在光标已离开的位置。低风险（用户会再点一次）。

#### D-21 双击托盘图标 = 面板来回切换两次
- 证据：`src/tray.rs:238-255` 只匹配 `TrayIconEvent::Click`，`DoubleClick` 变体被 `if let` 丢弃；
  且 crate 会为一次双击同时发 `WM_LBUTTONUP` 与 `WM_LBUTTONDBLCLK`
  （上游 `.../windows/mod.rs:446-471`），前者映射成 `Click{Up}` → `TogglePanel`（`src/tray.rs:293`）。
- 影响：用户双击图标时面板先藏再显示（或反之），看起来像"没反应/闪了一下"。
  面板状态栏文案还会说"已收进托盘"再立刻被"面板已唤回"覆盖（`src/app.rs:1038-1044`）。
- 修法：收到 `DoubleClick` 时抑制同一批次里的 `Click` 动作。

#### D-22 菜单是否"已经看过"的状态放在 `LexideckApp` 上，没有测试
- 证据：`src/app.rs:161-168`（`menu_open` / `menu_pos` / `menu_need_focus` / `menu_got_focus` 四个字段
  表达一个状态机）；`src/menu.rs:39` 的注释说 `item_center_y` 是"供自动化测试/脚本按固定偏移点击用"，
  但仓库里**没有任何测试或脚本使用它**（本次 grep 全仓未发现第二个调用点）。
- 影响：菜单交互（本项目最"平台敏感"的一段）只有真机手测覆盖（`docs/开发记录-P3.md:68-70`），
  而 `H`/`item_center_y` 的单测保护的是**排版常量**，不是交互。属于"测试看起来覆盖了、实际没有"的落差。

---

## 7 给下一步的优先级建议

1. **立刻**（成本低、用户可见）：D-1（菜单进任务栏）、D-2（菜单钳制 + 正确 DPI）、D-3（Focus 重试）。
   这三条都在同一个函数里（`src/app.rs:1073-1110`），可以一起改。
2. **下个版本**：D-5/D-6（托盘状态与关闭语义解耦 + 周期复查）、D-9（版本一致性 + release 跑测试）、
   D-13（崩溃日志）、D-8（补齐返回值检查）。
3. **文档更正**（零风险、立刻可做）：D-4（eframe `taskbar` 的事实）、D-16（体积数字）、
   D-18（feature 开关的注释保护）、D-9③④（CHANGELOG 与测试数量）。
4. **有余力**：D-7（固定 GUID 的托盘图标）、D-11（多屏原点）、D-12（PID 过滤替代 `FindWindowW`）、
   D-15（保留用户参数）、D-21（双击抑制）。

---

### 附：本次审查的验证方法（可复现）

- 全文精读 `src/tray.rs`(425) / `src/menu.rs`(188) / `src/autostart.rs`(296) / `src/single.rs`(184) /
  `src/window.rs`(242) / `src/main.rs`(42) / `Cargo.toml`(45) / `.cargo/config.toml`(4) /
  `.github/**` / `README.md`(186) / `docs/开发记录-P3.md`(282) / `CHANGELOG.md` / `CONTRIBUTING.md` / `AGENT.md`；
  另按需读了 `src/app.rs`(1763) 与 `src/control.rs` 的相关片段、`src/util.rs`、`src/config.rs`。
- 交叉核对上游源码（`~/.cargo/registry/src/index.crates.io-*/`）：
  `eframe-0.36.2`、`egui-0.36.2`、`egui-winit-0.36.2`、`winit-0.30.13`、`tray-icon-0.26.0`、`windows-sys-0.61.2`。
- 二进制实测：`Get-Item target/release/lexideck.exe` → 6,411,776 字节；
  全文 ASCII 扫描 → `VCRUNTIME140`/`MSVCP140`/`api-ms-win-crt`/`ucrtbase` 各 0 次。
- 仓库状态：`git status --porcelain` 空、`git tag` = `v0.1.0`/`v0.1.1`、HEAD = `c795d06`。
- 测试数量：`#[test]` 逐文件 grep 计数合计 81。
- 未做（超出只读审查范围）：真机运行、多屏/高 DPI 实测、注册表写入实测。
