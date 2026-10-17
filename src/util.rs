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

/// 原子写：先写同目录临时文件、再整体替换（Windows 上 rename 覆盖是原子的）。
///
/// 直接 `fs::write` 是「先截断再写」——写到一半被杀（教室一体机直接断电、
/// 强杀进程）会留下半截 JSON；而主库/状态文件的「读不懂就拒写」保护又会把
/// 之后的导入全挡死，只能靠人手工修。先写 `.<文件名>.tmp` 再替换，失败时
/// 旧文件保持完整。
pub fn write_atomic(path: &Path, data: &str) -> std::io::Result<()> {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}
