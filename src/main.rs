//! Lexideck —— 教室一体机上的英语词卡悬浮看板。
//! 形态：一个控制面板（主窗口）统一管理多个无边框悬浮窗。
//! 入口：读取设置、构造主窗口选项、运行 eframe。

// release 版不弹出控制台窗口（debug 版保留控制台，方便看日志）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod anim;
mod app;
mod autostart;
mod card;
mod config;
mod control;
mod deck;
mod float;
mod icons;
mod import;
mod library;
mod menu;
mod schedule;
mod single;
mod theme;
mod tray;
mod util;
mod window;

fn main() -> eframe::Result<()> {
    // P3c 单实例：已经有实例在跑时不另开窗口，而是把那个实例的面板唤到前台，然后自己退出。
    // 判定必须发生在建窗口之前 —— 这才叫「第二次双击不开新窗口」。
    if !single::install() {
        single::wake_existing();
        return Ok(());
    }

    let cfg = config::load(&config::settings_path());
    let opts = window::main_options(&cfg);
    eframe::run_native(
        "lexideck",
        opts,
        Box::new(move |cc| Ok(Box::new(app::LexideckApp::new(cc, cfg)))),
    )
}
