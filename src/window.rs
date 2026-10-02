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

/// 主窗口（控制面板）—— 可缩放，触摸屏也能拉；最小尺寸保证四板块都排得下
pub fn main_options(cfg: &Config) -> eframe::NativeOptions {
    let mut opts = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Lexideck 词卡看板")
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
}
