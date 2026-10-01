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
    /// 当前排版计划（词条 / 缩放 / 主题变化时置脏重建）
    plan: Option<CardPlan>,
    /// 计划对应的键：(词条, 缩放 bits, 主题)
    plan_key: Option<(String, u32, ThemeKind)>,
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
        let mut app = Self {
            cfg,
            lib,
            lib_path: path,
            lib_mtime: None,
            lib_corrupt: corrupt,
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
        for f in &mut self.floats {
            f.invalidate_plan();
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
        self.floats
            .iter()
            .enumerate()
            .map(|(i, f)| CardRow {
                id: f.id,
                word: self
                    .entry_by_key(&f.word)
                    .map(|e| e.word.clone())
                    .unwrap_or_else(|| f.word.clone()),
                source: format!("第 {} 位 · 手动展示", i + 1),
            })
            .collect()
    }

    fn build_plan_rows(&self) -> Vec<PlanRow> {
        self.sorted_indices_display()
            .into_iter()
            .map(|i| {
                let it = &self.lib.display_time[i];
                PlanRow {
                    word: it.word.clone(),
                    ranges: it
                        .ranges
                        .iter()
                        .map(|r| r.label())
                        .collect::<Vec<_>>()
                        .join("，"),
                    hide: if it.hide.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "策略内隐藏：{}",
                            it.hide
                                .iter()
                                .map(|h| deck::hide_label(h))
                                .collect::<Vec<_>>()
                                .join("/")
                        )
                    },
                }
            })
            .collect()
    }

    fn sorted_indices_display(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.lib.display_time.len()).collect();
        idx.sort_by(|a, b| {
            self.lib.display_time[*a]
                .key()
                .cmp(&self.lib.display_time[*b].key())
        });
        idx
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
    ) -> (Option<CardPlan>, egui::Vec2) {
        match entry {
            Some(e) => {
                let p = card::plan(ctx, e, th, scale);
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

    /// 让悬浮窗数量 / 内容 / 位置与 cfg.shown 保持一致
    fn reconcile_floats(&mut self, ctx: &egui::Context, th: &Theme) {
        let scale = self.cfg.font_scale * Self::screen_scale(ctx);
        let monitor = ctx
            .input(|i| i.viewport().monitor_size)
            .unwrap_or(egui::vec2(1920.0, 1080.0));
        let want: Vec<String> = self
            .cfg
            .shown
            .iter()
            .filter(|k| self.entry_by_key(k).is_some())
            .cloned()
            .take(control::MAX_FLOATS)
            .collect();

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

        let want: Vec<String> = want.into_iter().take(fit).collect();

        // 多余的窗关掉
        while self.floats.len() > want.len() {
            self.floats.pop();
        }
        for (i, f) in self.floats.iter_mut().enumerate() {
            if f.word != want[i] {
                f.word = want[i].clone();
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
            self.spawn_float(ctx, th, want[i].clone(), pos.get(i).copied(), scale);
        }
    }

    fn spawn_float(
        &mut self,
        ctx: &egui::Context,
        th: &Theme,
        word: String,
        pos: Option<egui::Pos2>,
        scale: f32,
    ) {
        if self.floats.len() >= control::MAX_FLOATS {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let (_, size) = Self::make_plan(ctx, self.entry_by_key(&word), th, scale);
        let mut fw = FloatWin {
            id,
            word: word.clone(),
            anim: Anim::new(),
            anim_at: None,
            plan_dirty: true,
            pos,
            plan: None,
            plan_key: None,
            size: Some(size),
        };
        fw.replay_anim();
        self.floats.push(fw);
    }

    /// 逐帧声明所有悬浮窗 viewport（含排版计划重建）
    fn draw_floats(&mut self, ctx: &egui::Context, th: &Theme) {
        let scale = self.cfg.font_scale * Self::screen_scale(ctx);
        let on_top = self.cfg.always_on_top;
        let theme_kind = th.kind;

        for f in self.floats.iter_mut() {
            let vid = ViewportId::from_hash_of(("lexideck-float", f.id));
            let entry = self.lib.words.iter().find(|w| w.key() == f.word);

            let key = entry.map(|e| (e.key(), f32::to_bits(scale), theme_kind));
            if f.plan_key != key {
                f.plan_key = key;
                f.invalidate_plan();
            }

            let size = f
                .size
                .unwrap_or(egui::vec2(card::CARD_W * scale, 140.0 * scale));
            let builder = window::float_viewport(f.id, f.pos, on_top, size);

            ctx.show_viewport_immediate(vid, builder, |ui, _class| {
                f.start_anim_if_due();
                if f.anim_at.is_some() {
                    ui.ctx().request_repaint_of(ui.ctx().viewport_id());
                }
                // ⚠️ 排版必须在这里做（悬浮窗自己的 pass 里）
                if f.plan_dirty {
                    f.plan_dirty = false;
                    f.plan = entry.map(|e| card::plan(ui.ctx(), e, th, scale));
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
            let lvl = if self.cfg.always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            };
            for f in &self.floats {
                let vid = ViewportId::from_hash_of(("lexideck-float", f.id));
                ctx.send_viewport_cmd_to(vid, ViewportCommand::WindowLevel(lvl));
            }
        }
        if res.reload {
            self.reload_library();
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
                self.status = Some(format!("已让 {n} 个词上屏"));
                // 重建窗（内容变了）
                self.floats.clear();
            }
        }
        if res.close_all {
            self.cfg.shown.clear();
            self.floats.clear();
            config::save(&config::settings_path(), &self.cfg);
            self.status = Some("已关闭全部卡片".into());
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

        self.place_once(&ctx);
        self.poll_library();
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
            effective: self.cfg.shown.len(),
            shown: self.floats.len(),
            warns: &self.warns,
            import: &import_ctx,
            confirm_exit: self.confirm_exit,
        };
        let res = control::draw(ui, &mut self.cfg, &mut self.panel, &cx, &rows);
        self.handle_control(&ctx, res);

        let busy = self
            .floats
            .iter()
            .any(|f| f.anim.is_playing() || f.anim_at.is_some());
        if busy {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(300));
        }
    }
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
    ctx.set_visuals(v);
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
