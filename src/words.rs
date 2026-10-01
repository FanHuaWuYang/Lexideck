//! 词表模块：宽容解析 + 类型推断 + 自动编码识别 + 文件加载 + 热更新检查。
//!
//! 设计目标（调研报告第 5 节）：任何一行坏内容都只影响那一行，
//! 绝不导致整份文件不可用、绝不丢内容。
//!
//! 语法（老师只需要记住第一条）：
//!   1. 一行一个词 / 一句 → 最简用法，直接一行一条记录
//!   2. `键: 值` 或 `键 值`（冒号和空格都可以）→ 挂到最近一条记录上
//!   3. 空行 或 `---` → 下一条记录
//!   4. 认不出的行 → 原样显示在那条记录下方，不算错误

use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// 主体：单词 / 短语 / 整句
    pub head: String,
    /// 规范字段（顺序固定，便于排版）
    pub fields: Vec<(String, String)>,
    /// 无法识别的行：原样保留，界面上下方小字显示
    pub extras: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub line: usize,
    pub text: String,
    pub msg: String,
}

/// 规范字段顺序
pub const CANON: [&str; 7] = ["音标", "词性", "释义", "例句", "例句翻译", "语法", "备注"];

/// 别名表，长键必须排在短键前面（"例句翻译" 要先于 "例句" 被匹配到）
const ALIASES: [&str; 47] = [
    // 4 字
    "例句翻译",
    "例句释义",
    "例句意思",
    // 多字母英文
    "partofspeech",
    "phonetic",
    "examplecn",
    "知识点",
    // 2~3 字
    "语法点",
    "发音",
    // 长英文
    "example",
    "meaning",
    "grammar",
    // 2 字中文
    "音标",
    "词性",
    "词类",
    "释义",
    "意思",
    "中文",
    "含义",
    "例句",
    "例子",
    "翻译",
    "语法",
    "备注",
    "说明",
    "注释",
    "单词",
    "短语",
    "句子",
    "标题",
    // 短英文
    "ipa",
    "pos",
    "pron",
    "sent",
    "trans",
    "egcn",
    "eg",
    "cn",
    "ph",
    "word",
    "title",
    "head",
    "note2",
    "note",
    "tip",
    // 1 字（放最后，避免抢先匹配）
    "例",
    "词",
];

/// 把别名映射到规范字段名
fn canon_of(key: &str) -> Option<&'static str> {
    let k = key.trim().to_lowercase();
    Some(match k.as_str() {
        "例句翻译" | "例句释义" | "例句意思" | "例译" | "examplecn" | "egcn" => {
            "例句翻译"
        }
        "音标" | "ph" | "ipa" | "phonetic" | "pron" | "发音" => "音标",
        "词性" | "pos" | "词类" | "partofspeech" => "词性",
        "释义" | "意思" | "中文" | "含义" | "meaning" | "cn" | "trans" | "翻译" => "释义",
        "例句" | "例子" | "例" | "example" | "eg" | "sent" => "例句",
        "语法" | "语法点" | "知识点" | "grammar" | "note2" => "语法",
        "备注" | "说明" | "注释" | "note" | "tip" => "备注",
        "标题" | "单词" | "短语" | "句子" | "词" | "word" | "title" | "head" => "标题",
        _ => return None,
    })
}

fn split_kv(line: &str) -> Option<(&str, &str)> {
    let idx = line.find([':', '：'])?;
    Some((&line[..idx], &line[idx..].trim_start_matches([':', '：'])))
}

/// 没有冒号时，看这一行是不是"以某个字段名开头"（老师常见的漏写冒号）
/// 只有"字段名 + 分隔符 + 内容"才算，避免把 position / translation 这类单词误判。
fn inline_key(t: &str) -> Option<(&'static str, &str)> {
    let low = t.to_lowercase();
    for a in ALIASES {
        if let Some(rest) = low.strip_prefix(a) {
            let rest_orig = &t[t.len() - rest.len()..];
            if rest_orig.is_empty() {
                return None; // 整行就是字段名，交给"新记录"处理
            }
            let first = rest_orig.chars().next().unwrap();
            if first.is_alphanumeric() {
                continue; // 后面紧跟字母数字 => 是别的单词，不是字段
            }
            let val = rest_orig
                .trim_start_matches([' ', '\t', ':', '：', '、', '-', '—', '|', '.', '）', ')'])
                .trim();
            return Some((canon_of(a)?, val));
        }
    }
    None
}

fn is_sep(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty()
        && t.chars().all(|c| matches!(c, '-' | '=' | '*' | '_' | '~'))
        && t.chars().count() >= 1
}

fn new_card() -> Card {
    Card {
        head: String::new(),
        fields: Vec::new(),
        extras: Vec::new(),
    }
}

fn push_field(c: &mut Card, canon: &str, val: &str) {
    if let Some(slot) = c.fields.iter_mut().find(|(n, _)| n == canon) {
        if !slot.1.is_empty() {
            slot.1.push_str(" / ");
        }
        slot.1.push_str(val);
    } else {
        c.fields.push((canon.to_string(), val.to_string()));
    }
}

/// 解析整份文本。永不返回 Err —— 空文件也会返回空卡片列表 + 提示。
pub fn parse(text: &str) -> (Vec<Card>, Vec<Warning>) {
    let mut cards: Vec<Card> = Vec::new();
    let mut warns: Vec<Warning> = Vec::new();
    let mut cur: Option<Card> = None;
    let mut cur_start_line = 1usize;

    for (i, raw) in text.lines().enumerate() {
        let lineno = i + 1;
        let line = raw.trim_end_matches('\r');
        let mut t = line.trim();
        if lineno == 1 {
            t = t.trim_start_matches('\u{feff}'); // 去掉记事本可能留下的 BOM
        }
        if t.is_empty() || is_sep(t) {
            if let Some(c) = cur.take() {
                cards.push(c);
            }
            continue;
        }
        if t.starts_with('#') || t.starts_with("//") {
            continue; // 注释行
        }

        // `键: 值`
        if let Some((k, v)) = split_kv(t) {
            if cur.is_none() {
                cur = Some(new_card());
                cur_start_line = lineno;
            }
            let c = cur.as_mut().unwrap();
            match canon_of(k) {
                Some("标题") => {
                    if c.head.is_empty() {
                        c.head = v.trim().to_string();
                    } else {
                        c.extras.push(v.trim().to_string());
                    }
                }
                Some(canon) => push_field(c, canon, v.trim()),
                None => c.extras.push(t.to_string()), // 认不出的键：原样保留
            }
            continue;
        }

        // 无冒号：可能是"漏写冒号"的字段行
        if let Some((canon, val)) = inline_key(t) {
            if cur.is_none() {
                cur = Some(new_card());
                cur_start_line = lineno;
            }
            let c = cur.as_mut().unwrap();
            if canon == "标题" {
                if c.head.is_empty() {
                    c.head = val.to_string();
                } else {
                    c.extras.push(t.to_string());
                }
            } else {
                push_field(c, canon, val);
            }
            continue;
        }

        // 普通一行：它就是新记录的主体（一行一个词/一句话）
        if let Some(c) = cur.take() {
            cards.push(c);
        }
        let mut c = new_card();
        c.head = t.to_string();
        cur = Some(c);
        cur_start_line = lineno;
    }
    if let Some(c) = cur.take() {
        cards.push(c);
    }

    // 规范化字段顺序；并保证每条记录都有可见正文（绝不丢内容）
    for c in cards.iter_mut() {
        let mut ordered: Vec<(String, String)> = Vec::new();
        for name in CANON {
            if let Some(pos) = c.fields.iter().position(|(n, _)| n == name) {
                ordered.push(c.fields.remove(pos));
            }
        }
        ordered.extend(c.fields.drain(..));
        c.fields = ordered;

        if c.head.is_empty() {
            let promoted_extra = if c.extras.is_empty() {
                None
            } else {
                Some(c.extras.remove(0))
            };
            let fallback = match promoted_extra {
                Some(x) => x,
                None => c
                    .fields
                    .iter()
                    .find(|(n, _)| n == "释义" || n == "例句" || n == "语法")
                    .map(|(_, v)| v.clone())
                    .or_else(|| c.fields.first().map(|(_, v)| v.clone()))
                    .unwrap_or_default(),
            };
            if !fallback.is_empty() {
                c.head = fallback.clone();
                warns.push(Warning {
                    line: cur_start_line,
                    text: fallback,
                    msg: "这一条没写主体（单词/句子），已用它的内容代替显示".into(),
                });
            }
        }
    }
    (cards, warns)
}

// ============================ 产品层：加载 / 推断 / 热更新 ============================

/// 卡片的三种形态（仅影响排版，不影响数据）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Word,
    Phrase,
    Sentence,
}

impl CardKind {
    /// 配置文件里用的键
    pub fn key(self) -> &'static str {
        match self {
            CardKind::Word => "word",
            CardKind::Phrase => "phrase",
            CardKind::Sentence => "sentence",
        }
    }
}

/// 解析 "类型: 句子"（或 type: sentence）这种显式覆盖行
fn override_of(line: &str) -> Option<CardKind> {
    let (k, v) = split_kv(line.trim())?;
    let key = k.trim().to_lowercase();
    if key != "类型" && key != "type" {
        return None;
    }
    match v.trim().to_lowercase().as_str() {
        "单词" | "单词卡" | "word" => Some(CardKind::Word),
        "短语" | "短语卡" | "phrase" => Some(CardKind::Phrase),
        "句子" | "句子卡" | "sentence" | "sent" => Some(CardKind::Sentence),
        _ => None,
    }
}

/// 自动推断卡片形态（报告 6.2 的规则）：
///   有 音标/词性 → 单词；结尾标点或 ≥4 个词 → 句子；其余 → 短语
/// 允许用 `类型: xxx` 显式覆盖。
pub fn infer_kind(card: &Card) -> CardKind {
    for e in &card.extras {
        if let Some(k) = override_of(e) {
            return k;
        }
    }
    if card.fields.iter().any(|(n, _)| n == "音标" || n == "词性") {
        return CardKind::Word;
    }
    let head = card.head.trim();
    let ends = head.ends_with('.')
        || head.ends_with('?')
        || head.ends_with('!')
        || head.ends_with('。')
        || head.ends_with('？')
        || head.ends_with('！');
    if ends || (head.contains(' ') && head.split_whitespace().count() >= 4) {
        CardKind::Sentence
    } else {
        CardKind::Phrase
    }
}

/// 读取文本：UTF-8（含 BOM）；失败则按 GB18030/GBK 解码（中国教室里最常见的编码）
pub fn read_text_any_encoding(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8(bytes[3..].to_vec()).ok();
    }
    match std::str::from_utf8(&bytes) {
        Ok(s) => Some(s.to_string()),
        Err(_) => {
            let (cow, _, _) = encoding_rs::GB18030.decode(&bytes);
            Some(cow.into_owned())
        }
    }
}

/// 词表文件名（按优先级找）
pub const WORD_FILE_NAMES: [&str; 3] = ["词表.txt", "words.txt", "data.txt"];

/// 在目录里找第一个存在的词表文件
pub fn find_word_file(dir: &Path) -> Option<PathBuf> {
    WORD_FILE_NAMES
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

/// 读一个词表文件并解析
pub fn load(path: &Path) -> Option<(Vec<Card>, Vec<Warning>)> {
    let text = read_text_any_encoding(path)?;
    Some(parse(&text))
}

/// 没找到词表文件时的内置兜底词表
pub fn builtin() -> (Vec<Card>, Vec<Warning>) {
    parse(include_str!("../default_words.txt"))
}

/// 取文件的修改时间（用于热更新轮询）
pub fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_word_list_one_per_line() {
        let (c, w) = parse("abandon\nbenefit\nclimb\n");
        assert_eq!(c.len(), 3, "一行一个词必须变成 3 条，而不是 1 条");
        assert_eq!(c[0].head, "abandon");
        assert_eq!(c[1].head, "benefit");
        assert!(w.is_empty());
    }

    #[test]
    fn english_words_are_not_mistaken_for_keys() {
        for w in [
            "position",
            "translation",
            "phone",
            "example2x",
            "none",
            "export",
        ] {
            let (c, _) = parse(&format!("{w}\n"));
            assert_eq!(c.len(), 1);
            assert_eq!(c[0].head, w, "{w} 被误判成字段了");
            assert!(c[0].fields.is_empty(), "{w} 不该产生字段");
        }
    }

    #[test]
    fn full_card_with_fullwidth_colon() {
        let src = "单词：abandon\n音标: /əˈbændən/\n词性: v.\n释义: 抛弃，放弃\n例句: He abandoned his car.\n例句翻译: 他弃车而去。\n";
        let (c, _) = parse(src);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].head, "abandon");
        assert_eq!(c[0].fields.len(), 5);
        assert_eq!(c[0].fields[0], ("音标".into(), "/əˈbændən/".into()));
    }

    #[test]
    fn missing_colon_still_becomes_field() {
        let (c, _) = parse("abandon\n音标 /əˈbændən/\n释义 抛弃\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].head, "abandon");
        assert_eq!(c[0].fields[0], ("音标".into(), "/əˈbændən/".into()));
        assert_eq!(c[0].fields[1], ("释义".into(), "抛弃".into()));
    }

    #[test]
    fn unknown_key_is_shown_not_lost() {
        let (c, _) = parse("abandon\n中文名: 抛弃\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].head, "abandon");
        assert_eq!(c[0].extras, vec!["中文名: 抛弃"]);
    }

    #[test]
    fn sentence_containing_colon_is_not_lost() {
        let (c, _) = parse("He said: hello to me.\n翻译: 他向我问好。\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].head, "He said: hello to me.");
        assert_eq!(c[0].fields[0], ("释义".into(), "他向我问好。".into()));
    }

    #[test]
    fn empty_head_never_hides_content() {
        let (c, w) = parse("例句: He abandoned his car.\n");
        assert_eq!(c.len(), 1);
        assert!(!c[0].head.is_empty(), "没有主体时必须用例句顶上，不能空白");
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn broken_json_is_just_plain_text() {
        // JSON 典型坏法：少逗号/少引号 —— 在 TXT 里只是普通文字，不会整份失效
        let src = "{\n \"word\": \"abandon\"\n \"meaning\": \"抛弃\"\n}\n";
        let (c, _) = parse(src);
        assert!(!c.is_empty());
    }

    #[test]
    fn blank_line_separates_records() {
        let (c, _) = parse("abandon\n释义: 抛弃\n\nbenefit\n释义: 受益\n");
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].head, "benefit");
    }

    #[test]
    fn dashes_separate_records() {
        let (c, _) = parse("abandon\n释义: 抛弃\n---\nbenefit\n释义: 受益\n");
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn sentence_card() {
        let src = "Good habits are hard to form but easy to live with.\n翻译: 好习惯难于养成，却易于相伴一生。\n语法: be hard to do 结构\n";
        let (c, _) = parse(src);
        assert_eq!(c.len(), 1);
        assert!(c[0].head.starts_with("Good habits"));
        assert_eq!(c[0].fields.len(), 2);
    }

    #[test]
    fn crlf_and_bom() {
        let src = "\u{feff}abandon\r\n音标: x\r\n";
        let (c, _) = parse(src);
        assert_eq!(c[0].head, "abandon");
        assert_eq!(c[0].fields[0].1, "x");
    }

    #[test]
    fn phrase_and_extra_form() {
        let src = "take part in\n释义: 参加\n备注: 后接名词或动名词\n";
        let (c, _) = parse(src);
        assert_eq!(c[0].head, "take part in");
        assert_eq!(c[0].fields.len(), 2);
    }

    #[test]
    fn only_garbage_no_panic() {
        let (c, _) = parse("\n\n\n\n***\n\n");
        assert_eq!(c.len(), 0);
    }

    // ---- 类型推断 ----

    #[test]
    fn infer_word_by_phonetic() {
        let (c, _) = parse("abandon\n音标: /x/\n");
        assert_eq!(infer_kind(&c[0]), CardKind::Word);
    }

    #[test]
    fn infer_word_by_pos() {
        let (c, _) = parse("abandon\n词性: v.\n");
        assert_eq!(infer_kind(&c[0]), CardKind::Word);
    }

    #[test]
    fn infer_sentence_by_punctuation() {
        let (c, _) = parse("Practice makes perfect.\n");
        assert_eq!(infer_kind(&c[0]), CardKind::Sentence);
    }

    #[test]
    fn infer_sentence_by_length() {
        let (c, _) = parse("the more you practice the better\n");
        assert_eq!(infer_kind(&c[0]), CardKind::Sentence);
    }

    #[test]
    fn infer_phrase_by_default() {
        let (c, _) = parse("take part in\n");
        assert_eq!(infer_kind(&c[0]), CardKind::Phrase);
    }

    #[test]
    fn explicit_override_wins() {
        let (c, _) = parse("as a result of\n类型: 短语\n");
        assert_eq!(infer_kind(&c[0]), CardKind::Phrase);

        let (c2, _) = parse("hello\n类型: 单词\n");
        assert_eq!(infer_kind(&c2[0]), CardKind::Word);
    }

    #[test]
    fn builtin_words_loads() {
        let (c, _) = builtin();
        assert!(c.len() >= 8, "内置词表应该至少有 8 条示例");
        let kinds: Vec<CardKind> = c.iter().map(infer_kind).collect();
        assert!(kinds.contains(&CardKind::Word));
        assert!(kinds.contains(&CardKind::Phrase));
        assert!(kinds.contains(&CardKind::Sentence));
    }
}
