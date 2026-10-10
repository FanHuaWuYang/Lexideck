//! 窗口定义：
//!   · 主窗口 = 控制面板（普通带边框窗口，统一管理所有悬浮窗）
//!   · 悬浮窗 = 无边框 / 透明 / 置顶的独立 viewport（尺寸随词卡内容）

use eframe::egui;
use egui::viewport::{ViewportBuilder, WindowLevel};

use crate::config::{Config, Layer};
use crate::menu;

/// 任务栏显隐：`WS_EX_TOOLWINDOW`（不进任务栏 / 不进 Alt+Tab）
/// 与 `WS_EX_APPWINDOW`（强制进任务栏）这两位，数值和 Win32 一致。
const WS_EX_TOOLWINDOW_BIT: isize = 0x0000_0080;
const WS_EX_APPWINDOW_BIT: isize = 0x0004_0000;

/// 按「要不要露在任务栏」算出新的扩展样式位（纯函数；两个标记互斥，单测覆盖）。
pub fn ex_style_for_taskbar(ex: isize, visible: bool) -> isize {
    if visible {
        (ex & !WS_EX_TOOLWINDOW_BIT) | WS_EX_APPWINDOW_BIT
    } else {
        (ex | WS_EX_TOOLWINDOW_BIT) & !WS_EX_APPWINDOW_BIT
    }
}

/// 让窗口从任务栏（以及 Alt+Tab 列表）里消失 / 回来。
///
/// 为什么需要：**eframe 0.36 没有实现 `ViewportBuilder::taskbar`** —— eframe 源码里
/// 搜不到这个字段，`with_taskbar(false)` 是空操作，窗口建出来一律带 `WS_EX_APPWINDOW`
/// （真机实测：面板与四张卡片全是 `APPWINDOW`）。`ViewportCommand` 里也没有对应变体
/// （查过 0.36.2 全部变体），所以建完窗口只能自己改扩展样式。
/// 「收进托盘」只是把面板挪到屏幕外 —— 窗口仍是「可见」态，任务栏按钮会一直留着
/// （用户实测报的就是这个）。不显示 = 置 `WS_EX_TOOLWINDOW` 并清 `WS_EX_APPWINDOW`，
/// 显示 = 反过来。改完必须发一次 `SWP_FRAMECHANGED`，否则按钮会赖着不走。
/// hwnd = 0（拿不到句柄）时静默跳过：少一道效果，不影响别的功能。
pub fn set_taskbar_visible(hwnd: isize, visible: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    };
    if hwnd == 0 {
        return;
    }
    unsafe {
        let h = hwnd as windows_sys::Win32::Foundation::HWND;
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let new = ex_style_for_taskbar(ex, visible);
        if new == ex {
            return; // 已经是对的状态，别白折腾（每次显示都改会让任务栏闪）
        }
        SetWindowLongPtrW(h, GWL_EXSTYLE, new);
        SetWindowPos(
            h,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// 卡片窗口标题（建窗口和事后按标题找句柄必须同一拼法）
pub fn float_title(id: usize) -> String {
    format!("词卡 #{id}")
}

/// 主窗口（控制面板）标题 —— 建窗口 / 按标题找句柄 / 拖拽取位置都用它认人
pub const MAIN_TITLE: &str = "Lexideck 词卡看板";

// 最近一次 WM_TOUCH 触点数据：(hwnd, x, y)，x/y 是**屏幕**物理像素。
// 触点的屏幕坐标是消息里自带的，与窗口位置无关 —— 这正是拖拽要的"原子"数据源。
// （对比：窗口内坐标 = 屏幕坐标 − 窗口位置，窗口一动读数就反向变化，再混上更新
// 滞后就会自激振荡：真机实测手指横拖 48px、窗口乱跳 ±100px。）
thread_local! {
    static LAST_TOUCH: std::cell::Cell<Option<(isize, f32, f32)>> =
        const { std::cell::Cell::new(None) };
}

/// 触点数据要与窗口对得上才可用（别的窗口上的触摸不能拿来拖本窗口）
fn pick_touch(hwnd: isize, stored: Option<(isize, f32, f32)>) -> Option<egui::Pos2> {
    match stored {
        Some((h, x, y)) if h == hwnd && hwnd != 0 => Some(egui::pos2(x, y)),
        _ => None,
    }
}

/// 本窗口最近一次触点的屏幕位置（物理像素）；最近一次触点不是本窗口的 = None
pub fn last_touch_screen_px(hwnd: isize) -> Option<egui::Pos2> {
    pick_touch(hwnd, LAST_TOUCH.with(|c| c.get()))
}

const WM_TOUCH: u32 = 0x0240;
const TOUCH_HOOK_ID: usize = 0x1DE5; // 子类 ID，本进程内唯一即可

/// 触摸钩子：只**读** WM_TOUCH 的屏幕坐标留档，不消费消息
/// （DefSubclassProc 往下传，winit 照常收触摸、egui 照常出指针事件）。
unsafe extern "system" fn touch_hook_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    umsg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
    _uidsubclass: usize,
    _dwrefdata: usize,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::Input::Pointer::{GetPointerInfo, POINTER_INFO};
    use windows_sys::Win32::UI::Input::Touch::{GetTouchInputInfo, TOUCHINPUT};
    if umsg == WM_TOUCH {
        let count = (wparam & 0xFFFF) as u32; // 低字 = 触点数量
        if count > 0 {
            let mut ti = TOUCHINPUT::default();
            let cb = std::mem::size_of::<TOUCHINPUT>() as i32;
            if GetTouchInputInfo(lparam as _, 1, &mut ti, cb) != 0 {
                // 单位是百分之一像素；坐标系是屏幕（不掺窗口位置、天然免疫移动滞后）
                let px = ti.x as f32 / 100.0;
                let py = ti.y as f32 / 100.0;
                LAST_TOUCH.with(|c| c.set(Some((hwnd as isize, px, py))));
            }
        }
    }
    // 触摸实际走的是 WM_POINTER（PT_TOUCH）线（本机实测；winit 开着 mouse-in-pointer）。
    // WM_TOUCH 那路留着当双保险 —— 两台机制哪台先响都行，都是屏幕坐标。
    if umsg == WM_POINTERDOWN || umsg == WM_POINTERUPDATE || umsg == WM_POINTERUP {
        let id16 = (wparam & 0xFFFF) as u32;
        let mut pi = unsafe { std::mem::zeroed::<POINTER_INFO>() };
        if GetPointerInfo(id16, &mut pi) != 0 && pi.pointerType == PT_TOUCH {
            let px = pi.ptPixelLocation.x as f32;
            let py = pi.ptPixelLocation.y as f32;
            LAST_TOUCH.with(|c| c.set(Some((hwnd as isize, px, py))));
        }
    }
    windows_sys::Win32::UI::Shell::DefSubclassProc(hwnd, umsg, wparam, lparam)
}

const WM_POINTERDOWN: u32 = 0x0245;
const WM_POINTERUPDATE: u32 = 0x0246;
const WM_POINTERUP: u32 = 0x0247;
const PT_TOUCH: i32 = 0x00000002; // POINTER_INFO.pointerType 是 i32

/// 给窗口挂触摸钩子（幂等：重复调用 = 无操作，可以直接每帧调；窗口销毁钩子自动消失）
pub fn ensure_touch_hook(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        let ok = windows_sys::Win32::UI::Shell::SetWindowSubclass(
            hwnd as windows_sys::Win32::Foundation::HWND,
            Some(touch_hook_proc),
            TOUCH_HOOK_ID,
            0,
        );
        if ok == 0 {
            eprintln!("[touch] SetWindowSubclass FAILED hwnd={hwnd}");
        }
    }
}

/// 按标题找窗口句柄（找不到返回 0）。本进程的卡片标题「词卡 #n」在同桌面上唯一，
/// 单实例设计保证不会跑到别的实例的窗口上去。
pub fn find_window_by_title(title: &str) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
    let mut wide: Vec<u16> = title.encode_utf16().collect();
    wide.push(0);
    unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) as isize }
}

/// 窗口当前是否露在任务栏（看 `WS_EX_APPWINDOW` 位）
pub fn taskbar_visible(hwnd: isize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_EXSTYLE};
    if hwnd == 0 {
        return false;
    }
    unsafe {
        let h = hwnd as windows_sys::Win32::Foundation::HWND;
        (GetWindowLongPtrW(h, GWL_EXSTYLE) & WS_EX_APPWINDOW_BIT) != 0
    }
}

/// 「卡片不该出现在任务栏」的自查自纠（每帧一次，成本两次 Win32 调用）。
///
/// 为什么不能建窗口时一次搞定：eframe 0.36 压根没用 `ViewportBuilder::taskbar`
/// （见 `set_taskbar_visible` 的说明），所以 `float_viewport` 里的 `with_taskbar(false)`
/// 是空操作，卡片建出来一律带 `WS_EX_APPWINDOW` → 任务栏上多出卡片按钮（真机实测四种
/// 层级/穿透组合全中）。卡片窗口在隐藏后重新显示时会被**重建**，所以这里不记「改过没有」，
/// 而是每帧读一眼样式、不对就改 —— 重建之后也能自动纠回来。
pub fn ensure_no_taskbar(id: usize) {
    let h = find_window_by_title(&float_title(id));
    if h == 0 {
        return; // 窗口还没建出来（或已经销毁），下一帧再说
    }
    if !taskbar_visible(h) {
        return; // 已经不在任务栏上了，省掉一次写样式
    }
    set_taskbar_visible(h, false);
}

/// 窗口**此刻**的外沿位置（物理像素，现场问系统要；0 = 找不到）。
///
/// ⚠️ 拖拽锚点可以用这个（只取一次的常数）；但**拖拽推进不能**用它去换算，
/// 「滞后的窗口位置 + 新鲜的窗口内指针」会自激振荡（真机实测过：手指横拖 +48px、
/// 窗口乱跳 ±100px）。推进坐标一律用 `last_touch_screen_px` 的 WM_TOUCH 屏幕值。
pub fn pos_now_px(hwnd: isize) -> Option<egui::Pos2> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;
    if hwnd == 0 {
        return None;
    }
    unsafe {
        let mut r = windows_sys::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd as windows_sys::Win32::Foundation::HWND, &mut r) == 0 {
            return None;
        }
        Some(egui::pos2(r.left as f32, r.top as f32))
    }
}

/// 手动拖窗的目标位置：窗口起点 + 指针自拖拽起点的**屏幕位移**。
///
/// 为什么不用 `ViewportCommand::StartDrag`：它落到系统那边是
/// `SendMessage(WM_NCLBUTTONDOWN, HTCAPTION)` 的**模态拖拽循环**，只跟物理鼠标走 ——
/// 触摸拖拽时循环收不到任何鼠标消息，窗口纹丝不动（教室一体机真机实测）。
/// 自己算位移对鼠标 / 触摸 / 触控笔一视同仁（触摸只在触摸发起的拖拽里走这条路）。
///
/// 坐标必须用**屏幕位置**而不是窗口内位置：窗口跟着动，窗口内读数会反向变化，
/// 用窗口内坐标算会自我抵消、变成拖不动（负反馈）。
pub fn drag_target(
    win0: egui::Pos2,
    ptr0_screen: egui::Pos2,
    ptr_screen: egui::Pos2,
) -> egui::Pos2 {
    egui::pos2(
        win0.x + (ptr_screen.x - ptr0_screen.x),
        win0.y + (ptr_screen.y - ptr0_screen.y),
    )
}

/// 穿透中的卡片要不要留在「触摸注册表」里？纯函数（单测钉住这层关系）：
/// 穿透开 → 不注册；穿透关 → 注册。
///
/// 为什么穿透必须连触摸一起处理：微软官方文档（WM_TOUCH 的备注）写得很直白 ——
/// **WM_TOUCH 不认 HTTRANSPARENT**；实际行为是触摸只发给「注册过触摸」的窗口，
/// 注册过就直达，`WS_EX_TRANSPARENT`（winit 实现鼠标穿透用的就是它）拦不住。
/// 教室一体机实测：开了穿透，手指点卡片照样被卡片吃掉（还会把卡片顶到最前）。
/// 把卡片从触摸注册里摘掉之后，触摸按「鼠标提升」路径走 —— 鼠标命中测试本来就会
/// 跳过 `WS_EX_TRANSPARENT` 的窗口，触摸于是落到下面的 PPT 上。
pub fn touch_should_be_registered(passthrough: bool) -> bool {
    !passthrough
}

/// 让卡片的触摸注册状态与穿透开关保持一致（每帧自查自纠，成本一次 Win32 查询）。
/// 卡片窗口在隐藏后重新显示时会被**重建**（重建 = winit 又把它注册进触摸），
/// 所以和 `ensure_no_taskbar` 同理，每帧纠正一次而不是只在切开关时设一次。
pub fn ensure_touch_state(id: usize, passthrough: bool) {
    use windows_sys::Win32::UI::Input::Touch::{
        IsTouchWindow, RegisterTouchWindow, UnregisterTouchWindow, TWF_WANTPALM,
    };
    let h = find_window_by_title(&float_title(id));
    if h == 0 {
        return; // 窗口还没建出来（或已销毁），下一帧再说
    }
    unsafe {
        let hwnd = h as windows_sys::Win32::Foundation::HWND;
        let mut flags: u32 = 0;
        let registered = IsTouchWindow(hwnd, &mut flags) != 0;
        let want = touch_should_be_registered(passthrough);
        if registered == want {
            return; // 已经是对的状态，省掉一次写
        }
        if want {
            RegisterTouchWindow(hwnd, TWF_WANTPALM);
        } else {
            UnregisterTouchWindow(hwnd);
        }
    }
}

/// 主窗口（控制面板）—— 可缩放，触摸屏也能拉；最小尺寸保证四板块都排得下
pub fn main_options(cfg: &Config) -> eframe::NativeOptions {
    let mut opts = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title(MAIN_TITLE)
            .with_inner_size([cfg.window_w, cfg.window_h])
            .with_min_inner_size([720.0, 460.0]) // 比这个再小说明列就挤没了
            .with_resizable(true)
            .with_decorations(false), // 自绘顶栏（深色+金黄设计的一部分）
        ..Default::default()
    };
    // 垂直同步（设置项 vsync，默认开）：eframe 0.36 里它在 glow_options 里，
    // 开 = 跟着显示器刷新率（60Hz 屏 60 帧、165Hz 屏 165 帧），关 = 不限帧。
    opts.glow_options.vsync = cfg.vsync;
    opts
}

/// 卡片层级 → 窗口层级。app 里两处要用（建浮窗、改层级时给已有浮窗重发命令），
/// 拎出来是为了能单测：层级和穿透是两件事，谁都不许影响谁。
pub fn level_of(layer: Layer) -> WindowLevel {
    match layer {
        Layer::Top => WindowLevel::AlwaysOnTop,
        Layer::Bottom => WindowLevel::AlwaysOnBottom,
    }
}

/// 悬浮窗 viewport（尺寸由词卡内容决定，创建后程序会自动微调）
///
/// `level` = 卡片层级（置底 / 置顶）；`passthrough` = 鼠标穿透（设计 §5.2）。
/// 层级直接写进 builder；穿透也写进 builder，切层级时 app 会再补发一次
/// `ViewportCommand::MousePassthrough`（对已存在的窗口，改 builder 不一定立即生效）。
pub fn float_viewport(
    id: usize,
    pos: Option<egui::Pos2>,
    level: WindowLevel,
    passthrough: bool,
    size: egui::Vec2,
) -> ViewportBuilder {
    let mut b = ViewportBuilder::default()
        .with_title(float_title(id))
        .with_inner_size([size.x, size.y])
        .with_min_inner_size([200.0, 80.0])
        .with_decorations(false) // 无边框
        .with_transparent(true) // 背景透明（卡片自己画）
        .with_window_level(level)
        .with_mouse_passthrough(passthrough)
        .with_active(false) // 出现时不抢焦点（PPT 要保持键盘焦点）
        .with_resizable(false) // 尺寸随内容，不允许手动拉伸
        .with_taskbar(false); // 不占任务栏
    if let Some(p) = pos {
        b = b.with_position(p);
    }
    b
}

/// 托盘右键菜单的 viewport（自绘，见 menu.rs）。
///
/// 和悬浮窗的差别：置顶、**要焦点**（失焦 = 点了别处 = 关闭；ESC 也要收得到），
/// 不吃穿透（菜单必须能点）。标题只给自检脚本找窗口用，界面上看不到（无边框）。
pub fn menu_viewport(pos: egui::Pos2) -> ViewportBuilder {
    ViewportBuilder::default()
        .with_title("Lexideck 托盘菜单")
        .with_inner_size([menu::W, menu::H])
        .with_decorations(false)
        .with_window_level(WindowLevel::AlwaysOnTop) // 浮在词卡/PPT 之上
        .with_active(true) // 打开时拿焦点（关闭判定用）
        .with_resizable(false)
        .with_taskbar(false)
        .with_position(pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 层级与穿透**各自独立**地传进 viewport builder —— 四种组合都必须能表达
    /// （旧实现只允许「置底不穿透」和「置顶+穿透」两档）。
    #[test]
    fn float_viewport_carries_level_and_passthrough_independently() {
        for (level, pt) in [
            (WindowLevel::AlwaysOnTop, true),
            (WindowLevel::AlwaysOnTop, false),
            (WindowLevel::AlwaysOnBottom, false),
            (WindowLevel::AlwaysOnBottom, true),
        ] {
            let b = float_viewport(1, None, level, pt, egui::vec2(100.0, 50.0));
            assert_eq!(b.window_level, Some(level), "层级要原样传下去");
            assert_eq!(
                b.mouse_passthrough,
                Some(pt),
                "穿透要原样传下去，不许被层级带跑"
            );
            assert_eq!(b.taskbar, Some(false), "悬浮窗永远不进任务栏");
        }
    }

    /// 卡片窗口标题：建窗口与事后按标题找句柄必须同一拼法
    #[test]
    fn float_title_matches() {
        assert_eq!(float_title(1), "词卡 #1");
        assert_eq!(float_title(12), "词卡 #12");
    }

    /// 层级两档的映射（默认置底）
    #[test]
    fn level_mapping() {
        assert_eq!(level_of(Layer::Top), WindowLevel::AlwaysOnTop);
        assert_eq!(level_of(Layer::Bottom), WindowLevel::AlwaysOnBottom);
    }

    /// 收进托盘 = 摘掉 APPWINDOW、置上 TOOLWINDOW；唤回 = 反过来；其它位一个都不许动。
    #[test]
    fn taskbar_style_bits() {
        // 实测的面板扩展样式：APPWINDOW | WINDOWEDGE | ACCEPTFILES
        let base = 0x0004_0110isize;
        let hidden = ex_style_for_taskbar(base, false);
        assert_eq!(hidden & WS_EX_APPWINDOW_BIT, 0, "收进托盘要摘掉 APPWINDOW");
        assert_eq!(
            hidden & WS_EX_TOOLWINDOW_BIT,
            WS_EX_TOOLWINDOW_BIT,
            "收进托盘要置上 TOOLWINDOW"
        );
        assert_eq!(
            hidden & !(WS_EX_TOOLWINDOW_BIT | WS_EX_APPWINDOW_BIT),
            base & !WS_EX_APPWINDOW_BIT,
            "除这两位外，其它样式位不许变"
        );

        let shown = ex_style_for_taskbar(hidden, true);
        assert_eq!(shown, base, "一来一回要回到原样（否则唤回后样式越改越乱）");

        // 幂等：状态已经对了就别再改（每帧都 SetWindowPos 会让任务栏闪）
        assert_eq!(ex_style_for_taskbar(hidden, false), hidden);
        assert_eq!(ex_style_for_taskbar(shown, true), shown);
    }

    /// 拖窗位移：跟着指针的屏幕位移走；指针没动窗口不动；负坐标（左屏）也要对
    #[test]
    fn drag_target_follows_pointer_screen_delta() {
        let w0 = egui::pos2(100.0, 50.0);
        let p0 = egui::pos2(300.0, 200.0);
        assert_eq!(
            drag_target(w0, p0, egui::pos2(280.0, 170.0)),
            egui::pos2(80.0, 20.0),
            "指针移动 (-20,-30) → 窗口跟着移动 (-20,-30)"
        );
        assert_eq!(drag_target(w0, p0, p0), w0, "指针没动窗口就不许动");
        // 多显示器虚拟桌面的左屏是负坐标：从 -1440 屏拖一点点，位移要原样保留
        let w1 = egui::pos2(-1440.0, 100.0);
        let p1 = egui::pos2(-1000.0, 300.0);
        assert_eq!(
            drag_target(w1, p1, egui::pos2(-990.0, 290.0)),
            egui::pos2(-1430.0, 90.0)
        );
    }

    /// 触点数据要与窗口对得上才可用（别的窗口的触点不能拿来拖本窗口；
    /// 没抓到触点、句柄对不上都按不可用处理）
    #[test]
    fn pick_touch_matches_window() {
        assert_eq!(
            pick_touch(7, Some((7, 100.0, 50.0))),
            Some(egui::pos2(100.0, 50.0))
        );
        assert_eq!(pick_touch(7, Some((9, 100.0, 50.0))), None);
        assert_eq!(pick_touch(0, Some((0, 1.0, 2.0))), None);
        assert_eq!(pick_touch(7, None), None);
    }

    /// 穿透与触摸注册的关系：穿透开 = 卡片退出触摸注册（触摸才能漏下去）；
    /// 穿透关 = 卡片保留触摸（卡片要能被触摸拖动）
    #[test]
    fn touch_registration_follows_passthrough() {
        assert!(
            !touch_should_be_registered(true),
            "穿透开：卡片必须退出触摸注册（WM_TOUCH 不认透明）"
        );
        assert!(
            touch_should_be_registered(false),
            "穿透关：卡片保留触摸注册（触摸要能拖卡片）"
        );
    }
}
