//! Lexideck —— 教室一体机上的英语词卡悬浮看板。
//! 形态：一个控制面板（主窗口）统一管理多个无边框悬浮窗。
//! 入口：读取设置、构造主窗口选项、运行 eframe。

// release 版不弹出控制台窗口（debug 版保留控制台，方便看日志）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anim;
mod app;
mod card;
mod config;
mod control;
mod deck;
mod float;
mod icons;
mod import;
mod library;
mod schedule;
mod theme;
mod util;
mod window;

fn main() -> eframe::Result<()> {
    let cfg = config::load(&config::settings_path());
    let opts = window::main_options(&cfg);
    eframe::run_native(
        "lexideck",
        opts,
        Box::new(move |cc| Ok(Box::new(app::LexideckApp::new(cc, cfg)))),
    )
}
