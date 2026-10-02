//! 应用状态机：控制面板（主窗口）+ 多个悬浮窗（独立 viewport）。
//! 数据来自 exe 同目录的主库 `lexideck.json`（见 library 模块）；
//! 卡片内容由「词表里勾选 → 立即展示」决定（P2 起还会由展示时间自动驱动）。

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{
    self,
    viewport::{ViewportCommand, ViewportId},
};

use crate::anim::Anim;
use crate::autostart;
use crate::card::{self, CardPlan};
use crate::config::{self, CloseAction, Config};
use crate::control::{
    self, CardRow, ConflictChoice, ImportCtx, PanelState, PlanRow, WordFilter, WordRow,
};
use crate::deck::{self, Deck, Entry};
use crate::float;
use crate::import;
use crate::library;
use crate::menu;
use crate::schedule;
use crate::single;
use crate::theme::{self, Theme, ThemeKind};
use crate::tray;
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

// 一次导入的过程状态（逻辑在 import 模块里，这里只持有）

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
    // ── 托盘（P3a）──
    /// 托盘句柄（第一帧懒创建；创建失败为 None，程序照常跑）
    tray: Option<tray::Tray>,
    /// 托盘是否已经尝试创建过（失败不重试）
    tray_tried: bool,
    /// 面板窗口当前是否可见（自己维护：ViewportInfo 里拿不到 Visible(false) 的状态）
    panel_visible: bool,
    /// 收进托盘前的位置（窗口被挪到屏幕外；唤回时放回去）
    panel_home: Option<egui::Pos2>,
    /// 面板窗口的原生句柄（每帧从 `eframe::Frame` 取一次；0 = 还没拿到）。
    /// 收进托盘时要用它摘掉任务栏按钮（见 window::set_taskbar_visible）。
    panel_hwnd: isize,
    /// 明确退出（面板退出按钮 / 托盘菜单「退出」）：绕过关闭拦截，不再 CancelClose
    exiting: bool,
    // ── 托盘右键菜单（P3a，自绘；见 menu.rs）──
    /// 菜单是否开着（右键图标打开；点条目 / ESC / 点了别处关闭）
    menu_open: bool,
    /// 打开时图标点击处的光标位置（物理像素），定位菜单用
    menu_pos: Option<(f32, f32)>,
    /// 还没给菜单请求过焦点：打开后第一帧请求一次（失焦即关，不能每帧都请求）
    menu_need_focus: bool,
    /// 菜单这一辈子拿到过焦点没有（拿到后失焦才判定为「点了别处」）
    menu_got_focus: bool,
    // ── 单实例（P3c）──
    /// 唤回事件的等待线程（有人又启动了一次 → 把面板拿到前台）。字段只为「持有」而存在。
    #[allow(dead_code)]
    single_watch: single::Watcher,
}

/// 关闭拦截里「再点一次」提示的固定文案（和底栏确认文案同一件事，过期后要认得出来）
const EXIT_CONFIRM_HINT: &str = "再点一次 ✕ 就退出（5 秒内有效）";

impl LexideckApp {
    /// 「收进托盘」时面板挪去的屏幕外位置（在常规多屏布局之外；见 hide_panel）
    const PARK_POS: egui::Pos2 = egui::Pos2::new(-32000.0, -32000.0);

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
            tray: None,
            tray_tried: false,
            panel_visible: true,
            panel_home: None,
            exiting: false,
            panel_hwnd: 0,
            menu_open: false,
            menu_pos: None,
            menu_need_focus: false,
            menu_got_focus: false,
            single_watch: single::watch(&cc.egui_ctx),
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

    /// 当前该按哪块屏算卡片（缩放系数 + 平铺锚点都跟着它）。
    ///
    /// 面板收进托盘 = 挪到 (-32000,-32000)：系统把它算成「**最近的**显示器」，
    /// 实时值会跳到另一块屏（P3a 实测：448 宽的卡隐藏后缩成 336、锚点也跑到左屏）。
    /// 所以规则是：面板可见 → 用实时值并记下来；收进托盘 → 用记下来的值。
    fn monitor_size(&self, ctx: &egui::Context) -> egui::Vec2 {
        let id = egui::Id::new("lexideck-monitor-size");
        if self.panel_visible {
            if let Some(ms) = ctx.input(|i| i.viewport().monitor_size) {
                if ms.x > 0.0 && ms.y > 0.0 {
                    ctx.data_mut(|d| d.insert_temp(id, ms));
                    return ms;
                }
            }
        }
        ctx.data(|d| d.get_temp::<egui::Vec2>(id))
            .unwrap_or(egui::vec2(1920.0, 1080.0))
    }

    /// 屏幕系数：不同分辨率下卡片相对屏幕的比例保持一致（1080p = 1.0）
    fn screen_scale(&self, ctx: &egui::Context) -> f32 {
        let ms = self.monitor_size(ctx);
        (ms.y / 1080.0).min(ms.x / 1920.0).clamp(0.5, 4.0)
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
        let scale = self.cfg.font_scale * self.screen_scale(ctx);
        let monitor = self.monitor_size(ctx);

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
        // 缺的窗补上。注意：窗口被**最小化**时 eframe 会走那条没有事件循环上下文的
        // 直接补画旁路（同 hide_panel 注释里的坑），此刻新建窗口会被静默跳过 →
        // egui 断言崩。所以最小化时先不建，等窗口恢复后下一帧再补。
        let minimized = ctx.input(|i| i.viewport().minimized).unwrap_or(false);
        if !minimized {
            while self.floats.len() < want.len() {
                let i = self.floats.len();
                let (w, h) = want[i].clone();
                self.spawn_float(ctx, th, w, h, pos.get(i).copied(), scale);
            }
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
        let scale = self.cfg.font_scale * self.screen_scale(ctx);
        // 层级与穿透各管各的（两者独立，四种组合都可能）
        let level = window::level_of(self.cfg.layer);
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
        let pos = match (self.cfg.window_x, self.cfg.window_y) {
            // x 明显在屏幕外 = 上一版「收进托盘」留下的脏位置，别用
            (Some(x), Some(y)) if x > -10000.0 => Some(egui::pos2(x, y)),
            _ => {
                let ms = self.monitor_size(ctx);
                Some(egui::pos2(60.0, (ms.y - self.cfg.window_h - 80.0).max(0.0)))
            }
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
            // 收进托盘时窗口被挪到屏幕外 —— 那个位置不能存进设置（下次启动就找不回来了）
            if self.panel_visible {
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
        }
        if dirty {
            config::save(&config::settings_path(), &self.cfg);
            self.last_persist = Instant::now();
        }
    }

    // ── 托盘 / 关闭拦截（P3a）──

    /// 托盘可用吗：设置成驻留托盘 **并且** 图标真的建起来了。
    /// 建不起来时不敢藏面板（没有唤回通道 = 把自己锁死），退化成「关闭 = 退出」。
    fn use_tray(&self) -> bool {
        self.cfg.close_action == CloseAction::Tray && self.tray.is_some()
    }

    /// 把面板收进托盘 = 挪到屏幕外（**不是** `Visible(false)`，原因见下）。
    ///
    /// eframe 0.36 对「不可见/最小化窗口」会走一条直接补画的旁路
    /// （run.rs `check_redraw_requests`，为修 egui#5229 的 Windows 问题所加）。
    /// 那条路**没有登记事件循环上下文**，于是「新建窗口」（= 新建浮窗/词卡）会被
    /// 静默跳过，egui 随即断言崩（`egui backend is implemented incorrectly`）——
    /// 实测：面板隐藏时从托盘点「显示全部卡片」必崩；课堂上新词到点上屏同样会崩。
    /// 挪到屏幕外则窗口仍是「可见」态，所有帧都走正常路径，建新窗口安全。
    fn hide_panel(&mut self, ctx: &egui::Context) {
        if self.panel_visible {
            // 记住露脸时的位置，唤回时放回去
            self.panel_home = ctx.input(|i| i.viewport().outer_rect).map(|r| r.min);
        }
        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Self::PARK_POS));
        // 只挪屏外不够：窗口还是「可见」态，任务栏按钮会一直留着（用户实测报过）。
        // 摘掉 WS_EX_APPWINDOW、置成 WS_EX_TOOLWINDOW —— 任务栏和 Alt+Tab 里都不再出现，
        // 但窗口本身仍是「可见」的，所有帧走正常路径（新建浮窗不会踩 eframe 那条旁路）。
        window::set_taskbar_visible(self.panel_hwnd, false);
        self.panel_visible = false;
    }

    /// 把面板放回屏幕里并拿到前台。三处共用：托盘图标 / 托盘菜单 / 第二次启动（单实例唤回）。
    /// 收进托盘时窗口被挪到了屏幕外，所以唤回必须显式放回原位。
    fn show_panel(&mut self, ctx: &egui::Context) {
        // 先把任务栏按钮还回来（收进托盘时摘掉了）
        window::set_taskbar_visible(self.panel_hwnd, true);
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        // 用记住的屏（此刻还没翻回可见，实时值会是屏外那个「最近的屏」）
        let ms = self.monitor_size(ctx);
        let fallback = Some(egui::pos2(60.0, (ms.y - self.cfg.window_h - 80.0).max(0.0)));
        if let Some(p) = self.panel_home.take().or(fallback) {
            ctx.send_viewport_cmd(ViewportCommand::OuterPosition(p));
        }
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        self.panel_visible = true;
    }

    /// 明确退出：置上 `exiting`（关闭拦截放行）→ 存设置 → 请 eframe 关根窗口。
    /// 面板退出按钮、托盘菜单「退出」都走这里。
    fn quit_now(&mut self, ctx: &egui::Context) {
        self.exiting = true;
        config::save(&config::settings_path(), &self.cfg);
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    /// 关闭拦截。放置必须在 place_once 之前：CancelClose 要和 close_requested
    /// 在**同一帧**发出去——eframe 只在这一帧的输出里找它。
    ///
    /// 覆盖的是系统级关闭（Alt+F4 / 任务栏「关闭窗口」）；面板自绘 ✕ 的两条路
    /// （托盘模式首击隐藏、退出模式二次确认）分别走 handle_control 的
    /// confirm_arm / exit 分支，不从这里过。
    fn intercept_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if self.exiting {
            return; // 明确退出：放行（eframe 接着退进程）
        }
        if self.use_tray() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.hide_panel(ctx);
            self.status =
                Some("已收进托盘（图标在右下角 ^ 折叠区；右键唤回，或再双击一次 exe）".into());
            return;
        }
        // 直接退出（或托盘不可用时的退化路径）：沿用已有的 5 秒二次确认
        if self.confirm_exit {
            // 5 秒内第二次：放行（不 CancelClose，eframe 会退出）
            return;
        }
        ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        self.confirm_exit = true;
        self.confirm_at = Some(Instant::now());
        self.status = Some(EXIT_CONFIRM_HINT.into());
    }

    /// 托盘懒创建 + 每帧把事件收干。
    /// 托盘必须在主线程、事件循环起来之后创建（win32 消息循环要求），
    /// 所以放在 ui() 里；失败只写一句 status，不重试。
    fn tray_tick(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if self.tray.is_none() && !self.tray_tried {
            self.tray_tried = true;
            match tray::Tray::create(ctx, panel_hwnd(frame)) {
                Ok(t) => {
                    self.tray = Some(t);
                    // 一次性确认：截图/用户都能看到"托盘建好了"（P3a 新功能，给个明确反馈）
                    self.status = Some(
                        "托盘图标已就绪：关掉面板＝收进托盘（图标在右下角 ^ 折叠区，可拖出来常驻）"
                            .into(),
                    );
                }
                Err(e) => {
                    self.status = Some(format!(
                        "托盘图标创建失败：{e}（关闭窗口按「直接退出」处理）"
                    ));
                }
            }
        }
        let actions: Vec<tray::TrayAction> = match &self.tray {
            Some(t) => {
                t.touch(); // 告诉心跳"这一帧来了"（冻住检测用，见 tray.rs 文件头）
                t.poll()
            }
            None => return,
        };
        for a in actions {
            self.do_tray_action(ctx, a);
        }
    }

    /// 执行一条托盘意图。图标点击与自绘菜单共用这条路由，保证两边行为一致。
    fn do_tray_action(&mut self, ctx: &egui::Context, a: tray::TrayAction) {
        match a {
            tray::TrayAction::OpenMenu { x, y } => {
                // 记下图标点击处（物理像素）；菜单在同一帧的 draw_menu 里画出来。
                // 菜单开着时再右键 = 换个位置重开，不叠窗。
                self.menu_pos = Some((x, y));
                self.menu_open = true;
                self.menu_need_focus = true;
            }
            tray::TrayAction::TogglePanel => {
                if self.panel_visible {
                    self.hide_panel(ctx);
                    self.status = Some(
                        "面板已收进托盘（图标在右下角 ^ 折叠区；右键唤回，或再双击一次 exe）"
                            .into(),
                    );
                } else {
                    self.show_panel(ctx);
                    self.status = Some("面板已唤回".into());
                }
            }
            // 复用面板里的动作，保持两边行为一致
            tray::TrayAction::ShowCards => {
                let res = control::ControlResult {
                    back_to_today: true,
                    ..Default::default()
                };
                self.handle_control(ctx, res);
            }
            tray::TrayAction::HideCards => {
                let res = control::ControlResult {
                    close_all: true,
                    ..Default::default()
                };
                self.handle_control(ctx, res);
            }
            tray::TrayAction::Quit => self.quit_now(ctx),
        }
    }

    /// 画托盘右键菜单（自绘，即时视口；机制见 menu.rs）。
    ///
    /// 为什么不用系统原生菜单：原生菜单走模态 TrackPopupMenu，会把 winit 主循环
    /// 冻住、并可能卡成「点不动的幽灵」（tray.rs 文件头有完整实测记录）。
    /// 自绘菜单只是普通的一帧里画出的小窗口：不冻主循环、样式和面板同源。
    /// 关闭条件：点了条目 / ESC / 失焦（点到别处——菜单都拿过一次焦点后才启用，
    /// 免得新窗口还没拿到焦点就被误判关掉）。
    fn draw_menu(&mut self, ctx: &egui::Context, th: &Theme) {
        if !self.menu_open {
            return;
        }
        let panel_visible = self.panel_visible;
        let need_focus = self.menu_need_focus;
        self.menu_need_focus = false;
        let ppp = ctx.pixels_per_point();
        let (x, y) = self.menu_pos.unwrap_or((0.0, 0.0));
        // 光标位置是物理像素、egui 用点；菜单右下角贴住点击处（托盘菜单惯例：向上向左展开）
        let pos = egui::pos2(x / ppp - menu::W, y / ppp - menu::H);
        let vid = ViewportId::from_hash_of(("lexideck-menu", 0u8));
        let mut act: Option<tray::TrayAction> = None;
        let mut close = false;
        let mut got_focus = self.menu_got_focus;
        ctx.show_viewport_immediate(vid, window::menu_viewport(pos), |ui, _class| {
            if need_focus {
                ui.ctx().send_viewport_cmd(ViewportCommand::Focus);
            }
            match ui.input(|i| i.viewport().focused) {
                Some(true) => got_focus = true,
                Some(false) if got_focus => close = true,
                _ => {}
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            act = menu::draw(ui, th, panel_visible);
        });
        self.menu_got_focus = got_focus;
        if act.is_some() || close {
            self.menu_open = false;
            self.menu_got_focus = false;
        }
        if let Some(a) = act {
            self.do_tray_action(ctx, a);
        }
    }

    // ── 意图处理 ──

    fn handle_control(&mut self, ctx: &egui::Context, res: control::ControlResult) {
        if res.style_changed {
            let lvl = window::level_of(self.cfg.layer);
            // 层级与穿透是两件事，两个都要发全（两者独立，四种组合都可能）
            let pt = self.cfg.passthrough;
            for f in &self.floats {
                let vid = ViewportId::from_hash_of(("lexideck-float", f.id));
                ctx.send_viewport_cmd_to(vid, ViewportCommand::WindowLevel(lvl));
                ctx.send_viewport_cmd_to(vid, ViewportCommand::MousePassthrough(pt));
            }
        }
        if res.open_taskbar_settings {
            open_taskbar_settings();
            self.status = Some(
                "已打开「任务栏」设置：把 Lexideck 从「其他系统托盘图标」里打开，图标就常驻了"
                    .into(),
            );
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
            if self.use_tray() {
                // 托盘模式：面板的 ✕ 首击 = 收进托盘（不进入退出确认）
                self.hide_panel(ctx);
                self.status =
                    Some("已收进托盘（图标在右下角 ^ 折叠区；右键唤回，或再双击一次 exe）".into());
            } else {
                self.confirm_exit = true;
            }
        }
        if res.minimize {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        if res.changed {
            config::save(&config::settings_path(), &self.cfg);
        }
        if res.exit {
            self.quit_now(ctx);
        }
        if res.quit_now {
            self.quit_now(ctx);
        }
        // 设置页「关闭窗口时」两档：更新 + 存盘 + 反馈
        if let Some(a) = res.set_close_action {
            self.cfg.close_action = a;
            config::save(&config::settings_path(), &self.cfg);
            self.status = Some(match a {
                CloseAction::Tray => {
                    "关闭窗口时：驻留托盘（点 ✕ 收进托盘；图标在右下角 ^ 折叠区）".into()
                }
                CloseAction::Exit => "关闭窗口时：直接退出（5 秒内点两次 ✕ 才会退）".into(),
            });
        }
        // 设置页「开机自启动」：写注册表（开关显示以注册表为准，所以写完成败都照实反馈）
        if let Some(on) = res.set_autostart {
            let exe = autostart::current_exe();
            let done = if on {
                autostart::enable(&exe)
            } else {
                autostart::disable()
            };
            match done {
                Ok(()) => {
                    self.cfg.autostart = on;
                    config::save(&config::settings_path(), &self.cfg);
                    self.status = Some(if on {
                        format!("开机自启动已开启：{}", exe.display())
                    } else {
                        "开机自启动已关闭（启动项已删掉）".into()
                    });
                }
                Err(e) => {
                    self.status = Some(format!(
                        "开机自启动没改成：{e}（开关位置显示的是注册表里的实际状态）"
                    ));
                }
            }
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

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 只有根 viewport 是"控制面板"
        if ctx.viewport_id() != egui::ViewportId::ROOT {
            return;
        }
        fps_tick(&ctx, "root");
        // 面板句柄每帧记一次：收进托盘时要靠它摘任务栏按钮（见 hide_panel）
        self.panel_hwnd = panel_hwnd(frame);

        // P3a：托盘懒创建 + 收事件；关闭拦截必须在 place_once 之前
        // （CancelClose 必须和 close_requested 同帧发出，eframe 只认那一帧）
        self.tray_tick(&ctx, frame);
        // P3c 单实例：别人又启动了一次（第二次双击 exe）→ 敲门线程已置标记，这里把面板唤到前台
        if single::take_wake() {
            self.show_panel(&ctx);
            self.status = Some("Lexideck 已经在跑：把面板唤到了前台".into());
        }
        self.intercept_close(&ctx);

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
                    // 确认过期就别在底栏留「再点一次…」的过期提示
                    if self.status.as_deref() == Some(EXIT_CONFIRM_HINT) {
                        self.status = None;
                    }
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
        self.draw_menu(&ctx, &th);

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

/// 打开 Windows 的「任务栏」设置页。
///
/// 用在哪：Windows 11 默认把新托盘图标收进右下角的 `^` 折叠区，**应用自己抢不到常驻位**
/// （只能由用户在设置里打开）—— 面板里给一个直达入口，省得用户找不到唤回面板的图标。
/// 目标页是「设置 → 个性化 → 任务栏」，里面就有「其他系统托盘图标」。
fn open_taskbar_settings() {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", "ms-settings:taskbar"])
        .spawn();
}

/// 面板窗口句柄（托盘唤醒要用，见 tray.rs 文件头）。拿不到就回 0 ——
/// 只是少一道唤醒保险，其它功能不受影响。
fn panel_hwnd(frame: &eframe::Frame) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match frame.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
        _ => 0,
    }
}

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
