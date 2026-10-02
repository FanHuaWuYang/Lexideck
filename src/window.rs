//! 窗口定义：
//!   · 主窗口 = 控制面板（普通带边框窗口，统一管理所有悬浮窗）
//!   · 悬浮窗 = 无边框 / 透明 / 置顶的独立 viewport（尺寸随词卡内容）

use eframe::egui;
use egui::viewport::{ViewportBuilder, WindowLevel};

use crate::config::Config;
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

/// 让面板窗口从任务栏（以及 Alt+Tab 列表）里消失 / 回来。
///
/// 为什么需要：egui 0.36 的 `with_taskbar` **只在建窗口时生效**，`ViewportCommand`
/// 里没有对应变体（查过 0.36.2 的全部变体），而「收进托盘」是把面板挪到屏幕外 ——
/// 窗口仍是「可见」态，任务栏按钮会一直留着（用户实测报的就是这个）。
/// 这里直接改扩展样式：不显示 = 置 `WS_EX_TOOLWINDOW` 并清 `WS_EX_APPWINDOW`，
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
        .with_title(format!("词卡 #{id}"))
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
