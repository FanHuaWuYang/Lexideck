//! 托盘右键菜单（自绘）。
//!
//! 为什么不用系统原生菜单（muda / TrackPopupMenu）：
//!   ① 它的模态循环会冻住 winit 主循环，点完条目后应用经常「睡着」不跑帧
//!      （要含假鼠标消息的心跳才叫得醒，见 tray.rs 文件头）；
//!   ② 心跳的唤醒消息又会反过来打断原生菜单自己的模态循环，
//!      约 1/2 概率把菜单卡成「点不动的幽灵」——两难，实测过。
//! 自绘菜单没有模态循环：它就是普通的一帧里声明的小窗口，行为完全可控，
//! 样式也能和面板保持一致（同一套 Theme）。
//!
//! 位置：贴在托盘图标左侧、底部对齐（Windows 托盘菜单的惯例）。
//! 交互：点条目 → 执行并关闭；ESC / 点别处（失焦）→ 关闭。

use eframe::egui;

use crate::theme::Theme;
use crate::tray::TrayAction;

/// 菜单宽度（点）
pub const W: f32 = 196.0;
/// 条目高度（点）
const ITEM_H: f32 = 34.0;
/// 外沿留白
const PAD: f32 = 6.0;
/// 条目左右留白
const PAD_X: f32 = 5.0;
/// 文字左缩进
const TEXT_IN: f32 = 12.0;
/// 每个分隔线占的高度（含上下空隙）
const SEP: f32 = 15.0;
/// 条目数（隐藏面板 / 显示全部卡片 / 隐藏全部卡片 / 退出）
const N_ITEMS: f32 = 4.0;
/// 分隔线数
const N_SEPS: f32 = 2.0;
/// 菜单总高：必须和下面的逐条目排布严格一致（自检脚本按固定偏移点条目）
pub const H: f32 = PAD * 2.0 + ITEM_H * N_ITEMS + SEP * N_SEPS;

/// 第 n 个条目（从 1 数）的纵向中心相对菜单顶部的高度。
/// 供自动化测试/脚本按固定偏移点击用；排版改动时这里要同步。
pub fn item_center_y(n: usize) -> f32 {
    match n {
        1 => PAD + ITEM_H * 0.5,
        2 => PAD + ITEM_H + SEP + ITEM_H * 0.5,
        3 => PAD + ITEM_H + SEP + ITEM_H + ITEM_H * 0.5,
        4 => PAD + ITEM_H + SEP + ITEM_H + ITEM_H + SEP + ITEM_H * 0.5,
        _ => H * 0.5,
    }
}

/// 画菜单，返回被点中的动作（None = 这一帧没点）。
pub fn draw(ui: &mut egui::Ui, th: &Theme, panel_visible: bool) -> Option<TrayAction> {
    let mut picked: Option<TrayAction> = None;
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 4.0, th.bg);
    ui.painter().rect_stroke(
        full,
        4.0,
        egui::Stroke::new(1.0, th.line),
        egui::StrokeKind::Inside,
    );

    let mut y = full.top() + PAD;
    item(
        ui,
        th,
        full,
        &mut y,
        if panel_visible {
            "隐藏面板"
        } else {
            "显示面板"
        },
        false,
        &mut picked,
        TrayAction::TogglePanel,
    );
    sep(ui, th, full, &mut y);
    item(
        ui,
        th,
        full,
        &mut y,
        "显示全部卡片",
        false,
        &mut picked,
        TrayAction::ShowCards,
    );
    item(
        ui,
        th,
        full,
        &mut y,
        "隐藏全部卡片",
        false,
        &mut picked,
        TrayAction::HideCards,
    );
    sep(ui, th, full, &mut y);
    item(
        ui,
        th,
        full,
        &mut y,
        "退出 Lexideck",
        true,
        &mut picked,
        TrayAction::Quit,
    );

    picked
}

/// 一个条目：悬停底色 + 左对齐文字；点了就记进 picked。
#[allow(clippy::too_many_arguments)]
fn item(
    ui: &mut egui::Ui,
    th: &Theme,
    full: egui::Rect,
    y: &mut f32,
    label: &str,
    danger: bool,
    picked: &mut Option<TrayAction>,
    action: TrayAction,
) {
    let r = egui::Rect::from_min_size(
        egui::pos2(full.left() + PAD_X, *y),
        egui::vec2(W - PAD_X * 2.0, ITEM_H),
    );
    let resp = ui.interact(r, ui.id().with(label), egui::Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(r, 3.0, th.hover_bg);
    }
    let color = if danger && resp.hovered() {
        th.warn
    } else {
        th.fg
    };
    ui.painter().text(
        egui::pos2(r.left() + TEXT_IN, r.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.5),
        color,
    );
    if resp.clicked() {
        *picked = Some(action);
    }
    *y += ITEM_H;
}

/// 分隔线：占 SEP 高度，线画在中间。
fn sep(ui: &mut egui::Ui, th: &Theme, full: egui::Rect, y: &mut f32) {
    let r = egui::Rect::from_min_size(
        egui::pos2(full.left() + PAD_X + 6.0, *y + SEP * 0.5),
        egui::vec2(W - PAD_X * 2.0 - 12.0, 1.0),
    );
    ui.painter().line_segment(
        [r.left_center(), r.right_center()],
        egui::Stroke::new(1.0, th.line),
    );
    *y += SEP;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 条目中心必须落在菜单高度内，且依次递增（脚本按固定偏移点击）
    #[test]
    fn item_centers_inside_and_ordered() {
        let c: Vec<f32> = (1..=4).map(item_center_y).collect();
        assert!(
            c.windows(2).all(|w| w[1] > w[0]),
            "条目中心应自上而下递增: {c:?}"
        );
        assert!(
            c.iter().all(|&y| y > 0.0 && y < H),
            "条目中心应在菜单内: {c:?}"
        );
    }

    /// 高度 = 排布求和（防改版式漏改 H）
    #[test]
    fn height_matches_layout() {
        let want = item_center_y(4) + ITEM_H * 0.5 + PAD; // 最后一个条目底 + 下留白
        assert!((H - want).abs() < 0.01, "H={H} 排布底={want}");
    }
}
