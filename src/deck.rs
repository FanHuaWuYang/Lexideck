//! 词库数据模型（v1.0 规范，权威定义见 `docs/词条格式规范-v1.0.md`）。
//!
//! 一个 JSON 文件 = 一本词卡册，含两部分：
//!   · `words`        —— 词条本体
//!   · `display_time` —— 每个词条的展示日期段（P1 只做校验与存储，展示行为在 P2 生效）
//!
//! 三层职责分开：
//!   · `parse_file`  —— 严格校验（格式不对就整份拒绝，不动主库）
//!   · `sanitize`    —— 宽容清洗（去空白、丢空项），只影响那一项
//!   · `plan_*`      —— 合并前的差异计划（新增 / 重复 / 冲突），由导入流程决定怎么处理

use std::path::Path;

use serde::{Deserialize, Serialize};

/// 可「关闭默认展示」的区块（单词本身不在此列，永远显示）。
pub const HIDE_KEYS: [&str; 6] = [
    "senses",
    "forms",
    "derivations",
    "phrases",
    "sentences",
    "note",
];

/// 区块键 → 给人看的中文名
pub fn hide_label(key: &str) -> &'static str {
    match key {
        "senses" => "词义",
        "forms" => "变形",
        "derivations" => "同根词",
        "phrases" => "短语",
        "sentences" => "例句",
        "note" => "备注",
        _ => "未知区块",
    }
}

pub fn is_hide_key(key: &str) -> bool {
    HIDE_KEYS.contains(&key)
}

/// 义项：词性 + 释义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Sense {
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub meaning: String,
}

/// 动词变形。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Forms {
    #[serde(default)]
    pub past: String,
    #[serde(default)]
    pub past_participle: String,
}

/// 相关短语 / 固定搭配。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Phrase {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub meaning: String,
}

/// 优质句子（+可选译文）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Sentence {
    #[serde(default)]
    pub en: String,
    #[serde(default)]
    pub zh: String,
}

/// 同根词变形（如 seek → seeker），带自己的词性与词义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Derivation {
    #[serde(default)]
    pub word: String,
    #[serde(default)]
    pub senses: Vec<Sense>,
}

/// 一个词条。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Entry {
    #[serde(default)]
    pub word: String,
    #[serde(default)]
    pub senses: Vec<Sense>,
    #[serde(default)]
    pub forms: Option<Forms>,
    #[serde(default)]
    pub derivations: Vec<Derivation>,
    #[serde(default)]
    pub phrases: Vec<Phrase>,
    #[serde(default)]
    pub sentences: Vec<Sentence>,
    #[serde(default)]
    pub note: String,
    /// 关闭默认展示的区块名（见 HIDE_KEYS）
    #[serde(default)]
    pub hide: Vec<String>,
}

impl Entry {
    /// 合并 / 查重用的键：word 去空白 + 小写
    pub fn key(&self) -> String {
        self.word.trim().to_lowercase()
    }

    /// 该区块是否被关闭默认展示
    pub fn hidden(&self, key: &str) -> bool {
        self.hide.iter().any(|h| h == key)
    }

    /// 变形行文本（写了且有内容就显示，显示与否由 hide 决定）
    pub fn forms_line(&self) -> Option<String> {
        let f = self.forms.as_ref()?;
        let past = f.past.trim();
        let pp = f.past_participle.trim();
        match (past.is_empty(), pp.is_empty()) {
            (false, false) => Some(format!("{past} {pp}")),
            (false, true) => Some(past.to_string()),
            (true, false) => Some(pp.to_string()),
            (true, true) => None,
        }
    }

    /// 第一个义项的中文释义（词表列表里显示用）
    pub fn first_meaning(&self) -> String {
        match self.senses.first() {
            Some(s) => {
                let pos = s.pos.trim();
                let m = s.meaning.trim();
                if pos.is_empty() {
                    m.to_string()
                } else {
                    format!("{pos} {m}")
                }
            }
            None => String::new(),
        }
    }

    /// 各区块条目数，用于冲突弹窗上的一行摘要
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.senses.is_empty() {
            parts.push(format!("{} 个义项", self.senses.len()));
        }
        if self.forms_line().is_some() {
            parts.push("变形".into());
        }
        if !self.derivations.is_empty() {
            parts.push(format!("{} 个同根词", self.derivations.len()));
        }
        if !self.phrases.is_empty() {
            parts.push(format!("{} 个短语", self.phrases.len()));
        }
        if !self.sentences.is_empty() {
            parts.push(format!("{} 条例句", self.sentences.len()));
        }
        if !self.note.trim().is_empty() {
            parts.push("备注".into());
        }
        if parts.is_empty() {
            "（没有内容）".into()
        } else {
            parts.join("，")
        }
    }

    /// 把所有区块拼接成一段可比对的文本（判断两份词条内容是否相同）
    fn fingerprint(&self) -> String {
        let mut s = String::new();
        for x in &self.senses {
            s.push_str(&format!("S|{}|{}\n", x.pos.trim(), x.meaning.trim()));
        }
        if let Some(f) = &self.forms {
            let (past, pp) = (f.past.trim(), f.past_participle.trim());
            if !past.is_empty() || !pp.is_empty() {
                s.push_str(&format!("F|{past}|{pp}\n"));
            }
        }
        for d in &self.derivations {
            s.push_str(&format!("D|{}", d.word.trim()));
            for x in &d.senses {
                s.push_str(&format!("|{}|{}", x.pos.trim(), x.meaning.trim()));
            }
            s.push('\n');
        }
        for p in &self.phrases {
            s.push_str(&format!("P|{}|{}\n", p.text.trim(), p.meaning.trim()));
        }
        for x in &self.sentences {
            s.push_str(&format!("E|{}|{}\n", x.en.trim(), x.zh.trim()));
        }
        s.push_str(&format!("N|{}\n", self.note.trim()));
        let mut hide: Vec<&str> = self.hide.iter().map(|h| h.as_str()).collect();
        hide.sort_unstable();
        s.push_str(&format!("H|{}\n", hide.join(",")));
        s
    }

    /// 内容是否与另一条一致（只要有一处不一样就算冲突）
    pub fn same_as(&self, other: &Entry) -> bool {
        self.fingerprint() == other.fingerprint()
    }
}

/// 一个展示日期段（含首尾两天）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Range {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

impl Range {
    pub fn label(&self) -> String {
        format!("{} → {}", self.from.trim(), self.to.trim())
    }
}

/// 展示时间里的一条：某个词的若干日期段（可覆盖该词在卡片上的显隐）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DisplayItem {
    #[serde(default)]
    pub word: String,
    #[serde(default)]
    pub ranges: Vec<Range>,
    #[serde(default)]
    pub hide: Vec<String>,
    /// 来源文件标签（导自哪份文件；只作展示，**不参与**冲突判定）
    #[serde(default)]
    pub source: String,
}

impl DisplayItem {
    pub fn key(&self) -> String {
        self.word.trim().to_lowercase()
    }
}

/// 整本词卡册 / 主库
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Deck {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub deck_name: String,
    #[serde(default)]
    pub words: Vec<Entry>,
    #[serde(default)]
    pub display_time: Vec<DisplayItem>,
}

// ── 日期 ──

/// 解析 `YYYY-MM-DD`（严格：必须补零，且是真实存在的日期，含闰年）。
pub fn parse_date(s: &str) -> Option<(i32, u32, u32)> {
    let s = s.trim();
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3
        || parts[0].len() != 4
        || parts[1].len() != 2
        || parts[2].len() != 2
        || parts.iter().any(|p| !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let y = parts[0].parse::<i32>().ok()?;
    let m = parts[1].parse::<u32>().ok()?;
    let d = parts[2].parse::<u32>().ok()?;
    if !(1..=12).contains(&m) {
        return None;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if leap {
                29
            } else {
                28
            }
        }
    };
    if d < 1 || d > dim {
        return None;
    }
    Some((y, m, d))
}

// ── 校验（格式问题 → 整份拒绝，不动主库） ──

fn as_str(v: &serde_json::Value, key: &str) -> Result<String, String> {
    match v.get(key) {
        None | Some(serde_json::Value::Null) => Ok(String::new()),
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(format!("`{key}` 必须是字符串")),
    }
}

fn as_array<'a>(
    v: &'a serde_json::Value,
    key: &str,
) -> Result<Option<&'a Vec<serde_json::Value>>, String> {
    match v.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Array(a)) => Ok(Some(a)),
        Some(_) => Err(format!("`{key}` 必须是数组")),
    }
}

/// 严格解析：格式有问题就返回**全部**问题（一次列清楚），不返回半个词库。
pub fn parse_file(text: &str) -> Result<Deck, Vec<String>> {
    let mut root: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            return Err(vec![format!(
                "JSON 语法错误（第 {} 行）：这一份文件没有被导入，主库未改动",
                e.line()
            )])
        }
    };
    // null 一律按"没填"处理：先递归删掉 null 键，验证与反序列化就永远同一套语义
    strip_nulls(&mut root);
    let mut errs: Vec<String> = Vec::new();

    match root.get("version") {
        None => errs.push("缺 `version`：顶层必须写 \"version\": 1".into()),
        Some(serde_json::Value::Number(n)) if n.as_u64() == Some(1) => {}
        Some(_) => errs.push("`version` 只支持 1".into()),
    }

    // ── 词条 ──
    let mut seen: Vec<(String, usize)> = Vec::new();
    if let Some(ws) = as_array(&root, "words").unwrap_or(None) {
        if ws.is_empty() {
            let dt_empty = root
                .get("display_time")
                .and_then(|v| v.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(true);
            if dt_empty {
                errs.push("`words` 和 `display_time` 都是空的：一份文件至少要有一个词条".into());
            }
        }
        for (i, w) in ws.iter().enumerate() {
            let at = format!("第 {} 个词条", i + 1);
            if !w.is_object() {
                errs.push(format!("{at}不是一个对象"));
                continue;
            }
            match as_str(w, "word") {
                Ok(s) if s.trim().is_empty() => errs.push(format!("{at}缺 `word`")),
                Ok(s) => {
                    let k = s.trim().to_lowercase();
                    if let Some((name, prev)) = seen.iter().find(|(p, _)| *p == k) {
                        errs.push(format!(
                            "「{}」在文件里出现了两次（第 {} 个和第 {} 个）",
                            name,
                            prev + 1,
                            i + 1
                        ));
                    } else {
                        seen.push((k, i));
                    }
                }
                Err(e) => errs.push(format!("{at}：{e}")),
            }
            match as_array(w, "senses") {
                Ok(Some(list)) if list.is_empty() => {
                    errs.push(format!("{at}「{}」没有词义", field_word(w)))
                }
                Ok(Some(list)) => {
                    for (j, s) in list.iter().enumerate() {
                        if let Err(e) = as_str(s, "meaning") {
                            errs.push(format!("{at}第 {} 个义项：{e}", j + 1));
                            continue;
                        }
                        if let Ok(m) = as_str(s, "meaning") {
                            if m.trim().is_empty() {
                                errs.push(format!(
                                    "{at}「{}」第 {} 个义项没有 meaning",
                                    field_word(w),
                                    j + 1
                                ));
                            }
                        }
                    }
                }
                Ok(None) => errs.push(format!("{at}「{}」没有词义", field_word(w))),
                Err(e) => errs.push(format!("{at}：{e}")),
            }
            check_hide(w, &at, &mut errs);
            for key in ["forms", "derivations", "phrases", "sentences", "note"] {
                if let Some(v) = w.get(key) {
                    if !v.is_null() {
                        let ok = match key {
                            "forms" => v.is_object(),
                            "note" => v.is_string(),
                            _ => v.is_array(),
                        };
                        if !ok {
                            errs.push(format!("{at}：`{key}` 类型不对（见词条格式规范）"));
                        }
                    }
                }
            }
        }
    } else if root.get("words").is_some() {
        errs.push("`words` 必须是数组".into());
    }

    // ── 展示时间 ──
    if let Some(items) = as_array(&root, "display_time").unwrap_or(None) {
        for (i, it) in items.iter().enumerate() {
            let at = format!("第 {} 条展示时间", i + 1);
            if !it.is_object() {
                errs.push(format!("{at}不是一个对象"));
                continue;
            }
            match as_str(it, "word") {
                Ok(s) if s.trim().is_empty() => errs.push(format!("{at}缺 `word`")),
                Err(e) => errs.push(format!("{at}：{e}")),
                _ => {}
            }
            match as_array(it, "ranges") {
                Ok(Some(rs)) if rs.is_empty() => errs.push(format!("{at}没有日期段")),
                Ok(Some(rs)) => {
                    for (j, r) in rs.iter().enumerate() {
                        let from = as_str(r, "from").unwrap_or_default();
                        let to = as_str(r, "to").unwrap_or_default();
                        let pf = parse_date(&from);
                        let pt = parse_date(&to);
                        if pf.is_none() {
                            errs.push(format!("{at}第 {} 段的 from 不是合法日期：{from:?}", j + 1));
                        }
                        if pt.is_none() {
                            errs.push(format!("{at}第 {} 段的 to 不是合法日期：{to:?}", j + 1));
                        }
                        if let (Some(a), Some(b)) = (pf, pt) {
                            if a > b {
                                errs.push(format!("{at}第 {} 段 from 晚于 to", j + 1));
                            }
                        }
                    }
                }
                Ok(None) => errs.push(format!("{at}缺 `ranges`")),
                Err(e) => errs.push(format!("{at}：{e}")),
            }
            check_hide(it, &at, &mut errs);
        }
    } else if root.get("display_time").is_some() {
        errs.push("`display_time` 必须是数组".into());
    }

    if root.get("words").is_none() && root.get("display_time").is_none() {
        errs.push("这份文件里既没有 `words` 也没有 `display_time`".into());
    }

    if !errs.is_empty() {
        return Err(errs);
    }
    match serde_json::from_value::<Deck>(root) {
        Ok(d) => Ok(d),
        Err(e) => Err(vec![format!("解析失败：{e}")]),
    }
}

/// 递归删掉所有值为 null 的键（数组里的 null 元素也去掉）
fn strip_nulls(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(m) => {
            m.retain(|_, x| !x.is_null());
            for (_, x) in m.iter_mut() {
                strip_nulls(x);
            }
        }
        serde_json::Value::Array(a) => {
            a.retain(|x| !x.is_null());
            for x in a.iter_mut() {
                strip_nulls(x);
            }
        }
        _ => {}
    }
}

/// 保序去重（`Vec::dedup` 只去相邻重复，不够）
fn dedup_in_place<T: PartialEq>(v: &mut Vec<T>) {
    let mut out: Vec<T> = Vec::with_capacity(v.len());
    for x in v.drain(..) {
        if !out.contains(&x) {
            out.push(x);
        }
    }
    *v = out;
}

fn field_word(v: &serde_json::Value) -> String {
    v.get("word")
        .and_then(|x| x.as_str())
        .unwrap_or("(无名)")
        .to_string()
}

fn check_hide(v: &serde_json::Value, at: &str, errs: &mut Vec<String>) {
    match as_array(v, "hide") {
        Ok(Some(list)) => {
            for h in list {
                match h.as_str() {
                    Some(s) if is_hide_key(s) => {}
                    Some(s) => errs.push(format!(
                        "{at}：`hide` 里有不认识的名字 {s:?}（只能是 senses / forms / derivations / phrases / sentences / note）"
                    )),
                    None => errs.push(format!("{at}：`hide` 里只能放字符串")),
                }
            }
        }
        Ok(None) => {}
        Err(e) => errs.push(format!("{at}：{e}")),
    }
}

// ── 清洗（宽容：只丢掉坏的那一项） ──

pub fn sanitize(deck: &mut Deck) -> Vec<String> {
    let warns = Vec::new();
    let mut kept = Vec::with_capacity(deck.words.len());
    for mut e in std::mem::take(&mut deck.words) {
        e.word = e.word.trim().to_string();
        e.note = e.note.trim().to_string();
        for s in &mut e.senses {
            s.pos = s.pos.trim().to_string();
            s.meaning = s.meaning.trim().to_string();
        }
        e.senses
            .retain(|s| !s.meaning.is_empty() || !s.pos.is_empty());
        for d in &mut e.derivations {
            d.word = d.word.trim().to_string();
            for s in &mut d.senses {
                s.pos = s.pos.trim().to_string();
                s.meaning = s.meaning.trim().to_string();
            }
            d.senses
                .retain(|s| !s.meaning.is_empty() || !s.pos.is_empty());
        }
        e.derivations.retain(|d| !d.word.is_empty());
        for p in &mut e.phrases {
            p.text = p.text.trim().to_string();
            p.meaning = p.meaning.trim().to_string();
        }
        e.phrases.retain(|p| !p.text.is_empty());
        for s in &mut e.sentences {
            s.en = s.en.trim().to_string();
            s.zh = s.zh.trim().to_string();
        }
        e.sentences.retain(|s| !s.en.is_empty());
        if let Some(f) = &mut e.forms {
            f.past = f.past.trim().to_string();
            f.past_participle = f.past_participle.trim().to_string();
        }
        e.hide.retain(|h| is_hide_key(h));
        dedup_in_place(&mut e.hide);
        if e.word.is_empty() {
            continue;
        }
        kept.push(e);
    }
    deck.words = kept;
    for it in &mut deck.display_time {
        it.word = it.word.trim().to_string();
        for r in &mut it.ranges {
            r.from = r.from.trim().to_string();
            r.to = r.to.trim().to_string();
        }
        it.hide.retain(|h| is_hide_key(h));
        dedup_in_place(&mut it.hide);
    }
    // P2：展示时间项记住"来自哪份文件"（只作展示，不参与冲突判定）。空白的用 deck_name 兜底。
    let deck_name = deck.deck_name.trim().to_string();
    let fallback = if deck_name.is_empty() {
        "未命名".to_string()
    } else {
        deck_name
    };
    for it in &mut deck.display_time {
        let s = it.source.trim().to_string();
        it.source = if s.is_empty() { fallback.clone() } else { s };
    }
    deck.display_time.retain(|it| !it.word.is_empty());
    warns
}

// ── 合并计划 ──

/// 词条合并的差异：新增 / 完全重复 / 同名但内容不同（冲突）
#[derive(Debug, Default)]
pub struct WordPlan {
    pub adds: Vec<Entry>,
    pub dupes: Vec<Entry>,
    pub conflicts: Vec<(Entry, Entry)>, // (主库现有, 文件里的新内容)
}

pub fn plan_word_merge(base: &[Entry], incoming: &[Entry]) -> WordPlan {
    let mut p = WordPlan::default();
    for e in incoming {
        match base.iter().find(|b| b.key() == e.key()) {
            Some(old) => {
                if old.same_as(e) {
                    p.dupes.push(e.clone());
                } else {
                    p.conflicts.push((old.clone(), e.clone()));
                }
            }
            None => p.adds.push(e.clone()),
        }
    }
    p
}

/// 展示时间的键：word + 日期段 + 覆盖显隐
fn display_fingerprint(it: &DisplayItem) -> String {
    let mut rs: Vec<String> = it
        .ranges
        .iter()
        .map(|r| format!("{}~{}", r.from.trim(), r.to.trim()))
        .collect();
    rs.sort();
    let mut hs: Vec<&str> = it.hide.iter().map(|h| h.as_str()).collect();
    hs.sort_unstable();
    format!("{}#{}#{}", it.key(), rs.join(","), hs.join(","))
}

/// 把一批展示时间并进主库：完全相同的丢掉，其余追加。返回 (新增, 重复)。
pub fn merge_display_time(
    base: &mut Vec<DisplayItem>,
    incoming: Vec<DisplayItem>,
) -> (usize, usize) {
    let mut have: Vec<String> = base.iter().map(display_fingerprint).collect();
    let (mut added, mut dup) = (0usize, 0usize);
    for it in incoming {
        let fp = display_fingerprint(&it);
        if have.contains(&fp) {
            dup += 1;
        } else {
            have.push(fp);
            base.push(it);
            added += 1;
        }
    }
    (added, dup)
}

/// 在词库里按 word 找词条下标（不区分大小写）
pub fn find_index(deck: &Deck, word: &str) -> Option<usize> {
    let k = word.trim().to_lowercase();
    deck.words.iter().position(|w| w.key() == k)
}

/// 在词库里按 word 取词条（不区分大小写）
pub fn find<'a>(deck: &'a Deck, word: &str) -> Option<&'a Entry> {
    find_index(deck, word).map(|i| &deck.words[i])
}

// ── 读写 ──

pub fn save(path: &Path, deck: &Deck) -> Result<(), String> {
    let s = serde_json::to_string_pretty(deck).map_err(|e| e.to_string())?;
    std::fs::write(path, s).map_err(|e| e.to_string())
}

/// 解析一个文件（含读取与编码回退）
pub fn load_file(path: &Path) -> Result<(Deck, Vec<String>), Vec<String>> {
    let text = crate::util::read_text_any_encoding(path)
        .ok_or_else(|| vec!["读不到文件（编码不支持或文件被占用）".to_string()])?;
    let mut deck = parse_file(&text)?;
    let warns = sanitize(&mut deck);
    Ok((deck, warns))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"{
      "version": 1,
      "deck_name": "Unit 1",
      "words": [
        {
          "word": "seek",
          "senses": [ { "pos": "v.", "meaning": "寻找；寻求；试图" } ],
          "forms": { "past": "sought", "past_participle": "sought" },
          "derivations": [ { "word": "seeker", "senses": [ { "pos": "n.", "meaning": "探索者" } ] } ],
          "phrases": [ { "text": "seek out", "meaning": "找出" } ],
          "sentences": [ { "en": "She sought advice.", "zh": "她寻求建议。" } ],
          "note": "写作高频",
          "hide": ["note"]
        }
      ]
    }"#;

    #[test]
    fn parses_full_entry() {
        let mut d = parse_file(FULL).unwrap();
        sanitize(&mut d);
        let e = &d.words[0];
        assert_eq!(e.word, "seek");
        assert_eq!(e.forms_line().as_deref(), Some("sought sought"));
        assert_eq!(e.derivations[0].word, "seeker");
        assert_eq!(e.derivations[0].senses[0].meaning, "探索者");
        assert_eq!(e.note, "写作高频");
        assert!(e.hidden("note"));
        assert!(!e.hidden("phrases"));
        assert_eq!(e.first_meaning(), "v. 寻找；寻求；试图");
    }

    #[test]
    fn rejects_missing_senses() {
        let src = r#"{ "version": 1, "words": [ { "word": "courage" } ] }"#;
        let errs = parse_file(src).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("没有词义")), "{errs:?}");
    }

    #[test]
    fn rejects_bad_hide_key() {
        let src = r#"{ "version": 1, "words": [ { "word": "a", "senses": [ { "meaning": "x" } ], "hide": ["sounds"] } ] }"#;
        let errs = parse_file(src).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("hide")), "{errs:?}");
    }

    #[test]
    fn rejects_bad_dates_and_reversed_range() {
        let src = r#"{ "version": 1, "display_time": [
            { "word": "seek", "ranges": [ { "from": "2026-02-30", "to": "2026-03-05" } ] },
            { "word": "deep", "ranges": [ { "from": "2026-03-05", "to": "2026-03-01" } ] } ] }"#;
        let errs = parse_file(src).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("2026-02-30")), "{errs:?}");
        assert!(errs.iter().any(|e| e.contains("晚于")), "{errs:?}");
    }

    #[test]
    fn rejects_duplicate_word_in_one_file() {
        let src = r#"{ "version": 1, "words": [
            { "word": "seek", "senses": [ { "meaning": "a" } ] },
            { "word": "Seek", "senses": [ { "meaning": "b" } ] } ] }"#;
        let errs = parse_file(src).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("出现了两次")), "{errs:?}");
    }

    #[test]
    fn accepts_display_time_only_file() {
        let src = r#"{ "version": 1, "display_time": [
            { "word": "seek", "ranges": [ { "from": "2026-10-06", "to": "2026-10-12" } ] } ] }"#;
        assert!(parse_file(src).is_ok());
    }

    #[test]
    fn date_parsing_is_strict() {
        assert!(parse_date("2024-02-29").is_some());
        assert!(parse_date("2026-02-29").is_none());
        assert!(parse_date("2026-13-01").is_none());
        assert!(parse_date("2026-1-1").is_none());
    }

    #[test]
    fn merge_plans_adds_dupes_conflicts() {
        let base = vec![
            Entry {
                word: "seek".into(),
                senses: vec![Sense {
                    pos: "v.".into(),
                    meaning: "旧".into(),
                }],
                ..Default::default()
            },
            Entry {
                word: "gain".into(),
                senses: vec![Sense {
                    pos: String::new(),
                    meaning: "获得".into(),
                }],
                ..Default::default()
            },
        ];
        let incoming = vec![
            Entry {
                word: "SEEK".into(),
                senses: vec![Sense {
                    pos: String::new(),
                    meaning: "新".into(),
                }],
                ..Default::default()
            },
            Entry {
                word: "gain".into(),
                senses: vec![Sense {
                    pos: String::new(),
                    meaning: "获得".into(),
                }],
                ..Default::default()
            },
            Entry {
                word: "courage".into(),
                senses: vec![Sense {
                    pos: String::new(),
                    meaning: "勇气".into(),
                }],
                ..Default::default()
            },
        ];
        let p = plan_word_merge(&base, &incoming);
        assert_eq!(p.adds.len(), 1);
        assert_eq!(p.dupes.len(), 1);
        assert_eq!(p.conflicts.len(), 1);
        assert_eq!(p.conflicts[0].0.senses[0].meaning, "旧");
        assert_eq!(p.conflicts[0].1.senses[0].meaning, "新");
    }

    #[test]
    fn display_time_merge_dedupes() {
        let mk = |w: &str, a: &str, b: &str| DisplayItem {
            word: w.into(),
            ranges: vec![Range {
                from: a.into(),
                to: b.into(),
            }],
            hide: vec![],
            source: "Unit 1".into(),
        };
        let mut base = vec![mk("seek", "2026-10-06", "2026-10-12")];
        let (added, dup) = merge_display_time(
            &mut base,
            vec![
                mk("seek", "2026-10-06", "2026-10-12"),
                mk("seek", "2026-11-01", "2026-11-07"),
            ],
        );
        assert_eq!((added, dup), (1, 1));
        assert_eq!(base.len(), 2);
    }

    /// null 与"不写这个字段"应当完全等价（原来验证放行、反序列化却报错）
    #[test]
    fn null_fields_mean_missing() {
        let t = r#"{"version":1,"deck_name":null,"words":[{"word":"seek","note":null,
            "senses":[{"pos":null,"meaning":"寻求"}]}]}"#;
        let d = parse_file(t).expect("null 应当按没填处理");
        assert_eq!(d.words[0].note, "");
        assert_eq!(d.words[0].senses[0].meaning, "寻求");
    }

    /// forms: {} 和"没有 forms"内容是一样的，不该弹冲突
    #[test]
    fn empty_forms_object_equals_no_forms() {
        let a =
            parse_file(r#"{"version":1,"words":[{"word":"seek","senses":[{"meaning":"寻求"}]}]}"#)
                .unwrap();
        let b = parse_file(
            r#"{"version":1,"words":[{"word":"seek","senses":[{"meaning":"寻求"}],"forms":{}}]}"#,
        )
        .unwrap();
        assert!(a.words[0].same_as(&b.words[0]));
    }

    /// 空 words 数组 + 有 display_time 是合法文件
    #[test]
    fn empty_words_array_with_display_time_is_ok() {
        let t = r#"{"version":1,"words":[],"display_time":[{"word":"seek",
            "ranges":[{"from":"2026-10-06","to":"2026-10-12"}]}]}"#;
        assert!(parse_file(t).is_ok());
    }

    /// hide 去重要去全部重复，不只是相邻的
    #[test]
    fn hide_is_deduped_not_only_adjacent() {
        let dir = std::env::temp_dir().join(format!("lexideck-test-dedup-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("d.json");
        let _ = std::fs::remove_file(&p);
        std::fs::write(
            &p,
            r#"{"version":1,"words":[{"word":"seek","hide":["note","senses","note"],
                "senses":[{"meaning":"寻求"}]}]}"#,
        )
        .unwrap();
        let (d, _) = load_file(&p).unwrap();
        assert_eq!(d.words[0].hide.len(), 2);
    }

    /// `skill-词表生成.md` 里每个 ```json 代码块都必须过得了 `parse_file`
    /// —— 文档里的示例一旦和校验器脱钩（字段改名、结构变了），这条会先炸。
    /// 裸词条示例（只有 word/senses 那种）自动包一层文件壳再试。
    #[test]
    fn skill_doc_examples_all_parse() {
        let doc = include_str!("../skill-词表生成.md");
        let mut blocks: Vec<String> = Vec::new();
        let mut cur: Option<String> = None;
        for line in doc.lines() {
            let t = line.trim();
            if cur.is_none() {
                if t == "```json" {
                    cur = Some(String::new());
                }
            } else if t == "```" {
                blocks.push(cur.take().unwrap());
            } else if let Some(b) = cur.as_mut() {
                b.push_str(line);
                b.push('\n');
            }
        }
        assert!(
            blocks.len() >= 5,
            "从文档里只抓到 {} 个 json 块，抽取逻辑可能坏了",
            blocks.len()
        );
        for (i, b) in blocks.iter().enumerate() {
            if parse_file(b).is_ok() {
                continue;
            }
            let wrapped = format!("{{\"version\":1,\"words\":[{b}]}}");
            if let Err(e) = parse_file(&wrapped) {
                panic!("文档第 {} 个 json 块过不了校验：{e:?}", i + 1);
            }
        }
    }
}
