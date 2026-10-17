//! 界面语言（中文 / English）。
//!
//! 所有界面文案通过 [`l`] 取当前语言版本：源码里照常写中文原文，英文表里登记对应翻译；
//! 英文表查不到的条目会回退成中文（不会出现空白）。
//! 带变量的状态行用 [`tf`]（命名占位符 `{name}`，中英模板共用一套占位符）。
//!
//! 2026-10-17 新增。语言存在 `设置.txt` 的 `lang = zh|en`。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    Zh,
    En,
}

impl Lang {
    pub fn parse(s: &str) -> Lang {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "english" | "英文" => Lang::En,
            _ => Lang::Zh,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }
}

static CUR: AtomicU8 = AtomicU8::new(0); // 0 = zh, 1 = en

pub fn set(lang: Lang) {
    CUR.store(
        match lang {
            Lang::Zh => 0,
            Lang::En => 1,
        },
        Ordering::Relaxed,
    );
}

pub fn current() -> Lang {
    if CUR.load(Ordering::Relaxed) == 1 {
        Lang::En
    } else {
        Lang::Zh
    }
}

fn en_map() -> &'static HashMap<&'static str, &'static str> {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| EN_TABLE.iter().copied().collect())
}

/// 取界面文案：中文原文 → 当前语言（英文查不到就回退中文）
pub fn l<'a>(zh: &'a str) -> &'a str {
    if current() == Lang::En {
        en_map().get(zh).copied().unwrap_or(zh)
    } else {
        zh
    }
}

/// 带变量的文案：`{name}` 占位符，中英模板共用。
pub fn tf(tpl: &'static str, args: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut s = l(tpl).to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), &v.to_string());
    }
    s
}

/// 中文原文 → English。表按使用场合分组，便于维护。
static EN_TABLE: &[(&str, &str)] = &[
    // ── 顶栏 / 标签页 ──
    ("词卡看板", "Word Card Board"),
    ("词表", "Words"),
    ("卡片", "Cards"),
    ("策略", "Plans"),
    ("设置", "Settings"),
    // ── 词表页 ──
    ("导入词表…", "Import words…"),
    ("重新读取", "Reload"),
    ("搜索单词或释义…", "Search word or meaning…"),
    ("全部", "All"),
    ("未选", "Unselected"),
    ("已选", "Selected"),
    ("词库还是空的 —— 点右上「导入词表…」选一份 JSON", "The library is empty — click \"Import words…\" at the top right and pick a JSON file"),
    ("没有符合条件的词条", "No entries match the filter"),
    ("勾几个词 → 右下角「立即展示」，卡片就出现在桌面上。", "Tick a few words, then click \"Show now\" at the bottom right — cards will appear on the desktop."),
    ("词条总数", "Entries"),
    ("展示时间条目", "Schedule items"),
    ("面板里不提供改词条内容的功能——改内容请改 JSON 再导入。", "The panel cannot edit entry content — edit the JSON and import it again."),
    ("格式不对的文件会被整份拒绝，主库不受影响。", "A malformed file is rejected as a whole; the library stays untouched."),
    ("按今日展示", "Show today's deck"),
    ("保存设置", "Save settings"),
    ("重播动画", "Replay animation"),
    ("设置已保存到 exe 旁的「设置.txt」（改完即时生效）", "Settings saved to the file next to the exe (applies immediately)."),
    ("再点一次「×」退出程序（5 秒内有效）", "Click \"×\" again to quit (valid for 5 seconds)"),
    // ── 卡片页 ──
    ("桌面卡片", "Desktop cards"),
    ("屏幕上现在的内容。位置由平铺规则决定（右上角起、往下排、满列向左），放不下按顺序截断。", "What is on screen now. Positions follow the tiling rule (from the top-right corner, going down, then leftwards); overflow is truncated in order."),
    ("今日生效", "Active today"),
    ("已上屏", "On screen"),
    ("字号档位", "Font size"),
    ("屏幕上的卡片", "Cards on screen"),
    ("顺序 = 上屏顺序", "Order = display order"),
    ("截断时先来先上", "First come, first shown"),
    ("桌面没有卡片 —— 今天没有排期，或在「词表」里勾几个词点「立即展示」", "No cards on the desktop — nothing is scheduled today, or tick some words in \"Words\" and click \"Show now\""),
    ("屏幕内容从哪来", "Where the screen content comes from"),
    ("手动展示", "Manual"),
    ("自动：按今天的策略上屏，跨过零点自动换下一批；今天没有排期就是空白屏幕。", "Auto: shows today's plan; switches to the next batch after midnight; a blank screen when nothing is scheduled."),
    ("手动：只显示你在「词表」里勾选的那几个词（调试用）；想回到自动就点右下角「按今日展示」。", "Manual: only shows the words you ticked in \"Words\" (for debugging); click \"Show today's deck\" to go back."),
    ("字号", "Font size"),
    ("全局一个档位，所有卡片一起变", "One global step; all cards change together"),
    ("复位 0.70", "Reset 0.70"),
    ("卡片可以拖开临时让位；下次重排（卡片增删、字号变化）会回到平铺位置。", "Cards can be dragged aside temporarily; the next re-layout (cards added/removed, font-size change) moves them back."),
    // ── 策略页 ──
    ("展示策略", "Display plans"),
    ("一条策略 = 一个词的一段展示时间。关掉它，这个词在这段时间就不上屏，永久生效，直到你在这里重新打开。", "One plan = one word's display window. Turning it off hides that word for this window, permanently, until you turn it back on here."),
    ("今天上屏", "On screen today"),
    ("被永久关闭", "Permanently off"),
    ("未来排期", "Upcoming"),
    ("主库里还没有展示时间 —— 导入一份带 display_time 的文件即可。", "No display windows yet — import a file containing display_time."),
    ("正在生效", "Active now"),
    ("今天就在屏幕上", "On screen today"),
    ("今天没有生效的策略（屏幕是空白的）", "No plans active today (the screen is blank)"),
    ("还没开始", "Not started"),
    ("按日期自动接上", "Starts automatically by date"),
    ("没有排在未来 —— 想提前排课就在词表 JSON 里加 display_time", "Nothing scheduled ahead — add display_time in the word-list JSON to plan in advance"),
    ("已经结束", "Ended"),
    ("还没有结束的展示策略", "No ended plans yet"),
    // ── 设置页 ──
    ("面板与卡片共用主题；设置自动存到 exe 旁的「设置.txt」，策略的开关存在「状态.json」。", "The panel and cards share the theme; settings are saved automatically next to the exe, plan switches go to the state file."),
    ("卡片层级", "Card layer"),
    ("其他课的 PPT 不能被挡住，所以默认置底", "Other lessons' PPT must not be covered, so bottom is the default"),
    ("置底（默认）", "Bottom (default)"),
    ("卡片贴在桌面最底层，任何窗口都会盖住它。代价：全屏放 PPT 时卡片看不见，只在课间/桌面状态看得到。", "Cards sit at the very bottom of the desktop; any window covers them. Cost: invisible during full-screen PPT — only visible between classes."),
    ("置顶", "Top"),
    ("卡片始终压在其它窗口最上层。代价：会占掉 PPT 右侧版面；若不配合穿透，点卡片会落在卡片上。", "Cards always stay above other windows. Cost: takes up the right side of the PPT; without passthrough, clicks land on the cards."),
    ("鼠标穿透", "Click-through"),
    ("和层级无关，四种组合都能选", "Independent of the layer; all four combinations are available"),
    ("不穿透（默认）", "Off (default)"),
    ("卡片照常接收鼠标：点卡片会落在卡片上，拖着卡片挪位置也要靠它。", "Cards receive the mouse as usual: clicks land on the cards, and dragging cards relies on it."),
    ("穿透", "On"),
    ("卡片收不到任何鼠标事件，点击全部穿过去落到 PPT 上（贴在 PPT 上层常显时用它）。", "Cards receive no mouse events at all; every click falls through to the PPT (use it when showing cards above the PPT)."),
    ("穿透是整个窗口粒度的：开启后卡片收不到任何鼠标事件，恢复通道只有这个面板 —— 面板里提供「关闭穿透」和「隐藏全部卡片」。", "Passthrough applies to the whole window: once on, cards receive no mouse events, and this panel is the only way back — it offers turning it off and hiding all cards."),
    ("隐藏全部卡片", "Hide all cards"),
    ("主题", "Theme"),
    ("只换配色，不动版式（版式一变，教室可读性要重测）", "Colors only — the layout stays fixed"),
    ("动画流畅度", "Animation smoothness"),
    ("卡片入场动画的帧率上限：跟着屏幕走最省，不限帧最快", "Frame-rate cap for card animations"),
    ("跟随显示器（默认）", "Follow display (default)"),
    ("帧率跟着屏幕刷新率走：60Hz 屏就是 60 帧，165Hz 屏最高 165 帧（这台实测 118 帧）。不撕裂，也不白烧 CPU。", "Follows the display refresh rate: 60 fps on a 60 Hz screen, up to 165 fps on a 165 Hz screen (118 fps measured here). No tearing, no wasted CPU."),
    ("不限帧（最快）", "Unlimited (fastest)"),
    ("关掉垂直同步，能跑多快跑多快（这台实测 177 帧）。代价：更吃 CPU/GPU，60Hz 屏上可能撕裂。", "Vsync off — as fast as it can go (177 fps measured here). Cost: more CPU/GPU, possible tearing on 60 Hz screens."),
    ("这一项在程序启动时生效，改完要重开一次 Lexideck；动画本身不受影响，只影响它每秒画多少帧。", "This setting applies at startup — restart Lexideck after changing it. Animations themselves are unaffected; only the frame rate changes."),
    ("关闭窗口时", "When closing the window"),
    ("面板顶栏的 ✕（或系统关闭）：驻留托盘 = 收进托盘；直接退出 = 走 5 秒二次确认", "The ✕ at the top bar (or the system close): \"Keep in tray\" = move to the tray; \"Quit\" = needs a second click within 5 seconds"),
    ("驻留托盘（默认）", "Keep in tray (default)"),
    ("关掉面板后程序继续跑，卡片不掉；点托盘图标（在右下角 ^ 折叠区）、用托盘右键菜单、或再双击一次 exe 都能唤回面板。", "The app keeps running with cards left on the desktop; get the panel back from the tray icon (in the ^ overflow area), the tray right-click menu, or by launching the exe again."),
    ("直接退出", "Quit"),
    ("关掉面板 = 退出程序（5 秒内点两次 ✕ 确认）。教室策略挡掉托盘图标时用这档。", "Closing the panel quits the app (click ✕ twice within 5 seconds). Use this when classroom policies block the tray icon."),
    ("退出 Lexideck", "Quit Lexideck"),
    ("点了立即退出（驻留托盘时也能从这里退）", "Quits immediately (works even when the panel is set to stay in the tray)"),
    ("打开任务栏设置", "Open taskbar settings"),
    ("开机自启动", "Start with Windows"),
    ("写当前用户的启动项，免管理员；开关位置反映注册表里的实际情况", "Writes the current user's startup entry; no admin needed; the switch reflects the real registry state"),
    ("重写为当前位置", "Rewrite to current location"),
    ("把启动项改成当前这个 exe（整个文件夹搬过位置以后点这里）", "Point the startup entry at this exe (use it after moving the folder)"),
    ("单实例", "Single instance"),
    ("已启用：第二次双击不会再开一个窗口，会把已有窗口（藏在托盘里也行）唤到前台", "Enabled: launching again won't open a second window — it brings the existing one to the front (even from the tray)"),
    ("解析与校验提示", "Parse & validation notes"),
    // ── 弹窗 / 行内标签 ──
    ("知道了", "Got it"),
    ("替换", "Replace"),
    ("跳过", "Skip"),
    ("全部替换", "Replace all"),
    ("全部跳过", "Skip all"),
    ("主库里", "In the library"),
    ("这份文件", "This file"),
    ("这份文件没有被导入，主库保持原样：", "This file was not imported; the library is unchanged:"),
    ("完整", "Complete"),
    ("已关闭", "Off"),
    ("出卡片", "Card shown"),
    ("（没记来源）", "(source not recorded)"),
    ("无，按词条自己的显隐来", "None — follows the entry's own hiding"),
    ("（导入时记下，只作展示）", " (recorded at import, display only)"),
    ("（只在这段时间里生效，不改词条本身）", " (applies only within this window; the entry itself is unchanged)"),
    ("来源文件", "Source file"),
    ("日期段", "Date range"),
    ("操作", "Actions"),
    ("这条已永久关闭，卡片不会出现在屏幕上", "This plan is permanently off; its card won't appear on screen"),
    ("点右侧开关可永久关闭这条策略的卡片", "Use the switch on the right to permanently turn this plan's card off"),
    ("当前", "Current"),
    ("版本", "Version"),
    ("今天生效", "Active today"),
    ("明天开始", "Starts tomorrow"),
    ("已结束", "Ended"),
    // ── 词条明细标签 ──
    ("词义", "Meaning"),
    ("变形", "Forms"),
    ("同根词", "Related words"),
    ("短语", "Phrases"),
    ("例句", "Examples"),
    ("备注", "Note"),
    // ── 状态行 ──
    ("已收进托盘（图标在右下角 ^ 折叠区；右键唤回，或再双击一次 exe）", "Moved to the tray (icon in the ^ overflow area; right-click to bring it back, or launch the exe again)"),
    ("托盘图标已就绪：关掉面板＝收进托盘（图标在右下角 ^ 折叠区，可拖出来常驻）", "Tray icon ready: closing the panel keeps the app in the tray (icon in the ^ overflow area; drag it out to pin it)"),
    ("面板已收进托盘（图标在右下角 ^ 折叠区；右键唤回，或再双击一次 exe）", "Panel moved to the tray (icon in the ^ overflow area; right-click to bring it back, or launch the exe again)"),
    ("面板已唤回", "Panel is back"),
    ("已打开「任务栏」设置：把 Lexideck 从「其他系统托盘图标」里打开，图标就常驻了", "Opened the taskbar settings: enable Lexideck under \"Other system tray icons\" to keep the icon around"),
    ("已重新读取主库", "Library reloaded"),
    ("先在词表里勾几个词，再点「立即展示」", "Tick some words in the word list first, then click \"Show now\""),
    ("已关闭全部卡片 · 恢复：底部「按今日展示」按钮，或托盘菜单「显示全部卡片」", "All cards closed · to restore: use \"Show today's deck\" at the bottom, or \"Show all cards\" in the tray menu"),
    ("已关闭全部卡片", "All cards closed"),
    ("已切到手动展示（还没勾词 —— 去「词表」勾几个词点「立即展示」）", "Switched to manual mode (nothing ticked yet — tick some words in \"Words\" and click \"Show now\")"),
    ("关闭窗口时：驻留托盘（点 ✕ 收进托盘；图标在右下角 ^ 折叠区）", "Close: keep in tray (✕ moves it to the tray; icon in the ^ overflow area)"),
    ("关闭窗口时：直接退出（5 秒内点两次 ✕ 才会退）", "Close: quit (needs two ✕ clicks within 5 seconds)"),
    ("开机自启动已关闭（启动项已删掉）", "Start with Windows disabled (startup entry removed)"),
    ("状态文件读不懂，已拒绝写入（避免覆盖坏文件）；请先修好或删掉 状态.json", "The state file is unreadable; writing refused (to protect it). Fix or delete 状态.json first."),
    ("导入失败：文件格式不对，主库未改动", "Import failed: invalid file format; the library is unchanged"),
    ("导入已中止：主库文件现在读不懂，先修好或删掉 lexideck.json 再来", "Import aborted: the library is unreadable right now — fix or delete lexideck.json first"),
    ("导入已中止：这期间主库被外部改过，请再导入一次", "Import aborted: the library changed on disk during the import — please import again"),
    ("Lexideck 已经在跑：把面板唤到了前台", "Lexideck is already running: the panel has been brought to the front"),
    ("主库文件不见了（被删或改名），现在用的是内存里那份", "The library file is gone (deleted or renamed); using the in-memory copy"),
    ("主库文件被外部改动，已重新读取", "The library file changed on disk; reloaded"),
    ("主库文件暂时不见了（被删或改名）——继续用内存里那份，文件回来会自动重读", "Library file temporarily missing (deleted or renamed) — continuing with the in-memory copy; it reloads when the file returns"),
    ("再点一次 ✕ 就退出（5 秒内有效）", "Click ✕ again to quit (valid for 5 seconds)"),
    // ── 托盘菜单 / tooltip ──
    ("隐藏面板", "Hide panel"),
    ("显示面板", "Show panel"),
    ("显示全部卡片", "Show all cards"),
    ("Lexideck — 词卡看板控制面板", "Lexideck — Word Card Board panel"),
    // ── 带变量的模板（tf 用，中英共用 {name} 占位符） ──
    ("词库：{name}", "Word library: {name}"),
    ("{n} 条词条 · {m} 条展示时间 · 导入后自动存盘", "{n} entries · {m} schedules"),
    ("立即展示（{n}）", "Show now ({n})"),
    ("运行中 · 今日 {today} · 生效 {active} 条 / 上屏 {shown} 张 · 主库 {lib}", "Running · {today} · {active} active / {shown} on screen · library {lib}"),
    ("{n} 条", "{n}"),
    ("展示已经结束的展示策略（{n} 条）", "Show ended plans ({n})"),
    ("还有 {d} 天开始", "Starts in {d} days"),
    ("已让 {n} 个词上屏（本次放得下 {fit} 张；放不下的排在后面，见「卡片」页）", "Showing {n} words ({fit} fit on screen; the rest were truncated — see \"Cards\")"),
    ("已让 {n} 个词上屏（手动展示）", "Showing {n} words (manual mode)"),
    ("{n} 个义项", "{n} senses"),
    ("{n} 个同根词", "{n} related words"),
    ("{n} 个短语", "{n} phrases"),
    ("{n} 条例句", "{n} examples"),
    ("（没有内容）", "(no content)"),
    ("(无名)", "(unnamed)"),
    ("未命名", "Untitled"),
    ("为什么卡片不能拖：位置完全由平铺规则决定，教室机器上被拖乱就没人能还原。", "Why cards can't be dragged: positions follow the tiling rule; once moved around on a classroom machine, nobody can put them back."),
    ("词库", "Word library"),
    ("词条只从 JSON 文件来。导入 = 并进主库：同名词条更新、其余追加。", "Entries come only from JSON files. Import merges into the library: same-name entries are updated, everything else is appended."),
    ("两种来源：按今日展示（自动）／手动展示（调试用）。启动时默认按今日展示。", "Two sources: today's plan (auto) / manual (debugging). Today's plan is the default at startup."),
    ("截断规则", "Truncation rule"),
    ("放不下就按顺序截断：先排的先上屏，后面的不显示，面板会把「生效 N 个 / 已显示 M 个」写清楚。", "When they don't fit, cards are truncated in order: first queued, first shown; the panel reports active N / on screen M."),
    ("为什么卡片不能拖", "Why cards can't be dragged"),
    ("位置完全由排列规则决定，教室机器上被拖乱就没人能还原。", "Positions follow the tiling rule completely; once messed up on a classroom machine, nobody can restore them."),
    ("本次生效", "Active this session"),
    ("屏幕上显示", "On screen"),
    ("怎么理解这一页", "How to read this page"),
    ("一条策略 = 一个词的一段展示时间。同一个词可以有好几段（比如第 5 周再轮一次），每段各自开关。", "One plan = one display window for one word. The same word can have several windows (e.g. again in week 5); each window has its own switch."),
    ("关掉会怎样", "What turning it off does"),
    ("关掉就是永久的：屏幕上不再出现这个词这段时间的卡片，第二天也不会自己恢复，必须在这里手动打开。", "It's permanent: no card for this word during this window, and it won't come back the next day by itself — you must turn it back on here."),
    ("关掉是不是删了内容", "Does turning it off delete content?"),
    ("不是。词条和日期段都还在主库里，只是这段时间不上屏。", "No. The entry and its date ranges stay in the library; they just don't go on screen during this window."),
    ("今天没有排期 → 屏幕空白（不沿用上一批）。", "Nothing scheduled today → blank screen (the previous batch is not reused)."),
    ("为什么默认置底", "Why bottom is the default"),
    ("其他课上 PPT 不能被卡片遮挡，所以默认藏在最底层；需要常显就切「置顶」。", "Cards must not cover the PPT in other lessons, so they hide at the very bottom by default; switch to Top when they must stay visible."),
    ("层级和穿透是两件事", "Layer and click-through are two things"),
    ("层级决定卡片压在谁上面，穿透决定点击能不能穿过卡片，两边各自独立、四种组合都能选。常见搭配是「置顶 + 穿透」：卡片常显，点击照常落到 PPT 上。", "The layer decides what covers what; click-through decides whether clicks pass through the cards. They are independent — all four combinations work. The usual pairing is Top + click-through: cards always visible, clicks still reach the PPT."),
    ("穿透的风险", "Risks of click-through"),
    ("开启后卡片点不到，只能从这个面板恢复。所以面板本身永远是正常可交互的。", "Once on, cards can't be clicked; this panel is the only way back — so the panel itself always stays interactive."),
    ("设置都写在哪", "Where settings live"),
    ("主库 lexideck.json、设置 设置.txt、策略开关 状态.json —— 全在 exe 同目录，整个文件夹拷走就带走全部状态。", "Library lexideck.json, settings 设置.txt, plan switches 状态.json — all next to the exe; copy the whole folder to take everything with you."),
    ("提示", "Note"),
    ("导入失败：写入主库出错（{e}），主库未改动", "Import failed: error writing the library ({e}); the library is unchanged"),
    ("已回到今日展示：今日生效 {n} 个", "Back to today's plan: {n} active"),
    ("已切回按今日展示：今日生效 {n} 个词", "Switched back to today's plan: {n} active"),
    ("已切到手动展示：显示勾选的 {n} 个词", "Switched to manual mode: showing {n} ticked words"),
    ("已开启，但指向的是旧位置：{p}", "On, but pointing at an old location: {p}"),
    ("读不到启动项：{e}", "Cannot read the startup entry: {e}"),
    ("已开启：登录这台机器后自动启动（登记的就是当前这个 exe）", "On: starts automatically after you log in (registered to this exact exe)"),
    ("未开启 —— 设置文件里记的是「开启」，但注册表里没有这一项（多半被安全软件清过）。拨一下开关重新登记。", "Off — the settings file says \"on\", but the registry entry is missing (probably cleaned by security software). Toggle once to register again."),
    ("未开启：开机后要手动打开 Lexideck", "Off: open Lexideck manually after boot"),
    ("开机自启动已开启：{p}", "Start with Windows enabled: {p}"),
    ("开机自启动没改成：{e}（开关位置显示的是注册表里的实际状态）", "Could not change the startup entry: {e} (the switch shows the real registry state)"),
    ("已关闭「{word}」这条策略", "Plan \"{word}\" turned off"),
    ("已打开「{word}」这条策略", "Plan \"{word}\" turned back on"),
    ("保存状态失败：{e}", "Failed to save the state: {e}"),
    ("日期变了：今日生效 {n} 个", "Date changed: {n} active today"),
    ("托盘图标创建失败（第 {n} 次）：{e}；关闭窗口按「直接退出」处理，稍后自动重试", "Tray icon creation failed (attempt {n}): {e}; closing the window will quit the app; retrying automatically"),
    ("托盘图标创建失败：{e}", "Tray icon creation failed: {e}"),
    ("托盘图标数据不合法：{e}", "Invalid tray icon data: {e}"),
    ("版本 {v}　·　github.com/FanHuaWuYang/Lexideck", "Version {v}　·　github.com/FanHuaWuYang/Lexideck"),
    ("免安装绿色版：设置、主库、状态都放在 exe 同目录，整个文件夹拷走就带走全部状态", "Portable: settings, library and state all live next to the exe — copy the whole folder to move everything"),
    ("界面语言", "Interface language"),
    ("切换界面语言（立即生效）", "Switch the interface language (applies immediately)"),
    ("中文", "中文"),
    ("纯白", "Plain"),
    ("浅青", "Mist"),
    ("黄黑", "Yellow & Black"),
    ("中文界面", "Chinese interface"),
    ("English", "English"),
    // ── 追加模板（2026-10-17 接入时补） ──
    ("{n} 已关闭展示", "{n} hidden"),
    ("隐藏 {n}", "Hidden {n}"),
    ("这段隐藏 {names}", "Hidden: {names}"),
    ("导入失败 · {f}", "Import failed · {f}"),
    ("…还有 {n} 条", "…and {n} more"),
    ("导入冲突 · {i} / {n}", "Import conflict · {i} / {n}"),
    ("「{word}」在主库里已经存在，内容和这份文件里的不一样：", "\"{word}\" already exists in the library with different content:"),
    ("「全部」= 本次导入剩下的 {n} 条不再逐条询问", "\"All\" = don't ask again for the remaining {n} items in this import"),
    ("自启动只在登录后把程序拉起来，卡片照常按今天该显示的词上屏；把启动项里这一条删掉就等于关闭。", "Startup only launches the app after login; cards follow today's plan as usual. Removing the startup entry is the same as turning it off."),
    ("托盘图标去哪了：Windows 11 默认把新图标收进任务栏右下角的 ^ 折叠区，不进常驻区（程序自己抢不进去）。", "Where did the tray icon go: Windows 11 puts new icons into the ^ overflow area at the bottom-right of the taskbar by default — apps cannot pin themselves there."),
    ("注意：托盘图标没能进入系统通知区（外壳静默拒绝了它）——已按「没有托盘」处理并自动重试；已知一类原因是 exe 所在的文件夹，把 exe 换到别的文件夹通常立刻恢复", "Note: the tray icon did not reach the notification area (silently rejected by the shell) — treated as \"no tray\" and retrying automatically; a known trigger is the exe's folder — moving the exe elsewhere usually fixes it"),
    ("托盘图标失联了（系统可能重启过桌面）——已按「没有托盘」处理并自动重建", "The tray icon went missing (the desktop shell may have restarted) — treated as \"no tray\" and rebuilt automatically"),
];
