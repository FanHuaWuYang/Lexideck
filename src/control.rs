//! 控制面板（v2：四个板块 —— 词表 / 卡片 / 策略 / 设置）。
//! 面板只产生"意图"（ControlResult），由 app 执行实际动作；这里不碰数据。
//!
//! 布局：顶栏（自绘，含拖拽与窗口按钮）→ 标签行 → 内容（主列 + 说明列）→ 底栏。
//! 面板可缩放：宽度不足时自动收起右侧说明列；所有控件尺寸按触摸放大（可点高度 ≥ 34）。

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, RichText, Stroke, StrokeKind};

use crate::config::Config;
use crate::theme::ThemeKind;

// ── 面板皮肤（与卡片主题联动：纯白 / 浅青 / 黄黑；切换即整体换装） ──

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
    pub row_sel: Color32,
    pub icon: Color32,
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
    txt3: Color32::from_rgb(0x85, 0x8B, 0x94),
    acc: Color32::from_rgb(0xF5, 0xD0, 0x1A),
    acc_dark: Color32::from_rgb(0x14, 0x14, 0x14),
    danger: Color32::from_rgb(0xFF, 0x7A, 0x7A),
    btn_bg: Color32::from_rgb(0x26, 0x2A, 0x2F),
    btn_hover: Color32::from_rgb(0x2E, 0x32, 0x3A),
    btn_line: Color32::from_rgb(0x33, 0x37, 0x3D),
    row_bg: Color32::from_rgb(0x14, 0x15, 0x18),
    row_line: Color32::from_rgb(0x24, 0x26, 0x2B),
    row_sel: Color32::from_rgb(0x25, 0x26, 0x1B),
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
    txt3: Color32::from_rgb(0x68, 0x6F, 0x77),
    acc: Color32::from_rgb(0x66, 0x8D, 0x82),
    acc_dark: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    danger: Color32::from_rgb(0xC4, 0x52, 0x4E),
    btn_bg: Color32::from_rgb(0xF1, 0xF3, 0xF5),
    btn_hover: Color32::from_rgb(0xE6, 0xEA, 0xEE),
    btn_line: Color32::from_rgb(0xD8, 0xDC, 0xE1),
    row_bg: Color32::from_rgb(0xF3, 0xF4, 0xF6),
    row_line: Color32::from_rgb(0xE5, 0xE7, 0xEA),
    row_sel: Color32::from_rgb(0xED, 0xF4, 0xF2),
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
    txt3: Color32::from_rgb(0x59, 0x72, 0x6C),
    acc: Color32::from_rgb(0x66, 0x8D, 0x82),
    acc_dark: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    danger: Color32::from_rgb(0xC0, 0x57, 0x4F),
    btn_bg: Color32::from_rgb(0xEC, 0xF5, 0xF3),
    btn_hover: Color32::from_rgb(0xDF, 0xEE, 0xEA),
    btn_line: Color32::from_rgb(0xC8, 0xDF, 0xDA),
    row_bg: Color32::from_rgb(0xEC, 0xF4, 0xF2),
    row_line: Color32::from_rgb(0xD9, 0xE8, 0xE4),
    row_sel: Color32::from_rgb(0xDF, 0xEE, 0xEA),
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
    static SKIN: std::cell::Cell<Skin> = const { std::cell::Cell::new(PLAIN) };
}

fn sk() -> Skin {
    SKIN.with(|s| s.get())
}

fn set_skin(kind: ThemeKind) {
    SKIN.with(|s| s.set(skin_for(kind)));
}

const TOP_H: f32 = 48.0;
const TABS_H: f32 = 44.0;
const BOTTOM_H: f32 = 58.0;
const PAD: f32 = 24.0;
const GAP: f32 = 32.0;
/// 窄于这个宽度就收起右侧说明列
const SIDE_MIN_W: f32 = 900.0;

/// 悬浮窗数量上限（实际上限由"屏幕放得下几张"决定，这里只是防御性封顶）
pub const MAX_FLOATS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTab {
    Words,
    Cards,
    Plans,
    Settings,
}

impl Default for PanelTab {
    fn default() -> Self {
        PanelTab::Words
    }
}

impl PanelTab {
    fn label(self) -> &'static str {
        match self {
            PanelTab::Words => "词表",
            PanelTab::Cards => "卡片",
            PanelTab::Plans => "策略",
            PanelTab::Settings => "设置",
        }
    }
}

/// 词表筛选
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WordFilter {
    All,
    Unselected,
    Selected,
}

/// 面板自己的瞬时状态（不写盘）
pub struct PanelState {
    pub tab: PanelTab,
    pub search: String,
    pub filter: WordFilter,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            tab: PanelTab::default(),
            search: String::new(),
            filter: WordFilter::All,
        }
    }
}

/// 冲突弹窗的四个选择
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    Skip,
    Replace,
    SkipAll,
    ReplaceAll,
}

#[derive(Default)]
pub struct ControlResult {
    pub changed: bool,     // cfg 有改动 → 存盘
    pub top_changed: bool, // 置顶开关变了
    pub reload: bool,      // 重新读取主库
    pub pick_import: bool, // 打开导入对话框
    pub show_selected: bool,
    pub close_all: bool,
    pub replay_anim: bool,
    pub select_all: bool,
    pub clear_selection: bool,
    pub toggle_select: Option<String>,
    pub toggle_expand: Option<String>,
    pub resolve: Option<ConflictChoice>,
    pub dismiss_dialog: bool,
    pub exit: bool,
    pub confirm_arm: bool,
    pub minimize: bool,
}

/// 词表列表里的一行（由 app 组装好，面板只画）
pub struct WordRow {
    pub key: String,
    pub word: String,
    pub meaning: String,
    /// 展开后显示的明细：(区块名, 内容, 是否被关闭展示)
    pub details: Vec<(String, String)>,
    /// 被关闭默认展示的区块名（行尾小标签）
    pub hidden: Vec<String>,
    pub selected: bool,
    pub expanded: bool,
}

/// 卡片列表里的一行
pub struct CardRow {
    pub id: usize,
    pub word: String,
    pub source: String,
}

/// 策略（展示时间）列表里的一行
pub struct PlanRow {
    pub word: String,
    pub ranges: String,
    pub hide: String,
}

/// 导入过程中的状态（冲突弹窗 / 结果）
pub struct ImportCtx<'a> {
    pub file: &'a str,
    /// 格式错误（有错就整份拒绝）
    pub errors: &'a [String],
    /// 冲突：当前第几个 / 共几个 + 两边摘要
    pub conflict: Option<(usize, usize, &'a str, &'a str, &'a str)>,
    /// 一次性结果说明
    pub done: Option<&'a str>,
}

pub struct Ctx<'a> {
    /// 主库文件名
    pub lib_label: &'a str,
    pub total_entries: usize,
    pub selected: usize,
    pub display_items: usize,
    pub card_rows: &'a [CardRow],
    pub plan_rows: &'a [PlanRow],
    /// 生效 / 已显示
    pub effective: usize,
    pub shown: usize,
    /// 解析与校验提示
    pub warns: &'a [String],
    pub import: &'a ImportCtx<'a>,
    pub confirm_exit: bool,
}

pub fn draw(
    ui: &mut egui::Ui,
    cfg: &mut Config,
    panel: &mut PanelState,
    cx: &Ctx,
    rows: &[WordRow],
) -> ControlResult {
    set_skin(ThemeKind::parse(&cfg.theme));
    let mut r = ControlResult::default();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, sk().bg);

    // ── 顶栏 ──
    let top = egui::Rect::from_min_max(full.min, egui::pos2(full.right(), full.top() + TOP_H));
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
                        r.exit = true;
                    }
                    if win_btn(ui, "minimize") {
                        r.minimize = true;
                    }
                });
            });
        },
    );
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
                for t in [
                    PanelTab::Words,
                    PanelTab::Cards,
                    PanelTab::Plans,
                    PanelTab::Settings,
                ] {
                    let (clicked, rect) = tab_item(ui, t.label(), panel.tab == t);
                    if clicked {
                        panel.tab = t;
                    }
                    tab_rects.push((t, rect));
                    ui.add_space(30.0);
                }
            });
        },
    );
    if let Some((_, tr)) = tab_rects.iter().find(|(t, _)| *t == panel.tab) {
        let ctx = ui.ctx();
        let x = ctx.animate_value_with_time(egui::Id::new("tab-ul-x"), tr.left(), 0.16);
        let w = ctx.animate_value_with_time(egui::Id::new("tab-ul-w"), tr.width(), 0.16);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, tr.bottom() - 2.0), egui::vec2(w, 2.0)),
            0.0,
            sk().acc,
        );
    }

    // ── 内容区 ──
    let content = egui::Rect::from_min_max(
        egui::pos2(full.left() + PAD, tabs.bottom() + 16.0),
        egui::pos2(full.right() - PAD, full.bottom() - BOTTOM_H - 12.0),
    );
    let wide = content.width() >= SIDE_MIN_W;
    let main_w = if wide {
        (content.width() - GAP) * 0.62
    } else {
        content.width()
    };
    let main = egui::Rect::from_min_max(
        content.min,
        egui::pos2(content.left() + main_w, content.bottom()),
    );
    let side = egui::Rect::from_min_max(egui::pos2(main.right() + GAP, content.top()), content.max);

    // 标签切换：内容淡入
    let fade = {
        let id = egui::Id::new("panel-tab-switch");
        let now = ui.input(|i| i.time);
        match ui.ctx().data_mut(|d| d.get_temp::<(PanelTab, f64)>(id)) {
            Some((t, t0)) if t == panel.tab => (((now - t0) / 0.18) as f32).clamp(0.0, 1.0),
            _ => {
                ui.ctx().data_mut(|d| d.insert_temp(id, (panel.tab, now)));
                0.0
            }
        }
    };
    let fade = 1.0 - (1.0 - fade).powi(3);
    if fade < 1.0 {
        ui.ctx().request_repaint();
    }
    let rise = (1.0 - fade) * 8.0;
    let main = main.translate(egui::vec2(0.0, rise));
    let side = side.translate(egui::vec2(0.0, rise));

    ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| {
        ui.multiply_opacity(fade);
        match panel.tab {
            PanelTab::Words => words_tab(ui, panel, cx, rows, &mut r),
            PanelTab::Cards => cards_tab(ui, cfg, cx, &mut r),
            PanelTab::Plans => plans_tab(ui, cx),
            PanelTab::Settings => settings_tab(ui, cfg, cx, &mut r),
        }
    });
    if wide {
        ui.scope_builder(egui::UiBuilder::new().max_rect(side), |ui| {
            ui.multiply_opacity(fade);
            help_pane(ui, panel.tab, cx);
        });
    }

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
                let dot = if cx.warns.is_empty() {
                    Color32::from_rgb(0x3E, 0xD6, 0x7A)
                } else {
                    sk().danger
                };
                ui.painter().circle_filled(dr.center(), 4.0, dot);
                ui.add_space(8.0);
                let status = if cx.confirm_exit {
                    "再点一次「×」退出程序（5 秒内有效）".to_string()
                } else {
                    format!("运行中 · {} · {} 条词条", cx.lib_label, cx.total_entries)
                };
                let color = if cx.confirm_exit {
                    sk().danger
                } else {
                    sk().txt3
                };
                ui.label(RichText::new(status).size(12.0).color(color));
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| match panel.tab {
                        PanelTab::Words => {
                            if gold_button(ui, None, &format!("立即展示（{}）", cx.selected))
                            {
                                r.show_selected = true;
                            }
                        }
                        PanelTab::Cards => {
                            if sq_btn_icon(ui, "close_all", "全部关闭", cx.shown > 0) {
                                r.close_all = true;
                            }
                        }
                        _ => {}
                    },
                );
            });
        },
    );

    // ── 弹窗（最上层，自己吞掉点击） ──
    dialogs(ui, cx, &mut r);

    r
}

// ══ 各标签页 ══

fn words_tab(
    ui: &mut egui::Ui,
    panel: &mut PanelState,
    cx: &Ctx,
    rows: &[WordRow],
    r: &mut ControlResult,
) {
    // 词库信息 + 导入
    card(ui, 74.0, false, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "deck");
            ui.add_space(14.0);
            title_sub_with_buttons(
                ui,
                &format!("词库：{}", cx.lib_label),
                &format!(
                    "{} 条词条 · {} 条展示时间 · 导入后自动存盘",
                    cx.total_entries, cx.display_items
                ),
                230.0,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn_icon(ui, "import", "导入词表…", true) {
                    r.pick_import = true;
                }
                ui.add_space(6.0);
                if sq_btn_icon(ui, "refresh", "重新读取", true) {
                    r.reload = true;
                }
            });
        });
    });
    ui.add_space(12.0);

    // 搜索 + 筛选
    ui.horizontal(|ui| {
        let w = (ui.available_width() - 250.0).max(160.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 36.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, sk().card);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, sk().btn_line),
            StrokeKind::Inside,
        );
        let inner = rect.shrink2(egui::vec2(10.0, 6.0));
        let te = egui::TextEdit::singleline(&mut panel.search)
            .hint_text("搜索单词或释义…")
            .desired_width(inner.width())
            .frame(egui::Frame::NONE)
            .font(FontId::proportional(13.0))
            .text_color(sk().txt);
        ui.put(inner, te);
        ui.add_space(10.0);
        for (f, label) in [
            (WordFilter::All, "全部"),
            (WordFilter::Unselected, "未选"),
            (WordFilter::Selected, "已选"),
        ] {
            let on = panel.filter == f;
            if seg_btn(ui, label, on) && !on {
                panel.filter = f;
            }
            ui.add_space(6.0);
        }
    });
    ui.add_space(10.0);

    // 列表
    let list_rect = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(list_rect, CornerRadius::ZERO, sk().card);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(list_rect.shrink(1.0))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    if rows.is_empty() {
        child.add_space(18.0);
        child.horizontal(|ui| {
            ui.add_space(14.0);
            let msg = if cx.total_entries == 0 {
                "词库还是空的 —— 点右上「导入词表…」选一份 JSON"
            } else {
                "没有符合条件的词条"
            };
            ui.label(RichText::new(msg).size(13.0).color(sk().txt3));
        });
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            // 行与行之间不留 item_spacing：否则那 3px 看着是行、点下去没反应
            ui.spacing_mut().item_spacing.y = 0.0;
            for row in rows {
                word_row(ui, row, r);
            }
        });
    ui.allocate_rect(list_rect, egui::Sense::hover());
}

fn cards_tab(ui: &mut egui::Ui, cfg: &mut Config, cx: &Ctx, r: &mut ControlResult) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| cards_body(ui, cfg, cx, r));
}

fn cards_body(ui: &mut egui::Ui, cfg: &mut Config, cx: &Ctx, r: &mut ControlResult) {
    card(ui, 74.0, false, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "float_list");
            ui.add_space(14.0);
            title_sub_with_buttons(
                ui,
                &format!("桌面卡片 · {} 张运行中", cx.shown),
                &format!(
                    "本次生效 {} 条 · 放不下时按顺序截断（已显示 {}）",
                    cx.effective, cx.shown
                ),
                300.0,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn_danger_icon(ui, "close_all", "全部关闭", cx.shown > 0) {
                    r.close_all = true;
                }
                ui.add_space(6.0);
                if sq_btn_icon(ui, "replay", "全部重播动画", cx.shown > 0) {
                    r.replay_anim = true;
                }
            });
        });
    });
    ui.add_space(12.0);

    let rows: Vec<&CardRow> = cx.card_rows.iter().collect();
    let h = (60.0 + rows.len().max(1) as f32 * 38.0).min(ui.available_height() - 90.0);
    card(ui, h.max(90.0), false, |ui| {
        ui.add_space(12.0);
        if rows.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(2.0);
                ui.label(
                    RichText::new("（桌面没有卡片 —— 去「词表」勾几个词，点右下角「立即展示」）")
                        .size(12.5)
                        .color(sk().txt3),
                );
            });
        }
        for row in rows {
            card_row(ui, row);
        }
    });
    ui.add_space(12.0);

    // 字号：全局一个档位
    card(ui, 74.0, false, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "font");
            ui.add_space(14.0);
            title_sub_with_buttons(
                ui,
                "文字大小",
                "全局一个档位，所有卡片一起变（换分辨率时还会按屏幕比例自动缩放）",
                170.0,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if sq_btn(ui, "A＋", true) {
                    cfg.font_scale = (cfg.font_scale + 0.05).min(2.5);
                    r.changed = true;
                }
                ui.add_space(6.0);
                num_box(ui, &format!("{:.0}%", cfg.font_scale * 100.0));
                ui.add_space(6.0);
                if sq_btn(ui, "A−", true) {
                    cfg.font_scale = (cfg.font_scale - 0.05).max(0.5);
                    r.changed = true;
                }
            });
        });
    });
}

fn plans_tab(ui: &mut egui::Ui, cx: &Ctx) {
    card(ui, 74.0, false, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "display");
            ui.add_space(14.0);
            title_sub(
                ui,
                &format!("展示时间 · {} 条", cx.display_items),
                "已随词表一并存入主库；自动换批在 P2 生效",
            );
        });
    });
    ui.add_space(12.0);

    let list_rect = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(list_rect, CornerRadius::ZERO, sk().card);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(list_rect.shrink(1.0))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    if cx.plan_rows.is_empty() {
        child.add_space(18.0);
        child.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new("主库里还没有展示时间 —— 导入一份带 display_time 的文件即可")
                    .size(13.0)
                    .color(sk().txt3),
            );
        });
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            for row in cx.plan_rows {
                plan_row(ui, row);
            }
        });
    ui.allocate_rect(list_rect, egui::Sense::hover());
}

fn settings_tab(ui: &mut egui::Ui, cfg: &mut Config, cx: &Ctx, r: &mut ControlResult) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| settings_body(ui, cfg, cx, r));
}

fn settings_body(ui: &mut egui::Ui, cfg: &mut Config, cx: &Ctx, r: &mut ControlResult) {
    card(ui, 74.0, false, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "theme");
            ui.add_space(14.0);
            title_sub_with_buttons(
                ui,
                "主题",
                "控制面板与卡片共用一套配色，切换即整体换装",
                250.0,
            );
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
    ui.add_space(12.0);

    card(ui, 74.0, false, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "pin");
            ui.add_space(14.0);
            title_sub_with_buttons(
                ui,
                "卡片层级",
                "置底：贴桌面显示，PPT 全屏时看不见卡片（默认）；置顶＋穿透在 P2 补上",
                165.0,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let on = cfg.always_on_top;
                if seg_btn(ui, "置顶", on) && !on {
                    cfg.always_on_top = true;
                    r.changed = true;
                    r.top_changed = true;
                }
                ui.add_space(6.0);
                if seg_btn(ui, "置底", !on) && on {
                    cfg.always_on_top = false;
                    r.changed = true;
                    r.top_changed = true;
                }
            });
        });
    });
    ui.add_space(12.0);

    card(ui, 74.0, false, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "about");
            ui.add_space(14.0);
            title_sub(
                ui,
                "Lexideck · 慕言",
                "v0.3 · 绿色单文件版（lexideck.exe）· 项目地址上传 GitHub 后补上",
            );
        });
    });
    ui.add_space(12.0);

    card(ui, 74.0, true, |ui| {
        ui.horizontal_centered(|ui| {
            icon_box(ui, "exit");
            ui.add_space(14.0);
            title_sub_with_buttons(ui, "退出程序", "关闭控制面板与全部桌面卡片", 140.0);
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
        ui.add_space(6.0);
    };
    let sep = |ui: &mut egui::Ui| {
        let (rr, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().rect_filled(rr, 0.0, sk().line);
        ui.add_space(12.0);
    };
    let stat = |ui: &mut egui::Ui, k: &str, v: &str| {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), egui::Sense::hover());
        let p = ui.painter();
        p.text(
            egui::pos2(rect.left() + 2.0, rect.center().y),
            Align2::LEFT_CENTER,
            k,
            FontId::proportional(12.5),
            sk().txt2,
        );
        p.text(
            egui::pos2(rect.right() - 2.0, rect.center().y),
            Align2::RIGHT_CENTER,
            v,
            FontId::proportional(12.5),
            sk().txt,
        );
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.bottom() - 1.0),
                egui::pos2(rect.right(), rect.bottom()),
            ),
            0.0,
            sk().line,
        );
    };

    match tab {
        PanelTab::Words => {
            h4(ui, "词库");
            p(
                ui,
                "词条只从 JSON 文件来。导入 = 并进主库：同名词条更新、其余追加。",
            );
            gold(ui, "勾几个词 → 右下角「立即展示」，卡片就出现在桌面上。");
            sep(ui);
            stat(ui, "词条总数", &cx.total_entries.to_string());
            stat(ui, "已选", &cx.selected.to_string());
            stat(
                ui,
                "展示时间条目",
                &format!("{}（P2 生效）", cx.display_items),
            );
            ui.add_space(12.0);
            mini(ui, "面板里不提供改词条内容的功能——改内容请改 JSON 再导入。");
            mini(ui, "格式不对的文件会被整份拒绝，主库不受影响。");
        }
        PanelTab::Cards => {
            h4(ui, "卡片");
            p(
                ui,
                "每个卡片一条内容，位置由平铺规则决定（右上角起、往下排、满列向左）。",
            );
            sep(ui);
            stat(ui, "本次生效", &cx.effective.to_string());
            stat(ui, "屏幕上显示", &cx.shown.to_string());
            ui.add_space(12.0);
            mini(ui, "放不下时按顺序截断：先显示的词先上屏。");
            mini(ui, "字号是全局档位，卡片本身不可拖拽（位置由规则定）。");
        }
        PanelTab::Plans => {
            h4(ui, "展示策略");
            p(
                ui,
                "一个词可以有多段日期；同一天多段生效时全部显示，没有日期覆盖时屏幕空白。",
            );
            sep(ui);
            stat(ui, "展示时间条目", &cx.display_items.to_string());
            ui.add_space(12.0);
            mini(ui, "P1 只做校验与存储，自动换批、策略开关在 P2 补齐。");
        }
        PanelTab::Settings => {
            h4(ui, "设置");
            p(
                ui,
                "面板与卡片共用主题；设置自动保存到 exe 旁的「设置.txt」。",
            );
            sep(ui);
            mini(
                ui,
                "程序不写注册表、不写 AppData —— 整个文件夹拷走就带走全部配置。",
            );
            mini(ui, "托盘驻留、开机自启动、单实例在 P3 加入。");
            if !cx.warns.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("提示").size(12.5).strong().color(sk().txt));
                ui.add_space(6.0);
                for w in cx.warns.iter().take(6) {
                    mini(ui, w);
                }
            }
        }
    }
}

// ══ 弹窗 ══

fn dialogs(ui: &mut egui::Ui, cx: &Ctx, r: &mut ControlResult) {
    let has_error = !cx.import.errors.is_empty();
    let has_conflict = cx.import.conflict.is_some();
    if !has_error && !has_conflict {
        return;
    }
    let full = ui.max_rect();
    // 遮罩：吞掉底下面板的点击
    egui::Area::new(egui::Id::new("lexideck-modal-block"))
        .order(egui::Order::Foreground)
        .fixed_pos(full.min)
        .show(ui.ctx(), |ui| {
            let (rect, resp) = ui.allocate_exact_size(full.size(), egui::Sense::click_and_drag());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(90));
            ui.allocate_rect(rect, egui::Sense::hover());
            let _ = resp;
        });

    let w = 560.0_f32.min(full.width() - 60.0);
    egui::Area::new(egui::Id::new("lexideck-modal"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(
            full.center().x - w / 2.0,
            (full.center().y - 150.0).max(full.top() + 40.0),
        ))
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(sk().card)
                .stroke(Stroke::new(1.0, sk().line2))
                .inner_margin(egui::Margin::same(22))
                .show(ui, |ui| {
                    ui.set_width(w - 44.0);
                    if has_error {
                        ui.label(
                            RichText::new(format!("导入失败 · {}", cx.import.file))
                                .size(15.0)
                                .strong()
                                .color(sk().txt),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new("这份文件没有被导入，主库保持原样：")
                                .size(13.0)
                                .color(sk().txt2),
                        );
                        ui.add_space(8.0);
                        for e in cx.import.errors.iter().take(10) {
                            ui.label(
                                RichText::new(format!("· {e}"))
                                    .size(12.5)
                                    .color(sk().danger),
                            );
                            ui.add_space(4.0);
                        }
                        if cx.import.errors.len() > 10 {
                            ui.label(
                                RichText::new(format!("…还有 {} 条", cx.import.errors.len() - 10))
                                    .size(12.0)
                                    .color(sk().txt3),
                            );
                        }
                        ui.add_space(16.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if gold_button(ui, None, "知道了") {
                                r.dismiss_dialog = true;
                            }
                        });
                    } else if let Some((i, n, word, old, new)) = cx.import.conflict {
                        ui.label(
                            RichText::new(format!("导入冲突 · {i} / {n}"))
                                .size(15.0)
                                .strong()
                                .color(sk().txt),
                        );
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!(
                                "「{word}」在主库里已经存在，内容和这份文件里的不一样："
                            ))
                            .size(13.0)
                            .color(sk().txt2),
                        );
                        ui.add_space(10.0);
                        cmp_line(ui, "主库里", old);
                        ui.add_space(6.0);
                        cmp_line(ui, "这份文件", new);
                        ui.add_space(18.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if gold_button(ui, None, "替换") {
                                r.resolve = Some(ConflictChoice::Replace);
                            }
                            ui.add_space(8.0);
                            if sq_btn(ui, "跳过", true) {
                                r.resolve = Some(ConflictChoice::Skip);
                            }
                            ui.add_space(16.0);
                            if sq_btn(ui, "全部替换", true) {
                                r.resolve = Some(ConflictChoice::ReplaceAll);
                            }
                            ui.add_space(6.0);
                            if sq_btn(ui, "全部跳过", true) {
                                r.resolve = Some(ConflictChoice::SkipAll);
                            }
                        });
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(format!(
                                "「全部」= 本次导入剩下的 {} 条不再逐条询问",
                                n - i
                            ))
                            .size(12.0)
                            .color(sk().txt3),
                        );
                    }
                });
        });
}

fn cmp_line(ui: &mut egui::Ui, who: &str, text: &str) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), egui::Sense::hover());
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
        who,
        FontId::proportional(12.5),
        sk().txt3,
    );
    p.text(
        egui::pos2(rect.left() + 84.0, rect.center().y),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(12.5),
        sk().txt,
    );
}

// ══ 列表行 ══

fn word_row(ui: &mut egui::Ui, row: &WordRow, r: &mut ControlResult) {
    let detail_lines = row.details.len();
    let h = if row.expanded {
        42.0 + detail_lines as f32 * 20.0 + 14.0
    } else {
        42.0
    };
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    if row.expanded && resp.clicked() {
        // 展开的行贴底时把它滚进视野，不然展开的内容看不到
        ui.scroll_to_rect(rect, None);
    }
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.10);
    let bg = if row.selected {
        sk().row_sel
    } else {
        mix(sk().card, sk().row_bg, hov)
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, bg);
    p.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - 1.0),
            egui::pos2(rect.right(), rect.bottom()),
        ),
        0.0,
        sk().row_line,
    );

    // 勾选框
    let box_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 12.0, rect.top() + 12.0),
        egui::vec2(18.0, 18.0),
    );
    p.rect_filled(
        box_rect,
        CornerRadius::ZERO,
        if row.selected { sk().acc } else { sk().card },
    );
    p.rect_stroke(
        box_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, if row.selected { sk().acc } else { sk().line2 }),
        StrokeKind::Inside,
    );
    if row.selected {
        check_mark(p, box_rect, sk().acc_dark);
    }

    let text_y = rect.top() + 21.0;
    let text_x = rect.left() + 42.0;
    // 右侧留出展开箭头的位置，剩下的宽度给"词 + 释义"
    let right_edge = rect.right() - 26.0;
    let half = ((right_edge - text_x) * 0.45).max(60.0);
    let g_word = ellipsize(p, &row.word, FontId::proportional(14.0), sk().txt, half);
    let word_w = g_word.size().x;
    p.galley(
        egui::pos2(text_x, text_y - g_word.size().y / 2.0),
        g_word,
        sk().txt,
    );
    let meaning_x = text_x + word_w + 16.0;
    let g_meaning = ellipsize(
        p,
        &row.meaning,
        FontId::proportional(13.0),
        sk().txt2,
        right_edge - meaning_x,
    );
    p.galley(
        egui::pos2(meaning_x, text_y - g_meaning.size().y / 2.0),
        g_meaning,
        sk().txt2,
    );
    tri(
        p,
        egui::pos2(rect.right() - 18.0, text_y),
        row.expanded,
        sk().txt3,
    );

    if row.expanded {
        let mut y = rect.top() + 46.0;
        for (label, value) in &row.details {
            p.text(
                egui::pos2(rect.left() + 42.0, y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(12.0),
                sk().txt3,
            );
            let vx = rect.left() + 106.0;
            let g_val = ellipsize(
                p,
                value,
                FontId::proportional(12.5),
                sk().txt2,
                rect.right() - 18.0 - vx,
            );
            p.galley(egui::pos2(vx, y - g_val.size().y / 2.0), g_val, sk().txt2);
            y += 20.0;
        }
        if !row.hidden.is_empty() {
            let mut x = rect.left() + 42.0;
            for key in &row.hidden {
                let label = format!("{} 已关闭展示", crate::deck::hide_label(key));
                let g = ui.painter().layout_no_wrap(
                    label.clone(),
                    FontId::proportional(11.5),
                    sk().txt3,
                );
                let w = g.size().x + 12.0;
                let tag = egui::Rect::from_min_size(egui::pos2(x, y - 9.0), egui::vec2(w, 18.0));
                p.rect_stroke(
                    tag,
                    CornerRadius::ZERO,
                    Stroke::new(1.0, sk().line2),
                    StrokeKind::Inside,
                );
                p.galley(egui::pos2(tag.left() + 6.0, y - 7.0), g, sk().txt3);
                x += w + 8.0;
            }
        }
    }

    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if pos.x <= rect.left() + 40.0 {
                r.toggle_select = Some(row.key.clone());
            } else {
                r.toggle_expand = Some(row.key.clone());
            }
        } else {
            r.toggle_expand = Some(row.key.clone());
        }
    }
}

fn card_row(ui: &mut egui::Ui, row: &CardRow) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
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
        format!("#{}", row.id),
        FontId::proportional(12.0),
        sk().txt3,
    );
    let cy = rect.center().y;
    let g_src = ellipsize(
        p,
        &row.source,
        FontId::proportional(12.0),
        sk().txt3,
        rect.width() * 0.45,
    );
    let src_w = g_src.size().x;
    p.galley(
        egui::pos2(rect.right() - 12.0 - src_w, cy - g_src.size().y / 2.0),
        g_src,
        sk().txt3,
    );
    let wx = rect.left() + 48.0;
    let g_word = ellipsize(
        p,
        &row.word,
        FontId::proportional(13.5),
        sk().txt,
        rect.right() - 24.0 - src_w - wx,
    );
    p.galley(egui::pos2(wx, cy - g_word.size().y / 2.0), g_word, sk().txt);
    ui.add_space(6.0);
}

fn plan_row(ui: &mut egui::Ui, row: &PlanRow) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, sk().row_bg);
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, sk().row_line),
        StrokeKind::Inside,
    );
    let cy = rect.center().y;
    let hide_w = if row.hide.is_empty() {
        0.0
    } else {
        let g = ellipsize(
            p,
            &row.hide,
            FontId::proportional(11.5),
            sk().txt3,
            rect.width() * 0.4,
        );
        let w = g.size().x;
        p.galley(
            egui::pos2(rect.right() - 12.0 - w, cy - g.size().y / 2.0),
            g,
            sk().txt3,
        );
        w + 14.0
    };
    let g_word = ellipsize(p, &row.word, FontId::proportional(13.5), sk().txt, 176.0);
    p.galley(
        egui::pos2(rect.left() + 12.0, cy - g_word.size().y / 2.0),
        g_word,
        sk().txt,
    );
    let rx = rect.left() + 200.0;
    let g_ranges = ellipsize(
        p,
        &row.ranges,
        FontId::proportional(12.5),
        sk().txt2,
        rect.right() - 12.0 - hide_w - rx,
    );
    p.galley(
        egui::pos2(rx, cy - g_ranges.size().y / 2.0),
        g_ranges,
        sk().txt2,
    );
    ui.add_space(6.0);
}

// ══ 小部件 ══

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

/// 画一个图标。全部走矢量（`icons.rs`），不用 PNG 素材：
/// 把 128px 位图缩到 20px 必然发糊，线条画法任意尺寸都清晰。
fn icon_paint(ui: &mut egui::Ui, rect: egui::Rect, name: &str, color: Color32) {
    crate::icons::paint(ui.painter(), rect, name, color);
}

fn icon_box(ui: &mut egui::Ui, icon: &str) {
    let s = sk();
    let (r, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, s.btn_bg);
    // 26 而不是 20：20px 下线条会挤成一团，大一圈清楚得多（外框仍是 40）
    let ic = egui::Rect::from_center_size(r.center(), egui::vec2(26.0, 26.0));
    icon_paint(ui, ic, icon, s.icon);
}

/// 小三角（展开/收起指示）。用矢量画，不依赖字体里有没有 ▾ ▴ 这两个字符
/// —— 缺字会渲染成"豆腐块"，在不同机器上表现不一致。
fn tri(p: &egui::Painter, c: egui::Pos2, up: bool, color: Color32) {
    let s = 4.0;
    let pts = if up {
        vec![
            egui::pos2(c.x, c.y - s),
            egui::pos2(c.x - s, c.y + s * 0.7),
            egui::pos2(c.x + s, c.y + s * 0.7),
        ]
    } else {
        vec![
            egui::pos2(c.x, c.y + s),
            egui::pos2(c.x - s, c.y - s * 0.7),
            egui::pos2(c.x + s, c.y - s * 0.7),
        ]
    };
    p.add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
}

/// 对勾（勾选框里），同样用矢量
fn check_mark(p: &egui::Painter, r: egui::Rect, color: Color32) {
    let s = r.shrink(4.0);
    let a = egui::pos2(s.left(), s.center().y + 1.0);
    let b = egui::pos2(s.left() + s.width() * 0.38, s.bottom() - 1.0);
    let c = egui::pos2(s.right(), s.top() + 1.0);
    let w = egui::Stroke::new(2.0, color);
    p.line_segment([a, b], w);
    p.line_segment([b, c], w);
}

/// 单行截断：面板里的文字一律不换行（行高固定、触摸行好点），
/// 超宽就裁尾 + 省略号，而不是让它压到右边的控件或溢出卡片外。
fn ellipsize(
    p: &egui::Painter,
    text: &str,
    font: FontId,
    color: Color32,
    max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let full = p.layout_no_wrap(text.to_string(), font.clone(), color);
    if full.size().x <= max_w || max_w <= 0.0 {
        return full;
    }
    // 逐字回退（行内文本都不长，几十次布局足够快）
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cand: String = chars.iter().collect::<String>() + "…";
        let g = p.layout_no_wrap(cand, font.clone(), color);
        if g.size().x <= max_w {
            return g;
        }
    }
    p.layout_no_wrap("…".to_string(), font, color)
}

fn title_sub(ui: &mut egui::Ui, title: &str, sub: &str) {
    title_sub_w(ui, title, sub, f32::INFINITY);
}

/// 标题 + 副标题。
/// `max_w` 用来给同一行右侧的按钮留位置 —— 不限宽的话副标题会顶到按钮底下
/// （窄窗口时尤其明显），所以超出的部分截断而不是换行（卡片高度是固定的）。
fn title_sub_w(ui: &mut egui::Ui, title: &str, sub: &str, max_w: f32) {
    // 关键：这里必须给两行文字一个**确定的行高盒子**。
    // 直接写 ui.vertical(...) 的话，这个竖向子区域会吃掉卡片剩下的全部高度，
    // 文字从盒子顶部开始排 —— 于是在 horizontal_centered 里，
    // 左侧图标方块按整卡居中、文字却贴在卡顶，看起来就是"文字被上移了"。
    let f1 = egui::FontId::proportional(14.5);
    let f2 = egui::FontId::proportional(12.5);
    let (h1, h2) = ui
        .ctx()
        .fonts_mut(|f| (f.row_height(&f1), f.row_height(&f2)));
    let h = h1 + 3.0 + h2;
    let w = max_w.min(ui.available_width()).max(60.0);
    ui.allocate_ui_with_layout(
        egui::vec2(w, h),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_max_width(w);
            ui.add(
                egui::Label::new(RichText::new(title).size(14.5).strong().color(sk().txt))
                    .wrap_mode(egui::TextWrapMode::Truncate),
            );
            ui.add_space(3.0);
            ui.add(
                egui::Label::new(RichText::new(sub).size(12.5).color(sk().txt2))
                    .wrap_mode(egui::TextWrapMode::Truncate),
            );
        },
    );
}

/// 同一行里「文字块 + 右侧按钮」的组合：先按按钮占宽留出余量，再画文字
fn title_sub_with_buttons(ui: &mut egui::Ui, title: &str, sub: &str, buttons_w: f32) {
    let rest = ui.available_width();
    title_sub_w(ui, title, sub, (rest - buttons_w).max(120.0));
}

fn win_btn(ui: &mut egui::Ui, icon: &str) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(40.0, 34.0), egui::Sense::click());
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

fn sq_btn(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    sq_btn_impl(ui, None, label, enabled, false)
}

fn sq_btn_icon(ui: &mut egui::Ui, icon: &str, label: &str, enabled: bool) -> bool {
    sq_btn_impl(ui, Some(icon), label, enabled, false)
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
    let w = (galley.size().x + icon_w + 22.0).max(34.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 34.0), egui::Sense::click());
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

fn num_box(ui: &mut egui::Ui, text: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(58.0, 34.0), egui::Sense::hover());
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
        text,
        FontId::proportional(13.0),
        sk().txt,
    );
}

fn seg_btn(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), sk().txt);
    let w = galley.size().x + 28.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 36.0), egui::Sense::click());
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

fn swatch_btn(ui: &mut egui::Ui, label: &str, dot: Color32, on: bool) -> bool {
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), sk().txt);
    let w = galley.size().x + 46.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 36.0), egui::Sense::click());
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
    p.rect_stroke(
        dot_rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, sk().line2),
        StrokeKind::Inside,
    );
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

fn gold_button(ui: &mut egui::Ui, icon: Option<&str>, label: &str) -> bool {
    let font = FontId::proportional(13.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), sk().acc_dark);
    let icon_w = if icon.is_some() { 24.0 } else { 0.0 };
    let w = galley.size().x + icon_w + 44.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 40.0), egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.10);
    let hover_col = if sk().is_dark {
        Color32::from_rgb(0xFF, 0xE2, 0x55)
    } else {
        Color32::from_rgb(0x58, 0x7C, 0x72)
    };
    let bg = mix(sk().acc, hover_col, hov);
    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
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
// 无头渲染测试：把面板在一个离屏 egui Context 里完整画一遍。
// 目的不是"像素对不对"，而是每个板块、每个弹窗都能走通不 panic，
// 并且确实产出了绘图指令（不是静默空白）。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn row(word: &str, expanded: bool) -> WordRow {
        WordRow {
            key: word.to_lowercase(),
            word: word.to_string(),
            meaning: "v. 测试释义".into(),
            details: vec![
                ("词义".into(), "v. 测试释义".into()),
                ("备注".into(), "这条备注被关闭展示".into()),
            ],
            hidden: vec!["note".into()],
            selected: true,
            expanded,
        }
    }

    fn import_ctx<'a>(
        errors: &'a [String],
        conflict: Option<(usize, usize, &'a str, &'a str, &'a str)>,
        file: &'a str,
    ) -> ImportCtx<'a> {
        ImportCtx {
            file,
            errors,
            conflict,
            done: None,
        }
    }

    /// 在一个离屏 Context 里画面板，返回**第二帧**产生的绘图指令数。
    ///
    /// 为什么要两帧：切标签有"淡入"动画，第一帧整体透明度是 0（egui 会跳过绘制），
    /// 只看第一帧会把"画得出来"误判成空白。
    fn render_at(
        tab: PanelTab,
        size: egui::Vec2,
        cx: &Ctx,
        rows: &[WordRow],
        cfg: &mut Config,
        panel: &mut PanelState,
    ) -> usize {
        let ctx = egui::Context::default();
        panel.tab = tab;
        let raw = |t: f64| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size)),
            time: Some(t),
            ..Default::default()
        };
        // 第一帧：登记"刚切过标签"，淡入从这里开始计时
        let mut first = ctx.run_ui(raw(0.0), |ui| {
            let _ = draw(ui, cfg, panel, cx, rows);
        });
        first.textures_delta.clear();
        // 第二帧：0.5s 后，淡入早已完成
        let mut out = ctx.run_ui(raw(0.5), |ui| {
            let _ = draw(ui, cfg, panel, cx, rows);
        });
        // 离屏测试没人消费纹理增量，清掉再丢弃（否则 epaint 会断言 panic）
        out.textures_delta.clear();
        out.shapes.len()
    }

    fn render(
        tab: PanelTab,
        cx: &Ctx,
        rows: &[WordRow],
        cfg: &mut Config,
        panel: &mut PanelState,
    ) -> usize {
        render_at(tab, egui::vec2(1004.0, 640.0), cx, rows, cfg, panel)
    }

    fn base_ctx<'a>(
        import: &'a ImportCtx<'a>,
        cards: &'a [CardRow],
        plans: &'a [PlanRow],
        warns: &'a [String],
    ) -> Ctx<'a> {
        Ctx {
            lib_label: "lexideck.json",
            total_entries: 4,
            selected: 2,
            display_items: 3,
            card_rows: cards,
            plan_rows: plans,
            effective: 3,
            shown: 2,
            warns,
            import,
            confirm_exit: false,
        }
    }

    /// 四个板块都能画出来，且都不是空白
    #[test]
    fn all_tabs_render_something() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let cards = vec![CardRow {
            id: 1,
            word: "seek".into(),
            source: "第 1 位 · 手动展示".into(),
        }];
        let plans = vec![PlanRow {
            word: "seek".into(),
            ranges: "2026-10-06 → 2026-10-12".into(),
            hide: String::new(),
        }];
        let cx = base_ctx(&import, &cards, &plans, &warns);
        let rows = vec![row("seek", false), row("gain", true)];
        let mut cfg = Config::default();
        for tab in [
            PanelTab::Words,
            PanelTab::Cards,
            PanelTab::Plans,
            PanelTab::Settings,
        ] {
            let mut panel = PanelState::default();
            let n = render(tab, &cx, &rows, &mut cfg, &mut panel);
            assert!(n > 25, "板块 {tab:?} 画出来的指令太少（{n}），疑似空白");
        }
    }

    /// 词表面板确实把词条行画出来了（和 0 行对比，而不是拍脑袋定阈值）
    #[test]
    fn word_rows_are_actually_drawn() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let cards: Vec<CardRow> = Vec::new();
        let plans: Vec<PlanRow> = Vec::new();
        let cx = base_ctx(&import, &cards, &plans, &warns);
        let mut cfg = Config::default();

        let mut p0 = PanelState::default();
        let empty = render(PanelTab::Words, &cx, &[], &mut cfg, &mut p0);

        let mut p1 = PanelState::default();
        let rows = vec![row("seek", false), row("gain", true)];
        let filled = render(PanelTab::Words, &cx, &rows, &mut cfg, &mut p1);
        assert!(
            filled > empty + 20,
            "词条行没画出来：空表 {empty} vs 两行 {filled}"
        );
    }

    /// 空词表不能崩（首次运行、导入前）
    #[test]
    fn empty_library_renders() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let cx = Ctx {
            total_entries: 0,
            selected: 0,
            display_items: 0,
            card_rows: &[],
            plan_rows: &[],
            effective: 0,
            shown: 0,
            ..base_ctx(&import, &[], &[], &warns)
        };
        let mut cfg = Config::default();
        let mut panel = PanelState::default();
        let n = render(PanelTab::Words, &cx, &[], &mut cfg, &mut panel);
        assert!(n > 50);
    }

    /// 冲突弹窗、格式错误弹窗、确认退出弹窗都要能画
    #[test]
    fn dialogs_render() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let cards: Vec<CardRow> = Vec::new();
        let plans: Vec<PlanRow> = Vec::new();
        let rows = vec![row("seek", false)];
        let mut cfg = Config::default();

        // 冲突弹窗
        let import = import_ctx(
            &errors,
            Some((1, 2, "seek", "v. 旧释义；n. 旧", "v. 新释义")),
            "importconflict.json",
        );
        let cx = base_ctx(&import, &cards, &plans, &warns);
        let mut panel = PanelState::default();
        let n = render(PanelTab::Words, &cx, &rows, &mut cfg, &mut panel);
        assert!(n > 50, "冲突弹窗没画出来");

        // 格式错误弹窗（多条错误 + 超长列表）
        let errs: Vec<String> = (0..14)
            .map(|i| format!("第 {i} 条词条：缺少 senses"))
            .collect();
        let import = import_ctx(&errs, None, "bad.json");
        let cx = base_ctx(&import, &cards, &plans, &warns);
        let mut panel = PanelState::default();
        let n = render(PanelTab::Words, &cx, &rows, &mut cfg, &mut panel);
        assert!(n > 50, "格式错误弹窗没画出来");
    }

    /// 窄窗口（最小尺寸 720×460）下也要画得出来，不能越界 panic
    #[test]
    fn narrow_window_renders() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = vec!["词条「x」缺少 senses，已跳过".into()];
        let import = import_ctx(&errors, None, "");
        let cards: Vec<CardRow> = Vec::new();
        let plans: Vec<PlanRow> = Vec::new();
        let cx = base_ctx(&import, &cards, &plans, &warns);
        let rows = vec![row("seek", true)];
        let mut cfg = Config::default();
        let mut panel = PanelState::default();

        let n = render_at(
            PanelTab::Words,
            egui::vec2(720.0, 460.0),
            &cx,
            &rows,
            &mut cfg,
            &mut panel,
        );
        assert!(n > 20, "窄窗口下几乎没画出东西（{n}）");
    }
}
