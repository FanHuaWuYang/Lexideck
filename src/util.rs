//! 小工具：宽容读取文本、文件时间戳。
//! （原先在 words.rs 里，随旧 TXT 词表一起平移过来。）

use std::path::Path;
use std::time::SystemTime;

/// 读取文本：UTF-8（含 BOM）；失败则按 GB18030/GBK 解码（中文环境最常见）。
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

/// 取文件的修改时间（用于热更新轮询）
pub fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}
