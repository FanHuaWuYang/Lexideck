//! 控制面板（深色 + 金黄，参考 LauncherX 风格重做）：
//!   批量管理悬浮窗（数量 / 换一批 / 全部操作）+ 词表导入 + 全局设置。
//! 面板只产生"意图"（ControlResult），由 app 执行实际动作。
//!
//! 布局：顶栏（自绘，含拖拽与窗口按钮）→ 标签行 → 内容（左卡片列 + 右说明列）→ 底栏（状态 + 主按钮）。
//! 所有尺寸以 1004×640 设计（见 window::main_options）。

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, RichText, Stroke, StrokeKind};

use crate::config::Config;
use crate::theme::ThemeKind;
use crate::words::Warning;

// ── 面板皮肤（与卡片主题联动：纯白 / 浅青 / 黄黑；切换即整体换装） ──
// 所有绘制统一从"当前皮肤"取色，见 sk()。

#[derive(Clone, Copy)]
pub struct Skin {
    pub bg: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub line: Color32,
    pub line2: Color32,
    pub txt: Color32,
    pub txt2: Color32,
    pub txt3: Color32,
    pub acc: Color32,
    pub acc_dark: Color32,
    pub danger: Color32,
    pub btn_bg: Color32,
    pub btn_hover: Color32,
    pub btn_line: Color32,
    pub row_bg: Color32,
    pub row_line: Color32,
    /// 图标/控件描线的主色
    pub icon: Color32,
    /// 是否深色底（影响 egui 默认控件与图标配色）
    pub is_dark: bool,
}

const YELLOW: Skin = Skin {
    bg: Color32::from_rgb(0x0B, 0x0B, 0x0D),
    card: Color32::from_rgb(0x1B, 0x1C, 0x1F),
    card_hover: Color32::from_rgb(0x21, 0x22, 0x27),
    line: Color32::from_rgb(0x26, 0x28, 0x2D),
    line2: Color32::from_rgb(0x3A, 0x3E, 0x45),
    txt: Color32::from_rgb(0xED, 0xEE, 0xF0),
    txt2: Color32::from_rgb(0x9B, 0xA0, 0xA8),
    txt3: Color32::from_rgb(0x7D, 0x83, 0x8C),
    acc: Color32::from_rgb(0xF5, 0xD0, 0x1A),
    acc_dark: Color32::from_rgb(0x14, 0x14, 0x14),
    danger: Color32::from_rgb(0xFF, 0x7A, 0x7A),
    btn_bg: Color32::from_rgb(0x26, 0x2A, 0x2F),
    btn_hover: Color32::from_rgb(0x2E, 0x32, 0x3A),
    btn_line: Color32::from_rgb(0x33, 0x37, 0x3D),
    row_bg: Color32::from_rgb(0x14, 0x15, 0x18),
    row_line: Color32::from_rgb(0x24, 0x26, 0x2B),
    icon: Color32::from_rgb(0xE8, 0xEA, 0xED),
    is_dark: true,
};

const PLAIN: Skin = Skin {
    bg: Color32::from_rgb(0xF4, 0xF5, 0xF6),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    card_hover: Color32::from_rgb(0xF9, 0xFA, 0xFB),
    line: Color32::from_rgb(0xE6, 0xE8, 0xEB),
    line2: Color32::from_rgb(0xD2, 0xD6, 0xDB),
    txt: Color32::from_rgb(0x20, 0x24, 0x28),
    txt2: Color32::from_rgb(0x5C, 0x64, 0x6D),
    txt3: Color32::from_rgb(0x8B, 0x92, 0x9A),
    acc: Color32::from_rgb(0x66, 0x8D, 0x82),
    acc_dark: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    danger: Color32::from_rgb(0xC4, 0x52, 0x4E),
    btn_bg: Color32::from_rgb(0xF1, 0xF3, 0xF5),
    btn_hover: Color32::from_rgb(0xE6, 0xEA, 0xEE),
    btn_line: Color32::from_rgb(0xD8, 0xDC, 0xE1),
    row_bg: Color32::from_rgb(0xF3, 0xF4, 0xF6),
    row_line: Color32::from_rgb(0xE5, 0xE7, 0xEA),
    icon: Color32::from_rgb(0x2A, 0x30, 0x36),
    is_dark: false,
};

const WULING: Skin = Skin {
    bg: Color32::from_rgb(0xE9, 0xF2, 0xF0),
    card: Color32::from_rgb(0xF8, 0xFC, 0xFB),
    card_hover: Color32::from_rgb(0xEF, 0xF7, 0xF5),
    line: Color32::from_rgb(0xD3, 0xE3, 0xE0),
    line2: Color32::from_rgb(0xB7, 0xD2, 0xCC),
    txt: Color32::from_rgb(0x1E, 0x3B, 0x36),
    txt2: Color32::from_rgb(0x4E, 0x6D, 0x67),
    txt3: Color32::from_rgb(0x7B, 0x94, 0x8E),
    acc: Color32::from_rgb(0x66, 0x8D, 0x82),
    acc_dark: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    danger: Color32::from_rgb(0xC0, 0x57, 0x4F),
    btn_bg: Color32::from_rgb(0xEC, 0xF5, 0xF3),
    btn_hover: Color32::from_rgb(0xDF, 0xEE, 0xEA),
    btn_line: Color32::from_rgb(0xC8, 0xDF, 0xDA),
    row_bg: Color32::from_rgb(0xEC, 0xF4, 0xF2),
    row_line: Color32::from_rgb(0xD9, 0xE8, 0xE4),
    icon: Color32::from_rgb(0x24, 0x44, 0x3E),
    is_dark: false,
};

pub fn skin_for(kind: ThemeKind) -> Skin {
    match kind {
        ThemeKind::Wuling => WULING,
        ThemeKind::Yellow => YELLOW,
        _ => PLAIN,
    }
}

thread_local! {
    static SKIN: std::cell::Cell<Skin> = std::cell::Cell::new(PLAIN);
}

/// 当前皮肤（draw() 开头设置一次）
fn sk() -> Skin {
    SKIN.with(|s| s.get())
}

fn set_skin(kind: ThemeKind) {
    SKIN.with(|s| s.set(skin_for(kind)));
}

const TOP_H: f32 = 48.0;
const TABS_H: f32 = 40.0;
const BOTTOM_H: f32 = 54.0;
const PAD: f32 = 24.0;
const LEFT_W: f32 = 592.0;
const GAP: f32 = 32.0;

pub const MAX_FLOATS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PanelTab {
    Floats,
    Words,
    Look,
    About,
}

impl Default for PanelTab {
    fn default() -> Self {
        PanelTab::Floats
    }
}

#[derive(Default)]
pub struct ControlResult {
    pub changed: bool,     // cfg 有改动 → 存盘
    pub top_changed: bool, // 置顶开关变了
    pub reload: bool,
    pub import: bool,
    pub count_set: Option<usize>, // 目标窗数（0 = 全部关闭）
    pub batch_prev: bool,
    pub batch_next: bool,
    pub replay_anim: bool,
    pub exit: bool,
    pub confirm_arm: bool, // 第一次点退出：进入"再点一次"状态
    pub minimize: bool,
}

/// 悬浮窗列表里每行的信息（纯展示）
pub struct FloatRow {
    pub id: usize,
    pub label: String,
}

/// 绘制需要的只读上下文
pub struct Ctx<'a> {
    pub file_label: &'a str,
    pub total_entries: usize, // 词表总条数
    pub batch: usize,         // 当前批次起点（筛选后列表中的序号）
    pub batch_total: usize,   // 筛选后列表长度
    pub count: usize,         // 当前实际窗数
    pub warns: &'a [Warning],
    pub import_status: Option<&'a str>,
    pub confirm_exit: bool,
}

pub fn draw(
    ui: &mut egui::Ui,
    cfg: &mut Config,
    tab: &mut PanelTab,
    cx: &Ctx,
    rows: &[FloatRow],
) -> ControlResult {
    set_skin(ThemeKind::parse(&cfg.theme));
    let mut r = ControlResult::default();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, sk().bg);

    // ── 顶栏 ──
    let top = egui::Rect::from_min_max(full.min, egui::pos2(full.right(), full.top() + TOP_H));
    // 拖拽区（避开右侧窗口按钮）
    let drag_rect = egui::Rect::from_min_max(top.min, egui::pos2(top.right() - 96.0, top.bottom()));
    let drag = ui.interact(drag_rect, ui.id().with("titlebar"), egui::Sense::drag());
    if drag.dragged() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(top.shrink2(egui::vec2(22.0, 0.0))),
        |ui| {
            ui.horizontal_centered(|ui| {
                let (lr, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
                ui.painter().rect_filled(lr, CornerRadius::ZERO, sk().acc);
                ui.painter().text(
                    lr.center(),
                    Align2::CENTER_CENTER,
                    "L",
                    FontId::proportional(13.0),
                    sk().acc_dark,
                );
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Lexideck")
                        .size(15.0)
                        .strong()
                        .color(sk().txt),
                );
                ui.add_space(4.0);
                ui.label(RichText::new("词卡看板").size(11.0).color(sk().txt3));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if win_btn(ui, "close") {
                        r.exit = true; // 由外层用 confirm 逻辑决定（见下）
                    }
                    if win_btn(ui, "minimize") {
                        r.minimize = true;
                    }
                });
            });
        },
    );
    // [×] 与「退出程序」共享确认逻辑
    if r.exit && !cx.confirm_exit {
        r.exit = false;
        r.confirm_arm = true;
    }

    // ── 标签行 ──
    let tabs = egui::Rect::from_min_max(
        egui::pos2(full.left(), top.bottom()),
        egui::pos2(full.right(), top.bottom() + TABS_H),
    );
    ui.painter().rect_filled(
        egui::Rect::from_min_max(egui::pos2(tabs.left(), tabs.bottom() - 1.0), tabs.max),
        0.0,
        sk().line,
    );
    let mut tab_rects: Vec<(PanelTab, egui::Rect)> = Vec::new();
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(tabs.shrink2(egui::vec2(22.0, 0.0))),
        |ui| {
            ui.horizontal_centered(|ui| {
                for (t, label) in [
                    (PanelTab::Floats, "悬浮窗"),
                    (PanelTab::Words, "词表"),
                    (PanelTab::Look, "外观"),
                    (PanelTab::About, "关于"),
                ] {
                    let (clicked, rect) = tab_item(ui, label, *tab == t);
                    if clicked {
                        *tab = t;
                    }
                    tab_rects.push((t, rect));
                    ui.add_space(30.0);
                }
            });
        },
    );
    // 激活下划线：跟手滑动到当前标签
    if let Some((_, tr)) = tab_rects.iter().find(|(t, _)| *t == *tab) {
        let ctx = ui.ctx();
        let x = ctx.animate_value_with_time(egui::Id::new("tab-ul-x"), tr.left(), 0.16);
        let w = ctx.animate_value_with_time(egui::Id::new("tab-ul-w"), tr.width(), 0.16);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, tr.bottom() - 2.0), egui::vec2(w, 2.0)),
            0.0,
            sk().acc,
        );
    }

    // ── 内容区（左卡片列 + 右说明列） ──
    let content = egui::Rect::from_min_max(
        egui::pos2(full.left() + PAD, tabs.bottom() + 18.0),
        egui::pos2(full.right() - PAD, full.bottom() - BOTTOM_H - 14.0),
    );
    let left = egui::Rect::from_min_max(
        content.min,
        egui::pos2(content.left() + LEFT_W, content.bottom()),
    );
    let right =
        egui::Rect::from_min_max(egui::pos2(left.right() + GAP, content.top()), content.max);

    // 标签切换：内容淡入 + 轻微上移
    let fade = {
        let id = egui::Id::new("panel-tab-switch");
        let now = ui.input(|i| i.time);
        match ui.ctx().data_mut(|d| d.get_temp::<(PanelTab, f64)>(id)) {
            Some((t, t0)) if t == *tab => (((now - t0) / 0.18) as f32).clamp(0.0, 1.0),
            _ => {
                ui.ctx().data_mut(|d| d.insert_temp(id, (*tab, now)));
                0.0
            }
        }
    };
    let fade = 1.0 - (1.0 - fade).powi(3);
    if fade < 1.0 {
        ui.ctx().request_repaint();
    }
    let rise = (1.0 - fade) * 8.0;
    let left = left.translate(egui::vec2(0.0, rise));
    let right = right.translate(egui::vec2(0.0, rise));

    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
        ui.multiply_opacity(fade);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match tab {
                PanelTab::Floats => floats_tab(ui, cfg, cx, rows, &mut r),
                PanelTab::Words => words_tab(ui, cfg, cx, &mut r),
                PanelTab::Look => look_tab(ui, cfg, cx, &mut r),
                PanelTab::About => about_tab(ui, cx, &mut r),
            });
    });
    ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
        ui.multiply_opacity(fade);
        help_pane(ui, *tab, cx);
    });

    // ── 底栏 ──
    let bottom =
        egui::Rect::from_min_max(egui::pos2(full.left(), full.bottom() - BOTTOM_H), full.max);
    ui.painter().rect_filled(
        egui::Rect::from_min_max(bottom.min, egui::pos2(bottom.right(), bottom.top() + 1.0)),
        0.0,
        sk().line,
    );
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(bottom.shrink2(egui::vec2(PAD, 0.0))),
        |ui| {
            ui.horizontal_centered(|ui| {
                let (dr, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(dr.center(), 4.0, Color32::from_rgb(0x3E, 0xD6, 0x7A));
                ui.add_space(8.0);
                let status = if cx.confirm_exit {
                    "再点一次「×」退出程序".to_string()
                } else {
                    format!(
                        "运行中 · {} · {} 条 · 设置自动保存",
                        cx.file_label, cx.total_entries
                    )
                };
                let color = if cx.confirm_exit {
                    sk().danger
                } else {
                    sk().txt3
                };
                ui.label(RichText::new(status).size(12.0).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if gold_button(ui, Some("next"), "下一批") {
                        r.batch_next = true;
                    }
                });
            });
        },
    );

    r
}

// ══ 各标签页内容 ══

fn floats_tab(
    ui: &mut egui::Ui,
    _cfg: &mut Config,
    cx: &Ctx,
    rows: &[FloatRow],
    r: &mut ControlResult,
) {
    // 列表分组卡
    let gh = 78.0 + rows.len().max(1) as f32 * 42.0;
    card(ui, gh, false, |ui| {
        ui.vertical(|ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                icon_box(ui, "float_list");
                ui.add_space(14.0);
                title_sub(
                    ui,
                    &format!("悬浮窗 · {} 个运行中", cx.count),
                    "每个窗固定一条内容，整批统一管理",
                );
            });
            ui.add_space(4.0);
            if rows.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("（没有悬浮窗——调大下面的数量，或点右下「下一批」）")
                            .size(12.5)
                            .color(sk().txt3),
                    );
                });
            }
            for row in rows {
                float_row(ui, row.id, &row.label);
            }
        });
    });
    ui.add_space(14.0);

    // 数量
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "count");
            ui.add_space(14.0);
            title_sub(ui, "悬浮窗数量", "增减时自动开 / 关到对应数量");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn(ui, "＋", true) {
                    r.count_set = Some((cx.count + 1).min(MAX_FLOATS));
                }
                ui.add_space(6.0);
                num_box(ui, cx.count);
                ui.add_space(6.0);
                if sq_btn(ui, "−", cx.count > 0) {
                    r.count_set = Some(cx.count - 1);
                }
            });
        });
    });
    ui.add_space(14.0);

    // 换一批
    let can_prev = cx.batch > 0;
    let can_next = cx.batch + cx.count < cx.batch_total;
    let meta = if cx.batch_total == 0 {
        "词表为空".to_string()
    } else {
        let a = (cx.batch + 1).min(cx.batch_total);
        let b = (cx.batch + cx.count).min(cx.batch_total);
        format!(
            "全部窗一起切到下一组 · 当前第 {}–{} 个 / 共 {} 个",
            a, b, cx.batch_total
        )
    };
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "batch");
            ui.add_space(14.0);
            title_sub(ui, "换一批", &meta);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn_icon(ui, "next", "下一批", can_next) {
                    r.batch_next = true;
                }
                ui.add_space(6.0);
                if sq_btn_icon(ui, "prev", "上一批", can_prev) {
                    r.batch_prev = true;
                }
            });
        });
    });
    ui.add_space(14.0);

    // 全部操作
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "actions");
            ui.add_space(14.0);
            title_sub(ui, "全部操作", "对所有悬浮窗生效");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn_danger_icon(ui, "close_all", "全部关闭", cx.count > 0) {
                    r.count_set = Some(0);
                }
                ui.add_space(6.0);
                if sq_btn_icon(ui, "replay", "重播动画", cx.count > 0) {
                    r.replay_anim = true;
                }
            });
        });
    });
}

fn words_tab(ui: &mut egui::Ui, cfg: &mut Config, cx: &Ctx, r: &mut ControlResult) {
    // 文件
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "deck");
            ui.add_space(14.0);
            title_sub(
                ui,
                &format!("词表：{}", cx.file_label),
                &format!("{} 条内容 · 保存后 2 秒内自动刷新", cx.total_entries),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn_icon(ui, "refresh", "重新读取", true) {
                    r.reload = true;
                }
            });
        });
    });
    ui.add_space(14.0);

    // 导入
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "import");
            ui.add_space(14.0);
            title_sub(ui, "导入词表", "选择一份 词表.json：同名词条更新、其余追加");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn(ui, "导入…", true) {
                    r.import = true;
                }
            });
        });
    });
    if let Some(s) = cx.import_status {
        ui.add_space(6.0);
        let color = if s.starts_with("已导入") {
            sk().acc
        } else {
            sk().danger
        };
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(s).size(12.5).color(color));
        });
    }
    ui.add_space(14.0);

    // 显示内容
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "display");
            ui.add_space(14.0);
            title_sub(ui, "显示内容", "悬浮窗与列表只显示所选类别");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (key, label) in [
                    ("sentence", "句子"),
                    ("phrase", "短语"),
                    ("word", "单词"),
                    ("all", "全部"),
                ] {
                    let on = cfg.filter == key;
                    if seg_btn(ui, label, on) && !on {
                        cfg.filter = key.to_string();
                        r.changed = true;
                    }
                    ui.add_space(6.0);
                }
            });
        });
    });
    ui.add_space(14.0);

    // 字号
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "font");
            ui.add_space(14.0);
            title_sub(ui, "文字大小", "影响所有悬浮窗卡片");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("字号 {:.0}%", cfg.font_scale * 100.0))
                        .size(12.5)
                        .color(sk().txt2),
                );
                ui.add_space(10.0);
                if sq_btn(ui, "A＋", true) {
                    cfg.font_scale = (cfg.font_scale + 0.05).min(2.5);
                    r.changed = true;
                }
                ui.add_space(6.0);
                if sq_btn(ui, "A−", true) {
                    cfg.font_scale = (cfg.font_scale - 0.05).max(0.5);
                    r.changed = true;
                }
            });
        });
    });

    // 解析提示（有的话）
    if !cx.warns.is_empty() {
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(4.0);
            let msg = if cx.warns.len() == 1 {
                cx.warns[0].msg.clone()
            } else {
                format!("词表有 {} 处小提示：{} …", cx.warns.len(), cx.warns[0].msg)
            };
            ui.label(RichText::new(msg).size(12.0).color(sk().txt3));
        });
    }
}

fn look_tab(ui: &mut egui::Ui, cfg: &mut Config, _cx: &Ctx, r: &mut ControlResult) {
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "theme");
            ui.add_space(14.0);
            title_sub(ui, "主题", "控制面板与悬浮窗卡片的配色");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for (kind, dot) in [
                    (ThemeKind::Yellow, Color32::from_rgb(0xFF, 0xFA, 0x00)),
                    (ThemeKind::Wuling, Color32::from_rgb(0xC5, 0xE0, 0xD9)),
                    (ThemeKind::Plain, Color32::from_rgb(0xFF, 0xFF, 0xFF)),
                ] {
                    let on = cfg.theme == kind.as_str();
                    if swatch_btn(ui, kind.label(), dot, on) && !on {
                        cfg.theme = kind.as_str().to_string();
                        r.changed = true;
                    }
                    ui.add_space(6.0);
                }
            });
        });
    });
    ui.add_space(14.0);

    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "pin");
            ui.add_space(14.0);
            title_sub(ui, "悬浮窗置顶", "悬浮窗始终盖在其他程序（PPT 等）上方");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let on = cfg.always_on_top;
                if seg_btn(ui, "关", !on) && on {
                    cfg.always_on_top = false;
                    r.changed = true;
                    r.top_changed = true;
                }
                ui.add_space(6.0);
                if seg_btn(ui, "开", on) && !on {
                    cfg.always_on_top = true;
                    r.changed = true;
                    r.top_changed = true;
                }
            });
        });
    });
}

fn about_tab(ui: &mut egui::Ui, cx: &Ctx, r: &mut ControlResult) {
    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "about");
            ui.add_space(14.0);
            title_sub(
                ui,
                "Lexideck 词卡看板",
                "v0.2 · 绿色单文件版（lexideck.exe）",
            );
        });
    });
    ui.add_space(14.0);

    card(ui, 68.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "exit");
            ui.add_space(14.0);
            title_sub(ui, "退出程序", "关闭控制面板与全部悬浮窗");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn_danger_icon(ui, "exit", "退出程序", true) {
                    if cx.confirm_exit {
                        r.exit = true;
                    } else {
                        r.confirm_arm = true;
                    }
                }
            });
        });
    });
}

// ══ 右栏说明 ══

fn help_pane(ui: &mut egui::Ui, tab: PanelTab, cx: &Ctx) {
    let h4 = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).size(13.5).strong().color(sk().txt));
        ui.add_space(8.0);
    };
    let p = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).size(13.0).color(sk().txt2));
        ui.add_space(10.0);
    };
    let gold = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).size(12.5).color(sk().acc));
        ui.add_space(8.0);
    };
    let mini = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).size(12.0).color(sk().txt3));
        ui.add_space(8.0);
    };
    let sep = |ui: &mut egui::Ui| {
        let (rr, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().rect_filled(rr, 0.0, sk().line);
        ui.add_space(12.0);
    };

    match tab {
        PanelTab::Floats => {
            h4(ui, "悬浮窗");
            p(
                ui,
                "每个悬浮窗固定显示一条内容；这里做整批管理——调窗数、换一批、重播动画。",
            );
            gold(ui, "拖动悬浮窗本身可移动位置，位置自动保存。");
            sep(ui);
            mini(
                ui,
                "窗数增减时自动开 / 关末尾的窗；新窗默认从屏幕右上角起、往下平铺。",
            );
            mini(
                ui,
                "「换一批」= 全部窗一起切到下一组；动画会从左到右错峰播放。",
            );
            if cx.count > 0 {
                mini(ui, "当前入口：右下角「▶ 下一批」可以直接推进到下一组。");
            }
        }
        PanelTab::Words => {
            h4(ui, "词表与显示");
            p(ui, "把「词表.json」放到程序旁边就行；老师换一份文件、保存，屏幕 2 秒内自动刷新，不用重启。");
            gold(
                ui,
                "「导入…」= 选择一份新词表并合并进来（同名更新、其余追加）。",
            );
            sep(ui);
            mini(ui, "「显示内容」只影响呈现，不会修改词表文件本身。");
            mini(
                ui,
                "字号以 100% 为基准；换分辨率时整体会按屏幕比例自动缩放。",
            );
        }
        PanelTab::Look => {
            h4(ui, "外观");
            p(
                ui,
                "控制面板与悬浮窗卡片共用同一套主题：切换即整体换装，不用重启。",
            );
            sep(ui);
            mini(
                ui,
                "黄黑 = 黑底荧光黄，对比度最高，适合亮堂的教室；纯白 / 浅青更素净。",
            );
        }
        PanelTab::About => {
            h4(ui, "关于");
            p(ui, "程序不写注册表、不写 AppData——设置与词表都在 exe 同目录，整个文件夹拷走就带走全部配置。");
            sep(ui);
            mini(
                ui,
                "设置自动保存到 exe 旁的「设置.txt」；词表文件保存后自动刷新。",
            );
            mini(ui, "透明 / 置顶 / 触摸等表现需要在教室一体机上最终确认。");
        }
    }
}

// ══ 小部件 ══

/// 卡片容器：整宽 + 固定高度，圆角深色底 + 细边；hover 变亮。
fn card<R>(
    ui: &mut egui::Ui,
    height: f32,
    hoverable: bool,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Response, R) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(w, height),
        if hoverable {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let hov = if hoverable {
        ui.ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), 0.12)
    } else {
        0.0
    };
    let p = ui.painter();
    p.rect_filled(
        rect,
        CornerRadius::ZERO,
        mix(sk().card, sk().card_hover, hov),
    );
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, mix(sk().line, sk().line2, hov)),
        StrokeKind::Inside,
    );
    let inner = rect.shrink2(egui::vec2(16.0, 0.0));
    let r = ui
        .scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
            ui.set_height(height);
            f(ui)
        })
        .inner;
    (resp, r)
}

/// 颜色插值（悬停过渡用）
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// 内嵌图标（白色 + Alpha，用 tint 上色；资源在 assets/icons/）
fn icon_widget(name: &str, color: Color32, size: egui::Vec2) -> Option<egui::Image<'static>> {
    let src = match name {
        "float_list" => egui::include_image!("../assets/icons/float_list.png"),
        "count" => egui::include_image!("../assets/icons/count.png"),
        "batch" => egui::include_image!("../assets/icons/batch.png"),
        "actions" => egui::include_image!("../assets/icons/actions.png"),
        "deck" => egui::include_image!("../assets/icons/deck.png"),
        "import" => egui::include_image!("../assets/icons/import.png"),
        "display" => egui::include_image!("../assets/icons/display.png"),
        "font" => egui::include_image!("../assets/icons/font.png"),
        "theme" => egui::include_image!("../assets/icons/theme.png"),
        "pin" => egui::include_image!("../assets/icons/pin.png"),
        "about" => egui::include_image!("../assets/icons/about.png"),
        "exit" => egui::include_image!("../assets/icons/exit.png"),
        "refresh" => egui::include_image!("../assets/icons/refresh.png"),
        "replay" => egui::include_image!("../assets/icons/replay.png"),
        "close_all" => egui::include_image!("../assets/icons/close_all.png"),
        "next" => egui::include_image!("../assets/icons/next.png"),
        "prev" => egui::include_image!("../assets/icons/prev.png"),
        "minimize" => egui::include_image!("../assets/icons/minimize.png"),
        "close" => egui::include_image!("../assets/icons/close.png"),
        _ => return None,
    };
    Some(egui::Image::new(src).tint(color).fit_to_exact_size(size))
}

fn icon_paint(ui: &mut egui::Ui, rect: egui::Rect, name: &str, color: Color32) {
    if let Some(img) = icon_widget(name, color, rect.size()) {
        ui.put(rect, img);
    }
}

fn icon_box(ui: &mut egui::Ui, icon: &str) {
    let s = sk();
    let (r, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, s.btn_bg);
    let ic = egui::Rect::from_center_size(r.center(), egui::vec2(20.0, 20.0));
    icon_paint(ui, ic, icon, s.icon);
}

fn title_sub(ui: &mut egui::Ui, title: &str, sub: &str) {
    ui.vertical(|ui| {
        ui.label(RichText::new(title).size(14.5).strong().color(sk().txt));
        ui.add_space(3.0);
        ui.label(RichText::new(sub).size(12.5).color(sk().txt2));
    });
}

fn float_row(ui: &mut egui::Ui, id: usize, label: &str) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, sk().row_bg);
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, sk().row_line),
        StrokeKind::Inside,
    );
    p.text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        Align2::LEFT_CENTER,
        format!("#{}", id),
        FontId::proportional(12.0),
        sk().txt3,
    );
    p.text(
        egui::pos2(rect.left() + 48.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.5),
        sk().txt,
    );
    ui.add_space(8.0);
}

fn win_btn(ui: &mut egui::Ui, icon: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(34.0, 30.0), egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.10);
    ui.painter().rect_filled(
        r,
        CornerRadius::ZERO,
        mix(Color32::TRANSPARENT, sk().card_hover, hov),
    );
    let color = mix(sk().txt2, sk().txt, hov);
    icon_paint(
        ui,
        egui::Rect::from_center_size(r.center(), egui::vec2(15.0, 15.0)),
        icon,
        color,
    );
    resp.clicked()
}

fn tab_item(ui: &mut egui::Ui, label: &str, active: bool) -> (bool, egui::Rect) {
    let font = FontId::proportional(13.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, sk().txt);
    let w = galley.size().x + 8.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, TABS_H), egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.10);
    let color = if active {
        sk().txt
    } else {
        mix(sk().txt2, sk().txt, hov)
    };
    let pos = egui::pos2(rect.left() + 4.0, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(pos, galley, color);
    (resp.clicked(), rect)
}

/// 方按钮（小按钮）。`enabled=false` 时变暗且不可点。
fn sq_btn(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    sq_btn_impl(ui, None, label, enabled, false)
}

fn sq_btn_icon(ui: &mut egui::Ui, icon: &str, label: &str, enabled: bool) -> bool {
    sq_btn_impl(ui, Some(icon), label, enabled, false)
}

fn sq_btn_danger(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    sq_btn_impl(ui, None, label, enabled, true)
}

fn sq_btn_danger_icon(ui: &mut egui::Ui, icon: &str, label: &str, enabled: bool) -> bool {
    sq_btn_impl(ui, Some(icon), label, enabled, true)
}

fn sq_btn_impl(
    ui: &mut egui::Ui,
    icon: Option<&str>,
    label: &str,
    enabled: bool,
    danger: bool,
) -> bool {
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), sk().txt);
    let icon_w = if icon.is_some() { 22.0 } else { 0.0 };
    let w = (galley.size().x + icon_w + 20.0).max(30.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 30.0), egui::Sense::click());
    let hov = if enabled {
        ui.ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), 0.10)
    } else {
        0.0
    };
    let base_txt = if sk().is_dark {
        Color32::from_rgb(0xDF, 0xE2, 0xE6)
    } else {
        sk().txt
    };
    let (bg, border, txt) = if !enabled {
        (sk().card, sk().line, sk().txt3)
    } else if danger {
        let hbg = if sk().is_dark {
            Color32::from_rgb(0x32, 0x24, 0x26)
        } else {
            Color32::from_rgb(0xFA, 0xEC, 0xEB)
        };
        let hbd = if sk().is_dark {
            Color32::from_rgb(0x5A, 0x3A, 0x3A)
        } else {
            Color32::from_rgb(0xE2, 0xB4, 0xB0)
        };
        (
            mix(sk().btn_bg, hbg, hov),
            mix(sk().btn_line, hbd, hov),
            mix(base_txt, sk().danger, hov),
        )
    } else {
        (
            mix(sk().btn_bg, sk().btn_hover, hov),
            mix(sk().btn_line, sk().line2, hov),
            mix(base_txt, sk().txt, hov),
        )
    };
    {
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::ZERO, bg);
        p.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
    }
    // 内容（图标 + 文本）整体居中
    let total = icon_w + galley.size().x;
    let mut x = rect.center().x - total / 2.0;
    if let Some(ic) = icon {
        let ir = egui::Rect::from_center_size(
            egui::pos2(x + 8.0, rect.center().y),
            egui::vec2(15.0, 15.0),
        );
        icon_paint(ui, ir, ic, txt);
        x += 22.0;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        txt,
    );
    enabled && resp.clicked()
}

fn num_box(ui: &mut egui::Ui, n: usize) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(44.0, 30.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, sk().row_bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, sk().row_line),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{}", n),
        FontId::proportional(13.0),
        sk().txt,
    );
}

/// 分段按钮（选中 = 强调色底）
fn seg_btn(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), sk().txt);
    let w = galley.size().x + 26.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 32.0), egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.10);
    let bg = if on {
        sk().acc
    } else {
        mix(sk().btn_bg, sk().btn_hover, hov)
    };
    let border = if on {
        sk().acc
    } else {
        mix(sk().btn_line, sk().line2, hov)
    };
    let base = if sk().is_dark {
        Color32::from_rgb(0xD8, 0xDC, 0xE1)
    } else {
        sk().txt
    };
    let txt = if on {
        sk().acc_dark
    } else {
        mix(base, sk().txt, hov)
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, bg);
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    p.text(rect.center(), Align2::CENTER_CENTER, label, font, txt);
    resp.clicked()
}

/// 色板按钮（左侧小色块 + 文字；选中描强调色边）
fn swatch_btn(ui: &mut egui::Ui, label: &str, dot: Color32, on: bool) -> bool {
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), sk().txt);
    let w = galley.size().x + 44.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 32.0), egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.10);
    let bg = mix(sk().btn_bg, sk().btn_hover, hov);
    let border = if on {
        sk().acc
    } else {
        mix(sk().btn_line, sk().line2, hov)
    };
    let base = if sk().is_dark {
        Color32::from_rgb(0xD8, 0xDC, 0xE1)
    } else {
        sk().txt
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, bg);
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let dot_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 16.0, rect.center().y),
        egui::vec2(12.0, 12.0),
    );
    p.rect_filled(dot_rect, CornerRadius::ZERO, dot);
    p.text(
        egui::pos2(rect.left() + 30.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font,
        if on {
            sk().txt
        } else {
            mix(base, sk().txt, hov)
        },
    );
    resp.clicked()
}

/// 主按钮（胶囊形；图标 + 文字）
fn gold_button(ui: &mut egui::Ui, icon: Option<&str>, label: &str) -> bool {
    let font = FontId::proportional(13.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), sk().acc_dark);
    let icon_w = if icon.is_some() { 24.0 } else { 0.0 };
    let w = galley.size().x + icon_w + 44.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 38.0), egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.10);
    let hover_col = if sk().is_dark {
        Color32::from_rgb(0xFF, 0xE2, 0x55)
    } else {
        Color32::from_rgb(0x58, 0x7C, 0x72)
    };
    let bg = mix(sk().acc, hover_col, hov);
    {
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::ZERO, bg);
    }
    let total = icon_w + galley.size().x;
    let mut x = rect.center().x - total / 2.0;
    if let Some(ic) = icon {
        let ir = egui::Rect::from_center_size(
            egui::pos2(x + 8.5, rect.center().y),
            egui::vec2(17.0, 17.0),
        );
        icon_paint(ui, ir, ic, sk().acc_dark);
        x += 24.0;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        sk().acc_dark,
    );
    resp.clicked()
}
