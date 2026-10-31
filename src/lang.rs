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
    ("词表", "Word list"),
    ("卡片", "Cards"),
    ("排期", "Schedule"),
    ("设置", "Settings"),
    // ── 词表页 ──
    ("导入词表…", "Import word list…"),
    ("重新读取", "Reload library"),
    ("搜索单词或释义…", "Search word or meaning…"),
    ("全部", "All"),
    ("未选", "Unselected"),
    ("已选", "Selected"),
    ("词库还是空的 —— 点右上「导入词表…」选一份 JSON", "The library is empty. Choose \"Import word list…\" at the top right to load a JSON file."),
    ("没有符合条件的词条", "No entries match the filter"),
    ("勾选几个词，点右下角「立即展示」，卡片就会出现在桌面上。", "Select a few words, then click \"Show now\" at the bottom right to put them on the desktop."),
    ("词条总数", "Entries"),
    ("展示时间条目", "Schedule items"),
    ("词条内容需修改 JSON 后重新导入。", "Edit entries in the JSON file and import again."),
    ("格式错误时整份拒绝，主库不变。", "Invalid files are rejected entirely; the library is unchanged."),
    ("按今日展示", "Show today's schedule"),
    ("保存设置", "Save settings"),
    ("重播动画", "Replay animation"),
    ("设置已保存（即时生效）", "Settings saved"),
    ("再点一次「×」退出程序（5 秒内有效）", "Click \"×\" again to quit (valid for 5 seconds)"),
    // ── 卡片页 ──
    ("桌面卡片", "Desktop cards"),
    ("屏幕当前内容。按平铺规则排列，超出部分按顺序截断。", "Current screen content; tiles flow from the top right, overflow is truncated in order."),
    ("今日生效", "Active today"),
    ("已上屏", "On screen"),
    ("字号档位", "Font size"),
    ("屏幕上的卡片", "Cards on screen"),
    ("顺序 = 上屏顺序", "Order = display order"),
    ("超出按序截断", "Truncated in order"),
    ("桌面上没有卡片——今天没有排期，或尚未在「词表」中勾选词条", "No cards on the desktop — nothing is scheduled today, or no words are selected in \"Word list\"."),
    ("内容来源", "Content source"),
    ("手动展示", "Manual"),
    ("按今日排期上屏，零点后自动切换；无排期时屏幕空白。", "Shows today's schedule; switches after midnight; blank when nothing is scheduled."),
    ("手动：只显示你在「词表」里勾选的那几个词（调试用）；想回到自动就点右下角「按今日展示」。", "Manual: only shows the words you selected in \"Word list\" (for debugging); click \"Show today's schedule\" to go back."),
    ("字号", "Font size"),
    ("统一字号", "Global size"),
    ("复位 0.70", "Reset 0.70"),
    ("可拖动卡片临时让位，重排后自动归位。", "Cards can be moved aside; they return on the next re-layout."),
    // ── 策略页 ──
    ("展示排期", "Display schedule"),
    ("一条排期对应一个词的一段展示时间。关闭后该时段不再上屏，直至手动重新开启。", "One entry = one display window for one word. Turning it off hides it until you turn it back on."),
    ("今天上屏", "On screen today"),
    ("被永久关闭", "Permanently off"),
    ("未来排期", "Upcoming"),
    ("暂无展示时间，请导入含 display_time 的文件", "No display windows; import a file with display_time"),
    ("正在生效", "Active now"),
    ("今日生效", "Active today"),
    ("今日无生效排期", "No schedules active today"),
    ("还没开始", "Not started"),
    ("到期自动生效", "Starts by date"),
    ("未来无排期；可在词表 JSON 中添加 display_time", "Nothing scheduled ahead; add display_time to the word list"),
    ("已经结束", "Ended"),
    ("暂无已结束排期", "No ended schedules"),
    // ── 设置页 ──
    ("设置自动保存至「设置.txt」，排期开关保存至「状态.json」。", "Settings are saved to 设置.txt; schedule switches to 状态.json."),
    ("卡片层级", "Card layer"),
    ("默认置底，避免遮挡课件", "Bottom by default to avoid covering slides"),
    ("置底（默认）", "Bottom (default)"),
    ("位于桌面最底层，任何窗口均可覆盖；全屏演示时不可见。", "Bottom layer; any window covers it and it is hidden during full-screen slides."),
    ("置顶", "Top"),
    ("始终位于其他窗口上方，占用屏幕右侧区域。", "Always above other windows; occupies the right side of the screen."),
    ("鼠标穿透", "Click-through"),
    ("与层级独立", "Independent of layer"),
    ("不穿透（默认）", "Off (default)"),
    ("卡片接收鼠标事件，可点击与拖动。", "Cards receive mouse input and can be clicked and dragged."),
    ("穿透", "On"),
    ("卡片不接收鼠标事件，点击传递至下层窗口。", "Cards receive no mouse input; clicks pass through."),
    ("穿透作用于整个卡片窗口。开启后仅可通过本面板关闭穿透或隐藏全部卡片。", "Click-through applies to the whole card window; use this panel to disable it or hide all cards."),
    ("隐藏全部卡片", "Hide all cards"),
    ("主题", "Theme"),
    ("仅更换配色", "Colors only"),
    ("动画流畅度", "Animation"),
    ("动画帧率上限", "Animation frame-rate cap"),
    ("跟随显示器（默认）", "Follow display (default)"),
    ("帧率跟随显示器刷新率，避免撕裂与额外占用。", "Frame rate follows the display refresh rate."),
    ("不限帧（最快）", "Unlimited (fastest)"),
    ("关闭垂直同步，以最高帧率绘制；占用较高，可能出现撕裂。", "Vsync off; highest frame rate, higher load, possible tearing."),
    ("重启程序后生效。", "Applies after restart."),
    ("关闭窗口时", "When closing the window"),
    ("面板顶栏的 ✕（或系统关闭）：驻留托盘 = 收进托盘；直接退出 = 走 5 秒二次确认", "The ✕ in the title bar (or the system close button): \"Keep in tray\" minimizes to the tray; \"Quit\" requires a second click within 5 seconds"),
    ("驻留托盘（默认）", "Keep in tray (default)"),
    ("关闭面板后程序继续运行，卡片保持显示；可通过托盘图标或再次启动唤回面板。", "The app keeps running; restore from the tray icon or by launching again."),
    ("直接退出", "Quit"),
    ("关闭面板即退出程序，需连续点击两次 ✕ 确认。", "Closing the panel quits the app; requires two ✕ clicks."),
    ("退出 Lexideck", "Quit Lexideck"),
    ("立即退出程序", "Quit now"),
    ("打开任务栏设置", "Open taskbar settings"),
    ("开机自启动", "Start with Windows"),
    ("当前用户启动项，无需管理员", "Current-user startup entry"),
    ("重写为当前位置", "Rewrite to current location"),
    ("把启动项改为当前程序位置（移动过程序文件夹后点这里）", "Point the startup entry at the current program location (use it after moving the program folder)."),
    ("单实例", "Single instance"),
    ("重复启动不会新建窗口，将唤回已有窗口。", "Launching again brings back the existing window."),
    ("解析与校验", "Parsing and validation"),
    // ── 弹窗 / 行内标签 ──
    ("知道了", "OK"),
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
    ("该排期已永久关闭", "This schedule is permanently off"),
    ("点右侧开关可永久关闭该排期", "Use the switch to permanently hide this schedule"),
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
    ("已收进托盘（图标在右下角 ^ 折叠区；右键唤回，或再次启动 Lexideck）", "Moved to the tray (icon in the ^ overflow area; right-click to restore, or launch the app again)."),
    ("托盘图标已就绪：关掉面板＝收进托盘（图标在右下角 ^ 折叠区，可拖出来常驻）", "Tray icon ready: closing the panel keeps the app in the tray (icon in the ^ overflow area; drag it out to pin it)."),
    ("面板已收进托盘（图标在右下角 ^ 折叠区；右键唤回，或再次启动 Lexideck）", "Panel moved to the tray (icon in the ^ overflow area; right-click to restore, or launch the app again)."),
    ("面板已唤回", "Panel is back"),
    ("已打开「任务栏」设置：把 Lexideck 从「其他系统托盘图标」里打开，图标就常驻了", "Opened the taskbar settings: enable Lexideck under \"Other system tray icons\" to keep the icon around"),
    ("已重新读取主库", "Library reloaded"),
    ("先在词表里勾几个词，再点「立即展示」", "Select some words in the word list first, then click \"Show now\"."),
    ("已关闭全部卡片 · 恢复：底部「按今日展示」按钮，或托盘菜单「显示全部卡片」", "All cards hidden. To restore: use \"Show today\'s schedule\" at the bottom, or \"Show all cards\" in the tray menu."),
    ("已关闭全部卡片", "All cards closed"),
    ("已切到手动展示（还没勾词 —— 去「词表」勾几个词点「立即展示」）", "Switched to manual mode (nothing selected yet — select words in \"Word list\" and click \"Show now\")."),
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
    ("词库：{name}", "Library: {name}"),
    ("{n} 条词条 · {m} 条展示时间 · 导入后自动存盘", "{n} entries · {m} schedules"),
    ("立即展示（{n}）", "Show now ({n})"),
    ("运行中 · 今日 {today} · 生效 {active} 条 / 上屏 {shown} 张 · 主库 {lib}", "Running · {today} · {active} active / {shown} on screen · library {lib}"),
    ("{n} 条", "{n}"),
    ("已结束排期（{n} 条）", "Ended schedules ({n})"),
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
    ("卡片位置由平铺规则决定，避免被随意拖动。", "Card positions follow the tiling rule."),
    ("词库", "Word library"),
    ("词条仅来自 JSON 文件；导入时同名更新、其余追加。", "Entries come from JSON files only; import merges by name."),
    ("来源：今日排期（自动）或手动选择（调试）。", "Source: today's schedule or manual selection."),
    ("截断规则", "Truncation rule"),
    ("超出屏幕时按顺序截断；面板显示生效数与上屏数。", "Overflow is truncated in order; counts are shown in the panel."),
    ("为什么卡片不能拖", "Why cards can't be dragged"),
    ("位置由排列规则决定，避免被随意拖动。", "Positions follow the layout rule."),
    ("本次生效", "Active this session"),
    ("屏幕上显示", "On screen"),
    ("页面说明", "About this page"),
    ("一条排期对应一个词的一段展示时间；同一词可有多段。", "One entry per word per window; a word may have several."),
    ("关闭后", "When turned off"),
    ("关闭后该时段永久不显示，需手动开启。", "Turning it off is permanent until re-enabled here."),
    ("是否删除内容", "Does it delete content"),
    ("不删除内容，仅停止上屏。", "Content is kept; only display stops."),
    ("无排期时屏幕空白，不沿用前一批。", "No schedule: blank screen."),
    ("默认置底", "Bottom by default"),
    ("避免遮挡其他课程的 PPT，默认置底；常显时切「置顶」。", "Bottom by default; switch to Top to keep cards visible."),
    ("层级与穿透", "Layer and click-through"),
    ("层级决定覆盖关系，穿透决定点击去向，两者独立。常用组合为「置顶 + 穿透」。", "Layer and click-through are independent; Top + click-through is common."),
    ("穿透说明", "Click-through notes"),
    ("开启后卡片不可点击，仅能通过本面板恢复。", "Once enabled, only this panel can restore interaction."),
    ("数据位置", "Data location"),
    ("主库、设置与状态文件均位于程序文件夹。", "Library, settings and state live in the program folder."),
    ("提示", "Note"),
    ("导入失败：写入主库出错（{e}），主库未改动", "Import failed: error writing the library ({e}); the library is unchanged"),
    ("已回到今日展示：今日生效 {n} 个", "Back to today's schedule: {n} active"),
    ("已切回按今日展示：今日生效 {n} 个词", "Switched back to today's schedule: {n} active"),
    ("已切到手动展示：显示勾选的 {n} 个词", "Switched to manual mode: showing {n} selected words"),
    ("已开启，但指向的是旧位置：{p}", "On, but pointing at an old location: {p}"),
    ("读不到启动项：{e}", "Cannot read the startup entry: {e}"),
    ("已开启：登录这台机器后自动启动（登记的是当前程序位置）", "On: starts automatically after you sign in (registered to the current program location)"),
    ("未开启 —— 设置文件里记的是「开启」，但注册表里没有这一项（多半被安全软件清过）。拨一下开关重新登记。", "Off — the settings file says \"on\", but the registry entry is missing (probably cleaned by security software). Toggle once to register again."),
    ("未开启：开机后要手动打开 Lexideck", "Off: open Lexideck manually after boot"),
    ("开机自启动已开启：{p}", "Start with Windows enabled: {p}"),
    ("开机自启动没改成：{e}（开关位置显示的是注册表里的实际状态）", "Could not change the startup entry: {e} (the switch shows the real registry state)"),
    ("保存状态失败：{e}", "Failed to save the state: {e}"),
    ("日期变了：今日生效 {n} 个", "Date changed: {n} active today"),
    ("托盘图标创建失败（第 {n} 次）：{e}；关闭窗口按「直接退出」处理，稍后自动重试", "Tray icon creation failed (attempt {n}): {e}; closing the window will quit the app; retrying automatically"),
    ("托盘图标创建失败：{e}", "Tray icon creation failed: {e}"),
    ("托盘图标数据不合法：{e}", "Invalid tray icon data: {e}"),
    ("版本 {v}　·　github.com/FanHuaWuYang/Lexideck", "Version {v}　·　github.com/FanHuaWuYang/Lexideck"),
    ("免安装绿色版：设置、主库、状态都保存在程序文件夹中，复制整个文件夹即可迁移。", "Portable: settings, library and state all live in the program folder; copy the folder to move everything."),
    ("界面语言", "Interface language"),
    ("立即生效", "Applies immediately"),
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
    ("自启动只在登录后把程序拉起来，卡片照常按今天该显示的词上屏；把启动项里这一条删掉就等于关闭。", "Startup only launches the app after sign-in; cards still follow today's schedule. Removing the startup entry is the same as turning it off."),
    ("托盘图标去哪了：Windows 11 默认把新图标收进任务栏右下角的 ^ 折叠区，不进常驻区（程序自己抢不进去）。", "Where did the tray icon go: Windows 11 puts new icons into the ^ overflow area at the bottom-right of the taskbar by default — apps cannot pin themselves there."),
    ("注意：托盘图标未能进入系统通知区（外壳静默拒绝了它）——已按「无托盘」处理并自动重试；已知一类原因是程序所在的文件夹，把程序换到其他文件夹通常可立即恢复", "Note: the tray icon did not reach the notification area (silently rejected by the shell) — treated as \"no tray\" and retrying automatically. A known trigger is the program folder; moving the program to another folder usually fixes it immediately."),
    ("托盘图标失联了（系统可能重启过桌面）——已按「没有托盘」处理并自动重建", "The tray icon went missing (the desktop shell may have restarted) — treated as \"no tray\" and rebuilt automatically"),
];
