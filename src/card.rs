//! 词卡版式引擎（v1.0 规范：区块流式堆叠、无标签、细分隔线、高度随内容）。
//!
//! 一次算好排版计划（文本行 + 分隔线 + 总高），悬浮窗据此设定窗口大小；
//! 每帧只负责把计划画出来 —— 测量与绘制永远一致。
//! 所有尺寸都按参考尺寸 × 缩放系数 k。
//!
//! 区块顺序：① 词头（+变形小字）② 词义 ③ 同根词 ④ 短语 ⑤ 例句 ⑥ 备注
//! 写了**并且**没被 `hide` 关掉，才会出现。

use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Galley, Pos2, Rect};

use crate::deck::{hide_label, Entry};
use crate::theme::Theme;

/// 参考卡片宽度（实际 = CARD_W × k）
pub const CARD_W: f32 = 640.0;
/// 右侧色条宽（参考尺寸）
pub const BAR_W: f32 = 14.1;

const PAD_L: f32 = 20.0;
const PAD_R: f32 = 8.0;

/// 版式尺寸表（参考尺寸；绘制时全部 × k）
mod sz {
    /// 主体字号候选（依次尝试，保证单行）
    pub const WORD: [f32; 4] = [60.0, 48.0, 40.0, 34.0];
    /// 超长主体（句子卡）折行时用的字号
    pub const WORD_WRAP: f32 = 38.0;
    pub const FORMS: f32 = 30.0;
    pub const SENSE: f32 = 28.0;
    pub const DERIV: f32 = 26.0;
    pub const DERIV_ZH: f32 = 23.0;
    pub const PHRASE: f32 = 26.0;
    pub const PHRASE_ZH: f32 = 23.0;
    pub const SEN: f32 = 26.0;
    pub const SEN_ZH: f32 = 22.0;
    pub const NOTE: f32 = 20.0;
    pub const B1_TOP: f32 = 12.0;
    pub const B1_BOTTOM: f32 = 8.0;
    pub const B23_TOP: f32 = 8.0;
    pub const B2_BOTTOM: f32 = 10.0;
    pub const B3_BOTTOM: f32 = 14.0;
}

/// 一张卡的排版计划
pub struct CardPlan {
    /// 卡片总高（已 × k）
    pub height: f32,
    /// (左上角, 文本)
    pub lines: Vec<(Pos2, Arc<Galley>)>,
    /// 区块之间的细分隔线
    pub seps: Vec<Rect>,
}

/// 排一张卡。区块为空或被 hide 关掉 → 直接不出现。
pub fn plan(ctx: &egui::Context, e: &Entry, th: &Theme, k: f32) -> CardPlan {
    let mut lines: Vec<(Pos2, Arc<Galley>)> = Vec::new();
    let mut seps: Vec<Rect> = Vec::new();

    let x0 = PAD_L * k;
    let content_w = (CARD_W - BAR_W - PAD_L - PAD_R) * k;
    let right = x0 + content_w;

    // 文本排版走 Painter（内部自动处理字体缓存锁，&self 即可用）
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("lexideck-card-plan"),
    ));
    let lay = |text: &str, size: f32, color: Color32, wrap_w: f32| -> Arc<Galley> {
        painter.layout(
            text.to_owned(),
            FontId::proportional(size * k),
            color,
            wrap_w,
        )
    };
    // 一行「英文 + 可选中文」：放得下就同行，放不下分两行
    let mut pair_row = |y: &mut f32,
                        lines: &mut Vec<(Pos2, Arc<Galley>)>,
                        left: &str,
                        right_txt: &str,
                        size_en: f32,
                        size_zh: f32,
                        color_en: Color32| {
        let eg = lay(left, size_en, color_en, f32::INFINITY);
        let ew = eg.size().x;
        let eh = eg.size().y;
        let zh = right_txt.trim();
        if zh.is_empty() {
            lines.push((egui::pos2(x0, *y), eg));
            *y += eh;
            return;
        }
        let zg = lay(zh, size_zh, th.fg3, f32::INFINITY);
        let zw = zg.size().x;
        let zh_h = zg.size().y;
        if ew + 10.0 * k + zw <= content_w {
            lines.push((egui::pos2(x0, *y), eg));
            lines.push((egui::pos2(x0 + ew + 10.0 * k, *y + eh - zh_h), zg));
            *y += eh.max(zh_h);
        } else {
            lines.push((egui::pos2(x0, *y), eg));
            *y += eh + 1.0 * k;
            lines.push((egui::pos2(x0, *y), zg));
            *y += zh_h;
        }
    };

    let mut y = sz::B1_TOP * k;

    // ── ① 词头：主体大字（保证单行）+ 变形小字跟在后面 ──
    let word = e.word.trim();
    let forms = if e.hidden("forms") {
        None
    } else {
        e.forms_line()
    };
    let forms_g = forms
        .as_deref()
        .map(|t| lay(t, sz::FORMS, th.accent, f32::INFINITY));
    let forms_w = forms_g
        .as_ref()
        .map(|g| g.size().x + 14.0 * k)
        .unwrap_or(0.0);

    let mut word_g = lay(word, *sz::WORD.last().unwrap(), th.fg, f32::INFINITY);
    for s in sz::WORD {
        let g = lay(word, s, th.fg, f32::INFINITY);
        if g.size().x + forms_w <= content_w {
            word_g = g;
            break;
        }
    }
    let mut wrapped = false;
    if word_g.size().x + forms_w > content_w {
        // 超长主体（句子卡）：折行显示
        word_g = lay(word, sz::WORD_WRAP, th.fg, content_w);
        wrapped = true;
    }
    let word_w = if wrapped { content_w } else { word_g.size().x };
    let word_h = word_g.size().y;
    lines.push((egui::pos2(x0, y), word_g.clone()));
    y += word_h;
    if let Some(fg) = &forms_g {
        let fh = fg.size().y;
        if !wrapped && word_w + forms_w <= content_w {
            // 同行（底部对齐）
            lines.push((egui::pos2(x0 + word_w + 14.0 * k, y - fh), fg.clone()));
        } else {
            y += 4.0 * k;
            lines.push((egui::pos2(x0, y), fg.clone()));
            y += fh;
        }
    }

    // ── ② 词义（词性 + 意思，紧贴主体下方） ──
    if !e.senses.is_empty() && !e.hidden("senses") {
        y += 6.0 * k;
        for (i, s) in e.senses.iter().enumerate() {
            if i > 0 {
                y += 2.0 * k;
            }
            let pos = s.pos.trim();
            if !pos.is_empty() {
                let pg = lay(pos, sz::SENSE, th.accent, f32::INFINITY);
                let pw = pg.size().x;
                let ph = pg.size().y;
                lines.push((egui::pos2(x0, y), pg));
                let mx = x0 + pw + 8.0 * k;
                let avail = (right - mx).max(40.0 * k);
                let mg = lay(s.meaning.trim(), sz::SENSE, th.fg, avail);
                let mh = mg.size().y;
                lines.push((egui::pos2(mx, y), mg));
                y += ph.max(mh);
            } else {
                let mg = lay(s.meaning.trim(), sz::SENSE, th.fg, content_w);
                let mh = mg.size().y;
                lines.push((egui::pos2(x0, y), mg));
                y += mh;
            }
        }
    }
    y += sz::B1_BOTTOM * k;

    // ── ③ 同根词（如 seeker  n. 探索者） ──
    if !e.derivations.is_empty() && !e.hidden("derivations") {
        seps.push(sep_rect(x0, right, y));
        y += sz::B23_TOP * k;
        for (i, d) in e.derivations.iter().enumerate() {
            if i > 0 {
                y += 2.0 * k;
            }
            let dw = lay(d.word.trim(), sz::DERIV, th.accent, f32::INFINITY);
            let dw_w = dw.size().x;
            let dw_h = dw.size().y;
            lines.push((egui::pos2(x0, y), dw));
            let meaning = d
                .senses
                .iter()
                .map(|s| {
                    let p = s.pos.trim();
                    let m = s.meaning.trim();
                    if p.is_empty() {
                        m.to_string()
                    } else {
                        format!("{p} {m}")
                    }
                })
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join("；");
            if meaning.is_empty() {
                y += dw_h;
            } else {
                let mx = x0 + dw_w + 12.0 * k;
                let avail = (right - mx).max(40.0 * k);
                let mg = lay(&meaning, sz::DERIV_ZH, th.fg, avail);
                let mh = mg.size().y;
                lines.push((egui::pos2(mx, y + dw_h - mh), mg));
                y += dw_h.max(mh);
            }
        }
        y += sz::B2_BOTTOM * k;
    }

    // ── ④ 短语 ──
    if !e.phrases.is_empty() && !e.hidden("phrases") {
        seps.push(sep_rect(x0, right, y));
        y += sz::B23_TOP * k;
        for (i, p) in e.phrases.iter().enumerate() {
            if i > 0 {
                y += 2.0 * k;
            }
            pair_row(
                &mut y,
                &mut lines,
                p.text.trim(),
                p.meaning.trim(),
                sz::PHRASE,
                sz::PHRASE_ZH,
                th.fg,
            );
        }
        y += sz::B2_BOTTOM * k;
    }

    // ── ⑤ 例句（英文 + 小一号译文） ──
    if !e.sentences.is_empty() && !e.hidden("sentences") {
        seps.push(sep_rect(x0, right, y));
        y += sz::B23_TOP * k;
        for (i, s) in e.sentences.iter().enumerate() {
            if i > 0 {
                y += 8.0 * k;
            }
            let eg = lay(s.en.trim(), sz::SEN, th.fg, content_w);
            let ehh = eg.size().y;
            lines.push((egui::pos2(x0, y), eg));
            y += ehh;
            let zh = s.zh.trim();
            if !zh.is_empty() {
                y += 1.0 * k;
                let zg = lay(zh, sz::SEN_ZH, th.fg3, content_w);
                let zhh = zg.size().y;
                lines.push((egui::pos2(x0, y), zg));
                y += zhh;
            }
        }
        y += sz::B3_BOTTOM * k;
    }

    // ── ⑥ 备注（小字） ──
    if !e.note.trim().is_empty() && !e.hidden("note") {
        seps.push(sep_rect(x0, right, y));
        y += sz::B23_TOP * k;
        let ng = lay(e.note.trim(), sz::NOTE, th.fg3, content_w);
        let nh = ng.size().y;
        lines.push((egui::pos2(x0, y), ng));
        y += nh + sz::B2_BOTTOM * k;
    }

    CardPlan {
        height: y,
        lines,
        seps,
    }
}

/// 把计划画出来（白底由调用方先铺）
pub fn paint(ui: &egui::Ui, plan: &CardPlan, th: &Theme) {
    let p = ui.painter();
    for s in &plan.seps {
        p.rect_filled(*s, 0.0, th.line);
    }
    for (pos, g) in &plan.lines {
        // 排版时已按主题色 layout，这里的回退色只在字形颜色为占位符时才用得上。
        p.galley(*pos, g.clone(), th.fg);
    }
}

fn sep_rect(x0: f32, x1: f32, y: f32) -> Rect {
    Rect::from_min_max(egui::pos2(x0, y), egui::pos2(x1, y + 1.0))
}

/// 词表列表里「已关闭展示」小标签用的名字（面板复用 deck 的映射）
pub fn hidden_label(key: &str) -> &'static str {
    hide_label(key)
}
