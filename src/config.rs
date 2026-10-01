//! 设置读写：全部存在 exe 同目录的 `设置.txt`（绿色版原则：不写 AppData）。
//!
//! 两条铁律：
//!   ① 读取永远宽容 —— 坏行/未知键直接忽略，用默认值，绝不因为设置文件坏了就不启动；
//!   ② 写入失败一律静默忽略 —— 受限账户/只读目录下也照常运行。

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub window_w: f32,
    pub window_h: f32,
    /// 窗口位置（None = 首次启动，程序自动放到右下角）
    pub window_x: Option<f32>,
    pub window_y: Option<f32>,
    /// 用户字号微调（A+ / A-）；总缩放 = 本值 × 屏幕系数（程序自动，见 app::screen_scale）
    pub font_scale: f32,
    pub always_on_top: bool,
    /// 筛选："all" | "word" | "phrase" | "sentence"
    pub filter: String,
    /// 主题："wuling" | "yellow"
    pub theme: String,
    /// 目标悬浮窗数量（批量管理；0 = 全部关闭）
    pub float_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_w: 600.0,
            window_h: 560.0,
            window_x: None,
            window_y: None,
            font_scale: 0.7,
            always_on_top: true,
            filter: "all".into(),
            theme: "plain".into(),
            float_count: 1,
        }
    }
}

/// exe 所在目录（绿色版：配置、词表都在这里）
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn settings_path() -> PathBuf {
    exe_dir().join("设置.txt")
}

/// 宽容读取：任何一行读不懂就用默认值，绝不失败
pub fn load(path: &Path) -> Config {
    let mut cfg = Config::default();
    let Ok(text) = std::fs::read_to_string(path) else {
        return cfg;
    };
    let text = text.trim_start_matches('\u{feff}');
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(idx) = line.find(['=', ':']) else {
            continue;
        };
        let key = line[..idx].trim();
        let val = line[idx + 1..].trim();
        match key {
            "window_w" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        cfg.window_w = v.clamp(320.0, 10000.0);
                    }
                }
            }
            "window_h" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        cfg.window_h = v.clamp(140.0, 10000.0);
                    }
                }
            }
            "window_x" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() && v >= 0.0 {
                        cfg.window_x = Some(v);
                    }
                }
            }
            "window_y" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() && v >= 0.0 {
                        cfg.window_y = Some(v);
                    }
                }
            }
            "font_scale" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        cfg.font_scale = v.clamp(0.5, 3.0);
                    }
                }
            }
            "always_on_top" => {
                cfg.always_on_top =
                    matches!(val.to_lowercase().as_str(), "true" | "1" | "on" | "是");
            }
            "filter" => {
                let v = val.to_lowercase();
                if matches!(v.as_str(), "all" | "word" | "phrase" | "sentence") {
                    cfg.filter = v;
                }
            }
            "theme" => {
                cfg.theme = val.to_string();
            }
            "float_count" => {
                if let Ok(v) = val.parse::<usize>() {
                    cfg.float_count = v.min(8);
                }
            }
            _ => {}
        }
    }
    cfg
}

/// 写入失败静默忽略（写不了就算了，程序照常跑）
pub fn save(path: &Path, cfg: &Config) {
    let mut s = String::new();
    s.push_str("# Lexideck 设置（这一行和以 # 开头的行会被忽略）\n");
    s.push_str(&format!("window_w = {}\n", cfg.window_w.round()));
    s.push_str(&format!("window_h = {}\n", cfg.window_h.round()));
    s.push_str(&format!(
        "window_x = {}\n",
        cfg.window_x.map(|v| v.round()).unwrap_or(-1.0)
    ));
    s.push_str(&format!(
        "window_y = {}\n",
        cfg.window_y.map(|v| v.round()).unwrap_or(-1.0)
    ));
    s.push_str(&format!("font_scale = {:.2}\n", cfg.font_scale));
    s.push_str(&format!("always_on_top = {}\n", cfg.always_on_top));
    s.push_str(&format!("filter = {}\n", cfg.filter));
    s.push_str(&format!("theme = {}\n", cfg.theme));
    s.push_str(&format!("float_count = {}\n", cfg.float_count));
    let _ = std::fs::write(path, s);
}
