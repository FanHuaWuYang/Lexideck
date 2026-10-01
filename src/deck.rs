//! 词卡册模块（JSON 词表，格式见 docs/词条格式规范-草稿.md 与 skill-词表生成.md）。
//!
//! 与旧版 TXT（words.rs）并存：程序优先加载 `词表.json`，找不到时回落到 TXT。
//! 设计目标：结构清晰、宽容解析（坏字段只影响那一项）、支持导入合并。

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 义项：词性 + 释义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Sense {
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub meaning: String,
}

/// 动词变形（`emphasize = true` 时展示在单词后方）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Forms {
    #[serde(default)]
    pub emphasize: bool,
    #[serde(default)]
    pub past: String,
    #[serde(default)]
    pub past_participle: String,
}

/// 相关短语。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Phrase {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub meaning: String,
}

/// 优质句子（+译文）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Sentence {
    #[serde(default)]
    pub en: String,
    #[serde(default)]
    pub zh: String,
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
    pub phrases: Vec<Phrase>,
    #[serde(default)]
    pub sentences: Vec<Sentence>,
    /// 计划展示天数（可选；省略 = 1）
    #[serde(default)]
    pub display_days: Option<u32>,
}

impl Entry {
    /// 展示天数（缺失或 <= 0 按 1 天计）。
    pub fn days(&self) -> u32 {
        self.display_days.filter(|d| *d > 0).unwrap_or(1)
    }

    /// 变形行文本：仅当 `emphasize` 且确有变形时给出，如 `"sought sought"`。
    pub fn forms_line(&self) -> Option<String> {
        let f = self.forms.as_ref()?;
        if !f.emphasize {
            return None;
        }
        let past = f.past.trim();
        let pp = f.past_participle.trim();
        match (past.is_empty(), pp.is_empty()) {
            (false, false) => Some(format!("{} {}", past, pp)),
            (false, true) => Some(past.to_string()),
            (true, false) => Some(pp.to_string()),
            (true, true) => None,
        }
    }
}

/// 整本词卡册（= 一个 `词表.json`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Deck {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub deck_name: String,
    #[serde(default)]
    pub words: Vec<Entry>,
}

/// 解析 JSON 文本 → (词卡册, 清洗提示)。
pub fn parse(text: &str) -> Result<(Deck, Vec<String>), String> {
    let mut d: Deck = serde_json::from_str(text)
        .map_err(|e| format!("JSON 解析失败（第 {} 行）：{}", e.line(), e))?;
    let warns = sanitize(&mut d);
    Ok((d, warns))
}

/// 清洗：去空白、丢掉空词条/空片段；返回给人看的提示。
pub fn sanitize(deck: &mut Deck) -> Vec<String> {
    let mut warns = Vec::new();
    let mut kept = Vec::with_capacity(deck.words.len());
    for (i, mut e) in std::mem::take(&mut deck.words).into_iter().enumerate() {
        e.word = e.word.trim().to_string();
        for s in &mut e.senses {
            s.pos = s.pos.trim().to_string();
            s.meaning = s.meaning.trim().to_string();
        }
        e.senses
            .retain(|s| !s.meaning.is_empty() || !s.pos.is_empty());
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
        if e.word.is_empty() {
            warns.push(format!("第 {} 条没有 word，已跳过", i + 1));
            continue;
        }
        if e.senses.is_empty() {
            warns.push(format!("「{}」没有释义", e.word));
        }
        kept.push(e);
    }
    deck.words = kept;
    warns
}

/// 合并（导入追加）：相同 `word`（忽略大小写）→ 以新词条更新；其余 → 追加到末尾。
/// 返回 (新增数, 更新数)。
pub fn merge(base: &mut Deck, incoming: Deck) -> (usize, usize) {
    let (mut added, mut updated) = (0usize, 0usize);
    for e in incoming.words {
        if let Some(slot) = base
            .words
            .iter_mut()
            .find(|w| w.word.eq_ignore_ascii_case(&e.word))
        {
            *slot = e;
            updated += 1;
        } else {
            base.words.push(e);
            added += 1;
        }
    }
    (added, updated)
}

/// 保存为整洁的 JSON（UTF-8）。
pub fn save(path: &Path, deck: &Deck) -> Result<(), String> {
    let s = serde_json::to_string_pretty(deck).map_err(|e| e.to_string())?;
    std::fs::write(path, s).map_err(|e| e.to_string())
}

/// 从文件加载（编码识别复用 words 模块；失败返回人话错误）。
pub fn load(path: &Path) -> Result<(Deck, Vec<String>), String> {
    let text = crate::words::read_text_any_encoding(path)
        .ok_or_else(|| "读不到文件（编码不支持或文件被占用）".to_string())?;
    parse(&text)
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
          "forms": { "emphasize": true, "past": "sought", "past_participle": "sought" },
          "phrases": [ { "text": "seek out", "meaning": "找出；搜寻到" } ],
          "sentences": [ { "en": "She sought advice.", "zh": "她寻求建议。" } ],
          "display_days": 2
        }
      ]
    }"#;

    #[test]
    fn parse_full_entry() {
        let (d, w) = parse(FULL).unwrap();
        assert!(w.is_empty());
        assert_eq!(d.words.len(), 1);
        let e = &d.words[0];
        assert_eq!(e.word, "seek");
        assert_eq!(e.senses[0].pos, "v.");
        assert_eq!(e.forms.as_ref().unwrap().past, "sought");
        assert_eq!(e.forms_line().as_deref(), Some("sought sought"));
        assert_eq!(e.phrases[0].text, "seek out");
        assert_eq!(e.sentences[0].en, "She sought advice.");
        assert_eq!(e.days(), 2);
    }

    #[test]
    fn minimal_entry_and_default_days() {
        let src = r#"{ "words": [ { "word": "courage", "senses": [ { "pos": "n.", "meaning": "勇气" } ] } ] }"#;
        let (d, _) = parse(src).unwrap();
        let e = &d.words[0];
        assert_eq!(e.days(), 1);
        assert!(e.forms_line().is_none());
        assert!(e.forms.is_none());
    }

    #[test]
    fn emphasize_false_hides_forms() {
        let src = r#"{ "words": [ { "word": "abandon", "senses": [],
            "forms": { "emphasize": false, "past": "abandoned", "past_participle": "abandoned" } } ] }"#;
        let (d, w) = parse(src).unwrap();
        assert!(d.words[0].forms_line().is_none());
        assert!(w.iter().any(|x| x.contains("没有释义")));
    }

    #[test]
    fn empty_word_is_skipped() {
        let src = r#"{ "words": [
            { "word": "  ", "senses": [ { "meaning": "x" } ] },
            { "word": "ok", "senses": [ { "meaning": "好" } ] } ] }"#;
        let (d, w) = parse(src).unwrap();
        assert_eq!(d.words.len(), 1);
        assert_eq!(d.words[0].word, "ok");
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn merge_updates_and_appends() {
        let (mut base, _) =
            parse(r#"{ "words": [ { "word": "seek", "senses": [ { "meaning": "旧" } ] } ] }"#)
                .unwrap();
        let (inc, _) = parse(
            r#"{ "words": [
                { "word": "SEEK", "senses": [ { "meaning": "新" } ] },
                { "word": "gain", "senses": [ { "meaning": "获得" } ] } ] }"#,
        )
        .unwrap();
        let (added, updated) = merge(&mut base, inc);
        assert_eq!((added, updated), (1, 1));
        assert_eq!(base.words.len(), 2);
        assert_eq!(base.words[0].senses[0].meaning, "新");
    }

    #[test]
    fn bad_json_reports_line() {
        let e = parse("{ \"words\": [ }").unwrap_err();
        assert!(e.contains("JSON 解析失败"));
    }
}
