//! 入场动画：闪烁 → 色块滑出 → 右侧留色条。
//!
//! 时序（2026-09-25 慕言当天两轮修正后定稿）：
//!   ① 0.00 → 0.70s 闪烁：色块用官网 flashing 节奏出现（纯色、无光晕）
//!   ② 0.70 → 1.90s 滑出：色块左缘向右扫过（easeOutExpo，1.2s）
//!   ③ 结束：色块停在右缘，留下一根竖色条
//!
//! 闪烁节奏与色块几何来自 2026-09-19 网页预演终稿（`入场动效预演-武陵浅青.html`）；
//! 滑出曲线经多轮实机试错：easeInOutCubic → easeInOutExpo → 快攻长拖尾 v1/v2 全部被否，
//! 2026-09-25 晚**最终回到第一版 `easeOutExpo`**（极快起步→急速减速），时长保留 1.2s。

use std::time::Instant;

/// 闪烁段结束（秒）
pub const FLASH_END: f32 = 0.70;
/// 色块滑出段时长（秒）
pub const WIPE_DUR: f32 = 1.2;
/// 全程时长（秒）
pub const TOTAL: f32 = FLASH_END + WIPE_DUR;

/// 官网 @keyframes flashing 的节奏采样点
/// （第三次闪 40% → 30% 提前收；全显 100% → 62% 提前）
const FLASH_PTS: [(f32, f32); 8] = [
    (0.00, 0.0),
    (0.10, 0.5),
    (0.11, 0.0),
    (0.20, 0.5),
    (0.21, 0.0),
    (0.30, 0.5),
    (0.31, 0.0),
    (0.62, 1.0),
];

/// 闪烁曲线：把 0→1 的进度映射为色块透明度
pub fn flashing(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    for i in 1..FLASH_PTS.len() {
        if u <= FLASH_PTS[i].0 {
            let (x0, y0) = FLASH_PTS[i - 1];
            let (x1, y1) = FLASH_PTS[i];
            let d = if (x1 - x0).abs() < f32::EPSILON {
                1.0
            } else {
                x1 - x0
            };
            return y0 + (y1 - y0) * ((u - x0) / d);
        }
    }
    1.0
}

/// easeOutExpo —— 第一版（慕言 2026-09-25 晚最终选定）：
/// 极快起步、急速减速，"唰"地扫过之后稳稳停住；无回弹、无过冲。
pub fn ease_out_expo(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return 1.0;
    }
    1.0 - (2.0f32).powf(-10.0 * t)
}

/// 一个悬浮窗的动画状态
#[derive(Debug, Clone)]
pub struct Anim {
    started: Option<Instant>,
}

impl Anim {
    pub fn new() -> Self {
        Self { started: None }
    }

    /// 从头播放一次
    pub fn play(&mut self) {
        self.started = Some(Instant::now());
    }

    pub fn is_playing(&self) -> bool {
        self.started.is_some()
    }

    /// 推进到当前时刻；播完自动回到常态并返回 None
    pub fn tick(&mut self) -> Option<f32> {
        let t0 = self.started?;
        let e = t0.elapsed().as_secs_f32();
        if e >= TOTAL {
            self.started = None;
            return None;
        }
        Some(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flashing_curve_hits_key_points() {
        assert!((flashing(0.0) - 0.0).abs() < 1e-6);
        assert!((flashing(0.10) - 0.5).abs() < 1e-6);
        assert!((flashing(0.105) - 0.25).abs() < 1e-6);
        assert!((flashing(0.11) - 0.0).abs() < 1e-6);
        assert!((flashing(0.62) - 1.0).abs() < 1e-6);
        assert!((flashing(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn expo_curve_fast_start_and_no_overshoot() {
        assert!((ease_out_expo(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_out_expo(1.0) - 1.0).abs() < 1e-6);
        // 起步极快：前 10% 的时间就走完一半路程
        assert!(ease_out_expo(0.1) > 0.49, "easeOutExpo 起步应该非常快");
        // 无回弹：全程不过冲
        let peak = (0..=100)
            .map(|i| ease_out_expo(i as f32 / 100.0))
            .fold(0.0f32, f32::max);
        assert!(
            peak <= 1.0 + 1e-6,
            "easeOutExpo 不应有过冲，实际峰值 {peak}"
        );
    }

    #[test]
    fn anim_ends_after_total() {
        let mut a = Anim::new();
        assert!(a.tick().is_none(), "没播放时应为常态");
        a.play();
        assert!(a.is_playing());
        let t = a.tick().unwrap();
        assert!(t < TOTAL);
    }
}
