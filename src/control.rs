//! 控制面板（v2：四个板块 —— 词表 / 卡片 / 策略 / 设置）。
//! 面板只产生"意图"（ControlResult），由 app 执行实际动作；这里不碰数据。
//!
//! 布局：顶栏（自绘，含拖拽与窗口按钮）→ 标签行 → 内容（主列 + 说明列）→ 底栏。
//! 面板可缩放：宽度不足时自动收起右侧说明列；所有控件尺寸按触摸放大（可点高度 ≥ 34）。

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, RichText, Stroke, StrokeKind};

use std::collections::HashSet;

use crate::config::{CloseAction, Config, Layer};
use crate::deck::{self, Range};
use crate::schedule::RangeState;
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
    /// 策略页里展开看明细的那几条（键 = `plan_key`）
    pub plans_open: HashSet<String>,
    /// 策略页「已经结束」那段是否展开（默认收起）
    pub past_open: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            tab: PanelTab::default(),
            search: String::new(),
            filter: WordFilter::All,
            plans_open: HashSet::new(),
            past_open: false,
        }
    }
}

/// 策略行的稳定键（词 + 日期段），展开状态与开关都按它认人
pub fn plan_key(word: &str, r: &Range) -> String {
    format!(
        "{}\u{1}{}\u{1}{}",
        word.trim().to_lowercase(),
        r.from.trim(),
        r.to.trim()
    )
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
    /// 回到「按今日展示」（屏幕内容交回展示时间引擎）
    pub back_to_today: bool,
    /// 永久关闭 / 打开一条策略（词 + 日期段）
    pub toggle_off: Option<(String, Range)>,
    /// 屏幕内容来源：Some(true) = 按今日展示，Some(false) = 手动展示
    pub set_screen_auto: Option<bool>,
    /// 关闭窗口时：Some(档位) = 用户在设置页换了档（app 侧更新 cfg + 存盘 + 反馈）
    pub set_close_action: Option<CloseAction>,
    /// 设置页「退出 Lexideck」按钮：明确退出（不走二次确认，绕过托盘关闭拦截）
    pub quit_now: bool,
    /// 面板产生的一句反馈，交给 app 显示在底栏
    pub status_msg: Option<String>,
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
    pub word: String,
    /// 这张卡的日期段（自动模式下有；手动展示为空）
    pub range_label: String,
    /// 来源文件（没有就写"手动展示"）
    pub source: String,
    /// 这张卡额外隐藏的区块（策略内隐藏 ∪ 词条自己的 hide）
    pub hide: Vec<String>,
}

/// 策略（展示时间）列表里的一行 —— 一条策略 = 一个词的一段展示时间
pub struct PlanRow {
    pub word: String,
    /// 原始日期段（开关回传要用它）
    pub range: Range,
    /// 今天落在段内 / 还没开始 / 已经结束
    pub state: RangeState,
    /// 被永久关闭（状态.json 里记着）
    pub off: bool,
    /// 来自哪份导入文件（只作展示）
    pub source: String,
    /// 这段策略里额外隐藏的区块名（`deck::HIDE_KEYS`）
    pub hide: Vec<String>,
    /// 展开看明细
    pub expanded: bool,
    /// 一句话状态（"今天生效" / "还有 3 天开始" / "已结束"）
    pub why: String,
}

impl PlanRow {
    pub fn key(&self) -> String {
        plan_key(&self.word, &self.range)
    }
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
    /// 屏幕内容来源：true = 按今日展示（自动）
    pub screen_auto: bool,
    /// 今天（YYYY-MM-DD），底栏状态行用
    pub today: &'a str,
    /// 最近一次操作的反馈（空串 = 显示默认状态行）
    pub status: &'a str,
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
            PanelTab::Plans => plans_tab(ui, panel, cx, &mut r),
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
    let (n_now, _, _) = plan_counts(cx.plan_rows);
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
                let (status, color) = if cx.confirm_exit {
                    (
                        "再点一次「×」退出程序（5 秒内有效）".to_string(),
                        sk().danger,
                    )
                } else if !cx.status.is_empty() {
                    (cx.status.to_string(), sk().txt2)
                } else {
                    (
                        format!(
                            "运行中 · 今日 {} · 生效 {} 条 / 上屏 {} 张 · 主库 {}",
                            cx.today, n_now, cx.shown, cx.lib_label
                        ),
                        sk().txt3,
                    )
                };
                // 状态文字是这一行里唯一会让位的东西：按钮的触摸尺寸（≥34）不能缩
                let tw = (ui.available_width() - 360.0).max(60.0);
                let (tr, _) = ui.allocate_exact_size(egui::vec2(tw, 20.0), egui::Sense::hover());
                let g = ellipsize(ui.painter(), &status, FontId::proportional(12.5), color, tw);
                ui.painter().galley(
                    egui::pos2(tr.left(), tr.center().y - g.size().y / 2.0),
                    g,
                    color,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match panel.tab {
                        PanelTab::Settings => {
                            if gold_button(ui, None, "保存设置") {
                                r.changed = true;
                                r.status_msg = Some(
                                    "设置已保存到 exe 旁的「设置.txt」（改完即时生效）".into(),
                                );
                            }
                        }
                        PanelTab::Cards | PanelTab::Plans => {
                            if gold_button(ui, None, "按今日展示") {
                                r.set_screen_auto = Some(true);
                            }
                        }
                        PanelTab::Words => {
                            if gold_button(ui, None, &format!("立即展示（{}）", cx.selected))
                            {
                                r.show_selected = true;
                            }
                        }
                    }
                    ui.add_space(10.0);
                    if quiet_btn(ui, "replay", "重播动画", cx.shown > 0) {
                        r.replay_anim = true;
                    }
                });
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
    h4(ui, "float_list", "桌面卡片");
    lead(
        ui,
        "屏幕上现在的内容。位置由平铺规则决定（右上角起、往下排、满列向左），放不下按顺序截断。",
    );

    stats(
        ui,
        &[
            ("今日生效", cx.effective.to_string(), StatKind::Acc),
            ("已上屏", cx.shown.to_string(), StatKind::Plain),
            (
                "字号档位",
                format!("{:.2}", cfg.font_scale),
                StatKind::Plain,
            ),
        ],
    );

    group_head(
        ui,
        "屏幕上的卡片",
        "顺序 = 上屏顺序",
        Some("截断时先来先上"),
    );
    rows_box(ui, |ui| {
        if cx.card_rows.is_empty() {
            empty_line(
                ui,
                "桌面没有卡片 —— 今天没有排期，或在「词表」里勾几个词点「立即展示」",
            );
        }
        for (i, row) in cx.card_rows.iter().enumerate() {
            card_row(ui, row, i + 1);
        }
    });

    group_head(ui, "屏幕内容从哪来", "", None);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if mode_btn(ui, "按今日展示", cx.screen_auto) && !cx.screen_auto {
            r.set_screen_auto = Some(true);
        }
        if mode_btn(ui, "手动展示", !cx.screen_auto) && cx.screen_auto {
            r.set_screen_auto = Some(false);
        }
    });
    ui.add_space(10.0);
    mini(
        ui,
        if cx.screen_auto {
            "自动：按今天的策略上屏，跨过零点自动换下一批；今天没有排期就是空白屏幕。"
        } else {
            "手动：只显示你在「词表」里勾选的那几个词（调试用）；想回到自动就点右下角「按今日展示」。"
        },
    );

    group_head(ui, "字号", "", Some("全局一个档位，所有卡片一起变"));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        if sq_btn(ui, "A−", cfg.font_scale > FONT_MIN + 1e-4) {
            cfg.font_scale = font_step(cfg.font_scale, -1);
            r.changed = true;
        }
        num_box(ui, &format!("{:.2}", cfg.font_scale));
        if sq_btn(ui, "A＋", cfg.font_scale < FONT_MAX - 1e-4) {
            cfg.font_scale = font_step(cfg.font_scale, 1);
            r.changed = true;
        }
        ui.add_space(6.0);
        if quiet_btn(
            ui,
            "refresh",
            "复位 0.70",
            (cfg.font_scale - FONT_DEF).abs() > 1e-3,
        ) {
            cfg.font_scale = FONT_DEF;
            r.changed = true;
        }
    });
    ui.add_space(14.0);
    mini(
        ui,
        "为什么卡片不能拖：位置完全由平铺规则决定，教室机器上被拖乱就没人能还原。",
    );
}

fn plans_tab(ui: &mut egui::Ui, panel: &mut PanelState, cx: &Ctx, r: &mut ControlResult) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| plans_body(ui, panel, cx, r));
}

fn plans_body(ui: &mut egui::Ui, panel: &mut PanelState, cx: &Ctx, r: &mut ControlResult) {
    h4(ui, "display", "展示策略");
    lead(
        ui,
        "一条策略 = 一个词的一段展示时间。关掉它，这个词在这段时间就不上屏，永久生效，直到你在这里重新打开。",
    );

    let (n_now, n_off, n_future) = plan_counts(cx.plan_rows);
    stats(
        ui,
        &[
            ("今日生效", n_now.to_string(), StatKind::Acc),
            ("今天上屏", cx.shown.to_string(), StatKind::Plain),
            (
                "被永久关闭",
                n_off.to_string(),
                if n_off > 0 {
                    StatKind::Warn
                } else {
                    StatKind::Plain
                },
            ),
            ("未来排期", n_future.to_string(), StatKind::Plain),
        ],
    );

    if cx.plan_rows.is_empty() {
        ui.add_space(10.0);
        mini(
            ui,
            "主库里还没有展示时间 —— 导入一份带 display_time 的文件即可。",
        );
        return;
    }

    let pick = |st: RangeState| -> Vec<&PlanRow> {
        cx.plan_rows.iter().filter(|p| p.state == st).collect()
    };
    let now = pick(RangeState::Active);
    let future = pick(RangeState::Future);
    let past = pick(RangeState::Past);

    group_head(
        ui,
        "正在生效",
        &format!("{} 条", now.len()),
        Some("今天就在屏幕上"),
    );
    if now.is_empty() {
        empty_line(ui, "今天没有生效的策略（屏幕是空白的）");
    } else {
        rows_box(ui, |ui| {
            for p in &now {
                plan_row(ui, p, panel, r);
            }
        });
    }

    group_head(
        ui,
        "还没开始",
        &format!("{} 条", future.len()),
        Some("按日期自动接上"),
    );
    if future.is_empty() {
        empty_line(
            ui,
            "没有排在未来 —— 想提前排课就在词表 JSON 里加 display_time",
        );
    } else {
        rows_box(ui, |ui| {
            for p in &future {
                plan_row(ui, p, panel, r);
            }
        });
    }

    group_head(ui, "已经结束", &format!("{} 条", past.len()), None);
    if past.is_empty() {
        empty_line(ui, "还没有结束的展示策略");
    } else {
        let label = format!("展示已经结束的展示策略（{} 条）", past.len());
        if sq_btn_icon(
            ui,
            if panel.past_open { "chev_up" } else { "chev" },
            &label,
            true,
        ) {
            panel.past_open = !panel.past_open;
        }
        if panel.past_open {
            ui.add_space(10.0);
            rows_box(ui, |ui| {
                for p in &past {
                    plan_row(ui, p, panel, r);
                }
            });
        }
    }
}

fn settings_tab(ui: &mut egui::Ui, cfg: &mut Config, cx: &Ctx, r: &mut ControlResult) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| settings_body(ui, cfg, cx, r));
}

fn settings_body(ui: &mut egui::Ui, cfg: &mut Config, cx: &Ctx, r: &mut ControlResult) {
    h4(ui, "geo", "设置");
    lead(
        ui,
        "面板与卡片共用主题；设置自动存到 exe 旁的「设置.txt」，策略的开关存在「状态.json」。",
    );

    group_head(
        ui,
        "卡片层级",
        "",
        Some("其他课的 PPT 不能被挡住，所以默认置底"),
    );
    let on_top = cfg.layer == Layer::Top;
    ui.horizontal(|ui| {
        let w = ((ui.available_width() - 10.0) / 2.0).max(170.0);
        if layer_option(
            ui,
            w,
            "置底（默认）",
            "卡片贴在桌面最底层，任何窗口都会盖住它。代价：全屏放 PPT 时卡片看不见，只在课间/桌面状态看得到。",
            !on_top,
        ) && on_top
        {
            set_layer(cfg, false, r);
        }
        ui.add_space(10.0);
        if layer_option(
            ui,
            w,
            "置顶 ＋ 鼠标穿透",
            "卡片始终在最上层，点击全部穿过去落到 PPT 上。代价：卡片会压在 PPT 右侧，视觉上占用版面。",
            on_top,
        ) && !on_top
        {
            set_layer(cfg, true, r);
        }
    });
    ui.add_space(12.0);
    callout(
        ui,
        "穿透是整个窗口粒度的：开启后卡片收不到任何鼠标事件，恢复通道只有这个面板 —— 面板里提供「关闭穿透」和「隐藏全部卡片」。",
    );
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if sq_btn_danger_icon(ui, "close_all", "隐藏全部卡片", cx.shown > 0) {
            r.close_all = true;
        }
    });

    ui.add_space(14.0);
    group_head(
        ui,
        "主题",
        "",
        Some("只换配色，不动版式（版式一变，教室可读性要重测）"),
    );
    ui.horizontal(|ui| {
        for kind in [ThemeKind::Plain, ThemeKind::Wuling, ThemeKind::Yellow] {
            let on = cfg.theme == kind.as_str();
            if theme_thumb(ui, kind, on) && !on {
                cfg.theme = kind.as_str().to_string();
                r.changed = true;
            }
            ui.add_space(8.0);
        }
        ui.add_space(6.0);
        ui.label(
            RichText::new(ThemeKind::parse(&cfg.theme).label())
                .size(12.5)
                .color(sk().txt3),
        );
    });

    ui.add_space(14.0);
    group_head(
        ui,
        "动画流畅度",
        "",
        Some("卡片入场动画的帧率上限：跟着屏幕走最省，不限帧最快"),
    );
    ui.horizontal(|ui| {
        let w = ((ui.available_width() - 10.0) / 2.0).max(170.0);
        if layer_option(
            ui,
            w,
            "跟随显示器（默认）",
            "帧率跟着屏幕刷新率走：60Hz 屏就是 60 帧，165Hz 屏最高 165 帧（这台实测 118 帧）。不撕裂，也不白烧 CPU。",
            cfg.vsync,
        ) && !cfg.vsync
        {
            cfg.vsync = true;
            r.changed = true;
        }
        ui.add_space(10.0);
        if layer_option(
            ui,
            w,
            "不限帧（最快）",
            "关掉垂直同步，能跑多快跑多快（这台实测 177 帧）。代价：更吃 CPU/GPU，60Hz 屏上可能撕裂。",
            !cfg.vsync,
        ) && cfg.vsync
        {
            cfg.vsync = false;
            r.changed = true;
        }
    });
    ui.add_space(8.0);
    callout(
        ui,
        "这一项在程序启动时生效，改完要重开一次 Lexideck；动画本身不受影响，只影响它每秒画多少帧。",
    );

    ui.add_space(18.0);
    sep_line(ui);
    group_head(
        ui,
        "关闭窗口时",
        "",
        Some("面板顶栏的 ✕（或系统关闭）：驻留托盘 = 收进托盘；直接退出 = 走 5 秒二次确认"),
    );
    ui.horizontal(|ui| {
        let w = ((ui.available_width() - 10.0) / 2.0).max(170.0);
        if layer_option(
            ui,
            w,
            "驻留托盘（默认）",
            "关掉面板后程序继续跑，卡片不掉；点托盘图标、或用托盘右键菜单可随时唤回面板。",
            cfg.close_action == CloseAction::Tray,
        ) && cfg.close_action != CloseAction::Tray
        {
            r.set_close_action = Some(CloseAction::Tray);
        }
        ui.add_space(10.0);
        if layer_option(
            ui,
            w,
            "直接退出",
            "关掉面板 = 退出程序（5 秒内点两次 ✕ 确认）。教室策略挡掉托盘图标时用这档。",
            cfg.close_action == CloseAction::Exit,
        ) && cfg.close_action != CloseAction::Exit
        {
            r.set_close_action = Some(CloseAction::Exit);
        }
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if sq_btn_danger_icon(ui, "exit", "退出 Lexideck", true) {
            r.quit_now = true;
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new("点了立即退出（驻留托盘时也能从这里退）")
                .size(12.0)
                .color(sk().txt3),
        );
    });

    ui.add_space(18.0);
    sep_line(ui);
    sett_line(ui, "开机自启动", "写当前用户的启动项，免管理员", Some("P3"));
    sett_line(
        ui,
        "单实例",
        "第二次启动不再开新窗口，把已有窗口唤到前台",
        Some("P3"),
    );
    sett_line(
        ui,
        "关于",
        "Lexideck · 慕言　·　github.com/FanHuaWuYang/Lexideck",
        None,
    );
    ui.add_space(12.0);
    if !cx.warns.is_empty() {
        mini(ui, "解析与校验提示");
        for w in cx.warns.iter().take(6) {
            mini(ui, w);
        }
    }
}

/// 层级两档的取值：(层级, 鼠标穿透)。置顶必然配穿透（设计 §5.2）
fn layer_pair(top: bool) -> (Layer, bool) {
    if top {
        (Layer::Top, true)
    } else {
        (Layer::Bottom, false)
    }
}

/// 切层级：写回 cfg 并让 app 去发 WindowLevel / MousePassthrough（top_changed）
fn set_layer(cfg: &mut Config, top: bool, r: &mut ControlResult) {
    let (layer, passthrough) = layer_pair(top);
    cfg.layer = layer;
    cfg.passthrough = passthrough;
    r.changed = true;
    r.top_changed = true;
}

// ══ 右栏说明 ══

fn help_pane(ui: &mut egui::Ui, tab: PanelTab, cx: &Ctx) {
    let hd = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).size(13.5).strong().color(sk().txt));
        ui.add_space(8.0);
    };
    let pg = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).size(13.0).color(sk().txt2));
        ui.add_space(10.0);
    };
    let gd = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).size(12.5).color(sk().acc));
        ui.add_space(8.0);
    };
    let mn = |ui: &mut egui::Ui, s: &str| {
        ui.label(RichText::new(s).size(12.0).color(sk().txt3));
        ui.add_space(6.0);
    };
    let sp = |ui: &mut egui::Ui| {
        let (rr, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().rect_filled(rr, 0.0, sk().line);
        ui.add_space(12.0);
    };
    let st = |ui: &mut egui::Ui, k: &str, v: &str| {
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
            hd(ui, "词库");
            pg(
                ui,
                "词条只从 JSON 文件来。导入 = 并进主库：同名词条更新、其余追加。",
            );
            gd(ui, "勾几个词 → 右下角「立即展示」，卡片就出现在桌面上。");
            sp(ui);
            st(ui, "词条总数", &cx.total_entries.to_string());
            st(ui, "已选", &cx.selected.to_string());
            st(ui, "展示时间条目", &cx.display_items.to_string());
            ui.add_space(12.0);
            mn(ui, "面板里不提供改词条内容的功能——改内容请改 JSON 再导入。");
            mn(ui, "格式不对的文件会被整份拒绝，主库不受影响。");
        }
        PanelTab::Cards => {
            hd(ui, "屏幕内容从哪来");
            pg(
                ui,
                "两种来源：按今日展示（自动）／手动展示（调试用）。启动时默认按今日展示。",
            );
            hd(ui, "截断规则");
            pg(
                ui,
                "放不下就按顺序截断：先排的先上屏，后面的不显示，面板会把「生效 N 个 / 已显示 M 个」写清楚。",
            );
            hd(ui, "为什么卡片不能拖");
            pg(ui, "位置完全由排列规则决定，教室机器上被拖乱就没人能还原。");
            sp(ui);
            st(ui, "本次生效", &cx.effective.to_string());
            st(ui, "屏幕上显示", &cx.shown.to_string());
        }
        PanelTab::Plans => {
            hd(ui, "怎么理解这一页");
            pg(
                ui,
                "一条策略 = 一个词的一段展示时间。同一个词可以有好几段（比如第 5 周再轮一次），每段各自开关。",
            );
            hd(ui, "关掉会怎样");
            pg(
                ui,
                "关掉就是永久的：屏幕上不再出现这个词这段时间的卡片，第二天也不会自己恢复，必须在这里手动打开。",
            );
            hd(ui, "关掉是不是删了内容");
            pg(ui, "不是。词条和日期段都还在主库里，只是这段时间不上屏。");
            sp(ui);
            st(ui, "展示时间条目", &cx.display_items.to_string());
            ui.add_space(12.0);
            mn(ui, "今天没有排期 → 屏幕空白（不沿用上一批）。");
        }
        PanelTab::Settings => {
            hd(ui, "为什么默认置底");
            pg(
                ui,
                "其他课上 PPT 不能被卡片遮挡，所以默认藏在最底层；需要常显就切「置顶 + 穿透」。",
            );
            hd(ui, "穿透的风险");
            pg(
                ui,
                "开启后卡片点不到，只能从这个面板恢复。所以面板本身永远是正常可交互的。",
            );
            hd(ui, "设置都写在哪");
            pg(
                ui,
                "主库 lexideck.json、设置 设置.txt、策略开关 状态.json —— 全在 exe 同目录，整个文件夹拷走就带走全部状态。",
            );
            if !cx.warns.is_empty() {
                sp(ui);
                mn(ui, "提示");
                for w in cx.warns.iter().take(6) {
                    mn(ui, w);
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

fn card_row(ui: &mut egui::Ui, row: &CardRow, pos: usize) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(rect, CornerRadius::ZERO, sk().card);
    p.rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 1.0)),
        0.0,
        sk().row_line,
    );
    let cy = rect.center().y;

    // 右侧：这张卡隐藏了哪些区块（没隐藏就是「完整」）
    let tail = if row.hide.is_empty() {
        "完整".to_string()
    } else {
        format!(
            "隐藏 {}",
            row.hide
                .iter()
                .map(|h| deck::hide_label(h))
                .collect::<Vec<_>>()
                .join("/")
        )
    };
    let tg = p.layout_no_wrap(tail, FontId::proportional(11.5), sk().txt3);
    let tw = tg.size().x + 16.0;
    let tr = egui::Rect::from_min_size(
        egui::pos2(rect.right() - PLAN_PAD_R - tw, cy - CHIP_H / 2.0),
        egui::vec2(tw, CHIP_H),
    );
    chip_frame(&p, tr, sk().line2, false);
    p.galley(
        egui::pos2(tr.left() + 8.0, cy - tg.size().y / 2.0),
        tg,
        sk().txt3,
    );

    // 序号
    let nr = egui::Rect::from_min_size(
        egui::pos2(rect.left() + PLAN_PAD_L, cy - 13.0),
        egui::vec2(26.0, 26.0),
    );
    p.text(
        nr.center(),
        Align2::CENTER_CENTER,
        format!("{pos}"),
        FontId::proportional(13.0),
        sk().txt3,
    );

    // 从右往左摆小标签，剩下的宽度给词
    let mut right = tr.left() - 10.0;
    if !row.source.is_empty() {
        let g = ellipsize(
            &p,
            &row.source,
            FontId::proportional(12.0),
            sk().txt3,
            170.0,
        );
        let w = g.size().x + 14.0;
        let cr = egui::Rect::from_min_size(
            egui::pos2(right - w, cy - CHIP_H / 2.0),
            egui::vec2(w, CHIP_H),
        );
        chip_frame(&p, cr, sk().line2, true);
        p.galley(
            egui::pos2(cr.left() + 7.0, cy - g.size().y / 2.0),
            g,
            sk().txt3,
        );
        right = cr.left() - 8.0;
    }
    if !row.range_label.is_empty() {
        let g = p.layout_no_wrap(
            row.range_label.clone(),
            FontId::proportional(12.5),
            sk().txt2,
        );
        let w = g.size().x + 14.0;
        let cr = egui::Rect::from_min_size(
            egui::pos2(right - w, cy - CHIP_H / 2.0),
            egui::vec2(w, CHIP_H),
        );
        chip_frame(&p, cr, sk().line2, false);
        p.galley(
            egui::pos2(cr.left() + 7.0, cy - g.size().y / 2.0),
            g,
            sk().txt2,
        );
        right = cr.left() - 8.0;
    }
    let x = rect.left() + PLAN_PAD_L + 38.0;
    let g = ellipsize(
        &p,
        &row.word,
        FontId::proportional(15.5),
        sk().txt,
        (right - x).max(40.0),
    );
    p.galley(egui::pos2(x, cy - g.size().y / 2.0), g, sk().txt);
}

/// 一条策略的版式尺寸（触摸规格：第一行 52 ≥ 50，开关 52×30，箭头 34×34）
const PLAN_L1_H: f32 = 52.0;
const PLAN_L2_H: f32 = 28.0;
/// 第二行小标签的中心相对「第一行下缘」的下移量。
/// 别用第二行居中（那是 +14）：第一行 52 高是触摸规格，词的下方本来就有留白，
/// 居中会让标签离「自己这行的词」和「下一行的词」几乎一样远（实测 40 : 40），
/// 看起来就像属于下一个元素。收到 +6 之后是 32 : 48，归属性一眼可辨。
const PLAN_L2_CHIP_DY: f32 = 6.0;
const PLAN_L3_H: f32 = 122.0;
const PLAN_PAD_L: f32 = 14.0;
const PLAN_PAD_R: f32 = 12.0;
const SWITCH_W: f32 = 52.0;
const SWITCH_H: f32 = 30.0;
const EXPAND_W: f32 = 34.0;
const CHIP_H: f32 = 22.0;

/// 一条策略 = 一行两段：
///   · 第一行：`词 + 日期段 + 「出卡片/已关闭」 + 开关 + 展开箭头`
///   · 第二行：`来源文件` / `这段隐藏 例句/备注` 两个小标签（有才显示）
/// 展开后是 4 行明细（来源文件 / 日期段+原因 / 这段隐藏 / 操作提示）。
///
/// 版式铁律：第一行的**词与日期段**是主信息 —— 任何情况下都完整显示、绝不重叠；
/// 真放不下时先让注释性的「出卡片/已关闭」和第二行小标签让位。
fn plan_row(ui: &mut egui::Ui, row: &PlanRow, panel: &mut PanelState, r: &mut ControlResult) {
    let key = row.key();
    let has_l2 = !row.source.is_empty() || !row.hide.is_empty();
    let h = PLAN_L1_H
        + if has_l2 { PLAN_L2_H } else { 0.0 }
        + if row.expanded { PLAN_L3_H } else { 0.0 };
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
    let l1 = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), PLAN_L1_H));
    let row_id = ui.id().with(("plan-row", key.clone()));
    let row_resp = ui.interact(rect, row_id, egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(row_id, row_resp.hovered(), 0.10);

    let cy1 = l1.center().y;
    let expand_r = egui::Rect::from_min_size(
        egui::pos2(l1.right() - PLAN_PAD_R - EXPAND_W, cy1 - 17.0),
        egui::vec2(EXPAND_W, 34.0),
    );
    let sw_r = egui::Rect::from_min_size(
        egui::pos2(expand_r.left() - 12.0 - SWITCH_W, cy1 - SWITCH_H / 2.0),
        egui::vec2(SWITCH_W, SWITCH_H),
    );

    let p = ui.painter().clone();
    p.rect_filled(
        rect,
        CornerRadius::ZERO,
        if row.off { sk().row_bg } else { sk().card },
    );
    if hov > 0.0 {
        let (a, b) = if row.off {
            (sk().row_bg, sk().row_sel)
        } else {
            (sk().card, sk().card_hover)
        };
        // 整行一起高亮：第二行那排小标签贴着第一行下缘，只高亮 52px 会在标签中间断开
        p.rect_filled(rect, CornerRadius::ZERO, mix(a, b, hov));
    }
    p.rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 1.0)),
        0.0,
        sk().row_line,
    );

    // ── 第一行：词 + 日期段（先量、再按可用宽度决定谁让位）──
    let word_col = if row.off { sk().txt3 } else { sk().txt };
    let chip_col = sk().txt2;
    let text_x = l1.left() + PLAN_PAD_L;
    let space = (sw_r.left() - 10.0 - text_x).max(60.0);
    let chip_g = p.layout_no_wrap(row.range.label(), FontId::proportional(12.5), chip_col);
    let chip_w = chip_g.size().x + 14.0;
    let swlab_g = p.layout_no_wrap(
        if row.off { "已关闭" } else { "出卡片" }.to_string(),
        FontId::proportional(12.0),
        sk().txt3,
    );
    let word_font = FontId::proportional(15.5);
    let word_full = p.layout_no_wrap(row.word.clone(), word_font.clone(), word_col);
    let need = word_full.size().x + 12.0 + chip_w;
    let show_swlab = need + swlab_g.size().x + 12.0 <= space;
    let room = space
        - if show_swlab {
            swlab_g.size().x + 12.0
        } else {
            0.0
        };
    let word_max = (room - chip_w - 12.0).max(36.0);
    // 词与日期段永远不重叠：只有窗口小到物理放不下时才截词（末位手段）
    let word_g = if word_full.size().x > word_max {
        ellipsize(&p, &row.word, word_font, word_col, word_max)
    } else {
        word_full
    };
    let wy = cy1 - word_g.size().y / 2.0;
    p.galley(egui::pos2(text_x, wy), word_g.clone(), word_col);
    if row.off {
        // 已永久关闭：划掉（预览页用 line-through）
        let y = wy + word_g.size().y * 0.56;
        p.line_segment(
            [
                egui::pos2(text_x, y),
                egui::pos2(text_x + word_g.size().x, y),
            ],
            Stroke::new(1.0, sk().txt3),
        );
    }
    let chip_r = egui::Rect::from_min_size(
        egui::pos2(text_x + word_g.size().x + 12.0, cy1 - CHIP_H / 2.0),
        egui::vec2(chip_w, CHIP_H),
    );
    chip_frame(&p, chip_r, sk().line2, false);
    p.galley(
        egui::pos2(chip_r.left() + 7.0, cy1 - chip_g.size().y / 2.0),
        chip_g,
        chip_col,
    );
    if show_swlab {
        p.galley(
            egui::pos2(
                sw_r.left() - 12.0 - swlab_g.size().x,
                cy1 - swlab_g.size().y / 2.0,
            ),
            swlab_g,
            sk().txt3,
        );
    }

    // ── 第二行：来源文件 / 这段隐藏（注释性小标签，放不下就整块不画）──
    if has_l2 {
        let y2 = l1.bottom() + PLAN_L2_CHIP_DY;
        let mut x2 = rect.left() + PLAN_PAD_L;
        let tail = rect.right() - PLAN_PAD_R;
        if !row.source.is_empty() {
            let g = ellipsize(
                &p,
                &row.source,
                FontId::proportional(12.0),
                sk().txt2,
                (rect.width() * 0.4).max(60.0),
            );
            let w = g.size().x + 14.0;
            if x2 + w <= tail {
                let cr = egui::Rect::from_min_size(
                    egui::pos2(x2, y2 - CHIP_H / 2.0),
                    egui::vec2(w, CHIP_H),
                );
                chip_frame(&p, cr, sk().line2, true);
                p.galley(
                    egui::pos2(cr.left() + 7.0, y2 - g.size().y / 2.0),
                    g,
                    sk().txt2,
                );
                x2 += w + 8.0;
            }
        }
        if !row.hide.is_empty() {
            let names = row
                .hide
                .iter()
                .map(|h| deck::hide_label(h))
                .collect::<Vec<_>>()
                .join(" / ");
            let g = ellipsize(
                &p,
                &format!("这段隐藏 {names}"),
                FontId::proportional(12.0),
                sk().acc,
                (rect.width() * 0.45).max(60.0),
            );
            let w = g.size().x + 14.0;
            if x2 + w <= tail {
                let cr = egui::Rect::from_min_size(
                    egui::pos2(x2, y2 - CHIP_H / 2.0),
                    egui::vec2(w, CHIP_H),
                );
                chip_frame(&p, cr, sk().acc, false);
                p.galley(
                    egui::pos2(cr.left() + 7.0, y2 - g.size().y / 2.0),
                    g,
                    sk().acc,
                );
            }
        }
    }

    // ── 展开后的 4 行明细 ──
    if row.expanded {
        let top = l1.bottom() + if has_l2 { PLAN_L2_H } else { 0.0 };
        let r2 = egui::Rect::from_min_max(
            egui::pos2(rect.left(), top),
            egui::pos2(rect.right(), rect.bottom()),
        );
        dashed_line(
            &p,
            egui::pos2(r2.left() + PLAN_PAD_L, r2.top()),
            egui::pos2(r2.right() - PLAN_PAD_R, r2.top()),
            sk().line,
        );
        let src = if row.source.is_empty() {
            "（没记来源）".to_string()
        } else {
            row.source.clone()
        };
        let hide_txt = if row.hide.is_empty() {
            "无，按词条自己的显隐来".to_string()
        } else {
            format!(
                "{}（只在这段时间里生效，不改词条本身）",
                row.hide
                    .iter()
                    .map(|h| deck::hide_label(h))
                    .collect::<Vec<_>>()
                    .join("、")
            )
        };
        let lines = [
            ("来源文件", format!("{src}（导入时记下，只作展示）")),
            ("日期段", format!("{}　·　{}", row.range.label(), row.why)),
            ("这段隐藏", hide_txt),
            (
                "操作",
                if row.off {
                    "这条已永久关闭，卡片不会出现在屏幕上".to_string()
                } else {
                    "点右侧开关可永久关闭这条策略的卡片".to_string()
                },
            ),
        ];
        let mut y = r2.top() + 24.0;
        for (k, v) in lines {
            p.text(
                egui::pos2(r2.left() + PLAN_PAD_L, y),
                Align2::LEFT_CENTER,
                k,
                FontId::proportional(12.5),
                sk().txt3,
            );
            let vx = r2.left() + PLAN_PAD_L + 68.0;
            let g = ellipsize(
                &p,
                &v,
                FontId::proportional(12.5),
                sk().txt2,
                r2.right() - PLAN_PAD_R - vx,
            );
            p.galley(egui::pos2(vx, y - g.size().y / 2.0), g, sk().txt2);
            y += 24.0;
        }
    }

    // ── 交互（后注册的在上层：点开关/箭头不会连带展开整行）──
    if switch_btn(ui, sw_r, ui.id().with(("plan-sw", key.clone())), !row.off) {
        r.toggle_off = Some((row.word.clone(), row.range.clone()));
    }
    if expand_btn(
        ui,
        expand_r,
        ui.id().with(("plan-exp", key.clone())),
        row.expanded,
    ) {
        toggle_open(panel, &key);
    }
    if row_resp.clicked() {
        toggle_open(panel, &key);
    }
}

/// 永久关闭一条策略的开关：开 = 出卡片（默认），关 = 已关闭
fn switch_btn(ui: &mut egui::Ui, rect: egui::Rect, id: egui::Id, on: bool) -> bool {
    let resp = ui.interact(rect, id, egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(id.with("hov"), resp.hovered(), 0.10);
    let t = ui.ctx().animate_bool_with_time(id.with("on"), on, 0.16);
    let p = ui.painter();
    let bg = if on {
        mix(sk().acc, sk().acc_dark, hov * 0.08)
    } else {
        mix(sk().btn_bg, sk().btn_hover, hov)
    };
    p.rect_filled(rect, CornerRadius::ZERO, bg);
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(
            1.0,
            if on {
                sk().acc
            } else {
                mix(sk().btn_line, sk().line2, hov)
            },
        ),
        StrokeKind::Inside,
    );
    let k = 22.0;
    let x = rect.left() + 3.0 + t * (rect.width() - k - 6.0);
    let knob =
        egui::Rect::from_min_size(egui::pos2(x, rect.center().y - k / 2.0), egui::vec2(k, k));
    let knob_col = if on {
        Color32::WHITE
    } else {
        mix(sk().txt3, sk().txt, hov)
    };
    p.rect_filled(knob, CornerRadius::ZERO, knob_col);
    resp.clicked()
}

/// 展开 / 收起一条策略的明细
fn expand_btn(ui: &mut egui::Ui, rect: egui::Rect, id: egui::Id, open: bool) -> bool {
    let resp = ui.interact(rect, id, egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(id.with("hov"), resp.hovered(), 0.10);
    if hov > 0.0 {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            // 淡入别用 mix(TRANSPARENT, ..)：alpha 被一起插值再预乘，中段会闪灰
            sk().btn_hover.gamma_multiply(hov),
        );
    }
    let col = mix(sk().txt3, sk().txt, hov);
    icon_paint(
        ui,
        egui::Rect::from_center_size(rect.center(), egui::vec2(15.0, 15.0)),
        if open { "chev_up" } else { "chev" },
        col,
    );
    resp.clicked()
}

fn toggle_open(panel: &mut PanelState, key: &str) {
    if !panel.plans_open.remove(key) {
        panel.plans_open.insert(key.to_string());
    }
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
        // 淡入别用 mix(TRANSPARENT, ..)：alpha 被一起插值再预乘，中段会闪灰
        // （实测采样 BCBDBD→D1D2D2→F9FAFB = 闪两帧深灰；gamma_multiply 才是真淡入）
        sk().card_hover.gamma_multiply(hov),
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

/// 字号档位的步进与范围（与卡片一致：0.05 一档，0.50–1.40）
const FONT_MIN: f32 = 0.5;
const FONT_MAX: f32 = 1.4;
const FONT_DEF: f32 = 0.7;
const FONT_STEP: f32 = 0.05;
const STAT_H: f32 = 72.0;

/// 字号步进：dir = +1 / -1，结果按两位小数取整再钳到 [0.50, 1.40]
fn font_step(cur: f32, dir: i32) -> f32 {
    let v = ((cur + dir as f32 * FONT_STEP) * 100.0).round() / 100.0;
    v.clamp(FONT_MIN, FONT_MAX)
}

/// 策略分组与计数：(今天生效且没关的条数, 被永久关闭的条数, 未来排期的条数)
fn plan_counts(rows: &[PlanRow]) -> (usize, usize, usize) {
    let mut now = 0;
    let mut off = 0;
    let mut future = 0;
    for r in rows {
        if r.off {
            off += 1;
        }
        match r.state {
            RangeState::Active if !r.off => now += 1,
            RangeState::Future => future += 1,
            _ => {}
        }
    }
    (now, off, future)
}

/// 板块标题（图标 + 15px 加粗）
fn h4(ui: &mut egui::Ui, icon: &str, title: &str) {
    ui.horizontal(|ui| {
        let (ir, _) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::hover());
        icon_paint(ui, ir, icon, sk().icon);
        ui.add_space(7.0);
        ui.label(RichText::new(title).size(15.0).strong().color(sk().txt));
    });
    ui.add_space(6.0);
}

/// 板块导语（会按可用宽度折行）
fn lead(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(text).size(13.0).color(sk().txt2),
    ));
    ui.add_space(14.0);
}

fn mini(ui: &mut egui::Ui, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(text).size(12.5).color(sk().txt3),
    ));
    ui.add_space(6.0);
}

fn group_head(ui: &mut egui::Ui, title: &str, count: &str, note: Option<&str>) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(13.5).strong().color(sk().txt));
        if !count.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new(count).size(12.0).color(sk().txt3));
        }
        if let Some(n) = note {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(n).size(12.0).color(sk().txt3));
            });
        }
    });
    ui.add_space(6.0);
}

/// 一列行的外框（预览页的 `.rows`：1px 线、直角、白底）
fn rows_box(ui: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui)) {
    let w = ui.available_width();
    egui::Frame::NONE
        .fill(sk().card)
        .stroke(Stroke::new(1.0, sk().line))
        .inner_margin(egui::Margin::same(0))
        .show(ui, |ui| {
            ui.set_width((w - 2.0).max(80.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            f(ui);
        });
    ui.add_space(6.0);
}

fn empty_line(ui: &mut egui::Ui, msg: &str) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 52.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, sk().card);
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, sk().line),
        StrokeKind::Inside,
    );
    p.text(
        egui::pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        msg,
        FontId::proportional(13.0),
        sk().txt3,
    );
    ui.add_space(6.0);
}

enum StatKind {
    Plain,
    Acc,
    Warn,
}

/// 统计格（等分一行，数字 22px）
fn stats(ui: &mut egui::Ui, items: &[(&str, String, StatKind)]) {
    let gap = 10.0;
    let n = items.len().max(1) as f32;
    let w = ((ui.available_width() - gap * (n - 1.0)) / n).max(90.0);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), STAT_H),
        egui::Sense::hover(),
    );
    let p = ui.painter().clone();
    for (i, (k, v, kind)) in items.iter().enumerate() {
        let r = egui::Rect::from_min_size(
            egui::pos2(rect.left() + i as f32 * (w + gap), rect.top()),
            egui::vec2(w, STAT_H),
        );
        p.rect_filled(r, CornerRadius::ZERO, sk().card);
        p.rect_stroke(
            r,
            CornerRadius::ZERO,
            Stroke::new(1.0, sk().line),
            StrokeKind::Inside,
        );
        p.text(
            egui::pos2(r.left() + 14.0, r.top() + 20.0),
            Align2::LEFT_CENTER,
            *k,
            FontId::proportional(12.0),
            sk().txt3,
        );
        let col = match kind {
            StatKind::Acc => sk().acc,
            StatKind::Warn => sk().danger,
            StatKind::Plain => sk().txt,
        };
        p.text(
            egui::pos2(r.left() + 14.0, r.top() + 47.0),
            Align2::LEFT_CENTER,
            v,
            FontId::proportional(22.0),
            col,
        );
    }
    ui.add_space(6.0);
}

/// 小标签的外框（`dashed` = 来源文件用的虚线框）
fn chip_frame(p: &egui::Painter, rect: egui::Rect, border: Color32, dashed: bool) {
    if dashed {
        let pts = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ];
        for s in egui::Shape::dashed_line(&pts, Stroke::new(1.0, border), 4.0, 3.0) {
            p.add(s);
        }
    } else {
        p.rect_stroke(
            rect,
            CornerRadius::ZERO,
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
    }
}

fn dashed_line(p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, color: Color32) {
    for s in egui::Shape::dashed_line(&[a, b], Stroke::new(1.0, color), 4.0, 4.0) {
        p.add(s);
    }
}

/// 红框警告（预览页的 `.callout`）
fn callout(ui: &mut egui::Ui, text: &str) {
    let inner_w = (ui.available_width() - 28.0).max(80.0);
    let g = ui.painter().layout(
        text.to_string(),
        FontId::proportional(12.5),
        sk().txt2,
        inner_w,
    );
    let h = g.size().y + 24.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
    let p = ui.painter();
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, sk().danger),
        StrokeKind::Inside,
    );
    p.galley(
        egui::pos2(rect.left() + 14.0, rect.top() + 12.0),
        g,
        sk().txt2,
    );
}

fn sep_line(ui: &mut egui::Ui) {
    let (r, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 0.0, sk().line);
    ui.add_space(10.0);
}

/// 设置页的一行（标题 + 说明 + 右侧 P3/P2 小标签）
fn sett_line(ui: &mut egui::Ui, title: &str, desc: &str, pill: Option<&str>) {
    let h = 58.0;
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 1.0)),
        0.0,
        sk().line,
    );
    p.text(
        egui::pos2(rect.left(), rect.top() + 21.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(14.0),
        sk().txt,
    );
    p.text(
        egui::pos2(rect.left(), rect.top() + 41.0),
        Align2::LEFT_CENTER,
        desc,
        FontId::proportional(12.5),
        sk().txt3,
    );
    if let Some(t) = pill {
        let g = p.layout_no_wrap(t.to_string(), FontId::proportional(11.5), sk().txt3);
        let w = g.size().x + 14.0;
        let pr = egui::Rect::from_center_size(
            egui::pos2(rect.right() - w / 2.0, rect.center().y),
            egui::vec2(w, 22.0),
        );
        p.rect_stroke(
            pr,
            CornerRadius::ZERO,
            Stroke::new(1.0, sk().line2),
            StrokeKind::Inside,
        );
        p.galley(
            egui::pos2(pr.left() + 7.0, rect.center().y - g.size().y / 2.0),
            g,
            sk().txt3,
        );
    }
}

/// 卡片层级的一档（整卡可点，选中 = 绿框 + 「当前」标签）
fn layer_option(ui: &mut egui::Ui, w: f32, title: &str, disc: &str, on: bool) -> bool {
    let pw = (w - 28.0).max(80.0);
    let g = ui
        .painter()
        .layout(title.to_string(), FontId::proportional(14.0), sk().txt, pw);
    let dg = ui
        .painter()
        .layout(disc.to_string(), FontId::proportional(12.5), sk().txt2, pw);
    let h = 12.0 + g.size().y + 6.0 + dg.size().y + 14.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.10);
    let p = ui.painter().clone();
    p.rect_filled(
        rect,
        CornerRadius::ZERO,
        mix(sk().card, sk().card_hover, hov),
    );
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, if on { sk().acc } else { sk().line2 }),
        StrokeKind::Inside,
    );
    if on {
        p.rect_stroke(
            rect.shrink(1.0),
            CornerRadius::ZERO,
            Stroke::new(1.0, sk().acc),
            StrokeKind::Inside,
        );
    }
    p.galley(
        egui::pos2(rect.left() + 14.0, rect.top() + 12.0),
        g.clone(),
        sk().txt,
    );
    if on {
        let bw = 34.0;
        let br = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 14.0 + g.size().x + 8.0, rect.top() + 12.0),
            egui::vec2(bw, g.size().y),
        );
        p.rect_filled(br, CornerRadius::ZERO, sk().acc);
        p.text(
            br.center(),
            Align2::CENTER_CENTER,
            "当前",
            FontId::proportional(11.0),
            if sk().is_dark {
                Color32::from_rgb(0x14, 0x14, 0x14)
            } else {
                Color32::WHITE
            },
        );
    }
    p.galley(
        egui::pos2(rect.left() + 14.0, rect.top() + 12.0 + g.size().y + 6.0),
        dg,
        sk().txt2,
    );
    resp.clicked()
}

/// 主题缩略图（上色带 + 下强调条；选中 = 2px 绿内框）
fn theme_thumb(ui: &mut egui::Ui, kind: ThemeKind, on: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(44.0, 34.0), egui::Sense::click());
    let bg = match kind {
        ThemeKind::Wuling => Color32::from_rgb(0xE9, 0xF2, 0xF0),
        ThemeKind::Yellow => Color32::from_rgb(0x0B, 0x0B, 0x0D),
        ThemeKind::Plain => Color32::from_rgb(0xF4, 0xF5, 0xF6),
    };
    let acc = match kind {
        ThemeKind::Yellow => Color32::from_rgb(0xF5, 0xD0, 0x1A),
        _ => Color32::from_rgb(0x66, 0x8D, 0x82),
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, bg);
    p.rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 9.0)),
        0.0,
        mix(bg, Color32::BLACK, 0.14),
    );
    p.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.left() + 5.0, rect.bottom() - 6.0),
            egui::pos2(rect.right() - 5.0, rect.bottom() - 3.0),
        ),
        0.0,
        acc,
    );
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, sk().line2),
        StrokeKind::Inside,
    );
    if on {
        p.rect_stroke(
            rect.shrink(1.0),
            CornerRadius::ZERO,
            Stroke::new(2.0, sk().acc),
            StrokeKind::Inside,
        );
    }
    let resp = resp.on_hover_text(kind.label());
    resp.clicked()
}

/// 内容来源的两档按钮（选中 = 绿底白字）
fn mode_btn(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let font = FontId::proportional(13.5);
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
        mix(sk().card, sk().card_hover, hov)
    };
    let txt = if on {
        sk().acc_dark
    } else {
        mix(sk().txt2, sk().txt, hov)
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, bg);
    p.rect_stroke(
        rect,
        CornerRadius::ZERO,
        Stroke::new(1.0, if on { sk().acc } else { sk().line2 }),
        StrokeKind::Inside,
    );
    p.text(rect.center(), Align2::CENTER_CENTER, label, font, txt);
    resp.clicked()
}

/// 安静按钮（无边框，用于「重播动画」「复位 0.70」）
fn quiet_btn(ui: &mut egui::Ui, icon: &str, label: &str, enabled: bool) -> bool {
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, sk().txt2);
    let icon_w = if icon.is_empty() { 0.0 } else { 20.0 };
    let w = (galley.size().x + icon_w + 20.0).max(34.0);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 34.0), egui::Sense::click());
    let hov = if enabled {
        ui.ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), 0.10)
    } else {
        0.0
    };
    if hov > 0.0 {
        ui.painter().rect_filled(
            rect,
            CornerRadius::ZERO,
            // 淡入别用 mix(TRANSPARENT, ..)：alpha 被一起插值再预乘，中段会闪灰
            sk().btn_hover.gamma_multiply(hov),
        );
    }
    let txt = if enabled {
        mix(sk().txt2, sk().txt, hov)
    } else {
        sk().txt3
    };
    let total = icon_w + galley.size().x;
    let mut x = rect.center().x - total / 2.0;
    if !icon.is_empty() {
        let ir = egui::Rect::from_center_size(
            egui::pos2(x + 7.5, rect.center().y),
            egui::vec2(15.0, 15.0),
        );
        icon_paint(ui, ir, icon, txt);
        x += icon_w;
    }
    ui.painter().galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        txt,
    );
    enabled && resp.clicked()
}

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

    fn range(from: &str, to: &str) -> Range {
        Range {
            from: from.into(),
            to: to.into(),
        }
    }

    fn plan(
        word: &str,
        from: &str,
        to: &str,
        state: RangeState,
        off: bool,
        hide: &[&str],
        expanded: bool,
    ) -> PlanRow {
        PlanRow {
            word: word.into(),
            range: range(from, to),
            state,
            off,
            source: "示例词表 · Unit 1".into(),
            hide: hide.iter().map(|s| s.to_string()).collect(),
            expanded,
            why: "今天生效".into(),
        }
    }

    fn card(word: &str, hide: &[&str]) -> CardRow {
        CardRow {
            word: word.into(),
            range_label: "2026-10-06 → 2026-10-12".into(),
            source: "示例词表 · Unit 1".into(),
            hide: hide.iter().map(|s| s.to_string()).collect(),
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

    /// 在离屏 Context 里跑 `f` 两帧，第二帧在 `pos` 按下再抬起（模拟拖屏点击）。
    /// 交互要靠**上一帧**登记过的控件矩形命中，所以必须先空跑一帧。
    fn run_with_click(
        size: egui::Vec2,
        pos: egui::Pos2,
        mut f: impl FnMut(&mut egui::Ui, &mut ControlResult),
    ) -> ControlResult {
        let ctx = egui::Context::default();
        let raw = |ev: Vec<egui::Event>, t: f64| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size)),
            time: Some(t),
            events: ev,
            ..Default::default()
        };
        let mut first = ctx.run_ui(raw(Vec::new(), 0.0), |ui| {
            let mut r = ControlResult::default();
            f(ui, &mut r);
        });
        first.textures_delta.clear();
        let ev = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            },
        ];
        let out = std::cell::RefCell::new(ControlResult::default());
        let mut second = ctx.run_ui(raw(ev, 0.5), |ui| {
            let mut r = ControlResult::default();
            f(ui, &mut r);
            *out.borrow_mut() = r;
        });
        second.textures_delta.clear();
        out.into_inner()
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
            screen_auto: true,
            today: "2026-10-06",
            status: "",
            warns,
            import,
            confirm_exit: false,
        }
    }

    /// 策略三段的计数：今天生效（且没关）/ 被永久关闭 / 未来排期
    #[test]
    fn plan_counts_group_and_count() {
        let rows = vec![
            plan(
                "seek",
                "2026-10-06",
                "2026-10-12",
                RangeState::Active,
                false,
                &[],
                false,
            ),
            plan(
                "deep",
                "2026-10-06",
                "2026-10-12",
                RangeState::Active,
                true,
                &[],
                false,
            ),
            plan(
                "gain",
                "2026-10-13",
                "2026-10-19",
                RangeState::Future,
                false,
                &[],
                false,
            ),
            plan(
                "form",
                "2026-09-01",
                "2026-09-07",
                RangeState::Past,
                false,
                &[],
                false,
            ),
        ];
        // deep 是"今天生效但被关掉" → 不算今日生效，但要算被永久关闭
        assert_eq!(plan_counts(&rows), (1, 1, 1));
        assert_eq!(plan_counts(&[]), (0, 0, 0));
    }

    /// 字号步进：0.05 一档、两位小数、钳在 0.50–1.40
    #[test]
    fn font_step_steps_and_clamps() {
        assert!((font_step(0.70, 1) - 0.75).abs() < 1e-6);
        assert!((font_step(0.70, -1) - 0.65).abs() < 1e-6);
        // 浮点误差不能累积出 0.7500001 这种档位
        assert!((font_step(0.75, 1) - 0.80).abs() < 1e-6);
        assert!((font_step(1.35, 1) - 1.40).abs() < 1e-6);
        assert!((font_step(1.40, 1) - 1.40).abs() < 1e-6, "上限 1.40");
        assert!((font_step(0.50, -1) - 0.50).abs() < 1e-6, "下限 0.50");
        assert!((font_step(0.52, -1) - 0.50).abs() < 1e-6);
    }

    /// 层级两档：置顶必然配鼠标穿透（设计 §5.2），置底一定不穿透
    #[test]
    fn layer_pair_maps_top_to_passthrough() {
        assert_eq!(layer_pair(true), (Layer::Top, true));
        assert_eq!(layer_pair(false), (Layer::Bottom, false));
    }

    /// 切层级要写回 cfg 并让 app 去发 WindowLevel / MousePassthrough
    #[test]
    fn set_layer_writes_cfg_and_reports_top_changed() {
        let mut cfg = Config::default();
        let mut r = ControlResult::default();
        set_layer(&mut cfg, true, &mut r);
        assert_eq!(cfg.layer, Layer::Top);
        assert!(cfg.passthrough);
        assert!(r.changed && r.top_changed);
        set_layer(&mut cfg, false, &mut r);
        assert_eq!(cfg.layer, Layer::Bottom);
        assert!(!cfg.passthrough);
        assert!(r.top_changed);
    }

    /// 策略行右侧的开关真的会回传「永久关闭这一条」
    #[test]
    fn plan_switch_click_reports_toggle_off() {
        let p = plan(
            "seek",
            "2026-10-06",
            "2026-10-12",
            RangeState::Active,
            false,
            &[],
            false,
        );
        let size = egui::vec2(600.0, PLAN_L1_H);
        let mut panel = PanelState::default();
        // 开关位置由版式决定：右起 = 展开箭头(34) + 12 + 开关(52)
        let sw_left = size.x - PLAN_PAD_R - EXPAND_W - 12.0 - SWITCH_W;
        let pos = egui::pos2(sw_left + SWITCH_W / 2.0, size.y / 2.0);
        let res = run_with_click(size, pos, |ui, r| plan_row(ui, &p, &mut panel, r));
        assert_eq!(
            res.toggle_off,
            Some(("seek".to_string(), range("2026-10-06", "2026-10-12"))),
            "点开关必须回传这一条策略"
        );
        assert!(panel.plans_open.is_empty(), "点开关不该连带展开整行");
    }

    /// 点行本体（不是开关）展开明细；开关和展开互不打架
    #[test]
    fn plan_row_click_expands_only() {
        let p = plan(
            "seek",
            "2026-10-06",
            "2026-10-12",
            RangeState::Active,
            false,
            &["sentences"],
            false,
        );
        let size = egui::vec2(600.0, PLAN_L1_H);
        let mut panel = PanelState::default();
        let res = run_with_click(size, egui::pos2(40.0, 26.0), |ui, r| {
            plan_row(ui, &p, &mut panel, r)
        });
        assert_eq!(res.toggle_off, None, "点词不该关掉策略");
        assert!(panel.plans_open.contains(&p.key()), "点行本体应展开");
    }

    /// 层级两档的卡片：整卡可点，选中态由 app 写回 cfg
    #[test]
    fn layer_option_click_reports_choice() {
        let size = egui::vec2(280.0, 200.0);
        let res = run_with_click(size, egui::pos2(140.0, 40.0), |ui, r| {
            if layer_option(ui, 280.0, "置顶 ＋ 鼠标穿透", "卡片始终在最上层", false)
            {
                let (layer, passthrough) = layer_pair(true);
                assert_eq!((layer, passthrough), (Layer::Top, true));
                r.changed = true;
                r.top_changed = true;
            }
        });
        assert!(res.top_changed && res.changed, "点「置顶」要回传层级切换");
    }

    /// 设置页「关闭窗口时」两档（P3a）：画布开高一点让整页可见，
    /// 在右半列逐行扫点击——点「直接退出」档位必须回传 set_close_action。
    /// （扫描而不是写死坐标：控件高度由文案换行数决定，写死坐标一改文案就假红。）
    #[test]
    fn settings_close_action_reports_choice() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let cx = base_ctx(&import, &[], &[], &warns);
        let rows: Vec<WordRow> = Vec::new();
        let size = egui::vec2(1004.0, 1400.0);
        let ctx = egui::Context::default();
        let raw = |ev: Vec<egui::Event>, t: f64| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size)),
            time: Some(t),
            events: ev,
            ..Default::default()
        };
        let mut cfg = Config::default();
        let mut panel = PanelState {
            tab: PanelTab::Settings,
            ..PanelState::default()
        };
        // 第一帧：登记控件矩形（交互命中靠上一帧）
        let mut first = ctx.run_ui(raw(Vec::new(), 0.0), |ui| {
            let _ = draw(ui, &mut cfg, &mut panel, &cx, &rows);
        });
        first.textures_delta.clear();

        let mut found = false;
        let mut t = 0.5;
        for y in (300..1380).step_by(3) {
            let pos = egui::pos2(460.0, y as f32);
            let ev = vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ];
            t += 0.05;
            let out = std::cell::RefCell::new(ControlResult::default());
            let mut f = ctx.run_ui(raw(ev, t), |ui| {
                *out.borrow_mut() = draw(ui, &mut cfg, &mut panel, &cx, &rows);
            });
            f.textures_delta.clear();
            if out.into_inner().set_close_action == Some(CloseAction::Exit) {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "点设置页「直接退出」档位应回传 set_close_action = Some(Exit)"
        );
    }

    /// 四个板块都能画出来，且都不是空白
    #[test]
    fn all_tabs_render_something() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let cards = vec![card("seek", &["note"])];
        let plans = vec![
            plan(
                "seek",
                "2026-10-06",
                "2026-10-12",
                RangeState::Active,
                false,
                &[],
                false,
            ),
            plan(
                "gain",
                "2026-10-13",
                "2026-10-19",
                RangeState::Future,
                false,
                &["sentences"],
                false,
            ),
            plan(
                "form",
                "2026-09-29",
                "2026-10-05",
                RangeState::Past,
                false,
                &["note"],
                false,
            ),
        ];
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

    /// 策略页的三段分组：展开「已经结束」才画那些行
    #[test]
    fn plans_past_group_is_collapsed_by_default() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let plans = vec![
            plan(
                "seek",
                "2026-10-06",
                "2026-10-12",
                RangeState::Active,
                false,
                &[],
                false,
            ),
            plan(
                "form",
                "2026-09-29",
                "2026-10-05",
                RangeState::Past,
                false,
                &["note"],
                false,
            ),
        ];
        let cx = base_ctx(&import, &[], &plans, &warns);
        let rows: Vec<WordRow> = Vec::new();
        let mut cfg = Config::default();

        let mut closed = PanelState::default();
        let n_closed = render(PanelTab::Plans, &cx, &rows, &mut cfg, &mut closed);
        let mut opened = PanelState {
            past_open: true,
            ..PanelState::default()
        };
        let n_open = render(PanelTab::Plans, &cx, &rows, &mut cfg, &mut opened);
        assert!(
            n_open > n_closed + 20,
            "展开「已经结束」应多画出行来：收起 {n_closed} vs 展开 {n_open}"
        );
    }

    /// 展开一条策略要多画 4 行明细
    #[test]
    fn plan_row_expanded_draws_details() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let mut cfg = Config::default();
        let rows: Vec<WordRow> = Vec::new();

        let closed_row = plan(
            "seek",
            "2026-10-06",
            "2026-10-12",
            RangeState::Active,
            false,
            &["sentences"],
            false,
        );
        let open_row = plan(
            "seek",
            "2026-10-06",
            "2026-10-12",
            RangeState::Active,
            false,
            &["sentences"],
            true,
        );

        let a = vec![closed_row];
        let cx_a = base_ctx(&import, &[], &a, &warns);
        let mut panel = PanelState::default();
        let closed = render(PanelTab::Plans, &cx_a, &rows, &mut cfg, &mut panel);

        let b = vec![open_row];
        let cx_b = base_ctx(&import, &[], &b, &warns);
        let mut panel2 = PanelState::default();
        let open = render(PanelTab::Plans, &cx_b, &rows, &mut cfg, &mut panel2);
        assert!(open > closed + 20, "展开应多画明细：{closed} → {open}");
    }

    /// 卡片页的卡片清单：有卡片要多画，且「隐藏/完整」两种标签都能画
    #[test]
    fn cards_list_draws_rows() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let mut cfg = Config::default();
        let rows: Vec<WordRow> = Vec::new();

        let none: Vec<CardRow> = Vec::new();
        let cx0 = base_ctx(&import, &none, &[], &warns);
        let mut panel = PanelState::default();
        let empty = render(PanelTab::Cards, &cx0, &rows, &mut cfg, &mut panel);

        let some = vec![card("seek", &["note"]), card("gain", &[])];
        let cx1 = base_ctx(&import, &some, &[], &warns);
        let mut panel2 = PanelState::default();
        let filled = render(PanelTab::Cards, &cx1, &rows, &mut cfg, &mut panel2);
        assert!(
            filled > empty + 20,
            "卡片行没画出来：空 {empty} vs 两行 {filled}"
        );
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

    /// 空词表 / 空策略都不能崩（首次运行、导入前）
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
        for tab in [
            PanelTab::Words,
            PanelTab::Cards,
            PanelTab::Plans,
            PanelTab::Settings,
        ] {
            let mut panel = PanelState::default();
            let n = render(tab, &cx, &[], &mut cfg, &mut panel);
            assert!(n > 40, "空库下 {tab:?} 几乎没画东西（{n}）");
        }
    }

    /// 冲突弹窗、格式错误弹窗都要能画
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

    /// 窄窗口（最小尺寸 720×460）下四个板块都要画得出来，不能越界 panic
    #[test]
    fn narrow_window_renders() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = vec!["词条「x」缺少 senses，已跳过".into()];
        let import = import_ctx(&errors, None, "");
        let cards = vec![card("seek", &["note"])];
        let plans = vec![
            plan(
                "seek",
                "2026-10-06",
                "2026-10-12",
                RangeState::Active,
                false,
                &["sentences", "note"],
                true,
            ),
            plan(
                "form",
                "2026-09-29",
                "2026-10-05",
                RangeState::Past,
                true,
                &[],
                false,
            ),
        ];
        let cx = base_ctx(&import, &cards, &plans, &warns);
        let rows = vec![row("seek", true)];
        let mut cfg = Config::default();
        for tab in [
            PanelTab::Words,
            PanelTab::Cards,
            PanelTab::Plans,
            PanelTab::Settings,
        ] {
            let mut panel = PanelState::default();
            let n = render_at(
                tab,
                egui::vec2(720.0, 460.0),
                &cx,
                &rows,
                &mut cfg,
                &mut panel,
            );
            assert!(n > 20, "窄窗口下 {tab:?} 几乎没画出东西（{n}）");
        }
    }

    /// 极窄 + 超长词：第一行的「词 + 日期段」不许重叠（各画各的区间，不越界）
    #[test]
    fn plan_row_survives_tiny_width() {
        let errors: Vec<String> = Vec::new();
        let warns: Vec<String> = Vec::new();
        let import = import_ctx(&errors, None, "");
        let plans = vec![plan(
            "Good habits are hard to form but easy to live with.",
            "2026-10-06",
            "2026-10-12",
            RangeState::Active,
            false,
            &["note", "sentences"],
            true,
        )];
        let cx = base_ctx(&import, &[], &plans, &warns);
        let rows: Vec<WordRow> = Vec::new();
        let mut cfg = Config::default();
        let mut panel = PanelState::default();
        let n = render_at(
            PanelTab::Plans,
            egui::vec2(320.0, 460.0),
            &cx,
            &rows,
            &mut cfg,
            &mut panel,
        );
        assert!(n > 20, "极窄窗口下策略行没画出来（{n}）");
    }
}
