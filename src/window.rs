//! 窗口定义：
//!   · 主窗口 = 控制面板（普通带边框窗口，统一管理所有悬浮窗）
//!   · 悬浮窗 = 无边框 / 透明 / 置顶的独立 viewport（尺寸随词卡内容）

use eframe::egui;
use egui::viewport::{ViewportBuilder, WindowLevel};

use crate::config::Config;

/// 主窗口（控制面板）—— 可缩放，触摸屏也能拉；最小尺寸保证四板块都排得下
pub fn main_options(cfg: &Config) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Lexideck 词卡看板")
            .with_inner_size([cfg.window_w, cfg.window_h])
            .with_min_inner_size([720.0, 460.0]) // 比这个再小说明列就挤没了
            .with_resizable(true)
            .with_decorations(false), // 自绘顶栏（深色+金黄设计的一部分）
        ..Default::default()
    }
}

/// 悬浮窗 viewport（尺寸由词卡内容决定，创建后程序会自动微调）
pub fn float_viewport(
    id: usize,
    pos: Option<egui::Pos2>,
    always_on_top: bool,
    size: egui::Vec2,
) -> ViewportBuilder {
    let mut b = ViewportBuilder::default()
        .with_title(format!("词卡 #{id}"))
        .with_inner_size([size.x, size.y])
        .with_min_inner_size([200.0, 80.0])
        .with_decorations(false) // 无边框
        .with_transparent(true) // 背景透明（卡片自己画）
        .with_window_level(if always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        })
        .with_active(false) // 出现时不抢焦点（PPT 要保持键盘焦点）
        .with_resizable(false) // 尺寸随内容，不允许手动拉伸
        .with_taskbar(false); // 不占任务栏
    if let Some(p) = pos {
        b = b.with_position(p);
    }
    b
}
