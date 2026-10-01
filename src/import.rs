//! 导入会话：把「一个文件并进主库」的全过程做成纯逻辑，和界面解耦。
//!
//! 流程固定：解析 → 格式校验 → 分类（新增 / 重复 / 冲突）→ 逐条问冲突 → 提交。
//! **提交前主库完全不动**：`apply()` 只返回新词库，写盘由调用方做，写失败就当这次导入没发生。

use crate::deck::{self, Deck, DisplayItem, Entry};

/// 用户对一条冲突的选择
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// 保留主库里的，跳过这条
    Skip,
    /// 用文件里的覆盖主库
    Replace,
    /// 本次导入剩下的全部跳过
    SkipAll,
    /// 本次导入剩下的全部替换
    ReplaceAll,
}

/// 导入结束的结果清单
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Summary {
    pub adds: usize,
    pub dupes: usize,
    pub replaced: usize,
    pub skipped: usize,
    /// 展示时间：新增条数 / 重复并入条数
    pub dt_added: usize,
    pub dt_dupe: usize,
    /// 展示时间里引用了主库没有的词（P2 会逐个弹窗问，现在至少要说出来）
    pub dt_unmatched: Vec<String>,
}

pub struct Session {
    pub file: String,
    adds: Vec<Entry>,
    dupes: usize,
    /// 还没问过的冲突（(主库, 文件)）
    conflicts: Vec<(Entry, Entry)>,
    total_conflicts: usize,
    replaced: Vec<Entry>,
    skipped: usize,
    display_in: Vec<DisplayItem>,
}

impl Session {
    /// 用「要导入的文件」和「当前主库」开一次会话
    pub fn new(file: String, incoming: Deck, base: &[Entry]) -> Self {
        let plan = deck::plan_word_merge(base, &incoming.words);
        Self {
            file,
            adds: plan.adds,
            dupes: plan.dupes.len(),
            total_conflicts: plan.conflicts.len(),
            conflicts: plan.conflicts,
            replaced: Vec::new(),
            skipped: 0,
            display_in: incoming.display_time,
        }
    }

    /// 当前要问的冲突：((主库, 文件), 第几条, 共几条)
    pub fn current(&self) -> Option<(&Entry, &Entry, usize, usize)> {
        let (old, new) = self.conflicts.first()?;
        let idx = self.total_conflicts - self.conflicts.len() + 1;
        Some((old, new, idx, self.total_conflicts))
    }

    pub fn has_conflict(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn total_conflicts(&self) -> usize {
        self.total_conflicts
    }

    /// 处理一条冲突；"全部 X" 会把剩下的一次性做完，之后 `has_conflict()` 即为 false
    pub fn resolve(&mut self, choice: Choice) {
        match choice {
            Choice::Skip => {
                if !self.conflicts.is_empty() {
                    self.conflicts.remove(0);
                }
                self.skipped += 1;
            }
            Choice::Replace => {
                if !self.conflicts.is_empty() {
                    let (_, new) = self.conflicts.remove(0);
                    self.replaced.push(new);
                }
            }
            Choice::SkipAll => {
                self.skipped += self.conflicts.len();
                self.conflicts.clear();
            }
            Choice::ReplaceAll => {
                for (_, new) in std::mem::take(&mut self.conflicts) {
                    self.replaced.push(new);
                }
            }
        }
    }

    /// 生成新词库 + 结果清单（不改动传入的 base）
    pub fn apply(&self, base: &Deck) -> (Deck, Summary) {
        let mut next = base.clone();
        next.version = 1;
        for e in &self.adds {
            next.words.push(e.clone());
        }
        for e in &self.replaced {
            match deck::find_index(&next, &e.word) {
                Some(i) => next.words[i] = e.clone(),
                None => next.words.push(e.clone()),
            }
        }
        // 主库里不存在的词：这些展示时间会原样存进去，但永远匹配不到卡片
        let mut dt_unmatched: Vec<String> = Vec::new();
        for it in &self.display_in {
            if deck::find_index(&next, &it.word).is_none() {
                let w = it.word.clone();
                if !dt_unmatched.contains(&w) {
                    dt_unmatched.push(w);
                }
            }
        }
        let (dt_added, dt_dupe) =
            deck::merge_display_time(&mut next.display_time, self.display_in.clone());
        let summary = Summary {
            adds: self.adds.len(),
            dupes: self.dupes,
            replaced: self.replaced.len(),
            skipped: self.skipped,
            dt_added,
            dt_dupe,
            dt_unmatched,
        };
        (next, summary)
    }

    /// 一句话结果（面板底部状态栏 / 导入结果提示都用它）
    pub fn summary_line(file: &str, s: &Summary) -> String {
        let mut m = format!("已导入「{file}」：新增 {}、重复并入 {}", s.adds, s.dupes);
        if s.replaced > 0 {
            m.push_str(&format!("、替换 {}", s.replaced));
        }
        if s.skipped > 0 {
            m.push_str(&format!("、跳过 {}", s.skipped));
        }
        if s.dt_added + s.dt_dupe > 0 {
            m.push_str(&format!(
                "；展示时间 新增 {} / 重复 {}（P2 生效）",
                s.dt_added, s.dt_dupe
            ));
        }
        if !s.dt_unmatched.is_empty() {
            let names = s.dt_unmatched.join("、");
            m.push_str(&format!(
                "；注意：展示时间里有 {} 个词不在词库里（{names}），这些时间点不会显示任何卡片",
                s.dt_unmatched.len()
            ));
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::{Range, Sense};

    fn entry(word: &str, meaning: &str) -> Entry {
        Entry {
            word: word.into(),
            senses: vec![Sense {
                pos: "v.".into(),
                meaning: meaning.into(),
            }],
            ..Default::default()
        }
    }

    fn deck_of(entries: Vec<Entry>) -> Deck {
        Deck {
            version: 1,
            deck_name: "test".into(),
            words: entries,
            display_time: vec![],
        }
    }

    /// 无冲突：直接并入，重复的不再重复添加
    #[test]
    fn clean_import_merges() {
        let base = deck_of(vec![entry("seek", "寻找")]);
        let incoming = deck_of(vec![entry("seek", "寻找"), entry("gain", "获得")]);
        let s = Session::new("f.json".into(), incoming, &base.words);
        assert!(!s.has_conflict());
        let (next, sum) = s.apply(&base);
        assert_eq!(next.words.len(), 2);
        assert_eq!(
            (sum.adds, sum.dupes, sum.replaced, sum.skipped),
            (1, 1, 0, 0)
        );
    }

    /// 逐条：跳过保留主库，替换写入新内容
    #[test]
    fn per_conflict_skip_and_replace() {
        let base = deck_of(vec![entry("seek", "旧"), entry("gain", "旧")]);
        let incoming = deck_of(vec![entry("seek", "新"), entry("gain", "新")]);
        let mut s = Session::new("f.json".into(), incoming, &base.words);
        assert_eq!(s.total_conflicts(), 2);
        let (old, new, idx, total) = s.current().unwrap();
        assert_eq!(
            (
                old.senses[0].meaning.as_str(),
                new.senses[0].meaning.as_str()
            ),
            ("旧", "新")
        );
        assert_eq!((idx, total), (1, 2));
        s.resolve(Choice::Skip);
        s.resolve(Choice::Replace);
        assert!(!s.has_conflict());
        let (next, sum) = s.apply(&base);
        assert_eq!(sum.skipped, 1);
        assert_eq!(sum.replaced, 1);
        assert_eq!(deck::find(&next, "seek").unwrap().senses[0].meaning, "旧");
        assert_eq!(deck::find(&next, "gain").unwrap().senses[0].meaning, "新");
    }

    /// 全部跳过 / 全部替换：剩下的不再逐条问
    #[test]
    fn skip_all_and_replace_all_short_circuit() {
        let base = deck_of(vec![entry("a", "旧"), entry("b", "旧"), entry("c", "旧")]);
        let incoming = deck_of(vec![entry("a", "新"), entry("b", "新"), entry("c", "新")]);

        let mut s = Session::new("f.json".into(), incoming.clone(), &base.words);
        s.resolve(Choice::Skip);
        s.resolve(Choice::SkipAll);
        assert!(!s.has_conflict());
        let (_, sum) = s.apply(&base);
        assert_eq!((sum.skipped, sum.replaced), (3, 0));

        let mut s = Session::new("f.json".into(), incoming, &base.words);
        s.resolve(Choice::Replace);
        s.resolve(Choice::ReplaceAll);
        let (next, sum) = s.apply(&base);
        assert_eq!((sum.skipped, sum.replaced), (0, 3));
        for w in ["a", "b", "c"] {
            assert_eq!(deck::find(&next, w).unwrap().senses[0].meaning, "新");
        }
    }

    /// 提交不碰原库（写盘失败也不能污染内存状态）
    #[test]
    fn apply_does_not_mutate_base() {
        let base = deck_of(vec![entry("seek", "旧")]);
        let incoming = deck_of(vec![entry("gain", "获得")]);
        let s = Session::new("f.json".into(), incoming, &base.words);
        let _ = s.apply(&base);
        assert_eq!(base.words.len(), 1);
    }

    /// 展示时间：同样的日期段算重复，不同的追加
    #[test]
    fn display_time_dedupes_on_import() {
        let base = deck_of(vec![entry("seek", "寻找")]);
        let mut incoming = deck_of(vec![]);
        incoming.display_time = vec![
            DisplayItem {
                word: "seek".into(),
                ranges: vec![Range {
                    from: "2026-10-06".into(),
                    to: "2026-10-12".into(),
                }],
                hide: vec![],
            },
            DisplayItem {
                word: "seek".into(),
                ranges: vec![Range {
                    from: "2026-10-06".into(),
                    to: "2026-10-12".into(),
                }],
                hide: vec![],
            },
            DisplayItem {
                word: "seek".into(),
                ranges: vec![Range {
                    from: "2026-11-01".into(),
                    to: "2026-11-07".into(),
                }],
                hide: vec![],
            },
        ];
        let s = Session::new("f.json".into(), incoming, &base.words);
        let (next, sum) = s.apply(&base);
        assert_eq!((sum.dt_added, sum.dt_dupe), (2, 1));
        assert_eq!(next.display_time.len(), 2);
    }

    /// 展示时间引用了主库里没有的词：结果清单必须说出来（原来静默吞掉）
    #[test]
    fn unmatched_display_time_is_reported() {
        let base = deck::parse_file(
            r#"{"version":1,"words":[{"word":"seek","senses":[{"meaning":"寻求"}]}]}"#,
        )
        .unwrap();
        let inc = deck::parse_file(
            r#"{"version":1,"words":[],"display_time":[{"word":"seekk",
                "ranges":[{"from":"2026-10-06","to":"2026-10-12"}]}]}"#,
        )
        .unwrap();
        let sess = Session::new("t.json".into(), inc, &base.words);
        let (_next, sum) = sess.apply(&base);
        assert_eq!(sum.dt_unmatched, vec!["seekk".to_string()]);
        let line = Session::summary_line("t.json", &sum);
        assert!(line.contains("seekk"), "结果清单里要出现那个词：{line}");
    }
}
