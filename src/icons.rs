//! 面板图标：全部用线条画出来，不依赖 PNG 素材。
//!
//! 原来是把 128px 的 PNG 缩到 20px 显示，缩小后必糊（采样丢失细节）；
//! 矢量画法在任意尺寸/缩放下都清晰，换主题也只改一个颜色参数。
//!
//! 坐标一律用 rect 内的归一化比例（0..1），所以缩放、DPI 变化都不用改。

use eframe::egui::{self, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

/// 在 `rect` 里画名为 `name` 的图标。未知名字什么都不画。
pub fn paint(p: &Painter, rect: Rect, name: &str, color: Color32) {
    let s = rect.width().min(rect.height());
    if s <= 0.0 {
        return;
    }
    // 线宽按尺寸走：20px 时约 1.5px，缩到 14px 也不会细到看不见
    let st = Stroke::new((s * 0.085).max(1.2), color);
    let x = |t: f32| rect.left() + rect.width() * t;
    let y = |t: f32| rect.top() + rect.height() * t;
    let q = |a: f32, b: f32| egui::pos2(x(a), y(b));
    let seg = |a: Pos2, b: Pos2| p.line_segment([a, b], st);
    let poly = |pts: Vec<Pos2>| p.line(pts, st);
    let squ = |a: f32, b: f32, c: f32, d: f32| {
        p.rect_stroke(
            Rect::from_min_max(q(a, b), q(c, d)),
            CornerRadius::ZERO,
            st,
            StrokeKind::Middle,
        )
    };
    let cross = |a: f32, b: f32, c: f32, d: f32| {
        seg(q(a, b), q(c, d));
        seg(q(c, b), q(a, d));
    };
    // 实心小三角（方向：下 / 右 / 上）
    let arrow = |tip: Pos2, dir: u8| {
        let k = s * 0.11;
        let pts = match dir {
            0 => vec![
                tip,
                egui::pos2(tip.x - k, tip.y - k),
                egui::pos2(tip.x + k, tip.y - k),
            ],
            1 => vec![
                tip,
                egui::pos2(tip.x - k, tip.y - k),
                egui::pos2(tip.x - k, tip.y + k),
            ],
            _ => vec![
                tip,
                egui::pos2(tip.x - k, tip.y + k),
                egui::pos2(tip.x + k, tip.y + k),
            ],
        };
        p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
    };

    match name {
        // 词库：一本合着的书（书脊 + 两道书页线）
        "deck" => {
            squ(0.14, 0.10, 0.86, 0.90);
            seg(q(0.38, 0.10), q(0.38, 0.90));
            seg(q(0.54, 0.38), q(0.74, 0.38));
        }
        // 导入：文件向下落进托盘
        "import" => {
            seg(q(0.50, 0.10), q(0.50, 0.52));
            seg(q(0.50, 0.52), q(0.32, 0.34));
            seg(q(0.50, 0.52), q(0.68, 0.34));
            poly(vec![
                q(0.18, 0.66),
                q(0.18, 0.88),
                q(0.82, 0.88),
                q(0.82, 0.66),
            ]);
        }
        // 卡片：两张叠放的卡片
        "float_list" => {
            squ(0.36, 0.08, 0.92, 0.50);
            squ(0.08, 0.46, 0.64, 0.88);
        }
        // 展示时间：日历
        "display" => {
            squ(0.14, 0.24, 0.86, 0.86);
            seg(q(0.14, 0.42), q(0.86, 0.42));
            seg(q(0.36, 0.12), q(0.36, 0.28));
            seg(q(0.64, 0.12), q(0.64, 0.28));
            p.circle_filled(q(0.50, 0.64), (s * 0.07).max(1.2), color);
        }
        // 主题：调色盘（圆 + 三个色点）
        "theme" => {
            squ(0.12, 0.12, 0.88, 0.88);
            p.rect_filled(
                Rect::from_min_max(q(0.50, 0.12), q(0.88, 0.88)),
                CornerRadius::ZERO,
                color,
            );
        }
        // 字体：一个大 A
        "font" => {
            seg(q(0.50, 0.10), q(0.16, 0.90));
            seg(q(0.50, 0.10), q(0.84, 0.90));
            seg(q(0.37, 0.70), q(0.63, 0.70));
        }
        // 置顶：图钉
        "pin" => {
            seg(q(0.18, 0.16), q(0.82, 0.16));
            seg(q(0.50, 0.88), q(0.50, 0.38));
            seg(q(0.30, 0.58), q(0.50, 0.38));
            seg(q(0.70, 0.58), q(0.50, 0.38));
        }
        // 关于：i
        "about" => {
            p.circle_stroke(q(0.5, 0.5), s * 0.38, st);
            seg(q(0.5, 0.28), q(0.5, 0.72));
        }
        // 退出：门 + 向右的箭头
        "exit" => {
            squ(0.10, 0.14, 0.54, 0.86);
            seg(q(0.46, 0.50), q(0.90, 0.50));
            seg(q(0.70, 0.32), q(0.90, 0.50));
            seg(q(0.70, 0.68), q(0.90, 0.50));
        }
        // 关闭：叉
        "close" => {
            cross(0.24, 0.24, 0.76, 0.76);
        }
        // 最小化：一条横线
        "minimize" => {
            seg(q(0.22, 0.56), q(0.78, 0.56));
        }
        // 全部关闭：框里打叉
        "close_all" => {
            cross(0.22, 0.22, 0.78, 0.78);
            seg(q(0.14, 0.14), q(0.14, 0.86));
            seg(q(0.86, 0.14), q(0.86, 0.86));
        }
        // 重新读取：缺口圆环 + 箭头
        "refresh" => {
            let n = 18;
            let pts: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let t = -1.9 + 5.1 * (i as f32 / n as f32);
                    egui::pos2(x(0.5 + 0.34 * t.cos()), y(0.5 + 0.34 * t.sin()))
                })
                .collect();
            p.line(pts, st);
            arrow(q(0.86, 0.30), 0);
        }
        // 重播：播放键 + 左边一条竖线（回到开头）
        "replay" => {
            p.add(Shape::convex_polygon(
                vec![q(0.38, 0.22), q(0.38, 0.78), q(0.80, 0.50)],
                color,
                Stroke::NONE,
            ));
            seg(q(0.22, 0.22), q(0.22, 0.78));
        }
        // 上一张 / 下一张
        "next" => {
            arrow(q(0.80, 0.50), 1);
        }
        "prev" => {
            arrow(q(0.20, 0.50), 2);
        }
        // 数量：三个点
        "count" => {
            for a in [0.24, 0.50, 0.76] {
                p.circle_filled(q(a, 0.5), (s * 0.09).max(1.2), color);
            }
        }
        _ => {}
    }
}
