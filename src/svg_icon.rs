//! 极简 SVG 图标渲染：解析仓库里的 `assets/icons/*.svg`，画成 egui 线段。
//!
//! 只支持图标集用到的子集：path（M/L/H/V/C/S/Q/T/A/Z，含相对命令）、
//! line、polyline、polygon、rect（含 rx）、circle。颜色统一由调用方给，
//! 忽略 SVG 里的 stroke/fill 颜色（`fill="currentColor"` 视为实心）。
//!
//! 目的：图标源码就是标准 SVG（可以在任何矢量工具里编辑/预览），
//! 运行时按屏幕尺寸缩放描边，不依赖位图，也不会在高 DPI 下发糊。

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct SubPath {
    pub pts: Vec<[f32; 2]>,
    pub closed: bool,
    pub fill: bool,
}

pub const VIEW: f32 = 24.0;

fn svg_sources() -> &'static [(&'static str, &'static str)] {
    &[
        ("about", include_str!("../assets/icons/about.svg")),
        ("chev", include_str!("../assets/icons/chev.svg")),
        ("chev_up", include_str!("../assets/icons/chev_up.svg")),
        ("close", include_str!("../assets/icons/close.svg")),
        ("close_all", include_str!("../assets/icons/close_all.svg")),
        ("count", include_str!("../assets/icons/count.svg")),
        ("deck", include_str!("../assets/icons/deck.svg")),
        ("display", include_str!("../assets/icons/display.svg")),
        ("exit", include_str!("../assets/icons/exit.svg")),
        ("float_list", include_str!("../assets/icons/float_list.svg")),
        ("font", include_str!("../assets/icons/font.svg")),
        ("geo", include_str!("../assets/icons/geo.svg")),
        ("import", include_str!("../assets/icons/import.svg")),
        ("minimize", include_str!("../assets/icons/minimize.svg")),
        ("next", include_str!("../assets/icons/next.svg")),
        ("pin", include_str!("../assets/icons/pin.svg")),
        ("prev", include_str!("../assets/icons/prev.svg")),
        ("refresh", include_str!("../assets/icons/refresh.svg")),
        ("replay", include_str!("../assets/icons/replay.svg")),
        ("theme", include_str!("../assets/icons/theme.svg")),
    ]
}

fn cache() -> &'static HashMap<&'static str, Vec<SubPath>> {
    static M: OnceLock<HashMap<&'static str, Vec<SubPath>>> = OnceLock::new();
    M.get_or_init(|| {
        svg_sources()
            .iter()
            .map(|(name, src)| (*name, parse(src)))
            .collect()
    })
}

/// 在 rect 里画图标。未知名字什么都不画。
pub fn paint(p: &Painter, rect: Rect, name: &str, color: Color32) {
    let Some(paths) = cache().get(name) else {
        return;
    };
    let side = rect.width().min(rect.height());
    if side <= 1.0 {
        return;
    }
    let scale = side / VIEW;
    let ox = rect.center().x - VIEW * scale * 0.5;
    let oy = rect.center().y - VIEW * scale * 0.5;
    let stroke = Stroke::new((1.75 * scale).max(1.1), color);
    let map = |q: [f32; 2]| egui::pos2(ox + q[0] * scale, oy + q[1] * scale);

    for sp in paths {
        if sp.pts.len() < 2 {
            if sp.fill && sp.pts.len() == 1 {
                p.circle_filled(map(sp.pts[0]), stroke.width * 0.6, color);
            }
            continue;
        }
        let pts: Vec<Pos2> = sp.pts.iter().map(|q| map(*q)).collect();
        if sp.fill {
            p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
        } else if sp.closed {
            p.add(Shape::closed_line(pts, stroke));
        } else {
            let first = pts[0];
            let last = pts[pts.len() - 1];
            p.line(pts, stroke);
            // 圆头端点（egui 的线没有 cap 设置，用小圆点补）
            p.circle_filled(first, stroke.width * 0.5, color);
            p.circle_filled(last, stroke.width * 0.5, color);
        }
    }
}

// ── SVG 解析 ─────────────────────────────────────────────────────────────

fn parse(src: &str) -> Vec<SubPath> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != b'>' {
            j += 1;
        }
        if j >= bytes.len() {
            break;
        }
        let raw = &src[start..j];
        i = j + 1;
        let tag = raw.trim_end_matches('/').trim();
        let name_end = tag.find(|c: char| c.is_whitespace()).unwrap_or(tag.len());
        let tag_name = &tag[..name_end];
        let attrs = parse_attrs(&tag[name_end..]);
        match tag_name {
            "path" => {
                if let Some(d) = attrs.get("d") {
                    let fill = attrs
                        .get("fill")
                        .map(|f| f == "currentColor")
                        .unwrap_or(false);
                    let mut sps = parse_path(d);
                    for sp in &mut sps {
                        sp.fill = fill;
                    }
                    out.extend(sps);
                }
            }
            "line" => {
                let p1 = (num(&attrs, "x1"), num(&attrs, "y1"));
                let p2 = (num(&attrs, "x2"), num(&attrs, "y2"));
                out.push(SubPath {
                    pts: vec![[p1.0, p1.1], [p2.0, p2.1]],
                    closed: false,
                    fill: false,
                });
            }
            "polyline" | "polygon" => {
                if let Some(pts) = attrs.get("points") {
                    let nums = numbers(pts);
                    let mut path = Vec::new();
                    for chunk in nums.chunks(2) {
                        if chunk.len() == 2 {
                            path.push([chunk[0], chunk[1]]);
                        }
                    }
                    out.push(SubPath {
                        pts: path,
                        closed: tag_name == "polygon",
                        fill: false,
                    });
                }
            }
            "rect" => {
                let x = num(&attrs, "x");
                let y = num(&attrs, "y");
                let w = num(&attrs, "width");
                let h = num(&attrs, "height");
                let r = num(&attrs, "rx").min(w * 0.5).min(h * 0.5);
                out.push(SubPath {
                    pts: rounded_rect(x, y, w, h, r),
                    closed: true,
                    fill: false,
                });
            }
            "circle" => {
                let cx = num(&attrs, "cx");
                let cy = num(&attrs, "cy");
                let r = num(&attrs, "r");
                let fill = attrs
                    .get("fill")
                    .map(|f| f == "currentColor")
                    .unwrap_or(false);
                out.push(SubPath {
                    pts: circle(cx, cy, r),
                    closed: true,
                    fill,
                });
            }
            _ => {}
        }
    }
    out
}

fn parse_attrs(s: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        let ks = i;
        while i < b.len() && b[i] != b'=' && !(b[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let key = s[ks..i].to_string();
        while i < b.len() && b[i] != b'=' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        i += 1;
        while i < b.len() && (b[i] as char).is_whitespace() {
            i += 1;
        }
        if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
            let quote = b[i];
            i += 1;
            let vs = i;
            while i < b.len() && b[i] != quote {
                i += 1;
            }
            m.insert(key, s[vs..i].to_string());
            i += 1;
        } else {
            let vs = i;
            while i < b.len() && !(b[i] as char).is_whitespace() {
                i += 1;
            }
            m.insert(key, s[vs..i].to_string());
        }
    }
    m
}

fn num(attrs: &HashMap<String, String>, key: &str) -> f32 {
    attrs
        .get(key)
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(0.0)
}

fn numbers(s: &str) -> Vec<f32> {
    // SVG 允许把负数直接接在前一个数后面（"1-2.2-5.3"），遇到 '-' 就要断开
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev_numeric = false;
    for ch in s.chars() {
        let is_num = ch.is_ascii_digit() || ch == '.' || ch == 'e' || ch == 'E';
        if ch == '-' || ch == '+' {
            if !cur.is_empty() {
                if let Ok(v) = cur.parse::<f32>() {
                    out.push(v);
                }
                cur.clear();
            }
            prev_numeric = false;
            cur.push(ch);
            continue;
        }
        if is_num {
            cur.push(ch);
            prev_numeric = true;
        } else if prev_numeric {
            if let Ok(v) = cur.parse::<f32>() {
                out.push(v);
            }
            cur.clear();
            prev_numeric = false;
        }
    }
    if !cur.is_empty() {
        if let Ok(v) = cur.parse::<f32>() {
            out.push(v);
        }
    }
    out
}

fn circle(cx: f32, cy: f32, r: f32) -> Vec<[f32; 2]> {
    let n = 40;
    let mut v = Vec::with_capacity(n);
    for k in 0..n {
        let a = std::f32::consts::TAU * k as f32 / n as f32;
        v.push([cx + r * a.cos(), cy + r * a.sin()]);
    }
    v
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<[f32; 2]> {
    if r <= 0.01 {
        return vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    }
    let mut v = Vec::new();
    let corners = [
        (x + w - r, y + r, -std::f32::consts::FRAC_PI_2, 0.0),
        (x + w - r, y + h - r, 0.0, std::f32::consts::FRAC_PI_2),
        (
            x + r,
            y + h - r,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
        ),
        (
            x + r,
            y + r,
            std::f32::consts::PI,
            1.5 * std::f32::consts::PI,
        ),
    ];
    for (cx, cy, a0, a1) in corners {
        let steps = 8;
        for k in 0..=steps {
            let a = a0 + (a1 - a0) * k as f32 / steps as f32;
            v.push([cx + r * a.cos(), cy + r * a.sin()]);
        }
    }
    v
}

// ── path d 解析 ─────────────────────────────────────────────────────────

fn parse_path(d: &str) -> Vec<SubPath> {
    let mut out: Vec<SubPath> = Vec::new();
    let mut cur = [0.0f32, 0.0];
    let mut start = [0.0f32, 0.0];
    let mut last_ctrl: Option<[f32; 2]> = None;
    let mut cmd = ' ';
    let mut nums: Vec<f32> = numbers(d);
    // 按命令字母切分
    let mut tokens: Vec<(char, Vec<f32>)> = Vec::new();
    let mut text = String::new();
    let mut idx = 0;
    let bs: Vec<char> = d.chars().collect();
    while idx < bs.len() {
        let ch = bs[idx];
        if ch.is_ascii_alphabetic() {
            if !text.trim().is_empty() {
                tokens.push((cmd, numbers(&text)));
                text.clear();
            }
            cmd = ch;
            idx += 1;
        } else {
            text.push(ch);
            idx += 1;
        }
    }
    if !text.trim().is_empty() {
        tokens.push((cmd, numbers(&text)));
    }
    nums.clear();

    for (c, args) in tokens {
        let rel = c.is_ascii_lowercase();
        let mut c = c.to_ascii_uppercase();
        let mut k = 0;
        // 有些命令可以连续多组参数（如 L 1 2 3 4）
        let step = match c {
            'M' | 'L' | 'T' => 2,
            'H' | 'V' => 1,
            'C' => 6,
            'S' | 'Q' => 4,
            'A' => 7,
            _ => 0,
        };
        if c == 'Z' {
            if let Some(sp) = out.last_mut() {
                sp.closed = true;
            }
            cur = start;
            continue;
        }
        while k + step <= args.len() && step > 0 {
            let a = &args[k..k + step];
            k += step;
            match c {
                'M' => {
                    let p = if rel {
                        [cur[0] + a[0], cur[1] + a[1]]
                    } else {
                        [a[0], a[1]]
                    };
                    cur = p;
                    start = p;
                    out.push(SubPath {
                        pts: vec![p],
                        closed: false,
                        fill: false,
                    });
                    // 后续隐式 L（相对命令继续按相对处理）
                    c = 'L';
                }
                'L' => {
                    let p = if rel {
                        [cur[0] + a[0], cur[1] + a[1]]
                    } else {
                        [a[0], a[1]]
                    };
                    if let Some(sp) = out.last_mut() {
                        sp.pts.push(p);
                    }
                    cur = p;
                }
                'H' => {
                    let x = if rel { cur[0] + a[0] } else { a[0] };
                    if let Some(sp) = out.last_mut() {
                        sp.pts.push([x, cur[1]]);
                    }
                    cur = [x, cur[1]];
                }
                'V' => {
                    let y = if rel { cur[1] + a[0] } else { a[0] };
                    if let Some(sp) = out.last_mut() {
                        sp.pts.push([cur[0], y]);
                    }
                    cur = [cur[0], y];
                }
                'C' => {
                    let c1 = if rel {
                        [cur[0] + a[0], cur[1] + a[1]]
                    } else {
                        [a[0], a[1]]
                    };
                    let c2 = if rel {
                        [cur[0] + a[2], cur[1] + a[3]]
                    } else {
                        [a[2], a[3]]
                    };
                    let p = if rel {
                        [cur[0] + a[4], cur[1] + a[5]]
                    } else {
                        [a[4], a[5]]
                    };
                    sample_cubic(&mut out, cur, c1, c2, p);
                    last_ctrl = Some(c2);
                    cur = p;
                }
                'S' => {
                    let c1 = match last_ctrl {
                        Some(l) => [2.0 * cur[0] - l[0], 2.0 * cur[1] - l[1]],
                        None => cur,
                    };
                    let c2 = if rel {
                        [cur[0] + a[0], cur[1] + a[1]]
                    } else {
                        [a[0], a[1]]
                    };
                    let p = if rel {
                        [cur[0] + a[2], cur[1] + a[3]]
                    } else {
                        [a[2], a[3]]
                    };
                    sample_cubic(&mut out, cur, c1, c2, p);
                    last_ctrl = Some(c2);
                    cur = p;
                }
                'Q' => {
                    let q = if rel {
                        [cur[0] + a[0], cur[1] + a[1]]
                    } else {
                        [a[0], a[1]]
                    };
                    let p = if rel {
                        [cur[0] + a[2], cur[1] + a[3]]
                    } else {
                        [a[2], a[3]]
                    };
                    let c1 = [
                        cur[0] + 2.0 / 3.0 * (q[0] - cur[0]),
                        cur[1] + 2.0 / 3.0 * (q[1] - cur[1]),
                    ];
                    let c2 = [
                        p[0] + 2.0 / 3.0 * (q[0] - p[0]),
                        p[1] + 2.0 / 3.0 * (q[1] - p[1]),
                    ];
                    sample_cubic(&mut out, cur, c1, c2, p);
                    cur = p;
                }
                'T' => {
                    let q = match last_ctrl {
                        Some(l) => [2.0 * cur[0] - l[0], 2.0 * cur[1] - l[1]],
                        None => cur,
                    };
                    let p = if rel {
                        [cur[0] + a[0], cur[1] + a[1]]
                    } else {
                        [a[0], a[1]]
                    };
                    let c1 = [
                        cur[0] + 2.0 / 3.0 * (q[0] - cur[0]),
                        cur[1] + 2.0 / 3.0 * (q[1] - cur[1]),
                    ];
                    let c2 = [
                        p[0] + 2.0 / 3.0 * (q[0] - p[0]),
                        p[1] + 2.0 / 3.0 * (q[1] - p[1]),
                    ];
                    sample_cubic(&mut out, cur, c1, c2, p);
                    cur = p;
                }
                'A' => {
                    let p = if rel {
                        [cur[0] + a[5], cur[1] + a[6]]
                    } else {
                        [a[5], a[6]]
                    };
                    sample_arc(&mut out, cur, p, a[0], a[1], a[2], a[3] != 0.0, a[4] != 0.0);
                    cur = p;
                }
                _ => {}
            }
        }
    }
    out.retain(|sp| sp.pts.len() > 1 || sp.fill);
    out
}

fn sample_cubic(out: &mut Vec<SubPath>, p0: [f32; 2], c1: [f32; 2], c2: [f32; 2], p1: [f32; 2]) {
    let n = 14;
    if let Some(sp) = out.last_mut() {
        for k in 1..=n {
            let t = k as f32 / n as f32;
            let mt = 1.0 - t;
            let x = mt * mt * mt * p0[0]
                + 3.0 * mt * mt * t * c1[0]
                + 3.0 * mt * t * t * c2[0]
                + t * t * t * p1[0];
            let y = mt * mt * mt * p0[1]
                + 3.0 * mt * mt * t * c1[1]
                + 3.0 * mt * t * t * c2[1]
                + t * t * t * p1[1];
            sp.pts.push([x, y]);
        }
    }
}

/// SVG 圆弧（端点式参数）→ 折线。实现自 SVG 规范的附录 F.6。
fn sample_arc(
    out: &mut Vec<SubPath>,
    p0: [f32; 2],
    p1: [f32; 2],
    rx: f32,
    ry: f32,
    rot_deg: f32,
    large: bool,
    sweep: bool,
) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 0.01 || ry < 0.01 {
        if let Some(sp) = out.last_mut() {
            sp.pts.push(p1);
        }
        return;
    }
    let phi = rot_deg.to_radians();
    let (cos, sin) = (phi.cos(), phi.sin());
    let dx = (p0[0] - p1[0]) / 2.0;
    let dy = (p0[1] - p1[1]) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let sign = if large != sweep { 1.0 } else { -1.0 };
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let coef = if den == 0.0 {
        0.0
    } else {
        sign * (num / den).sqrt()
    };
    let cx1 = coef * (rx * y1 / ry);
    let cy1 = coef * (-ry * x1 / rx);
    let cx = cos * cx1 - sin * cy1 + (p0[0] + p1[0]) / 2.0;
    let cy = sin * cx1 + cos * cy1 + (p0[1] + p1[1]) / 2.0;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| {
        let dot = ux * vx + uy * vy;
        let len = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
        let mut a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            a = -a;
        }
        a
    };
    let ux = (x1 - cx1) / rx;
    let uy = (y1 - cy1) / ry;
    let vx = (-x1 - cx1) / rx;
    let vy = (-y1 - cy1) / ry;
    let theta1 = ang(1.0, 0.0, ux, uy);
    let mut dtheta = ang(ux, uy, vx, vy);
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f32::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f32::consts::TAU;
    }
    let n = ((dtheta.abs() / (std::f32::consts::TAU / 48.0)).ceil() as usize).max(6);
    if let Some(sp) = out.last_mut() {
        for k in 1..=n {
            let t = theta1 + dtheta * k as f32 / n as f32;
            let x = cx + rx * t.cos() * cos - ry * t.sin() * sin;
            let y = cy + rx * t.cos() * sin + ry * t.sin() * cos;
            sp.pts.push([x, y]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_parses_into_paths() {
        for (name, src) in svg_sources() {
            let paths = parse(src);
            assert!(!paths.is_empty(), "{name} 没解析出任何路径");
            for sp in &paths {
                assert!(
                    sp.pts.len() >= 2 || sp.fill,
                    "{name} 有只含 {} 个点的子路径",
                    sp.pts.len()
                );
                for p in &sp.pts {
                    assert!(
                        p[0].is_finite() && p[1].is_finite(),
                        "{name} 出现非有限坐标"
                    );
                    assert!(
                        p[0] > -4.0 && p[0] < 28.0 && p[1] > -4.0 && p[1] < 28.0,
                        "{name} 坐标越界 {:?}",
                        p
                    );
                }
            }
            println!(
                "{name}: {} 子路径, 点数 {:?}",
                paths.len(),
                paths.iter().map(|s| s.pts.len()).collect::<Vec<_>>()
            );
        }
    }
}
