//! 设置读写：全部存在 exe 同目录的 `设置.txt`（绿色版原则：不写 AppData）。
//!
//! 两条铁律：
//!   ① 读取永远宽容 —— 坏行/未知键直接忽略，用默认值，绝不因为设置文件坏了就不启动；
//!   ② 写入失败一律静默忽略 —— 受限账户/只读目录下也照常运行。

use std::path::{Path, PathBuf};

/// 卡片层级（设计 §5.2）：默认**置底**（贴桌面最底层，PPT 全屏时被盖住）；
/// 置顶时配合鼠标穿透，卡片常显但点击穿过去落到 PPT 上。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layer {
    #[default]
    Bottom,
    Top,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Bottom => "bottom",
            Layer::Top => "top",
        }
    }
    pub fn parse(s: &str) -> Option<Layer> {
        match s.trim().to_lowercase().as_str() {
            "top" | "置顶" => Some(Layer::Top),
            "bottom" | "置底" => Some(Layer::Bottom),
            _ => None,
        }
    }
}

/// 老设置里 `always_on_top` 这类开关的宽松解析
fn parse_bool(s: &str) -> bool {
    matches!(s.trim().to_lowercase().as_str(), "true" | "1" | "on" | "是")
}

#[derive(Debug, Clone)]
pub struct Config {
    pub window_w: f32,
    pub window_h: f32,
    /// 窗口位置（None = 首次启动，程序自动放到右下角）
    pub window_x: Option<f32>,
    pub window_y: Option<f32>,
    /// 用户字号微调（A+ / A-）；总缩放 = 本值 × 屏幕系数（程序自动，见 app::screen_scale）
    pub font_scale: f32,
    /// 卡片层级：置底（默认）/ 置顶
    pub layer: Layer,
    /// 鼠标穿透（设计 §5.2）：开启后卡片窗口收不到任何鼠标事件，只能从面板关掉
    pub passthrough: bool,
    /// 垂直同步：开（默认）= 跟着显示器刷新率走，不撕裂、不白烧 CPU/GPU；
    /// 关 = 不限帧（高刷屏上想看满帧、或面板在 60Hz 屏而卡片在 165Hz 屏时用）
    pub vsync: bool,
    /// 主题："plain" | "wuling" | "yellow"
    pub theme: String,
    /// 当前在桌面上的卡片（词条 word，小写；顺序 = 平铺顺序）
    pub shown: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            window_w: 1004.0,
            window_h: 640.0,
            window_x: None,
            window_y: None,
            font_scale: 0.7,
            layer: Layer::Bottom,
            passthrough: false,
            vsync: true,
            theme: "plain".into(),
            shown: Vec::new(),
        }
    }
}

/// exe 所在目录（绿色版：配置、主库都在这里）
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
                        cfg.window_w = v.clamp(560.0, 10000.0);
                    }
                }
            }
            "window_h" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        cfg.window_h = v.clamp(360.0, 10000.0);
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
            "layer" => {
                if let Some(l) = Layer::parse(val) {
                    cfg.layer = l;
                }
            }
            "passthrough" => {
                cfg.passthrough = parse_bool(val);
            }
            "vsync" => {
                cfg.vsync = parse_bool(val);
            }
            // 老键兼容：P1 的设置文件里是 always_on_top = true|false
            "always_on_top" => {
                cfg.layer = if parse_bool(val) {
                    Layer::Top
                } else {
                    Layer::Bottom
                };
            }
            "theme" => {
                cfg.theme = val.to_string();
            }
            "shown" => {
                // 新格式是 JSON 数组：词条本身可能带逗号（"Well, done."），逗号分隔存不住
                let t = val.trim();
                cfg.shown = if t.starts_with('[') {
                    serde_json::from_str::<Vec<String>>(t)
                        .map(clean_keys)
                        .unwrap_or_default()
                } else {
                    // 兼容旧格式
                    clean_keys(t.split([',', '，']).map(|s| s.to_string()).collect())
                };
            }
            _ => {}
        }
    }
    cfg
}

/// 统一 key 的形态：去空白、转小写、丢掉空串
fn clean_keys(v: Vec<String>) -> Vec<String> {
    v.into_iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
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
    s.push_str(&format!("layer = {}\n", cfg.layer.as_str()));
    s.push_str(&format!("passthrough = {}\n", cfg.passthrough));
    s.push_str(&format!("vsync = {}\n", cfg.vsync));
    s.push_str(&format!("theme = {}\n", cfg.theme));
    s.push_str(&format!(
        "shown = {}\n",
        serde_json::to_string(&cfg.shown).unwrap_or_else(|_| "[]".into())
    ));
    let _ = std::fs::write(path, s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shown_round_trips() {
        let dir = std::env::temp_dir().join("lexideck-test-cfg");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("设置.txt");
        let mut c = Config::default();
        c.shown = vec!["seek".into(), "abandon".into()];
        save(&p, &c);
        let back = load(&p);
        assert_eq!(back.shown, vec!["seek".to_string(), "abandon".to_string()]);
    }

    #[test]
    fn broken_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("lexideck-test-cfg2");
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("设置.txt");
        std::fs::write(&p, "window_w = abc\n????\nshown = ,,\n").unwrap();
        let c = load(&p);
        assert_eq!(c.window_w, 1004.0);
        assert!(c.shown.is_empty());
    }

    /// 词条本身带逗号（"as, well as"）时也要能原样存回来
    #[test]
    fn shown_keeps_commas_in_words() {
        let dir = std::env::temp_dir().join(format!("lexideck-test-cfg-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("设置.txt");
        let _ = std::fs::remove_file(&p);
        let mut c = Config::default();
        c.shown = vec!["as, well as".into(), "hello".into()];
        save(&p, &c);
        let back = load(&p);
        assert_eq!(
            back.shown,
            vec!["as, well as".to_string(), "hello".to_string()]
        );
    }

    /// 层级两档：新键 layer/passthrough 往返；老键 always_on_top 迁成 layer
    #[test]
    fn layer_round_trips_and_old_key_migrates() {
        let dir =
            std::env::temp_dir().join(format!("lexideck-test-cfg-layer-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("设置.txt");
        let _ = std::fs::remove_file(&p);

        let mut c = Config::default();
        assert_eq!(c.layer, Layer::Bottom, "默认置底");
        assert!(!c.passthrough, "默认不穿透");
        c.layer = Layer::Top;
        c.passthrough = true;
        save(&p, &c);
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("layer = top"), "{text}");
        assert!(text.contains("passthrough = true"), "{text}");
        assert!(
            !text.contains("always_on_top"),
            "新文件不该再写老键：{text}"
        );
        let back = load(&p);
        assert_eq!(back.layer, Layer::Top);
        assert!(back.passthrough);

        // 老文件只有 always_on_top = true → 迁成置顶
        std::fs::write(&p, "always_on_top = true\n").unwrap();
        assert_eq!(load(&p).layer, Layer::Top);
        // always_on_top = false → 置底
        std::fs::write(&p, "always_on_top = false\n").unwrap();
        assert_eq!(load(&p).layer, Layer::Bottom);
        let _ = std::fs::remove_file(&p);
    }
}
