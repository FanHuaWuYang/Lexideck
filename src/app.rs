//! 应用状态机：控制面板（主窗口）+ 多个悬浮窗（独立 viewport）。
//! 数据来自 exe 同目录的主库 `lexideck.json`（见 library 模块）；
//! 卡片内容由「词表里勾选 → 立即展示」决定（P2 起还会由展示时间自动驱动）。

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{
    self,
    viewport::{ViewportCommand, ViewportId, WindowLevel},
};

use crate::anim::Anim;
use crate::card::{self, CardPlan};
use crate::config::{self, Config};
use crate::control::{
    self, CardRow, ConflictChoice, ImportCtx, PanelState, PlanRow, WordFilter, WordRow,
};
use crate::deck::{self, Deck, Entry};
use crate::float;
use crate::import;
use crate::library;
use crate::schedule;
use crate::theme::{self, Theme, ThemeKind};
use crate::window;

/// 一个悬浮窗的运行时状态
pub struct FloatWin {
    pub id: usize,
    /// 这个窗显示哪个词条（小写 word）
    pub word: String,
    pub anim: Anim,
    /// 入场动画的预定开播时刻（批量操作时错峰延时；到点后在窗口自己的帧里启动）。
    /// 窗口从创建到真正出现有几百毫秒的延迟，如果在创建那一刻就计时，
    /// 等画面出来时动画已经跑掉一大截（闪烁几乎看不到）。
    anim_at: Option<Instant>,
    /// 计划是否需要在悬浮窗自己的绘制周期里重建。
    ///
    /// ⚠️ 踩过的坑：`card::plan` 必须**在悬浮窗自己的 pass 里**调用。
    /// 在主窗口那一 pass 里预先排好的 galley 拿到悬浮窗来画，字形 UV 会取错位置：
    /// **行位置是对的，但每个字都是错的形状**。所以计划改为惰性重建。
    plan_dirty: bool,
    pos: Option<egui::Pos2>,
    /// 当前排版计划（词条 / 缩放 / 主题 / 策略内隐藏变化时置脏重建）
    plan: Option<CardPlan>,
    /// 计划对应的键：(词条, 缩放 bits, 主题, 策略内额外隐藏)
    plan_key: Option<(String, u32, ThemeKind, Vec<String>)>,
    /// 这条策略内额外隐藏的区块（来自展示时间项的 `hide`；并进卡片的隐藏判断）
    pub hide_extra: Vec<String>,
    /// 当前目标窗口尺寸
    size: Option<egui::Vec2>,
}

impl FloatWin {
    fn invalidate_plan(&mut self) {
        self.plan = None;
        self.plan_dirty = true;
    }

    pub fn replay_anim(&mut self) {
        if self.anim.is_playing() {
            return;
        }
        self.anim_at = Some(Instant::now());
    }

    pub fn replay_anim_delayed(&mut self, ms: u64) {
        if self.anim.is_playing() {
            return;
        }
        self.anim_at = Some(Instant::now() + Duration::from_millis(ms));
    }

    fn start_anim_if_due(&mut self) {
        if let Some(t) = self.anim_at {
            if Instant::now() >= t {
                self.anim_at = None;
                self.anim.play();
            }
        }
    }
}

/// 一次导入的过程状态（逻辑在 import 模块里，这里只持有）

/// 屏幕上卡片内容的来源（设计 §5.5）：启动时必须是 Auto。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    /// 自动：今天生效的展示策略（去掉被关闭的），按主库顺序
    Auto,
    /// 手动：面板里勾选后点「立即展示」的那几个词
    Manual,
}

pub struct LexideckApp {
    cfg: Config,
    // ── 数据 ──
    lib: Deck,
    lib_path: PathBuf,
    lib_mtime: Option<SystemTime>,
    warns: Vec<String>,
    // ── 词表 UI 状态 ──
    panel: PanelState,
    selected: HashSet<String>,
    expanded: HashSet<String>,
    // ── 卡片 ──
    floats: Vec<FloatWin>,
    next_id: usize,
    /// 屏幕上真正放得下的张数
    shown_fit: usize,
    /// 主库文件存在但读不懂：此时禁止任何写盘，免得把坏文件覆盖掉
    lib_corrupt: bool,
    // ── 展示时间（P2）──
    /// 被永久关闭的展示策略（exe 同目录 `状态.json`）
    sc: schedule::State,
    sc_path: PathBuf,
    /// 状态文件是否可信（读不懂 = false；此时拒绝写盘，绝不覆盖坏文件）
    sc_ok: bool,
    /// 启动时定的"今天"；跨日时更新
    today: schedule::Date,
    /// 当前屏幕上的卡片来源（启动 = Auto）
    screen: Screen,
    /// 退出二次确认的武装时刻（5 秒后自动失效，免得"点过一次以后 × 就裸奔"）
    confirm_at: Option<Instant>,
    // ── 导入 ──
    import: Option<import::Session>,
    import_error: Option<(String, Vec<String>)>,
    status: Option<String>,
    // ── 其它 ──
    last_poll: Instant,
    last_persist: Instant,
    placed: bool,
    applied_theme: Option<ThemeKind>,
    confirm_exit: bool,
}

impl LexideckApp {
    pub fn new(cc: &eframe::CreationContext<'_>, cfg: Config) -> Self {
        setup_fonts(cc);
        let path = library::lib_path();
        let loaded = library::load(&path);
        let (lib, warns, corrupt) = (loaded.deck, loaded.warns, loaded.corrupt);
        // 展示时间引擎：读关闭状态（读不懂 → sc_ok = false，之后拒绝写盘）、
        // 清掉主库里已经不存在的关闭项、定下"今天"
        let sc_path = schedule::state_path();
        let (sc, sc_ok) = schedule::load_state(&sc_path);
        let today = schedule::today();
        let mut app = Self {
            cfg,
            lib,
            lib_path: path,
            lib_mtime: None,
            lib_corrupt: corrupt,
            sc,
            sc_path,
            sc_ok,
            today,
            screen: Screen::Auto, // 启动按今日展示上屏（不再直接用 cfg.shown）
            confirm_at: None,
            warns,
            panel: PanelState::default(),
            selected: HashSet::new(),
            expanded: HashSet::new(),
            floats: Vec::new(),
            next_id: 1,
            shown_fit: 0,
            import: None,
            import_error: None,
            status: None,
            last_poll: Instant::now(),
            last_persist: Instant::now(),
            placed: false,
            applied_theme: None,
            confirm_exit: false,
        };
        app.lib_mtime = library::mtime(&app.lib_path);
        app.prune_selection();
        app.prune_state();
        app
    }

    // ── 主库 ──

    fn reload_library(&mut self) {
        let loaded = library::load(&self.lib_path);
        self.lib = loaded.deck;
        self.lib_corrupt = loaded.corrupt;
        self.warns = loaded.warns;
        self.lib_mtime = library::mtime(&self.lib_path);
        self.prune_selection();
        self.prune_state();
        for f in &mut self.floats {
            f.invalidate_plan();
        }
    }

    /// 主库里已经不存在的关闭项清掉（启动 / 重载 / 导入后）。
    /// 主库读不懂时不动它：否则会拿空库把状态里的关闭项全删掉。
    fn prune_state(&mut self) {
        if self.lib_corrupt {
            return;
        }
        if self.sc.prune(&self.lib) && self.sc_ok {
            let _ = schedule::save_state(&self.sc_path, &self.sc);
        }
    }

    /// 勾选状态里已经不存在的词（导入/重载后）清掉
    fn prune_selection(&mut self) {
        let keys: HashSet<String> = self.lib.words.iter().map(|w| w.key()).collect();
        self.selected.retain(|k| keys.contains(k));
        self.expanded.retain(|k| keys.contains(k));
        let shown: Vec<String> = self
            .cfg
            .shown
            .iter()
            .filter(|k| keys.contains(*k))
            .cloned()
            .collect();
        self.cfg.shown = shown;
    }

    fn save_library(&self) -> Result<(), String> {
        library::save(&self.lib_path, &self.lib)
    }

    /// 每 2 秒看看主库文件是否被外部改过（老师手改 JSON）
    fn poll_library(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(2000) {
            return;
        }
        self.last_poll = Instant::now();
        let m = library::mtime(&self.lib_path);
        if m == self.lib_mtime {
            return;
        }
        // 文件被删/改名也要反应（m 变 None）
        let gone = m.is_none();
        self.reload_library();
        self.status = Some(if gone {
            "主库文件不见了（被删或改名），现在用的是内存里那份".into()
        } else {
            "主库文件被外部改动，已重新读取".into()
        });
    }

    // ── 视图行 ──

    fn entry_by_key(&self, key: &str) -> Option<&Entry> {
        self.lib.words.iter().find(|w| w.key() == key)
    }

    /// 词条按首字母排序后的下标
    fn sorted_indices(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.lib.words.len()).collect();
        idx.sort_by(|a, b| {
            let (x, y) = (&self.lib.words[*a], &self.lib.words[*b]);
            x.key().cmp(&y.key())
        });
        idx
    }

    fn build_word_rows(&self) -> Vec<WordRow> {
        let q = self.panel.search.trim().to_lowercase();
        self.sorted_indices()
            .into_iter()
            .filter_map(|i| {
                let e = &self.lib.words[i];
                let key = e.key();
                let sel = self.selected.contains(&key);
                match self.panel.filter {
                    WordFilter::Selected if !sel => return None,
                    WordFilter::Unselected if sel => return None,
                    _ => {}
                }
                if !q.is_empty() {
                    let hay = format!("{} {}", e.word, e.first_meaning()).to_lowercase();
                    if !hay.contains(&q) {
                        return None;
                    }
                }
                let mut details: Vec<(String, String)> = Vec::new();
                if !e.senses.is_empty() {
                    details.push((
                        "词义".into(),
                        e.senses
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
                            .collect::<Vec<_>>()
                            .join("　｜　"),
                    ));
                }
                if let Some(f) = e.forms_line() {
                    details.push(("变形".into(), f));
                }
                if !e.derivations.is_empty() {
                    details.push((
                        "同根词".into(),
                        e.derivations
                            .iter()
                            .map(|d| {
                                let m = d
                                    .senses
                                    .iter()
                                    .map(|s| {
                                        let p = s.pos.trim();
                                        if p.is_empty() {
                                            s.meaning.trim().to_string()
                                        } else {
                                            format!("{p} {}", s.meaning.trim())
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("；");
                                if m.is_empty() {
                                    d.word.clone()
                                } else {
                                    format!("{} {}", d.word, m)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("　｜　"),
                    ));
                }
                if !e.phrases.is_empty() {
                    details.push((
                        "短语".into(),
                        e.phrases
                            .iter()
                            .map(|p| {
                                let m = p.meaning.trim();
                                if m.is_empty() {
                                    p.text.clone()
                                } else {
                                    format!("{} {}", p.text, m)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("　｜　"),
                    ));
                }
                if !e.sentences.is_empty() {
                    details.push((
                        "例句".into(),
                        e.sentences
                            .iter()
                            .map(|s| {
                                let z = s.zh.trim();
                                if z.is_empty() {
                                    s.en.clone()
                                } else {
                                    format!("{}（{z}）", s.en)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("　｜　"),
                    ));
                }
                if !e.note.trim().is_empty() {
                    details.push(("备注".into(), e.note.clone()));
                }
                Some(WordRow {
                    key: key.clone(),
                    word: e.word.clone(),
                    meaning: e.first_meaning(),
                    details,
                    hidden: e.hide.clone(),
                    selected: sel,
                    expanded: self.expanded.contains(&key),
                })
            })
            .collect()
    }

    fn build_card_rows(&self) -> Vec<CardRow> {
        // 自动模式下把「今天生效的段」和「来源文件」查出来挂到卡片行上；
        // 手动展示没有日期段，来源就写"手动展示"。
        let act = schedule::active_today(&self.lib, self.today, &self.sc);
        self.floats
            .iter()
            .map(|f| {
                let a = act.iter().find(|a| a.word.trim().to_lowercase() == f.word);
                let mut hide = f.hide_extra.clone();
                if let Some(e) = self.entry_by_key(&f.word) {
                    for h in &e.hide {
                        if !hide.contains(h) {
                            hide.push(h.clone());
                        }
                    }
                }
                CardRow {
                    word: self
                        .entry_by_key(&f.word)
                        .map(|e| e.word.clone())
                        .unwrap_or_else(|| f.word.clone()),
                    range_label: a
                        .map(|a| {
                            a.ranges
                                .iter()
                                .map(|r| r.label())
                                .collect::<Vec<_>>()
                                .join("，")
                        })
                        .unwrap_or_default(),
                    source: a
                        .map(|a| a.source.clone())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "手动展示".to_string()),
                    hide,
                }
            })
            .collect()
    }

    /// 策略页的一行 = 主库里一个词的一段日期（主库顺序，先导入的在前）
    fn build_plan_rows(&self) -> Vec<PlanRow> {
        let today = self.today;
        let mut out = Vec::new();
        for it in &self.lib.display_time {
            for r in &it.ranges {
                let state = schedule::range_state(r, today);
                let why = match state {
                    schedule::RangeState::Active => "今天生效".to_string(),
                    schedule::RangeState::Future => {
                        let d = schedule::days_between(
                            today,
                            deck::parse_date(&r.from).unwrap_or(today),
                        );
                        if d <= 1 {
                            "明天开始".to_string()
                        } else {
                            format!("还有 {d} 天开始")
                        }
                    }
                    schedule::RangeState::Past => "已结束".to_string(),
                };
                out.push(PlanRow {
                    word: it.word.clone(),
                    range: r.clone(),
                    state,
                    off: self.sc.is_off(&it.word, r),
                    source: it.source.clone(),
                    hide: it.hide.clone(),
                    expanded: self
                        .panel
                        .plans_open
                        .contains(&control::plan_key(&it.word, r)),
                    why,
                });
            }
        }
        out
    }

    // ── 卡片 ──

    /// 屏幕系数：不同分辨率下卡片相对屏幕的比例保持一致（1080p = 1.0）
    fn screen_scale(ctx: &egui::Context) -> f32 {
        match ctx.input(|i| i.viewport().monitor_size) {
            Some(ms) if ms.x > 0.0 && ms.y > 0.0 => {
                (ms.y / 1080.0).min(ms.x / 1920.0).clamp(0.5, 4.0)
            }
            _ => 1.0,
        }
    }

    fn make_plan(
        ctx: &egui::Context,
        entry: Option<&Entry>,
        th: &Theme,
        scale: f32,
        extra_hide: &[String],
    ) -> (Option<CardPlan>, egui::Vec2) {
        match entry {
            Some(e) => {
                let p = card::plan(ctx, e, th, scale, extra_hide);
                let size = egui::vec2(card::CARD_W * scale, p.height);
                (Some(p), size)
            }
            None => (None, egui::vec2(card::CARD_W * scale, 140.0 * scale)),
        }
    }

    /// 平铺：右上角起、自上而下、满一列向左开新列；放不下就截断。
    /// 返回 (落点, 放得下的张数)
    fn layout_cards(
        sizes: &[egui::Vec2],
        monitor: egui::Vec2,
        scale: f32,
    ) -> (Vec<egui::Pos2>, usize) {
        let gap = 18.0 * scale;
        let margin = 40.0 * scale;
        let top = 48.0 * scale;
        let bottom = 20.0 * scale;
        let w = card::CARD_W * scale;
        let max_cols = (((monitor.x - margin * 2.0 + gap) / (w + gap)).floor()).max(1.0) as usize;
        let mut out: Vec<egui::Pos2> = Vec::new();
        let mut y = top;
        let mut col = 0usize;
        for h in sizes {
            if y + h.y > monitor.y - bottom {
                col += 1;
                y = top;
                // 换到新列的头也放不下（单张就比屏幕高）→ 到此为止，按顺序截断
                if top + h.y > monitor.y - bottom {
                    break;
                }
            }
            if col >= max_cols {
                break;
            }
            let x = monitor.x - margin - w - col as f32 * (w + gap);
            if x < 0.0 {
                break;
            }
            out.push(egui::pos2(x, y));
            y += h.y + gap;
        }
        let n = out.len();
        (out, n)
    }

    /// 让悬浮窗数量 / 内容 / 位置与"当前屏幕来源"保持一致：
    /// Auto = 今日生效的词（带策略内隐藏），Manual = cfg.shown。截断/平铺算法不变。
    fn reconcile_floats(&mut self, ctx: &egui::Context, th: &Theme) {
        let scale = self.cfg.font_scale * Self::screen_scale(ctx);
        let monitor = ctx
            .input(|i| i.viewport().monitor_size)
            .unwrap_or(egui::vec2(1920.0, 1080.0));

        // 要显示的 (词条 key, 策略内额外隐藏)
        let want: Vec<(String, Vec<String>)> = match self.screen {
            Screen::Auto => {
                let mut out: Vec<(String, Vec<String>)> = Vec::new();
                // active_today 已按主库顺序（先导入的先上屏），并去掉被关闭的策略
                for a in schedule::active_today(&self.lib, self.today, &self.sc) {
                    // 映射到主库里的词条 key；库里没有的词跳过
                    let key = a.word.trim().to_lowercase();
                    if let Some(e) = self.lib.words.iter().find(|w| w.key() == key) {
                        out.push((e.key(), a.hide));
                    }
                    if out.len() >= control::MAX_FLOATS {
                        break;
                    }
                }
                out
            }
            Screen::Manual => self
                .cfg
                .shown
                .iter()
                .filter(|k| self.entry_by_key(k).is_some())
                .map(|k| (k.clone(), Vec::new()))
                .take(control::MAX_FLOATS)
                .collect(),
        };

        // 先估算每张的高度：已有的用真实尺寸，没建的按经验值
        let est = 150.0 * scale;
        let sizes: Vec<egui::Vec2> = want
            .iter()
            .enumerate()
            .map(|(i, _)| {
                self.floats
                    .get(i)
                    .and_then(|f| f.size)
                    .unwrap_or(egui::vec2(card::CARD_W * scale, est))
            })
            .collect();
        let (pos, fit) = Self::layout_cards(&sizes, monitor, scale);
        self.shown_fit = fit;

        let want: Vec<(String, Vec<String>)> = want.into_iter().take(fit).collect();

        // 多余的窗关掉
        while self.floats.len() > want.len() {
            self.floats.pop();
        }
        for (i, f) in self.floats.iter_mut().enumerate() {
            let (w, h) = &want[i];
            if f.word != *w || f.hide_extra != *h {
                f.word = w.clone();
                f.hide_extra = h.clone();
                f.plan_key = None;
                f.invalidate_plan();
            }
            let target = pos.get(i).copied();
            if target != f.pos {
                f.pos = target;
                if let Some(p) = target {
                    let vid = ViewportId::from_hash_of(("lexideck-float", f.id));
                    ctx.send_viewport_cmd_to(vid, ViewportCommand::OuterPosition(p));
                }
            }
        }
        // 缺的窗补上
        while self.floats.len() < want.len() {
            let i = self.floats.len();
            let (w, h) = want[i].clone();
            self.spawn_float(ctx, th, w, h, pos.get(i).copied(), scale);
        }
    }

    fn spawn_float(
        &mut self,
        ctx: &egui::Context,
        th: &Theme,
        word: String,
        hide_extra: Vec<String>,
        pos: Option<egui::Pos2>,
        scale: f32,
    ) {
        if self.floats.len() >= control::MAX_FLOATS {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let (_, size) = Self::make_plan(ctx, self.entry_by_key(&word), th, scale, &hide_extra);
        let mut fw = FloatWin {
            id,
            word: word.clone(),
            anim: Anim::new(),
            anim_at: None,
            plan_dirty: true,
            pos,
            plan: None,
            plan_key: None,
            hide_extra,
            size: Some(size),
        };
        fw.replay_anim();
        self.floats.push(fw);
    }

    /// 逐帧声明所有悬浮窗 viewport（含排版计划重建）
    fn draw_floats(&mut self, ctx: &egui::Context, th: &Theme) {
        let scale = self.cfg.font_scale * Self::screen_scale(ctx);
        // 层级两档（设计 §5.2）：置底 = AlwaysOnBottom（默认），置顶 = AlwaysOnTop
        let level = match self.cfg.layer {
            config::Layer::Top => WindowLevel::AlwaysOnTop,
            config::Layer::Bottom => WindowLevel::AlwaysOnBottom,
        };
        let passthrough = self.cfg.passthrough;
        let theme_kind = th.kind;

        for f in self.floats.iter_mut() {
            let vid = ViewportId::from_hash_of(("lexideck-float", f.id));
            let entry = self.lib.words.iter().find(|w| w.key() == f.word);

            let key = entry.map(|e| {
                (
                    e.key(),
                    f32::to_bits(scale),
                    theme_kind,
                    f.hide_extra.clone(),
                )
            });
            if f.plan_key != key {
                f.plan_key = key;
                f.invalidate_plan();
            }

            let size = f
                .size
                .unwrap_or(egui::vec2(card::CARD_W * scale, 140.0 * scale));
            let builder = window::float_viewport(f.id, f.pos, level, passthrough, size);

            ctx.show_viewport_immediate(vid, builder, |ui, _class| {
                fps_tick(ui.ctx(), "card");
                f.start_anim_if_due();
                if f.anim_at.is_some() {
                    ui.ctx().request_repaint_of(ui.ctx().viewport_id());
                }
                // ⚠️ 排版必须在这里做（悬浮窗自己的 pass 里）
                if f.plan_dirty {
                    f.plan_dirty = false;
                    let plan = entry.map(|e| card::plan(ui.ctx(), e, th, scale, &f.hide_extra));
                    f.plan = plan;
                    if let Some(p) = &f.plan {
                        let want = egui::vec2(card::CARD_W * scale, p.height);
                        if Some(want) != f.size {
                            f.size = Some(want);
                            ui.ctx().send_viewport_cmd(ViewportCommand::InnerSize(want));
                        }
                    }
                }
                float::draw(ui, &mut f.anim, f.plan.as_ref(), th, scale);
            });
        }
    }

    // ── 主窗口几何 ──

    fn place_once(&mut self, ctx: &egui::Context) {
        if self.placed {
            return;
        }
        let vp = ctx.input(|i| i.viewport().clone());
        let pos = match (self.cfg.window_x, self.cfg.window_y) {
            (Some(x), Some(y)) => Some(egui::pos2(x, y)),
            _ => vp
                .monitor_size
                .map(|ms| egui::pos2(60.0, (ms.y - self.cfg.window_h - 80.0).max(0.0))),
        };
        if let Some(p) = pos {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(p));
            self.placed = true;
        }
    }

    fn persist_geometry(&mut self, ctx: &egui::Context) {
        if self.last_persist.elapsed() < Duration::from_millis(2000) {
            return;
        }
        let vp = ctx.input(|i| i.viewport().clone());
        let mut dirty = false;
        if let Some(r) = vp.inner_rect {
            let w = r.width().round();
            let h = r.height().round();
            if (w - self.cfg.window_w).abs() > 1.0 {
                self.cfg.window_w = w;
                dirty = true;
            }
            if (h - self.cfg.window_h).abs() > 1.0 {
                self.cfg.window_h = h;
                dirty = true;
            }
        }
        if let Some(r) = vp.outer_rect {
            let (x, y) = (r.min.x.round(), r.min.y.round());
            let changed = match (self.cfg.window_x, self.cfg.window_y) {
                (Some(px), Some(py)) => (x - px).abs() > 1.0 || (y - py).abs() > 1.0,
                _ => true,
            };
            if changed {
                self.cfg.window_x = Some(x);
                self.cfg.window_y = Some(y);
                dirty = true;
            }
        }
        if dirty {
            config::save(&config::settings_path(), &self.cfg);
            self.last_persist = Instant::now();
        }
    }

    // ── 意图处理 ──

    fn handle_control(&mut self, ctx: &egui::Context, res: control::ControlResult) {
        if res.top_changed {
            let lvl = match self.cfg.layer {
                config::Layer::Top => WindowLevel::AlwaysOnTop,
                config::Layer::Bottom => WindowLevel::AlwaysOnBottom,
            };
            // 层级与穿透是两件事，两个都要发全
            let pt = self.cfg.passthrough;
            for f in &self.floats {
                let vid = ViewportId::from_hash_of(("lexideck-float", f.id));
                ctx.send_viewport_cmd_to(vid, ViewportCommand::WindowLevel(lvl));
                ctx.send_viewport_cmd_to(vid, ViewportCommand::MousePassthrough(pt));
            }
        }
        if res.reload {
            self.reload_library(); // 里面已经 prune_state
            self.status = Some("已重新读取主库".into());
        }
        if res.pick_import {
            self.pick_import();
        }
        if let Some(choice) = res.resolve {
            self.resolve_conflict(choice);
        }
        if res.dismiss_dialog {
            self.import_error = None;
            self.status = None;
        }
        if let Some(k) = res.toggle_select {
            if !self.selected.remove(&k) {
                self.selected.insert(k);
            }
        }
        if let Some(k) = res.toggle_expand {
            if !self.expanded.remove(&k) {
                self.expanded.insert(k);
            }
        }
        if res.select_all {
            for e in &self.lib.words {
                self.selected.insert(e.key());
            }
        }
        if res.clear_selection {
            self.selected.clear();
        }
        if res.show_selected {
            if self.selected.is_empty() {
                self.status = Some("先在词表里勾几个词，再点「立即展示」".into());
            } else {
                // 按词表顺序展示，保证每次结果一致
                let mut keys: Vec<String> = self
                    .sorted_indices()
                    .into_iter()
                    .map(|i| self.lib.words[i].key())
                    .filter(|k| self.selected.contains(k))
                    .collect();
                keys.truncate(control::MAX_FLOATS);
                let n = keys.len();
                self.cfg.shown = keys;
                config::save(&config::settings_path(), &self.cfg);
                self.screen = Screen::Manual; // 「立即展示」= 手动模式
                self.status = Some(format!("已让 {n} 个词上屏（手动展示）"));
                // 重建窗（内容变了）
                self.floats.clear();
            }
        }
        if res.close_all {
            self.cfg.shown.clear();
            self.floats.clear();
            self.screen = Screen::Manual;
            config::save(&config::settings_path(), &self.cfg);
            self.status = Some("已关闭全部卡片".into());
        }
        if res.back_to_today {
            self.screen = Screen::Auto;
            self.floats.clear();
            let (_, words, _) = schedule::today_counts(&self.lib, self.today, &self.sc);
            self.status = Some(format!("已回到今日展示：今日生效 {words} 个"));
        }
        // 卡片页的两档内容来源（面板切「按今日展示 / 手动展示」）
        if let Some(auto) = res.set_screen_auto {
            self.floats.clear();
            if auto {
                self.screen = Screen::Auto;
                let (_, words, _) = schedule::today_counts(&self.lib, self.today, &self.sc);
                self.status = Some(format!("已切回按今日展示：今日生效 {words} 个词"));
            } else {
                self.screen = Screen::Manual;
                self.status = Some(if self.cfg.shown.is_empty() {
                    "已切到手动展示（还没勾词 —— 去「词表」勾几个词点「立即展示」）".into()
                } else {
                    format!("已切到手动展示：显示勾选的 {} 个词", self.cfg.shown.len())
                });
            }
        }
        // 面板产生的一句反馈（如「设置已保存」）压过上面的状态行
        if let Some(s) = res.status_msg {
            self.status = Some(s);
        }
        if let Some((word, rng)) = res.toggle_off {
            self.toggle_strategy(&word, &rng);
        }
        if res.replay_anim {
            for (i, f) in self.floats.iter_mut().enumerate() {
                f.replay_anim_delayed(i as u64 * 200);
            }
        }
        if res.confirm_arm {
            self.confirm_exit = true;
        }
        if res.minimize {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        if res.changed {
            config::save(&config::settings_path(), &self.cfg);
        }
        if res.exit {
            config::save(&config::settings_path(), &self.cfg);
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    /// 永久关闭 / 打开一条策略（词 + 日期段）。
    /// 状态文件读不懂时**拒绝写入**，绝不覆盖坏文件。
    fn toggle_strategy(&mut self, word: &str, rng: &deck::Range) {
        if !self.sc_ok {
            self.status = Some(
                "状态文件读不懂，已拒绝写入（避免覆盖坏文件）；请先修好或删掉 状态.json".into(),
            );
            return;
        }
        let now_off = !self.sc.is_off(word, rng);
        self.sc.set_off(word, rng, now_off);
        match schedule::save_state(&self.sc_path, &self.sc) {
            Ok(()) => {
                self.status = Some(if now_off {
                    format!("已关闭「{word}」这条策略")
                } else {
                    format!("已打开「{word}」这条策略")
                });
                // 如果是 Auto，重建卡片（下一帧按新集合上屏）
                if self.screen == Screen::Auto {
                    self.floats.clear();
                }
            }
            Err(e) => {
                // 写盘失败就把内存里的改动退回去，别让两边不一致
                self.sc.set_off(word, rng, !now_off);
                self.status = Some(format!("保存状态失败：{e}"));
            }
        }
    }

    // ── 导入 ──

    fn pick_import(&mut self) {
        let picked = rfd::FileDialog::new()
            .set_title("选择要导入的词表（.json）")
            .add_filter("词表文件", &["json"])
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "词表".into());
        match deck::load_file(&path) {
            Err(errs) => {
                self.import_error = Some((name, errs));
                self.status = Some("导入失败：文件格式不对，主库未改动".into());
            }
            Ok((incoming, _warns)) => {
                let sess = import::Session::new(name, incoming, &self.lib.words);
                let has = sess.has_conflict();
                self.import = Some(sess);
                if !has {
                    self.finish_import();
                }
            }
        }
    }

    fn resolve_conflict(&mut self, choice: ConflictChoice) {
        let Some(sess) = self.import.as_mut() else {
            return;
        };
        sess.resolve(match choice {
            ConflictChoice::Skip => import::Choice::Skip,
            ConflictChoice::Replace => import::Choice::Replace,
            ConflictChoice::SkipAll => import::Choice::SkipAll,
            ConflictChoice::ReplaceAll => import::Choice::ReplaceAll,
        });
        if self.import.as_ref().is_some_and(|s| !s.has_conflict()) {
            self.finish_import();
        }
    }

    fn finish_import(&mut self) {
        let Some(sess) = self.import.take() else {
            return;
        };
        let file = sess.file.clone();
        // 主库读不懂时绝不写盘：否则等于拿空库把坏文件覆盖掉，原始内容全丢
        if self.lib_corrupt {
            self.status =
                Some("导入已中止：主库文件现在读不懂，先修好或删掉 lexideck.json 再来".into());
            return;
        }
        // 冲突弹窗停留期间主库被外部改过 → 放弃本次，免得基于旧内容算重复词
        if library::mtime(&self.lib_path) != self.lib_mtime {
            self.reload_library();
            self.status = Some("导入已中止：这期间主库被外部改过，请再导入一次".into());
            return;
        }
        let (next, summary) = sess.apply(&self.lib);
        match library::save(&self.lib_path, &next) {
            Ok(()) => {
                self.lib = next;
                self.lib_mtime = library::mtime(&self.lib_path);
                self.prune_selection();
                self.prune_state();
                // 被替换的词如果正在屏上，卡片要立刻按新内容重排
                for f in &mut self.floats {
                    f.invalidate_plan();
                }
                self.status = Some(import::Session::summary_line(&file, &summary));
            }
            Err(e) => {
                self.status = Some(format!("导入失败：写入主库出错（{e}），主库未改动"));
            }
        }
    }

    fn conflict_view(&self) -> Option<(usize, usize, String, String, String)> {
        let sess = self.import.as_ref()?;
        let (old, new, idx, total) = sess.current()?;
        Some((idx, total, new.word.clone(), old.summary(), new.summary()))
    }
}

impl eframe::App for LexideckApp {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        // 全透明清屏：悬浮窗需要真透明 —— 闪烁段整窗只有色块，色块半透明时背后透出桌面。
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 只有根 viewport 是"控制面板"
        if ctx.viewport_id() != egui::ViewportId::ROOT {
            return;
        }
        fps_tick(&ctx, "root");

        self.place_once(&ctx);
        self.poll_library();
        // 跨日换批：日期一变，Auto 模式把卡片清空，下一帧按新的一天重建（带入场动画）
        let now_day = schedule::today();
        if now_day != self.today {
            self.today = now_day;
            if self.screen == Screen::Auto {
                self.floats.clear();
            }
            let (_, words, _) = schedule::today_counts(&self.lib, self.today, &self.sc);
            self.status = Some(format!("日期变了：今日生效 {words} 个"));
        }
        // 退出确认只在 5 秒内有效
        if self.confirm_exit {
            match self.confirm_at {
                Some(t) if t.elapsed() >= Duration::from_secs(5) => {
                    self.confirm_exit = false;
                    self.confirm_at = None;
                }
                None => self.confirm_at = Some(Instant::now()),
                _ => {}
            }
        } else {
            self.confirm_at = None;
        }
        self.persist_geometry(&ctx);

        let kind = ThemeKind::parse(&self.cfg.theme);
        let th = theme::get(kind);
        if self.applied_theme != Some(kind) {
            apply_panel_visuals(&ctx, kind);
            self.applied_theme = Some(kind);
        }

        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, control::skin_for(kind).bg);

        self.reconcile_floats(&ctx, &th);
        self.draw_floats(&ctx, &th);

        let rows = self.build_word_rows();
        let card_rows = self.build_card_rows();
        let plan_rows = self.build_plan_rows();
        let today_label = format!(
            "{:04}-{:02}-{:02}",
            self.today.0, self.today.1, self.today.2
        );
        let conflict = self.conflict_view();
        let errors: Vec<String> = self
            .import_error
            .as_ref()
            .map(|(_, e)| e.clone())
            .unwrap_or_default();
        let (file,) = self
            .import_error
            .as_ref()
            .map(|(f, _)| (f.clone(),))
            .unwrap_or_else(|| {
                self.import
                    .as_ref()
                    .map(|s| (s.file.clone(),))
                    .unwrap_or_default()
            });
        let session_file = self
            .import
            .as_ref()
            .map(|s| s.file.clone())
            .unwrap_or_default();

        let import_ctx = ImportCtx {
            file: if !file.is_empty() {
                &file
            } else {
                &session_file
            },
            errors: &errors,
            conflict: conflict
                .as_ref()
                .map(|(i, n, w, o, nw)| (*i, *n, w.as_str(), o.as_str(), nw.as_str())),
            done: self.status.as_deref(),
        };
        let cx = control::Ctx {
            lib_label: library::LIB_NAME,
            total_entries: self.lib.words.len(),
            selected: self.selected.len(),
            display_items: self.lib.display_time.len(),
            card_rows: &card_rows,
            plan_rows: &plan_rows,
            effective: schedule::today_counts(&self.lib, self.today, &self.sc).1,
            shown: self.floats.len(),
            screen_auto: self.screen == Screen::Auto,
            today: &today_label,
            status: self.status.as_deref().unwrap_or(""),
            warns: &self.warns,
            import: &import_ctx,
            confirm_exit: self.confirm_exit,
        };
        let res = control::draw(ui, &mut self.cfg, &mut self.panel, &cx, &rows);
        self.handle_control(&ctx, res);

        // 面板自己不必全速重绘：卡片动画是由各自的浮窗视口用 request_repaint_of
        // 驱动的（见 draw_floats）。这里如果写成「有卡片在动画就 ctx.request_repaint()」，
        // 会把每帧绘制很重的面板一起拉进全速渲染，反而把卡片入场动画的帧率拖下来
        // —— 表现就是「动画卡卡的」。面板 5fps 足够（点一下会立刻触发重绘）。
        ctx.request_repaint_after(Duration::from_millis(300));
    }
}

// ── 帧率探针（诊断用） ──

/// 设了 `LEXIDECK_FPS_LOG=<文件>` 时，把每帧间隔按来源追加进该文件，每行 `来源 毫秒`：
///   `root` = 控制面板那一趟；`card` = 某个悬浮窗的那一趟。
/// 用途：判断「卡片动画卡」是被谁拖慢的 —— 面板太慢 / 浮窗自己太慢 / 根本没人驱动重绘。
/// 只在设了环境变量时才工作，平时零开销。
fn fps_tick(ctx: &egui::Context, tag: &str) {
    let Ok(path) = std::env::var("LEXIDECK_FPS_LOG") else {
        return;
    };
    let id = egui::Id::new(("lexideck-fps-probe", tag));
    let out = ctx.data_mut(|d| {
        let p: &mut FpsProbe = d.get_temp_mut_or_insert_with(id, FpsProbe::default);
        let now = std::time::Instant::now();
        let dt = p.last.map(|l| now.duration_since(l).as_secs_f32() * 1000.0);
        p.last = Some(now);
        let Some(dt) = dt else { return None };
        p.buf.push_str(&format!("{tag} {dt:.1}\n"));
        // 攒够一把或「这一帧明显空了」就落盘，保证动画结束后的尾帧也能及时看到
        if p.buf.len() < 512 && dt < 100.0 {
            return None;
        }
        Some(std::mem::take(&mut p.buf))
    });
    if let Some(s) = out {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = f.write_all(s.as_bytes());
        }
    }
}

#[derive(Default, Clone)]
struct FpsProbe {
    last: Option<std::time::Instant>,
    buf: String,
}

// ── 字体 / 视觉 ──

/// 从系统加载字体（egui 自带字体不含 CJK 和多数音标字符）
fn setup_fonts(cc: &eframe::CreationContext<'_>) {
    egui_extras::install_image_loaders(&cc.egui_ctx);
    let mut fonts = egui::FontDefinitions::default();
    let cjk_candidates: [(&str, u32); 4] = [
        ("C:\\Windows\\Fonts\\msyh.ttc", 0),
        ("C:\\Windows\\Fonts\\msyh.ttf", 0),
        ("C:\\Windows\\Fonts\\simhei.ttf", 0),
        ("C:\\Windows\\Fonts\\simsun.ttc", 0),
    ];
    for (path, index) in cjk_candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fd = egui::FontData::from_owned(bytes);
            fd.index = index;
            fonts.font_data.insert("cjk".to_owned(), fd.into());
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                fam.insert(0, "cjk".to_owned());
            }
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                fam.push("cjk".to_owned());
            }
            break;
        }
    }
    let mut ipa_pos = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .and_then(|f| f.iter().position(|n| n == "cjk"))
        .map(|i| i + 1)
        .unwrap_or(0);
    for (key, path) in [
        ("ipa1", "C:\\Windows\\Fonts\\segoeui.ttf"),
        ("ipa2", "C:\\Windows\\Fonts\\arial.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(key.to_owned(), egui::FontData::from_owned(bytes).into());
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                let at = ipa_pos.min(fam.len());
                fam.insert(at, key.to_owned());
                ipa_pos += 1;
            }
            if let Some(fam) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                fam.push(key.to_owned());
            }
        }
    }
    cc.egui_ctx.set_fonts(fonts);
}

/// 控制面板 egui 皮肤（跟随主题）
fn apply_panel_visuals(ctx: &egui::Context, kind: ThemeKind) {
    let s = control::skin_for(kind);
    let mut v = if s.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = s.bg;
    v.window_fill = s.bg;
    v.window_stroke = egui::Stroke::new(1.0, s.line);
    v.override_text_color = Some(s.txt);
    v.widgets.noninteractive.bg_fill = s.card;
    v.widgets.inactive.weak_bg_fill = s.btn_bg;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, s.btn_line);
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, s.txt);
    v.widgets.hovered.weak_bg_fill = s.btn_hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, s.line2);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, s.txt);
    v.widgets.active.weak_bg_fill = s.btn_hover;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, s.txt);
    let r = egui::CornerRadius::ZERO;
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.corner_radius = r;
    v.widgets.active.corner_radius = r;
    v.widgets.open.corner_radius = r;
    v.selection.bg_fill = s.acc;
    v.selection.stroke = egui::Stroke::new(1.0, s.acc_dark);
    // 滚动条滑块的颜色：egui 在 foreground_color=false 时取 widgets.*.bg_fill。
    // 默认浅色主题给的是中灰，而 floating 样式下用的是 fg_stroke（=正文色，近黑）——
    // 在纯白面板上就是一条扎眼的黑条，这里显式指定成浅灰/雾青。
    v.widgets.inactive.bg_fill = s.line2;
    v.widgets.hovered.bg_fill = s.acc;
    v.widgets.active.bg_fill = s.acc;
    v.extreme_bg_color = s.bg; // 滚动条轨道：跟面板同色，等于不画
    ctx.set_visuals(v);
    // 滚动条：细、平时不显示、鼠标进到面板才淡淡露出、直接指向时才明显
    // （egui 0.36 里没有 Context::style_mut，要用 all_styles_mut 改两套主题的 Style）
    ctx.all_styles_mut(|st| {
        let sc = &mut st.spacing.scroll;
        // 关键：不要 floating。浮动滚动条是「压在内容上」的，会盖住右边那点文字
        // 和元素边框（用户明确指出这个不行）。改成占自己一条道：内容自动让出
        // bar_width + bar_inner_margin 的宽度，谁也不会被盖住。
        sc.floating = false;
        sc.foreground_color = false; // 滑块颜色走 widgets.*.bg_fill（上面设的浅灰/雾青）
        sc.bar_width = 8.0;
        sc.handle_min_length = 28.0;
        sc.bar_inner_margin = 4.0;
        sc.bar_outer_margin = 0.0;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 单张比屏幕还高的卡：按顺序截断，绝不硬塞到屏幕上
    #[test]
    fn oversized_card_is_truncated() {
        let monitor = egui::vec2(1920.0, 1080.0);
        let sizes = vec![egui::vec2(640.0, 2000.0), egui::vec2(640.0, 200.0)];
        let (pos, fit) = LexideckApp::layout_cards(&sizes, monitor, 1.0);
        assert_eq!(fit, 0);
        assert!(pos.is_empty());
    }

    /// 一列排满后向左开新列
    #[test]
    fn cards_fill_right_column_then_go_left() {
        let monitor = egui::vec2(1920.0, 1080.0);
        let sizes = vec![egui::vec2(640.0, 480.0); 3];
        let (pos, fit) = LexideckApp::layout_cards(&sizes, monitor, 1.0);
        assert_eq!(fit, 3);
        assert_eq!(pos[0].x, pos[1].x);
        assert!(pos[2].x < pos[1].x, "第一列排满后应向左开第二列");
    }
}
