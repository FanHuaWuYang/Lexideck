//! 三套配色主题（全部直角）：
//!   · 纯白   —— 暂定默认：白底 + 灰阶字 + 武陵青点缀（慕言 2026-09-25 指定"先用纯白"）
//!   · 浅青   —— 武陵浅青底版本（预演页配色，回校实测对比用）
//!   · 黄黑   —— 终末地官网风（深底荧光黄，备份）
//!
//! 浅色两套的字色/线色直接取自 9-19 预演终稿；色块用中青绿 #668D82
//! （预演页注释：浅底上淡青会"化掉"，必须深一档才闪得出来）。

use eframe::egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    Plain,
    Wuling,
    Yellow,
}

impl ThemeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ThemeKind::Plain => "plain",
            ThemeKind::Wuling => "wuling",
            ThemeKind::Yellow => "yellow",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::Plain => "纯白",
            ThemeKind::Wuling => "浅青",
            ThemeKind::Yellow => "黄黑",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "wuling" | "浅青" => ThemeKind::Wuling,
            "yellow" | "huang" | "黄黑" => ThemeKind::Yellow,
            _ => ThemeKind::Plain,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub kind: ThemeKind,
    /// 卡片底色（纯白主题为不透明白）
    pub bg: Color32,
    /// 主体文字
    pub fg: Color32,
    /// 次要文字（音标、释义）
    pub fg2: Color32,
    /// 更弱文字（例句、备注）
    pub fg3: Color32,
    /// 强调色（释义）
    pub accent: Color32,
    /// 动画色块 / 右侧色条 / 选中按钮底
    pub block: Color32,
    /// 色块上的文字（选中按钮文字）
    pub on_block: Color32,
    /// 边框 / 分隔线
    pub line: Color32,
    /// 警告文字（点缀橙）
    pub warn: Color32,
    /// 悬停底色
    pub hover_bg: Color32,
    /// 是否浅底
    pub is_light: bool,
}

pub fn get(kind: ThemeKind) -> Theme {
    match kind {
        ThemeKind::Plain => Theme {
            kind,
            bg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            fg: Color32::from_rgb(0x2F, 0x30, 0x30),
            fg2: Color32::from_rgb(0x4E, 0x72, 0x68),
            fg3: Color32::from_rgb(0x8E, 0xA1, 0x9B),
            accent: Color32::from_rgb(0x4E, 0x72, 0x68),
            block: Color32::from_rgb(0x66, 0x8D, 0x82),
            on_block: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            line: Color32::from_rgb(0xC5, 0xE0, 0xD9),
            warn: Color32::from_rgb(0xE0, 0x7A, 0x5F),
            hover_bg: Color32::from_rgb(0xF1, 0xF7, 0xF5),
            is_light: true,
        },
        ThemeKind::Wuling => Theme {
            kind,
            bg: Color32::from_rgb(0xEF, 0xF5, 0xF3),
            fg: Color32::from_rgb(0x2F, 0x30, 0x30),
            fg2: Color32::from_rgb(0x4E, 0x72, 0x68),
            fg3: Color32::from_rgb(0x8E, 0xA1, 0x9B),
            accent: Color32::from_rgb(0x4E, 0x72, 0x68),
            block: Color32::from_rgb(0x66, 0x8D, 0x82),
            on_block: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            line: Color32::from_rgb(0xC5, 0xE0, 0xD9),
            warn: Color32::from_rgb(0xE0, 0x7A, 0x5F),
            hover_bg: Color32::from_rgb(0xE6, 0xF1, 0xED),
            is_light: true,
        },
        ThemeKind::Yellow => Theme {
            kind,
            bg: Color32::from_rgb(0x19, 0x19, 0x19),
            fg: Color32::from_rgb(0xF2, 0xF2, 0xF2),
            fg2: Color32::from_rgb(0xC9, 0xC9, 0xC9),
            fg3: Color32::from_rgb(0x8E, 0x8E, 0x8E),
            accent: Color32::from_rgb(0xFF, 0xFA, 0x00),
            block: Color32::from_rgb(0xFF, 0xFA, 0x00),
            on_block: Color32::from_rgb(0x14, 0x14, 0x14),
            line: Color32::from_rgb(0x3D, 0x3D, 0x3D),
            warn: Color32::from_rgb(0xFF, 0xD2, 0x4A),
            hover_bg: Color32::from_rgb(0x2C, 0x2C, 0x2C),
            is_light: false,
        },
    }
}
