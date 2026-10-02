//! 系统托盘（P3a）：图标 + 右键菜单 + 事件回收。
//!
//! 三条硬约束（设计 §6.4 / §8 已核对）：
//!   ① 图标必须在**主线程、事件循环跑起来之后**创建（win32 消息循环要求），
//!      所以不在 `LexideckApp::new` 里建，而是等 `ui()` 第一帧懒创建；
//!   ② `TrayIcon` 句柄必须**长期持有**（随最后一个实例 drop 就从托盘消失），
//!      由调用方存进自己的结构体字段；
//!   ③ 事件不自己挂 handler，走 crate 自带 channel（`TrayIconEvent::receiver()`），
//!      每帧 `poll()` 收干。实测：自挂 `set_event_handler` 会偶发丢事件/卡顿。
//!      代价是事件最多等一帧（≤250ms）才落地，够用。
//!
//! 左键单击 = 切换面板；右键 = 打开**自绘菜单**（src/menu.rs，无模态循环）。
//!
//! # 为什么建完还要自检（`verify`，2026-10-02 加）
//!
//! `TrayIconBuilder::build()` 返回 `Ok` 只说明 `Shell_NotifyIcon` 的 `NIM_ADD`
//! 返回了 TRUE —— **不代表图标真的进了通知区**。实测到过一类「静默丢弃」：
//! 同一份 exe 放在 `RustroverProjects\Lexideck\` 这个文件夹里跑时，`build()` 成功、
//! 但 `Shell_NotifyIconGetRect` 定位不到图标、注册表
//! `HKCU\Control Panel\NotifyIconSettings` 里也没建条目（＝外壳压根没登记它）；
//! 把同一个 exe 放到别的任何目录，图标立刻正常。触发条件还没查明，
//! 所以**不猜原因，只回问外壳一次**：`verify()` 用 `Shell_NotifyIconGetRect` 判，
//! 查不到就如实当成失败（调用方见 app.rs 的 tray_tick 自检分支）。
//! 教训写进 README「坑 10」：`build()` 成功不能当成「图标已就绪」。
//!
//! # 为什么要 poke + 心跳（含实测记录）
//!
//! 曾经用系统原生菜单（muda/TrackPopupMenu）：它是**系统级模态循环**，
//! 点完菜单项之后 winit 的主循环会"冻住"——进程活着、消息泵还在转
//! （主线程停在 winit 的 PeekMessage 里）、CPU 0%、一帧都不跑；连 posted 的
//! WM_CLOSE 都不派发，只有**真实鼠标输入**能把它叫醒（4 次里复现 3 次）。
//! 光靠 `ctx.request_repaint()` 没用 —— 它也是 posted 消息，一样被吞。
//! winit 源码里对系统菜单冻结用的招数是给窗口发一条假的鼠标移动
//! （`PostMessageW(window, WM_MOUSEMOVE, ..)`）；实测有效：冻住时发一条，
//! 2 秒内恢复约 36 帧（零成本 A/B：F2==F1 冻住 → poke → F3 恢复）。
//!
//! 原生菜单现已**整个弃用**（右键改成自绘菜单，见 menu.rs：没有任何模态循环，
//! 从根上消灭了这类冻结）；这条心跳作为**通用保险**留下 —— 系统文件对话框等
//! 其它模态路径同样可能触发这种"睡死"，留着它，冻住也能自愈：
//!   ① 正常情况下 `ctx.request_repaint()` 就被叫醒，不发 poke；
//!   ② 250ms 心跳线程：如果最近一帧超过 `FROZEN_MS` 没来，就补一发假的
//!      WM_MOUSEMOVE —— 只在疑似冻住时发（lparam 用光标真实位置，不给假坐标）。
//! 开销与现有空闲节流 `request_repaint_after(300ms)` 同量级；心跳线程随 `Tray` 的 Drop 结束。
//! 图标逐字节画（雾青圆角方块 + 白色「L」，和面板顶栏标记同源），不引图片素材。

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use eframe::egui;
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::Shell::{Shell_NotifyIconGetRect, NOTIFYICONIDENTIFIER};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetWindowThreadProcessId, PostMessageW, WM_MOUSEMOVE,
};

/// 托盘产生的意图；app 每帧收干后执行。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayAction {
    /// 图标左键单击 / 菜单「显示 / 隐藏面板」：切换面板可见性
    TogglePanel,
    /// 图标右键：打开自绘菜单（x, y = 点击处的光标位置，物理像素）
    OpenMenu { x: f32, y: f32 },
    /// 菜单「显示全部卡片」（等价面板里的「按今日展示」）
    ShowCards,
    /// 菜单「隐藏全部卡片」（等价面板里的「隐藏全部卡片」）
    HideCards,
    /// 菜单「退出 Lexideck」：明确退出（绕过关闭拦截）
    Quit,
}

/// 托盘句柄。**必须长期持有**：drop 掉图标就从托盘消失。
/// Drop 同时会停掉心跳线程。
pub struct Tray {
    // 只为「持有」而存在的字段：没有别处读它，但一 drop 图标就没了。
    #[allow(dead_code)]
    icon: TrayIcon,
    /// 心跳线程开关（Drop 置 false，线程自己退出）
    alive: Arc<AtomicBool>,
    /// 最近一帧的时刻（app 每帧 `touch()`）；心跳据此判断主循环冻没冻
    last_frame_ms: Arc<AtomicU64>,
    /// crate 内部隐藏消息窗口的句柄（自检用；懒查找 + 缓存，0 = 还没找到/没找到）
    msg_hwnd: Cell<isize>,
    /// 自检是否已通过（通过后不再重复扫，见 `verify`）
    verified: Cell<bool>,
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Relaxed);
    }
}

/// 图标边长。Windows 会按托盘实际尺寸自行缩放（通常显示 16×16）。
const ICON_SIZE: u32 = 32;

/// 心跳间隔：保证有事件时最迟这么久动作落地（见文件头「为什么要 poke + 心跳」）。
const HEARTBEAT_MS: u64 = 250;

/// 超过这么久没有新帧就按「主循环冻住了」处理（正常空闲是 250~300ms 出一帧）。
const FROZEN_MS: u64 = 800;

/// crate 内部那个隐藏消息窗口的类名（tray-icon 0.26 定的常量）。
/// 自检要按「进程 + 类名」找到它，才能拿它的 (hWnd, uID) 回问外壳。
const MSG_WINDOW_CLASS: &str = "tray_icon_app";

/// 自检时扫的 uID 上限：crate 的 uID 是**进程内递增计数器**（从 0 起，实测命中的是 2）。
/// 一个进程里正常只会有 1~2 个托盘图标，扫到 32 足够；成本是几十次壳调用，只在自检时跑。
const UID_SWEEP_MAX: u32 = 32;

impl Tray {
    /// 创建托盘图标。必须**在主线程、事件循环跑起来之后**调用。
    /// 失败只回 `Err(说明)`，绝不 panic —— 调用方降级为「没有托盘照常用」。
    ///
    /// 注意：返回 `Ok` 只代表 `NIM_ADD` 报告成功，**不保证图标真的进了通知区** ——
    /// 调用方还要过一道 [`Tray::verify`]（见文件头）。
    pub fn create(ctx: &egui::Context, hwnd: isize) -> Result<Tray, String> {
        let icon = make_icon()?;
        let icon = TrayIconBuilder::new()
            .with_tooltip("Lexideck — 词卡看板控制面板")
            .with_icon(icon)
            // 不挂任何原生菜单：右键由我们自己弹自绘菜单（menu.rs）。
            // 挂原生菜单 = 系统模态 TrackPopupMenu = 冻主循环（见文件头）。
            .build()
            .map_err(|e| format!("托盘图标创建失败：{e}"))?;

        // 心跳线程：request_repaint 之外，疑似冻住时补一发假的鼠标移动（文件头有实测记录）。
        // Drop 时随 alive 结束。
        let alive = Arc::new(AtomicBool::new(true));
        let last_frame_ms = Arc::new(AtomicU64::new(now_ms()));
        Tray::trace(&format!("创建托盘：面板 hwnd = {hwnd:#x}"));
        {
            let (alive, last_frame_ms, ctx) = (alive.clone(), last_frame_ms.clone(), ctx.clone());
            std::thread::spawn(move || {
                while alive.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(HEARTBEAT_MS));
                    if !alive.load(Ordering::Relaxed) {
                        break;
                    }
                    ctx.request_repaint();
                    // 主循环冻住时上面那行是叫不醒它的（posted 消息不派发），
                    // 只有假鼠标移动能叫醒 —— 见文件头。
                    let stale = now_ms().saturating_sub(last_frame_ms.load(Ordering::Relaxed));
                    if stale > FROZEN_MS {
                        Tray::trace(&format!("疑似冻住（{stale}ms 没出帧）→ 发唤醒消息"));
                        poke(hwnd);
                    }
                }
            });
        }

        Ok(Tray {
            icon,
            alive,
            last_frame_ms,
            msg_hwnd: Cell::new(0),
            verified: Cell::new(false),
        })
    }

    /// 自检：图标到底进没进系统通知区。
    ///
    /// 做法是**回问外壳**：找到 crate 那个隐藏消息窗口，用它的 `hWnd` + 逐个 `uID`
    /// 调 `Shell_NotifyIconGetRect` —— 外壳认得这个图标才会返回成功（并给出它在托盘里的位置）。
    /// 一个都问不到 = 图标没进托盘（`NIM_ADD` 报成功也没用，见文件头）。
    ///
    /// 通过一次就记住（`verified`），之后不再扫。
    pub fn verify(&self) -> bool {
        if self.verified.get() {
            return true;
        }
        let hwnd = self.msg_hwnd();
        if hwnd.is_null() {
            Tray::trace("自检：找不到 crate 的消息窗口（tray_icon_app）");
            return false;
        }
        for uid in 0..=UID_SWEEP_MAX {
            let id = NOTIFYICONIDENTIFIER {
                cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
                hWnd: hwnd,
                uID: uid,
                guidItem: unsafe { std::mem::zeroed() },
            };
            let mut rect = RECT::default();
            // S_OK（0）= 外壳知道这个 (hWnd, uID) 的图标，并报得出它在托盘里的位置。
            if unsafe { Shell_NotifyIconGetRect(&id, &mut rect) } == 0 {
                self.verified.set(true);
                Tray::trace(&format!(
                    "自检通过：图标在托盘里 uID={uid} rect=({},{})-({},{})",
                    rect.left, rect.top, rect.right, rect.bottom
                ));
                return true;
            }
        }
        Tray::trace(&format!(
            "自检未通过：外壳认不出本进程的托盘图标（uID 0~{UID_SWEEP_MAX} 全问不到）"
        ));
        false
    }

    /// crate 内部隐藏消息窗口的句柄（懒查找 + 缓存；找不到返回 0）。
    fn msg_hwnd(&self) -> HWND {
        let cached = self.msg_hwnd.get();
        if cached != 0 {
            return cached as HWND;
        }
        let mut target = MsgWindowTarget {
            pid: std::process::id(),
            hwnd: 0,
        };
        unsafe {
            EnumWindows(
                Some(enum_msg_window),
                &mut target as *mut MsgWindowTarget as isize,
            );
        }
        self.msg_hwnd.set(target.hwnd);
        target.hwnd as HWND
    }

    /// app 每帧调一次：心跳用它判断主循环冻没冻（见文件头）。
    pub fn touch(&self) {
        self.last_frame_ms.store(now_ms(), Ordering::Relaxed);
    }

    /// 诊断用（debug 版控制台可见）：创建时与每次唤醒尝试都留一行。
    fn trace(msg: &str) {
        eprintln!("[tray] {msg}");
    }

    /// 每帧把事件收干，翻译成动作列表（没有事件的帧就是空表）。
    /// 走 crate 自带 channel：见文件头 ③ 的实测记录（自己挂 handler 会偶发丢事件）。
    pub fn poll(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button,
                button_state,
                position,
                ..
            } = ev
            {
                // position 是物理像素（点击处的光标位置）；换算成点、定位菜单交给 app
                if let Some(a) =
                    click_action(button, button_state, position.x as f32, position.y as f32)
                {
                    out.push(a);
                }
            }
        }
        out
    }
}

/// `EnumWindows` 回调要找的东西：本进程里那个类名 `tray_icon_app` 的窗口。
struct MsgWindowTarget {
    pid: u32,
    hwnd: isize,
}

/// `EnumWindows` 回调：只认「属于本进程 + 类名是 tray_icon_app」的顶层窗口。
/// 走到就停下（返回 0 终止枚举），结果写回 `MsgWindowTarget.hwnd`。
unsafe extern "system" fn enum_msg_window(hwnd: HWND, lparam: isize) -> i32 {
    let target = &mut *(lparam as *mut MsgWindowTarget);
    let mut wnd_pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut wnd_pid);
    if wnd_pid != target.pid {
        return 1; // 继续枚举
    }
    let mut buf = [0u16; 64];
    let n = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    if n > 0 && String::from_utf16_lossy(&buf[..n as usize]) == MSG_WINDOW_CLASS {
        target.hwnd = hwnd as isize;
        return 0; // 找到了，停
    }
    1
}

/// 托盘点击 → 动作。纯映射（托盘在 CI 里点不了，接线是否正确全靠它 + 单测保证）。
///
/// 只认「抬起」：一次物理点击会来「按下 + 抬起」两条事件，不滤就会触发两次。
/// 左键 = 切换面板；右键 = 开自绘菜单。
fn click_action(
    button: MouseButton,
    state: MouseButtonState,
    x: f32,
    y: f32,
) -> Option<TrayAction> {
    match (button, state) {
        (MouseButton::Left, MouseButtonState::Up) => Some(TrayAction::TogglePanel),
        (MouseButton::Right, MouseButtonState::Up) => Some(TrayAction::OpenMenu { x, y }),
        _ => None,
    }
}

/// 毫秒时间戳（只用来比较多久没出帧，系统时钟就够）。
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 唤醒被系统模态菜单冻住的主循环：给窗口发一条"鼠标在原地动了一下"的消息。
///
/// 为什么是鼠标移动：主循环冻住时 posted 的普通消息（WM_CLOSE、egui 的唤醒）都不派发，
/// 只有假鼠标移动能叫醒 —— winit 源码自己对系统菜单冻结用的就是这一招。lparam 用光标
/// 真实位置（等价于"鼠标没动"），不给窗口灌假坐标。拿不到句柄（hwnd = 0）就跳过：
/// 只是少一道保险，其它功能不受影响。
fn poke(hwnd: isize) {
    if hwnd == 0 {
        Tray::trace("拿不到面板句柄，跳过唤醒");
        return;
    }
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        let mut lp = 0isize;
        if GetCursorPos(&mut pt) != 0 && ScreenToClient(hwnd as HWND, &mut pt) != 0 {
            lp = ((pt.y as isize) << 16) | ((pt.x as isize) & 0xFFFF);
        }
        PostMessageW(hwnd as HWND, WM_MOUSEMOVE, 0, lp);
        Tray::trace(&format!("已发唤醒消息 → hwnd={hwnd:#x} lparam={lp}"));
    }
}

/// 生成托盘图标：雾青（#668D82）圆角方块 + 中心白色「L」。
fn make_icon() -> Result<Icon, String> {
    let rgba = icon_rgba(ICON_SIZE);
    Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE).map_err(|e| format!("托盘图标数据不合法：{e}"))
}

/// 逐像素画图标（32bpp RGBA）。公开成独立函数，单测只查数据、不碰 win32 句柄。
fn icon_rgba(size: u32) -> Vec<u8> {
    let fg = [0x66u8, 0x8D, 0x82]; // 雾青 = 主题强调色
    let radius = 7.0f32;
    let n = size as f32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            if !inside_round_rect(px, py, n, n, radius) {
                continue; // 圆角之外保持全透明
            }
            // 白色「L」：竖笔 (x 9..15, y 8..24) + 横笔 (x 9..24, y 20..24)
            let stem = (9.0..15.0).contains(&px) && (8.0..24.0).contains(&py);
            let foot = (9.0..24.0).contains(&px) && (20.0..24.0).contains(&py);
            let c = if stem || foot { [255, 255, 255] } else { fg };
            let i = ((y * size + x) * 4) as usize;
            rgba[i] = c[0];
            rgba[i + 1] = c[1];
            rgba[i + 2] = c[2];
            rgba[i + 3] = 255;
        }
    }
    rgba
}

/// 点 (px,py) 是否落在 (0,0)-(w,h) 的圆角矩形里（圆角半径 r）
fn inside_round_rect(px: f32, py: f32, w: f32, h: f32, r: f32) -> bool {
    if px < 0.0 || py < 0.0 || px > w || py > h {
        return false;
    }
    let dx = px - px.clamp(r, w - r);
    let dy = py - py.clamp(r, h - r);
    dx * dx + dy * dy <= r * r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 点击映射：左键抬起 = 切换面板；右键抬起 = 开菜单（带坐标）；只认抬起
    #[test]
    fn click_mapping() {
        assert_eq!(
            click_action(MouseButton::Left, MouseButtonState::Up, 10.0, 20.0),
            Some(TrayAction::TogglePanel)
        );
        assert_eq!(
            click_action(MouseButton::Right, MouseButtonState::Up, 10.0, 20.0),
            Some(TrayAction::OpenMenu { x: 10.0, y: 20.0 })
        );
        assert_eq!(
            click_action(MouseButton::Left, MouseButtonState::Down, 0.0, 0.0),
            None,
            "按下不触发（否则一次点击切换两次）"
        );
        assert_eq!(
            click_action(MouseButton::Right, MouseButtonState::Down, 0.0, 0.0),
            None
        );
        assert_eq!(
            click_action(MouseButton::Middle, MouseButtonState::Up, 0.0, 0.0),
            None
        );
    }

    /// 自检要按「进程 + 类名」找窗口，类名写错就永远找不到 —— 钉住这个常量。
    #[test]
    fn msg_window_class_and_sweep() {
        assert_eq!(MSG_WINDOW_CLASS, "tray_icon_app");
        assert!(UID_SWEEP_MAX >= 2, "实测命中的 uID 是 2，扫描范围要覆盖它");
    }

    /// 图标数据：尺寸正确、圆角外透明、底色雾青、中心是白色「L」
    #[test]
    fn icon_rgba_shape_and_pixels() {
        let px = icon_rgba(ICON_SIZE);
        assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        let at = |x: u32, y: u32| {
            let i = ((y * ICON_SIZE + x) * 4) as usize;
            (px[i], px[i + 1], px[i + 2], px[i + 3])
        };
        assert_eq!(at(0, 0).3, 0, "圆角之外应透明");
        assert_eq!(at(31, 31).3, 0, "圆角之外应透明");
        assert_eq!(at(16, 16).3, 255, "方块内不透明");
        assert_eq!(at(12, 16), (255, 255, 255, 255), "「L」竖笔是白色");
        assert_eq!(at(20, 22), (255, 255, 255, 255), "「L」横笔是白色");
        assert_eq!(at(16, 10), (0x66, 0x8D, 0x82, 255), "底色是雾青");
    }
}
