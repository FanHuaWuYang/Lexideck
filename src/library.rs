//! 主库（唯一事实来源）：exe 同目录的 `lexideck.json`。
//!
//! · 导入 = 把文件并进主库；导入源本身不再被引用
//! · 旧版遗留的 `词表.json` 只在主库不存在时读一次（自动升级），不做长期兼容
//! · 所有写入失败都静默忽略（受限目录下也要能跑）

use std::path::{Path, PathBuf};

use crate::config;
use crate::deck::{self, Deck};
use crate::util;

/// 主库文件名
pub const LIB_NAME: &str = "lexideck.json";
/// 上一版用过的文件名（仅用于一次性升级）
const LEGACY_NAMES: [&str; 1] = ["词表.json"];

pub fn lib_path() -> PathBuf {
    config::exe_dir().join(LIB_NAME)
}

/// 一次读取的结果
pub struct Loaded {
    pub deck: Deck,
    pub warns: Vec<String>,
    /// 文件在、但读不懂。此时调用方**绝不允许**写盘，否则会把坏文件覆盖成空库
    pub corrupt: bool,
}

/// 读主库；没有就返回空库（不报错——第一次运行本来就没有）
pub fn load(path: &Path) -> Loaded {
    let mut warns = Vec::new();
    if path.is_file() {
        match deck::load_file(path) {
            Ok((d, w)) => {
                warns.extend(w);
                return Loaded {
                    deck: d,
                    warns,
                    corrupt: false,
                };
            }
            Err(errs) => {
                // 主库坏了：绝不覆盖，原样保留，只把问题报出去
                warns.extend(errs);
                warns.push("主库存在但读不懂：已保留原文件不动，请修好它或删掉它再导入。".into());
                return Loaded {
                    deck: Deck::default(),
                    warns,
                    corrupt: true,
                };
            }
        }
    }
    // 升级：老版本留下的 词表.json
    for name in LEGACY_NAMES {
        let legacy = path.with_file_name(name);
        if legacy.is_file() {
            if let Ok((d, w)) = deck::load_file(&legacy) {
                warns.extend(w);
                warns.push(format!("已从旧文件「{name}」读取词库"));
                let mut d = d;
                d.version = 1;
                let _ = deck::save(path, &d);
                return Loaded {
                    deck: d,
                    warns,
                    corrupt: false,
                };
            }
        }
    }
    Loaded {
        deck: Deck::default(),
        warns,
        corrupt: false,
    }
}

/// 兼容旧调用：只要词库和提示
pub fn load_or_empty(path: &Path) -> (Deck, Vec<String>) {
    let l = load(path);
    (l.deck, l.warns)
}

pub fn save(path: &Path, d: &Deck) -> Result<(), String> {
    deck::save(path, d)
}

pub fn mtime(path: &Path) -> Option<std::time::SystemTime> {
    util::mtime(path)
}

/// 主库里存着多少个词的展示时间
pub fn display_count(d: &Deck) -> usize {
    d.display_time.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_library_is_not_an_error() {
        let dir = std::env::temp_dir().join("lexideck-test-lib-empty");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("nope.json");
        let _ = std::fs::remove_file(&p);
        let (d, w) = load_or_empty(&p);
        assert!(d.words.is_empty());
        assert!(w.is_empty());
    }
}
