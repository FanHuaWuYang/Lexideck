//! 面板图标：全部由 `assets/icons/*.svg` 的矢量路径画出来。
//!
//! 图标源码是标准 SVG（24×24、1.75 描边、统一留白），
//! 运行时解析成折线并按屏幕尺寸缩放描边，不依赖位图、任意 DPI 都清晰。

use eframe::egui::{Color32, Painter, Rect};

/// 在 `rect` 里画名为 `name` 的图标。未知名字什么都不画。
pub fn paint(p: &Painter, rect: Rect, name: &str, color: Color32) {
    crate::svg_icon::paint(p, rect, name, color);
}
