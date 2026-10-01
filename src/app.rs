//! 应用状态机：控制面板（主窗口）+ 多个悬浮窗（独立 viewport）。
//! 词表加载优先级：`词表.json`（新格式，见 docs/词条格式规范-草稿.md）→ 旧 TXT → 内置示例。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{
    self,
    viewport::{ViewportCommand, ViewportId, WindowLevel},
};

use crate::anim::Anim;
use crate::card::{self, CardPlan};
use crate::config::{self, Config};
use crate::control;
use crate::deck::{self, Entry};
use crate::float;
use crate::theme::{self, Theme, ThemeKind};
use crate::window;
use crate::words::{self, Card, CardKind, Warning};

/// 一个悬浮窗的运行时状态
pub struct FloatWin {
    pub id: usize,
    pub anim: Anim,
    /// 入场动画的预定开播时刻（批量操作时错峰延时；到点后在窗口自己的帧里启动）。
    /// 窗口从创建到真正出现有几百毫秒的延迟，如果在创建那一刻就计时，
    /// 等画面出来时动画已经跑掉一大截（闪烁几乎看不到）。
    /// 所以：先记一个"待播时刻"，等窗口自己的帧把它兑现。
    anim_at: Option<Instant>,
    /// 计划是否需要在悬浮窗自己的绘制周期里重建。
    ///
    /// ⚠️ 这是踩过的坑：`card::plan` 必须**在悬浮窗自己的 pass 里**调用。
    /// 早期版本在主窗口（控制面板）那一 pass 里用 `ctx.layer_painter(...)` 预先排版，
    /// 再把 galley 拿到悬浮窗的 pass 里绘制 —— 排版得到的字形 UV 绑定的是排版当时的
    /// 图集状态，等到另一个 pass 才真正画出来，字形就取错了位置：**行位置是对的，但
    /// 每个字都是错的形状**（看起来像"乱字"）。所以计划改为惰性重建。
    plan_dirty: bool,
    pos: Option<egui::Pos2>,
    /// 当前排版计划（词条 / 缩放 / 主题变化时置脏重建）
    plan: Option<CardPlan>,
    /// 计划对应的键：(词条全局序号, 缩放 bits, 主题)
    plan_key: Option<(usize, u32, ThemeKind)>,
    /// 当前目标窗口尺寸（变化时向该 viewport 发送调整指令）
    size: Option<egui::Vec2>,
}

impl FloatWin {
    /// 作废排版计划（词条 / 缩放 / 主题变化时调用）
    fn invalidate_plan(&mut self) {
        self.plan = None;
        self.plan_dirty = true;
    }

    /// 请求播放一次入场动画（真正开始计时推迟到该窗下一帧绘制时）
    pub fn replay_anim(&mut self) {
        if self.anim.is_playing() {
            return;
        }
        self.anim_at = Some(Instant::now());
    }

    /// 错峰延时重播（批量操作用）：ms 毫秒后再开始
    pub fn replay_anim_delayed(&mut self, ms: u64) {
        if self.anim.is_playing() {
            return;
        }
        self.anim_at = Some(Instant::now() + Duration::from_millis(ms));
    }

    /// 悬浮窗自己的绘制循环里调用：到点/到帧就启动计时
    fn start_anim_if_due(&mut self) {
        if let Some(t) = self.anim_at {
            if Instant::now() >= t {
                self.anim_at = None;
                self.anim.play();
            }
        }
    }
}

pub struct LexideckApp {
    cfg: Config,
    // 词表数据
    entries: Vec<Entry>,
    kinds: Vec<CardKind>,
    warns: Vec<Warning>,
    file_label: String,
    word_path: Option<PathBuf>,
    mtime: Option<SystemTime>,
    // 悬浮窗
    floats: Vec<FloatWin>,
    next_id: usize,
    // 节流 / 一次性
    last_poll: Instant,
    last_persist: Instant,
    placed: bool,
    applied_theme: Option<ThemeKind>,
    /// 批量管理：当前批次起始下标（在筛选后的列表里）
    batch: usize,
    /// 控制面板当前标签页
    panel_tab: control::PanelTab,
    /// 上次导入的结果提示（一句话）
    import_status: Option<String>,
    confirm_exit: bool,
}

impl LexideckApp {
    pub fn new(cc: &eframe::CreationContext<'_>, cfg: Config) -> Self {
        setup_fonts(cc);
        let mut app = Self {
            cfg,
            entries: Vec::new(),
            kinds: Vec::new(),
            warns: Vec::new(),
            file_label: "内置示例词表".into(),
            word_path: None,
            mtime: None,
            floats: Vec::new(),
            next_id: 1,
            last_poll: Instant::now(),
            last_persist: Instant::now(),
            placed: false,
            applied_theme: None,
            batch: 0,
            panel_tab: control::PanelTab::default(),
            import_status: None,
            confirm_exit: false,
        };
        app.reload_words();
        app
    }

    // ── 词表加载 ──

    /// 目录里的词表文件：优先 `词表.json`，其次旧 TXT 系列
    fn find_word_file(dir: &Path) -> Option<PathBuf> {
        let j = dir.join("词表.json");
        if j.is_file() {
            return Some(j);
        }
        words::find_word_file(dir)
    }

    /// 按扩展名选择加载器；返回统一的 (词条, 提示)
    fn load_entries(path: &Path) -> Result<(Vec<Entry>, Vec<Warning>), String> {
        let is_json = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if is_json {
            let (deck, warns) = deck::load(path)?;
            let warns = warns
                .into_iter()
                .map(|msg| Warning {
                    line: 0,
                    text: String::new(),
                    msg,
                })
                .collect();
            Ok((deck.words, warns))
        } else {
            let (cards, warns) = words::load(path)
                .ok_or_else(|| "读不到文件（编码不支持或文件被占用）".to_string())?;
            Ok((cards.into_iter().map(card_to_entry).collect(), warns))
        }
    }

    fn reload_words(&mut self) {
        let dir = config::exe_dir();
        if let Some(path) = Self::find_word_file(&dir) {
            self.mtime = words::mtime(&path);
            self.word_path = Some(path.clone());
            self.file_label = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "词表".into());
            match Self::load_entries(&path) {
                Ok((entries, warns)) => {
                    self.set_entries(entries, warns);
                }
                Err(err) => {
                    // 解析失败：不丢现有内容，只把错误显示出来
                    self.warns = vec![Warning {
                        line: 0,
                        text: String::new(),
                        msg: err,
                    }];
                }
            }
            return;
        }
        if self.entries.is_empty() {
            let (cards, warns) = words::builtin();
            self.file_label = "内置示例词表".into();
            self.word_path = None;
            self.mtime = None;
            self.set_entries(cards.into_iter().map(card_to_entry).collect(), warns);
        }
    }

    fn force_reload(&mut self) {
        match self.word_path.clone() {
            Some(path) => match Self::load_entries(&path) {
                Ok((entries, warns)) => {
                    self.mtime = words::mtime(&path);
                    self.set_entries(entries, warns);
                }
                Err(err) => {
                    self.warns = vec![Warning {
                        line: 0,
                        text: String::new(),
                        msg: err,
                    }];
                }
            },
            None => self.reload_words(),
        }
    }

    fn set_entries(&mut self, entries: Vec<Entry>, warns: Vec<Warning>) {
        self.kinds = entries.iter().map(kind_of).collect();
        self.entries = entries;
        self.warns = warns;
        // 词表换了：所有悬浮窗的排版计划作废，等它们各自的那一帧重建
        for f in &mut self.floats {
            f.invalidate_plan();
        }
    }

    /// 每 2 秒检查一次词表文件是否被改动（老师保存后自动刷新）
    fn poll_reload(&mut self) {
        if self.last_poll.elapsed() < Duration::from_millis(2000) {
            return;
        }
        self.last_poll = Instant::now();
        match self.word_path.clone() {
            Some(path) => {
                let m = words::mtime(&path);
                if m.is_some() && m != self.mtime {
                    self.mtime = m;
                    if let Ok((entries, warns)) = Self::load_entries(&path) {
                        self.set_entries(entries, warns);
                    }
                }
            }
            None => {
                if Self::find_word_file(&config::exe_dir()).is_some() {
                    self.reload_words();
                }
            }
        }
    }

    // ── 列表 / 筛选 ──

    fn filtered(&self) -> Vec<usize> {
        let f = self.cfg.filter.as_str();
        self.kinds
            .iter()
            .enumerate()
            .filter(|(_, k)| f == "all" || k.key() == f)
            .map(|(i, _)| i)
            .collect()
    }

    /// 批量管理核心：让悬浮窗数量、批次与目标保持一致（每帧开始时调用）。
    /// - 词表为空时最多留 1 个占位窗；
    /// - 数量按设置增减（增则生成新窗、减则关掉末尾的窗）；
    /// - 批次越界自动回夹。
    fn reconcile_floats(&mut self, ctx: &egui::Context, th: &Theme) {
        let len = self.filtered().len();
        let want = if len == 0 {
            self.cfg.float_count.min(1)
        } else {
            self.cfg.float_count.min(len)
        }
        .min(control::MAX_FLOATS);
        while self.floats.len() > want {
            self.floats.pop();
        }
        while self.floats.len() < want {
            self.spawn_float(ctx, th);
        }
        let max_batch = len.saturating_sub(self.floats.len());
        if self.batch > max_batch {
            self.batch = max_batch;
        }
    }

    // ── 排版计划 / 悬浮窗管理 ──

    /// 屏幕系数：不同分辨率下卡片相对屏幕的比例保持一致（1080p = 1.0）。
    /// 例：1920×1080 → 1.0；3840×2160 → 2.0（像素翻倍，视觉占比一致）。
    fn screen_scale(ctx: &egui::Context) -> f32 {
        match ctx.input(|i| i.viewport().monitor_size) {
            Some(ms) if ms.x > 0.0 && ms.y > 0.0 => {
                (ms.y / 1080.0).min(ms.x / 1920.0).clamp(0.5, 4.0)
            }
            _ => 1.0,
        }
    }

    /// 生成某个词条的排版计划与目标窗口尺寸（无词条时给占位尺寸）
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

    fn spawn_float(&mut self, ctx: &egui::Context, th: &Theme) {
        if self.floats.len() >= control::MAX_FLOATS {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let scale = self.cfg.font_scale * Self::screen_scale(ctx);

        // 先按该窗将要显示的那条内容算好窗口尺寸，再放位置。
        // ⚠️ 这里只借排版量一个"高度预估"，算完立刻丢掉 —— 真正的排版计划在悬浮窗
        // 自己的 pass 里重建（原因见 FloatWin::plan_dirty 的注释）。
        let list = self.filtered();
        let gidx = list.get(self.batch + self.floats.len()).copied();
        let entry = gidx.map(|i| &self.entries[i]);
        let (_, size) = Self::make_plan(ctx, entry, th, scale);

        // 默认位置：屏幕右上角起、往下平铺（新增的接在已有卡片下方）
        let stack_gap = 18.0;
        let used: f32 = self
            .floats
            .iter()
            .map(|f| f.size.map(|s| s.y).unwrap_or(0.0) + stack_gap)
            .sum();
        let pos = ctx.input(|i| i.viewport().monitor_size).map(|ms| {
            egui::pos2(
                (ms.x - size.x - 48.0).max(0.0),
                (48.0 + used).min((ms.y - size.y - 24.0).max(48.0)),
            )
        });
        let mut fw = FloatWin {
            id,
            anim: Anim::new(),
            anim_at: None,
            plan_dirty: true,
            pos,
            plan: None,
            plan_key: gidx.map(|i| (i, f32::to_bits(scale), th.kind)),
            size: Some(size),
        };
        // 不要在这里计时：此刻窗口还没创建。等它下一帧画出来再启动（见 start_anim_if_due）。
        fw.replay_anim();
        self.floats.push(fw);
    }

    fn build_rows(&self) -> Vec<control::FloatRow> {
        let list = self.filtered();
        self.floats
            .iter()
            .enumerate()
            .map(|(slot, f)| {
                let label = list
                    .get(self.batch + slot)
                    .map(|&i| self.entries[i].word.trim().to_string())
                    .unwrap_or_else(|| "（空）".to_string());
                control::FloatRow { id: f.id, label }
            })
            .collect()
    }

    /// 逐帧声明所有悬浮窗 viewport（含排版计划重建与窗口尺寸跟踪）
    fn draw_floats(&mut self, ctx: &egui::Context, th: &Theme) {
        let scale = self.cfg.font_scale * Self::screen_scale(ctx);
        let on_top = self.cfg.always_on_top;
        let list = self.filtered();
        let theme_kind = th.kind;
        let batch = self.batch;

        for (slot, f) in self.floats.iter_mut().enumerate() {
            let vid = ViewportId::from_hash_of(("lexideck-float", f.id));
            // 该窗在本次批次里对应的词条（列表由 reconcile_floats 保证边界安全）
            let gidx = list.get(batch + slot).copied();

            // 词条换了 / 缩放变了 / 主题换了 → 作废计划，在它自己的 pass 里重建
            let key = gidx.map(|i| (i, f32::to_bits(scale), theme_kind));
            if f.plan_key != key {
                f.plan_key = key;
                f.invalidate_plan();
            }

            let size = f
                .size
                .unwrap_or(egui::vec2(card::CARD_W * scale, 140.0 * scale));
            let builder = window::float_viewport(f.id, f.pos, on_top, size);
            let entry = gidx.map(|i| &self.entries[i]);

            ctx.show_viewport_immediate(vid, builder, |ui, _class| {
                f.start_anim_if_due();
                if f.anim_at.is_some() {
                    // 待播期间保持重绘，到点准时开播
                    ui.ctx().request_repaint_of(ui.ctx().viewport_id());
                }

                // ⚠️ 排版必须在这里做（悬浮窗自己的 pass 里）。原因见 FloatWin::plan_dirty 注释：
                // 在主窗口那一 pass 里预先排好的 galley 拿到这里来画，字形会取错。
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

    // ── 主窗口（控制面板）几何 ──

    /// 首次显示时放置主窗口（恢复上次位置，或默认左下角）
    fn place_once(&mut self, ctx: &egui::Context) {
        if self.placed {
            return;
        }
        let vp = ctx.input(|i| i.viewport().clone());
        let pos = match (self.cfg.window_x, self.cfg.window_y) {
            (Some(x), Some(y)) => Some(egui::pos2(x, y)),
            _ => vp
                .monitor_size
                .map(|ms| egui::pos2(80.0, (ms.y - self.cfg.window_h - 90.0).max(0.0))),
        };
        if let Some(p) = pos {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(p));
            self.placed = true;
        }
    }

    /// 定期把主窗口几何写回设置文件（节流 2 秒）
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
            self.force_reload();
        }
        if res.import {
            self.import_flow(ctx);
        }
        if let Some(n) = res.count_set {
            self.cfg.float_count = n.min(control::MAX_FLOATS);
            config::save(&config::settings_path(), &self.cfg);
        }
        if res.batch_prev || res.batch_next {
            let step = self.floats.len().max(1);
            let max_batch = self.filtered().len().saturating_sub(self.floats.len());
            self.batch = if res.batch_next {
                (self.batch + step).min(max_batch)
            } else {
                self.batch.saturating_sub(step)
            };
            self.replay_all_delayed();
        }
        if res.replay_anim {
            self.replay_all_delayed();
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

    /// 全部悬浮窗错峰重播（从左到右依次开始，节奏更连贯）
    fn replay_all_delayed(&mut self) {
        for (i, f) in self.floats.iter_mut().enumerate() {
            f.replay_anim_delayed(i as u64 * 90);
        }
    }

    /// 导入词表：原生对话框选文件 → 与「词表.json」合并（同名更新、其余追加）→ 重载
    fn import_flow(&mut self, ctx: &egui::Context) {
        let picked = rfd::FileDialog::new()
            .set_title("选择要导入的词表（.json / .txt）")
            .add_filter("词表文件", &["json", "txt"])
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        let (src_entries, src_warns) = match Self::load_entries(&path) {
            Ok(v) => v,
            Err(err) => {
                self.import_status = Some(format!("导入失败：{err}"));
                ctx.request_repaint();
                return;
            }
        };
        let src_n = src_entries.len();
        // 目标：exe 旁的「词表.json」（以单词为准合并；不存在则新建）
        let target = config::exe_dir().join("词表.json");
        let (mut base, mut warns) = if target.is_file() {
            match Self::load_entries(&target) {
                Ok((entries, warns)) => (entries, warns),
                Err(_) => (self.entries.clone(), Vec::new()),
            }
        } else {
            (self.entries.clone(), Vec::new())
        };
        let mut deck = deck::Deck {
            version: 1,
            deck_name: "教室词卡看板".into(),
            words: std::mem::take(&mut base),
        };
        let incoming = deck::Deck {
            version: 1,
            deck_name: String::new(),
            words: src_entries,
        };
        let (added, updated) = deck::merge(&mut deck, incoming);
        warns.extend(src_warns);
        match deck::save(&target, &deck) {
            Ok(()) => {
                self.import_status =
                    Some(format!("已导入 {src_n} 条：新增 {added}、更新 {updated}"));
                self.mtime = words::mtime(&target);
                self.word_path = Some(target);
                self.file_label = "词表.json".into();
                self.set_entries(deck.words, warns);
            }
            Err(err) => {
                self.import_status = Some(format!("导入失败：写入词表.json 出错（{err}）"));
            }
        }
        ctx.request_repaint();
    }
}

impl eframe::App for LexideckApp {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        // 全透明清屏：悬浮窗需要真透明 —— 闪烁段整窗只有色块，色块半透明时背后透出桌面。
        // 主窗口（控制面板）的底色改由 ui() 自己铺。
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 只有根 viewport 是"控制面板"。悬浮窗是 show_viewport_immediate 的即时子
        // viewport，它的画面由回调闭包（float::draw）负责，跟这里无关。
        //
        // 这道闸不能用"画布尺寸猜"（比如 600×560）——面板一旦被拖动改过尺寸就会猜错，
        // 猜错的后果是把整幅控制面板按面板版式压进悬浮窗那小块画布（字挤成一团）。
        // ctx.viewport_id() 是 egui 给的权威身份，任何尺寸下都不会误判。
        if ctx.viewport_id() != egui::ViewportId::ROOT {
            // 说明子窗口绘制串线了。这里直接不画，绝不能跑面板逻辑——
            // 否则整幅控制面板会按面板版式压进悬浮窗的小画布。
            return;
        }

        self.place_once(&ctx);
        self.poll_reload();
        self.persist_geometry(&ctx);

        let kind = ThemeKind::parse(&self.cfg.theme);
        let th = theme::get(kind);
        if self.applied_theme != Some(kind) {
            apply_panel_visuals(&ctx, kind);
            self.applied_theme = Some(kind);
        }

        // 主窗口背景（清屏色已是全透明，这里自己铺底；跟随主题换肤）
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, control::skin_for(kind).bg);

        // 批量管理：窗数 / 批次与设置保持一致（含首帧自动开窗）
        self.reconcile_floats(&ctx, &th);

        // 悬浮窗（独立 viewport）
        self.draw_floats(&ctx, &th);

        // 控制面板（主窗口内容）
        let rows = self.build_rows();
        let filtered_len = self.filtered().len();
        let cx = control::Ctx {
            file_label: &self.file_label,
            total_entries: self.entries.len(),
            batch: self.batch,
            batch_total: filtered_len,
            count: self.floats.len(),
            warns: &self.warns,
            import_status: self.import_status.as_deref(),
            confirm_exit: self.confirm_exit,
        };
        let res = control::draw(ui, &mut self.cfg, &mut self.panel_tab, &cx, &rows);
        self.handle_control(&ctx, res);

        // 重绘驱动：动画（含待播）时全速，平时低频（热更新 / 几何跟踪）
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

// ── 转换 / 推断 ──

/// 旧 TXT 词条 → 新结构（尽力转换：语法行并入短语区，其余字段暂不展示）
fn card_to_entry(c: Card) -> Entry {
    let get = |name: &str| -> Option<String> {
        c.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    let mut senses = Vec::new();
    let pos = get("词性").unwrap_or_default();
    if let Some(m) = get("释义") {
        senses.push(deck::Sense {
            pos: pos.clone(),
            meaning: m,
        });
    } else if !pos.is_empty() {
        senses.push(deck::Sense {
            pos,
            meaning: String::new(),
        });
    }
    let mut sentences = Vec::new();
    if let Some(en) = get("例句") {
        sentences.push(deck::Sentence {
            en,
            zh: get("例句翻译").unwrap_or_default(),
        });
    }
    let mut phrases = Vec::new();
    if let Some(g) = get("语法") {
        phrases.push(deck::Phrase {
            text: format!("语法：{g}"),
            meaning: String::new(),
        });
    }
    Entry {
        word: c.head.trim().to_string(),
        senses,
        forms: None,
        phrases,
        sentences,
        display_days: None,
    }
}

/// 从词条推断类别（只影响控制面板的筛选）
fn kind_of(e: &Entry) -> CardKind {
    if e.senses.iter().any(|s| !s.pos.trim().is_empty()) {
        return CardKind::Word;
    }
    let w = e.word.trim();
    let ends = w.ends_with(['.', '?', '!', '。', '？', '！']);
    if ends || (w.contains(' ') && w.split_whitespace().count() >= 4) {
        CardKind::Sentence
    } else {
        CardKind::Phrase
    }
}

// ── 字体 / 视觉 ──

/// 从系统加载字体（egui 自带字体不含 CJK 和多数音标字符）：
///   · CJK 主字体：雅黑 ttc → 雅黑 ttf → 黑体 → 宋体
///   · IPA 回退：雅黑缺 ə / ˈ 等音标字符，用 Segoe UI / Arial 兜底
fn setup_fonts(cc: &eframe::CreationContext<'_>) {
    // PNG 图标加载器（assets/icons/*.png 走 include_image! 内嵌）
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
    // IPA 回退字体：插在 cjk 之后、默认字体之前
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

/// 控制面板 egui 皮肤（跟随主题：纯白 / 浅青 = 浅色；黄黑 = 深色）
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
