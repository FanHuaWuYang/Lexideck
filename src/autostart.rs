//! 开机自启动（P3b）：写当前用户的启动项（`HKCU\...\CurrentVersion\Run`），免管理员。
//!
//! 三条约定：
//!   ① **事实以注册表为准**：设置页显示的开关状态每次都去读注册表（`query()`），
//!      而不是信 `设置.txt` —— 注册表项被安全软件清掉、exe 被挪到别的目录，都要如实反映。
//!      `设置.txt` 里那份只记「人的意图」（整个文件夹拷到新机器时，这个意图跟着走）；
//!      两者不一致时，以注册表为准，面板照实说。
//!   ② 免管理员：只碰 `HKEY_CURRENT_USER`，绝不碰 `HKLM`。
//!   ③ 写失败（受限账户 / 组策略）只回一句错误说明，设置页原样显示，绝不 panic。
//!
//! 为什么直接用 windows-sys 调注册表、不引第三方 crate：winit/托盘已经把 windows-sys
//! 带进依赖树，这里只要多开两个 feature，不为这一件事多编一个包（设计 §8 的风格）。

use std::path::{Path, PathBuf};

/// 启动项在 Run 键下的值名（注册表里显示的名字）
pub const VALUE_NAME: &str = "Lexideck";

/// 启动项现状（设置页据此显示；开关位置也跟它走）
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// 没有这一项 → 未开启
    Off,
    /// 有这一项，且指向当前 exe → 已开启
    On,
    /// 有这一项，但指向的不是当前 exe（exe 挪过目录 / 被别的东西改过）：里面是注册表里的旧路径
    Stale(String),
    /// 读注册表失败（原因写在这里）
    Unknown(String),
}

/// 当前 exe 的完整路径（启动项要指向它）
pub fn current_exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lexideck.exe"))
}

/// 注册表里存的值 → 路径：去掉两侧引号与空白。
/// Run 项的常规写法是带引号的完整路径（路径含空格时引号是官方推荐写法）。
pub fn parse_target(raw: &str) -> String {
    raw.trim().trim_matches('"').trim().to_string()
}

/// 两个路径是否指同一个文件：Windows 不区分大小写，末尾的斜杠忽略。
/// 空串永远算「不相同」——空值是坏了，不是「一致」。
pub fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_ascii_lowercase();
    !a.trim().is_empty() && norm(a) == norm(b)
}

/// 写进注册表的值：带引号的完整路径（含空格也不怕）
pub fn value_data(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

#[cfg(windows)]
mod imp {
    use super::{current_exe, parse_target, same_path, value_data, State, VALUE_NAME};
    use std::path::Path;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, REG_VALUE_TYPE,
    };

    /// 当前用户的启动项位置（`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`）
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    /// 宽字符（结尾补 0）：所有 Win32 W 系列 API 都要这个形态
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// REG_SZ 的字节序列：UTF-16LE，结尾补一个 0（cbData 要含结尾 0）
    fn sz_bytes(s: &str) -> Vec<u8> {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(|c| c.to_le_bytes())
            .collect()
    }

    fn err(code: WIN32_ERROR) -> String {
        format!("注册表错误码 {code}")
    }

    /// 读现状。这是设置页显示的唯一来源（每次现读）。
    pub fn query() -> State {
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let rc = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                KEY_READ,
                &mut key,
            );
            if rc == ERROR_FILE_NOT_FOUND {
                return State::Off; // 连 Run 键都没有 = 什么都没登记
            }
            if rc != ERROR_SUCCESS {
                return State::Unknown(err(rc));
            }

            let name = wide(VALUE_NAME);
            let mut ty: REG_VALUE_TYPE = 0;
            let mut size: u32 = 0;
            let rc = RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null(),
                &mut ty,
                std::ptr::null_mut(),
                &mut size,
            );
            if rc == ERROR_FILE_NOT_FOUND {
                RegCloseKey(key);
                return State::Off;
            }
            if rc != ERROR_SUCCESS {
                RegCloseKey(key);
                return State::Unknown(err(rc));
            }

            let mut buf = vec![0u8; size as usize];
            let rc = RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null(),
                &mut ty,
                buf.as_mut_ptr(),
                &mut size,
            );
            RegCloseKey(key);
            if rc != ERROR_SUCCESS {
                return State::Unknown(err(rc));
            }

            // REG_SZ = UTF-16LE（可能带结尾 0；size 是字节数）
            let u16s: Vec<u16> = buf
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let target = parse_target(&String::from_utf16_lossy(&u16s));
            if target.is_empty() {
                State::Stale("（值内容为空）".into())
            } else if same_path(&target, &current_exe().to_string_lossy()) {
                State::On
            } else {
                State::Stale(target)
            }
        }
    }

    /// 写启动项：值 = 带引号的当前 exe 全路径。Run 键一定在，只开不建（少一个 Win32_Security 依赖）。
    pub fn enable(exe: &Path) -> Result<(), String> {
        let bytes = sz_bytes(&value_data(exe));
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let rc = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            );
            if rc != ERROR_SUCCESS {
                return Err(err(rc));
            }
            let name = wide(VALUE_NAME);
            let rc = RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                bytes.as_ptr(),
                bytes.len() as u32,
            );
            RegCloseKey(key);
            if rc != ERROR_SUCCESS {
                return Err(err(rc));
            }
        }
        Ok(())
    }

    /// 删启动项。「本来就没有」也算成功（幂等）。
    pub fn disable() -> Result<(), String> {
        unsafe {
            let mut key: HKEY = std::ptr::null_mut();
            let rc = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            );
            if rc == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            if rc != ERROR_SUCCESS {
                return Err(err(rc));
            }
            let name = wide(VALUE_NAME);
            let rc = RegDeleteValueW(key, name.as_ptr());
            RegCloseKey(key);
            if rc != ERROR_SUCCESS && rc != ERROR_FILE_NOT_FOUND {
                return Err(err(rc));
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::State;
    use std::path::Path;

    /// 非 Windows：这一项没有意义，安静地当「未开启」
    pub fn query() -> State {
        State::Off
    }
    pub fn enable(_exe: &Path) -> Result<(), String> {
        Ok(())
    }
    pub fn disable() -> Result<(), String> {
        Ok(())
    }
}

pub use imp::{disable, enable, query};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_strips_quotes_and_space() {
        assert_eq!(
            parse_target("\"C:\\Lexideck\\lexideck.exe\""),
            "C:\\Lexideck\\lexideck.exe"
        );
        assert_eq!(
            parse_target("  C:\\Lexideck\\lexideck.exe  "),
            "C:\\Lexideck\\lexideck.exe"
        );
        // 引号外还有参数（别人手写的项可能带 --foo）：这里只做去引号，不解析命令行
        assert_eq!(parse_target("\"C:\\a b\\l.exe\""), "C:\\a b\\l.exe");
        assert_eq!(parse_target(""), "");
    }

    #[test]
    fn same_path_ignores_case_and_trailing_slash() {
        assert!(same_path(
            "C:\\Lexideck\\lexideck.exe",
            "c:\\lexideck\\LEXIDECK.EXE"
        ));
        assert!(same_path("C:\\Lexideck\\", "c:\\lexideck"));
        assert!(!same_path(
            "C:\\Lexideck\\lexideck.exe",
            "D:\\Lexideck\\lexideck.exe"
        ));
        // 空值不是「一致」：值被清空属于坏了，要走 Stale
        assert!(!same_path("", ""));
    }

    #[test]
    fn value_data_is_quoted_full_path() {
        assert_eq!(
            value_data(Path::new("C:\\Program Files\\Lexideck\\lexideck.exe")),
            "\"C:\\Program Files\\Lexideck\\lexideck.exe\""
        );
    }

    /// 现状的判定逻辑（不碰注册表）：把 query() 里那段纯逻辑单列出来验一遍
    fn classify(raw: &str, cur: &str) -> State {
        let target = parse_target(raw);
        if target.is_empty() {
            State::Stale("（值内容为空）".into())
        } else if same_path(&target, cur) {
            State::On
        } else {
            State::Stale(target)
        }
    }

    #[test]
    fn classify_three_cases() {
        let cur = "C:\\Lexideck\\lexideck.exe";
        assert_eq!(classify("\"C:\\Lexideck\\lexideck.exe\"", cur), State::On);
        assert_eq!(
            classify("\"D:\\旧目录\\lexideck.exe\"", cur),
            State::Stale("D:\\旧目录\\lexideck.exe".into())
        );
        assert_eq!(classify("", cur), State::Stale("（值内容为空）".into()));
    }
}
