//! 悬浮窗绘制：白底直角卡片 + 词卡内容（card 模块的排版计划）+ 入场动画。
//! 悬浮窗是纯展示面：一个窗固定显示一个词，操作全部在控制面板。
//!
//! 动画覆盖层画在最上层（同一帧内后画的在上）：闪烁段 = 整窗半透明色块；
//! 滑出段 = 从左向右收窄的色块（easeOutExpo）；常态 = 右侧一根竖色条。

use eframe::egui::{self, RichText};
use egui::viewport::ViewportCommand;

use crate::anim::{self, Anim};
use crate::card::{self, CardPlan};
use crate::theme::Theme;

/// 在一个悬浮窗的根 Ui 上绘制一帧（egui 0.36 的 viewport 回调直接提供 Ui）
pub fn draw(ui: &mut egui::Ui, anim: &mut Anim, plan: Option<&CardPlan>, th: &Theme, scale: f32) {
    let ctx = ui.ctx().clone();
    let full = ui.max_rect();
    let painter = ui.painter().clone();

    // 拖拽（整窗可拖，没有其它可点元素）
    let drag = ui.interact(full, ui.id().with("drag"), egui::Sense::drag());
    if drag.dragged() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }

    // 动画状态（每帧读一次）
    let t = anim.tick();
    let in_flash = matches!(t, Some(tt) if tt < anim::FLASH_END);

    // 动画进行中：主动给自己的 viewport 要下一帧。
    // 不能只靠根窗口（控制面板）的重绘节奏——它的常态节流是 300ms，一旦它慢下来，
    // 浮窗的入场动画就会一卡一卡地停在中间态（看起来像画面坏了），
    // 必须等用户点一下面板触发重绘才接着走完。这里让浮窗自己驱动自己。
    if t.is_some() {
        ctx.request_repaint_of(ctx.viewport_id());
    }

    // ── 卡片本体（白底 + 内容） ──
    // 闪烁段完全不画：那一刻整窗只有色块，色块半透明时背后透出桌面。
    if !in_flash {
        painter.rect_filled(full, 0.0, th.bg);
        match plan {
            Some(p) => {
                card::paint(ui, p, th);
            }
            None => {
                ui.add_space(26.0 * scale);
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(24.0 * scale);
                    ui.label(
                        RichText::new("（没有可显示的内容）")
                            .size(15.0 * scale)
                            .color(th.fg3),
                    );
                });
            }
        }
    }

    // ── 入场动画覆盖（最上层） ──
    let bar_w = card::BAR_W * scale;
    match t {
        Some(t) if t < anim::FLASH_END => {
            let a = anim::flashing(t / anim::FLASH_END);
            let c = th.block.to_array();
            let col = egui::Color32::from_rgba_unmultiplied(
                c[0],
                c[1],
                c[2],
                (a * 255.0).clamp(0.0, 255.0) as u8,
            );
            painter.rect_filled(full, 0.0, col);
        }
        Some(t) => {
            let k = ((t - anim::FLASH_END) / anim::WIPE_DUR).clamp(0.0, 1.0);
            let e = anim::ease_out_expo(k);
            let bx = (full.left() + (full.width() - bar_w) * e).min(full.right());
            let r = egui::Rect::from_min_max(egui::pos2(bx, full.top()), full.max);
            painter.rect_filled(r, 0.0, th.block);
        }
        None => {
            let r =
                egui::Rect::from_min_max(egui::pos2(full.right() - bar_w, full.top()), full.max);
            painter.rect_filled(r, 0.0, th.block);
        }
    }
}
