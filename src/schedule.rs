//! 展示时间引擎（P2）。
//!
//! 三条规则（`Lexideck.md` §3.3）：
//!   · 以**物理天**为单位（本机本地日期）
//!   · 同一天有多段生效 → **全部显示**
//!   · 当天没有任何段生效 → **屏幕空白**（不沿用上一批）
//!
//! 另外管两件事：
//!   · 把主库里的展示时间展开成一条条「**策略**」= 一个词的一段日期（慕言 2026-10-01 定）
//!   · 策略的**永久关闭**状态，存在 exe 同目录的 `状态.json`（写进去就是永久的，直到手动打开）

use crate::config::exe_dir;
use crate::deck::{self, Deck, Range};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 日期 = (年, 月, 日)
pub type Date = (i32, u32, u32);

// ────────────────────────── 本机日期 ──────────────────────────

#[cfg(windows)]
mod sys {
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    pub struct SystemTime {
        pub year: u16,
        pub month: u16,
        pub day_of_week: u16,
        pub day: u16,
        pub hour: u16,
        pub minute: u16,
        pub second: u16,
        pub ms: u16,
    }
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetLocalTime(t: *mut SystemTime);
    }
}

/// 本机本地日期。
///
/// 这个软件只跑 Windows，直接向系统要 —— 不引第三方日期库，也就没有时区换算、
/// 夏令时算错的余地（依赖树里本来也没有日期库）。
///
/// **测试开关**：环境变量 `LEXIDECK_TODAY=2026-10-06` 就当成那天。用来验证换批、
/// 空白日、已结束折叠，不必真去改系统时钟。
pub fn today() -> Date {
    if let Ok(v) = std::env::var("LEXIDECK_TODAY") {
        if let Some(d) = deck::parse_date(&v) {
            return d;
        }
    }
    #[cfg(windows)]
    {
        let mut st = sys::SystemTime::default();
        // SAFETY: 传给 GetLocalTime 的是本函数栈上一个合法的 POD 结构体指针
        unsafe { sys::GetLocalTime(&mut st) };
        if st.year > 0 {
            return (st.year as i32, st.month as u32, st.day as u32);
        }
    }
    (1970, 1, 1)
}

/// 公历 → 天数（1970-01-01 = 0）。用来算相隔几天，也能给日期排序。
pub fn days(d: Date) -> i64 {
    let (mut y, m, dd) = (d.0 as i64, d.1 as i64, d.2 as i64);
    if m <= 2 {
        y -= 1;
    }
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // 3 月 = 0
    let doy = (153 * mp + 2) / 5 + dd - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// 相隔天数（b - a）
pub fn days_between(a: Date, b: Date) -> i64 {
    days(b) - days(a)
}

// ────────────────────────── 日期段状态 ──────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeState {
    /// 今天落在段内
    Active,
    /// 还没开始
    Future,
    /// 已经过完
    Past,
}

pub fn range_state(r: &Range, today: Date) -> RangeState {
    match (deck::parse_date(&r.from), deck::parse_date(&r.to)) {
        (Some(f), Some(t)) => {
            if today < f {
                RangeState::Future
            } else if today > t {
                RangeState::Past
            } else {
                RangeState::Active
            }
        }
        // 校验过的文件不会走到这里；真坏了就只能当过期，不能让它一直上屏
        _ => RangeState::Past,
    }
}

// ────────────────────────── 策略 ──────────────────────────

/// 一条策略 = 一个词的一段日期
#[derive(Debug, Clone, PartialEq)]
pub struct Strategy {
    /// 词（原样）
    pub word: String,
    pub range: Range,
    pub state: RangeState,
    /// 来自哪份导入文件（没记就空）
    pub source: String,
    /// 这段策略里额外隐藏的区块（来自它所属的展示时间项）
    pub hide: Vec<String>,
}

/// 把主库的展示时间展开成一条条策略（顺序 = 主库顺序，先导入的在前）
pub fn strategies(deck: &Deck) -> Vec<Strategy> {
    let mut out = Vec::new();
    for it in &deck.display_time {
        for r in &it.ranges {
            out.push(Strategy {
                word: it.word.clone(),
                range: r.clone(),
                state: RangeState::Past, // 由调用方带 today 再定，这里给个占位
                source: it.source.clone(),
                hide: it.hide.clone(),
            });
        }
    }
    out
}

/// 同 strategies()，但带上"今天"的状态
pub fn strategies_on(deck: &Deck, today: Date) -> Vec<Strategy> {
    strategies(deck)
        .into_iter()
        .map(|mut s| {
            s.state = range_state(&s.range, today);
            s
        })
        .collect()
}

// ────────────────────────── 关闭状态（状态.json）──────────────────────────

/// 被关掉的一条策略（词 + 单段日期）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OffItem {
    #[serde(default)]
    pub word: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

impl OffItem {
    pub fn new(word: &str, r: &Range) -> Self {
        Self {
            word: word.trim().to_lowercase(),
            from: r.from.trim().to_string(),
            to: r.to.trim().to_string(),
        }
    }
    pub fn key(&self) -> String {
        format!("{}\u{1}{}\u{1}{}", self.word, self.from, self.to)
    }
    pub fn label(&self) -> String {
        format!("{}　{} → {}", self.word, self.from, self.to)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct State {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub off: Vec<OffItem>,
}

impl State {
    pub fn is_off(&self, word: &str, r: &Range) -> bool {
        let k = OffItem::new(word, r).key();
        self.off.iter().any(|o| o.key() == k)
    }

    /// 开关一条策略；返回是否有变化
    pub fn set_off(&mut self, word: &str, r: &Range, off: bool) -> bool {
        let item = OffItem::new(word, r);
        let k = item.key();
        let mut changed = false;
        self.off.retain(|o| {
            if o.key() == k {
                changed = true;
                // off = true 表示"要关"→ 保留这一条；false 表示"要打开"→ 丢掉这一条
                off
            } else {
                true
            }
        });
        if off && !changed {
            self.off.push(item);
            changed = true;
        }
        changed
    }

    /// 主库里已经不存在的关闭项清掉（导入/换库之后）
    pub fn prune(&mut self, deck: &Deck) -> bool {
        let live: Vec<(String, String, String)> = strategies(deck)
            .iter()
            .map(|s| {
                (
                    s.word.trim().to_lowercase(),
                    s.range.from.trim().to_string(),
                    s.range.to.trim().to_string(),
                )
            })
            .collect();
        let before = self.off.len();
        self.off
            .retain(|o| live.contains(&(o.word.clone(), o.from.clone(), o.to.clone())));
        before != self.off.len()
    }
}

pub fn state_path() -> PathBuf {
    exe_dir().join("状态.json")
}

/// 读取关闭状态。返回值第二个是"文件是否可信"：
/// 文件不存在 = 可信（就是全开）；读不懂 = 不可信，调用方**不要**覆盖它。
pub fn load_state(path: &Path) -> (State, bool) {
    if !path.exists() {
        return (State::default(), true);
    }
    match std::fs::read_to_string(path) {
        Ok(t) => match serde_json::from_str::<State>(t.trim_start_matches('\u{feff}')) {
            Ok(mut s) => {
                s.version = 1;
                (s, true)
            }
            Err(_) => (State::default(), false),
        },
        Err(_) => (State::default(), false),
    }
}

pub fn save_state(path: &Path, s: &State) -> Result<(), String> {
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

// ────────────────────────── 今天上屏什么 ──────────────────────────

/// 今天要上屏的一个词（同一词的多个生效段合并成一条）
#[derive(Debug, Clone, PartialEq)]
pub struct Active {
    pub word: String,
    pub source: String,
    /// 今天生效的那些段
    pub ranges: Vec<Range>,
    /// 策略内的隐藏项（多段合并时取并集）
    pub hide: Vec<String>,
}

/// 今天生效且没被关掉的词，按主库顺序（= 先导入的先上屏，截断顺序可预期）
pub fn active_today(deck: &Deck, today: Date, st: &State) -> Vec<Active> {
    let mut out: Vec<Active> = Vec::new();
    for it in &deck.display_time {
        let live: Vec<Range> = it
            .ranges
            .iter()
            .filter(|r| range_state(r, today) == RangeState::Active)
            .filter(|r| !st.is_off(&it.word, r))
            .cloned()
            .collect();
        if live.is_empty() {
            continue;
        }
        let key = it.word.trim().to_lowercase();
        if let Some(a) = out.iter_mut().find(|a| a.word.trim().to_lowercase() == key) {
            for r in live {
                if !a.ranges.contains(&r) {
                    a.ranges.push(r);
                }
            }
            for h in &it.hide {
                if !a.hide.contains(h) {
                    a.hide.push(h.clone());
                }
            }
        } else {
            out.push(Active {
                word: it.word.clone(),
                source: it.source.clone(),
                ranges: live,
                hide: it.hide.clone(),
            });
        }
    }
    out
}

/// 面板要的三个数：今天生效的策略条数 / 生效词数 / 被关掉的条数
pub fn today_counts(deck: &Deck, today: Date, st: &State) -> (usize, usize, usize) {
    let all = strategies_on(deck, today);
    let active = all.iter().filter(|s| s.state == RangeState::Active).count();
    let off = all
        .iter()
        .filter(|s| s.state == RangeState::Active && st.is_off(&s.word, &s.range))
        .count();
    let words = all
        .iter()
        .filter(|s| s.state == RangeState::Active && !st.is_off(&s.word, &s.range))
        .map(|s| s.word.trim().to_lowercase())
        .collect::<std::collections::HashSet<_>>()
        .len();
    (active, words, off)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::{DisplayItem, Range};

    fn r(from: &str, to: &str) -> Range {
        Range {
            from: from.into(),
            to: to.into(),
        }
    }

    fn item(word: &str, ranges: Vec<Range>) -> DisplayItem {
        DisplayItem {
            word: word.into(),
            ranges,
            hide: vec![],
            source: "Unit 2".into(),
        }
    }

    #[test]
    fn days_are_sane() {
        assert_eq!(days((1970, 1, 1)), 0);
        assert_eq!(days((1970, 1, 2)), 1);
        assert_eq!(days_between((2026, 10, 1), (2026, 10, 6)), 5);
        // 跨闰年 2 月
        assert_eq!(days_between((2024, 2, 28), (2024, 3, 1)), 2);
        assert_eq!(days_between((2023, 2, 28), (2023, 3, 1)), 1);
    }

    #[test]
    fn range_state_boundaries() {
        let rr = r("2026-10-06", "2026-10-12");
        assert_eq!(range_state(&rr, (2026, 10, 5)), RangeState::Future);
        assert_eq!(range_state(&rr, (2026, 10, 6)), RangeState::Active);
        assert_eq!(range_state(&rr, (2026, 10, 12)), RangeState::Active);
        assert_eq!(range_state(&rr, (2026, 10, 13)), RangeState::Past);
    }

    #[test]
    fn empty_day_shows_nothing() {
        let deck = Deck {
            display_time: vec![item("seek", vec![r("2026-10-06", "2026-10-12")])],
            ..Default::default()
        };
        let st = State::default();
        assert_eq!(active_today(&deck, (2026, 10, 5), &st).len(), 0);
        assert_eq!(active_today(&deck, (2026, 10, 13), &st).len(), 0);
        assert_eq!(active_today(&deck, (2026, 10, 9), &st).len(), 1);
    }

    #[test]
    fn multi_range_same_day_merges_into_one_card() {
        let deck = Deck {
            display_time: vec![item(
                "seek",
                vec![r("2026-10-06", "2026-10-12"), r("2026-10-09", "2026-10-20")],
            )],
            ..Default::default()
        };
        let a = active_today(&deck, (2026, 10, 9), &State::default());
        assert_eq!(a.len(), 1, "同一个词同一天只出一张卡");
        assert_eq!(a[0].ranges.len(), 2, "但它那两个段都算生效");
    }

    #[test]
    fn off_strategy_leaves_screen() {
        let deck = Deck {
            display_time: vec![
                item("seek", vec![r("2026-10-06", "2026-10-12")]),
                item("deep", vec![r("2026-10-06", "2026-10-12")]),
            ],
            ..Default::default()
        };
        let mut st = State::default();
        assert_eq!(active_today(&deck, (2026, 10, 9), &st).len(), 2);
        assert!(st.set_off("SEEK", &r("2026-10-06", "2026-10-12"), true));
        let a = active_today(&deck, (2026, 10, 9), &st);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].word, "deep");
        // 关掉是永久的：换到同一段的另一天仍然不出
        assert_eq!(active_today(&deck, (2026, 10, 11), &st).len(), 1);
        // 手动打开就恢复
        st.set_off("seek", &r("2026-10-06", "2026-10-12"), false);
        assert_eq!(active_today(&deck, (2026, 10, 11), &st).len(), 2);
    }

    #[test]
    fn off_only_affects_that_range() {
        let deck = Deck {
            display_time: vec![item(
                "seek",
                vec![r("2026-10-06", "2026-10-12"), r("2026-11-02", "2026-11-08")],
            )],
            ..Default::default()
        };
        let mut st = State::default();
        st.set_off("seek", &r("2026-10-06", "2026-10-12"), true);
        assert_eq!(active_today(&deck, (2026, 10, 9), &st).len(), 0);
        assert_eq!(
            active_today(&deck, (2026, 11, 3), &st).len(),
            1,
            "另一段不受影响"
        );
    }

    #[test]
    fn order_follows_library() {
        let deck = Deck {
            display_time: vec![
                item("zebra", vec![r("2026-10-06", "2026-10-12")]),
                item("apple", vec![r("2026-10-06", "2026-10-12")]),
            ],
            ..Default::default()
        };
        let a = active_today(&deck, (2026, 10, 9), &State::default());
        assert_eq!(a[0].word, "zebra", "按主库顺序，不按字母顺序");
    }

    #[test]
    fn hide_union_when_two_items_share_word() {
        let mut a = item("seek", vec![r("2026-10-06", "2026-10-12")]);
        a.hide = vec!["note".into()];
        let mut b = item("seek", vec![r("2026-10-08", "2026-10-15")]);
        b.hide = vec!["sentences".into()];
        let deck = Deck {
            display_time: vec![a, b],
            ..Default::default()
        };
        let act = active_today(&deck, (2026, 10, 9), &State::default());
        assert_eq!(act.len(), 1);
        assert!(act[0].hide.contains(&"note".to_string()));
        assert!(act[0].hide.contains(&"sentences".to_string()));
    }

    #[test]
    fn counts_add_up() {
        let deck = Deck {
            display_time: vec![
                item("seek", vec![r("2026-10-06", "2026-10-12")]),
                item("deep", vec![r("2026-10-06", "2026-10-12")]),
                item("future", vec![r("2026-11-01", "2026-11-07")]),
                item("done", vec![r("2026-09-01", "2026-09-07")]),
            ],
            ..Default::default()
        };
        let mut st = State::default();
        st.set_off("deep", &r("2026-10-06", "2026-10-12"), true);
        let (active, words, off) = today_counts(&deck, (2026, 10, 9), &st);
        assert_eq!((active, words, off), (2, 1, 1));
    }

    #[test]
    fn state_round_trips_and_broken_file_is_not_trusted() {
        let dir = std::env::temp_dir().join(format!("lexideck-test-state-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("状态.json");
        let _ = std::fs::remove_file(&p);

        // 文件不存在 = 可信、全开
        let (s, ok) = load_state(&p);
        assert!(ok);
        assert!(s.off.is_empty());

        let mut s = State::default();
        s.set_off("seek", &r("2026-10-06", "2026-10-12"), true);
        save_state(&p, &s).unwrap();
        let (back, ok) = load_state(&p);
        assert!(ok);
        assert!(back.is_off("Seek", &r("2026-10-06", "2026-10-12")));

        // 坏文件：不可信，调用方不能覆盖它
        std::fs::write(&p, "{ 这不是 json").unwrap();
        let (bad, ok) = load_state(&p);
        assert!(!ok);
        assert!(bad.off.is_empty());

        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn prune_drops_offs_for_words_no_longer_scheduled() {
        let deck = Deck {
            display_time: vec![item("seek", vec![r("2026-10-06", "2026-10-12")])],
            ..Default::default()
        };
        let mut st = State::default();
        st.set_off("seek", &r("2026-10-06", "2026-10-12"), true);
        st.set_off("gone", &r("2026-10-06", "2026-10-12"), true);
        assert!(st.prune(&deck));
        assert_eq!(st.off.len(), 1);
        assert!(st.is_off("seek", &r("2026-10-06", "2026-10-12")));
    }
}
