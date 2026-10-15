//! Bounded pixel-spaced patterns for retained lines (native Series::Line ignores dashes).
use crate::{PlotPoint, Ranges, line_draw::LinePattern, time_series::SeriesData};
use kurbo::Point;

/// Reduced/step-expanded data in; NaN-separated dash segments out.
/// Overscan preserves nearby pans. Extremely dense traces lengthen pattern spacing
/// to limit dash count rather than dropping tail geometry or allocating unbounded buffers.
pub fn prepare(
    data: &SeriesData,
    ranges: Ranges,
    pixels: (f64, f64),
    pattern: LinePattern,
    width: f64,
) -> SeriesData {
    if pattern == LinePattern::Solid {
        return data.clone();
    }
    if pattern == LinePattern::None {
        return SeriesData::new(vec![]).unwrap();
    }
    let (w, h) = pixels;
    if !(w > 0.0 && h > 0.0) {
        return SeriesData::new(vec![]).unwrap();
    }
    let map = |p: PlotPoint| {
        Point::new(
            (p.x - ranges.x.min) / ranges.x.span() * w,
            (p.y - ranges.y.min) / ranges.y.span() * h,
        )
    };
    let unmap = |p: Point| PlotPoint {
        x: ranges.x.min + p.x / w * ranges.x.span(),
        y: ranges.y.min + p.y / h * ranges.y.span(),
    };
    let mut segments = Vec::new();
    let mut previous = None;
    let mut total = 0.0;
    for pair in data.samples().windows(2) {
        let (a, b) = (map(pair[0]), map(pair[1]));
        let Some((a, b)) = clip(a, b, (-w, 2.0 * w, -h, 2.0 * h)) else {
            previous = None;
            continue;
        };
        let length = a.distance(b);
        if length == 0.0 {
            continue;
        }
        let connected = previous == Some(a);
        segments.push((a, b, length, connected));
        previous = Some(b);
        total += length;
    }
    let lengths: &[f64] = match pattern {
        LinePattern::Dashed => &[6.0, 3.0],
        LinePattern::Dotted => &[0.05, 3.0],
        LinePattern::DashDot => &[6.0, 3.0, 0.05, 3.0],
        _ => unreachable!(),
    };
    let unit = width.clamp(0.5, 20.0);
    let period = lengths.iter().sum::<f64>() * unit;
    let scale = (total / (period * 16_384.0)).max(1.0) * unit;
    let mut index = 0;
    let mut left = lengths[0] * scale;
    let mut open = false;
    let mut out = Vec::new();
    for (a, b, length, connected) in segments {
        if !connected {
            if open {
                out.push(PlotPoint {
                    x: unmap(a).x,
                    y: f64::NAN,
                });
            }
            open = false;
            index = 0;
            left = lengths[0] * scale;
        }
        let mut at = 0.0;
        while at < length {
            let step = left.min(length - at);
            let end = (at + step).min(length);
            // Guard tiny residuals at long segments from preventing progress.
            if end <= at {
                break;
            }
            if index % 2 == 0 {
                if !open {
                    out.push(unmap(a.lerp(b, at / length)));
                }
                out.push(unmap(a.lerp(b, end / length)));
                open = true;
            }
            at = end;
            left -= step;
            if left <= 1e-9 * scale {
                if open {
                    out.push(PlotPoint {
                        x: unmap(a.lerp(b, at / length)).x,
                        y: f64::NAN,
                    });
                    open = false;
                }
                index = (index + 1) % lengths.len();
                left = lengths[index] * scale;
            }
        }
    }
    SeriesData::new(out).expect("patterns retain ordered timestamps")
}

fn clip(
    a: Point,
    b: Point,
    (xmin, xmax, ymin, ymax): (f64, f64, f64, f64),
) -> Option<(Point, Point)> {
    if !a.is_finite() || !b.is_finite() {
        return None;
    }
    let delta = b - a;
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for (origin, d, min, max) in [(a.x, delta.x, xmin, xmax), (a.y, delta.y, ymin, ymax)] {
        if d == 0.0 {
            if origin < min || origin > max {
                return None;
            }
        } else {
            let (t0, t1) = ((min - origin) / d, (max - origin) / d);
            low = low.max(t0.min(t1));
            high = high.min(t0.max(t1));
            if low > high {
                return None;
            }
        }
    }
    Some((a.lerp(b, low), a.lerp(b, high)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patterns_have_pixel_spacing_and_none_has_no_line() {
        let data = SeriesData::new(vec![
            PlotPoint { x: 0.0, y: 1.0 },
            PlotPoint { x: 100.0, y: 1.0 },
        ])
        .unwrap();
        let ranges = Ranges::new((0.0, 100.0), (0.0, 2.0));
        for pattern in [
            LinePattern::Dashed,
            LinePattern::Dotted,
            LinePattern::DashDot,
        ] {
            let prepared = prepare(&data, ranges, (100.0, 100.0), pattern, 1.0);
            assert!(prepared.samples().iter().any(|p| p.y.is_nan()));
            assert!(
                prepared
                    .samples()
                    .iter()
                    .filter(|p| p.y.is_finite())
                    .all(|p| p.y == 1.0)
            );
            let expected = if pattern == LinePattern::Dotted {
                0.05
            } else {
                6.0
            };
            assert!((prepared.samples()[1].x - expected).abs() < 1e-9);
            let zoomed = prepare(
                &data,
                Ranges::new((0.0, 50.0), (0.0, 2.0)),
                (100.0, 100.0),
                pattern,
                1.0,
            );
            assert!((zoomed.samples()[1].x - expected / 2.0).abs() < 1e-9);
        }
        assert!(
            prepare(&data, ranges, (100.0, 100.0), LinePattern::None, 1.0)
                .samples()
                .is_empty()
        );
    }
    #[test]
    fn dense_steps_remain_bounded_and_keep_gaps() {
        let data = SeriesData::new(
            (0..20_000)
                .map(|x| PlotPoint {
                    x: x as f64,
                    y: if x % 99 == 0 {
                        f64::NAN
                    } else {
                        (x % 2) as f64
                    },
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let steps = crate::line_draw::prepare(&data, crate::line_draw::Connection::Post);
        let patterned = prepare(
            &steps,
            Ranges::new((0.0, 20_000.0), (0.0, 1.0)),
            (1000.0, 1000.0),
            LinePattern::Dotted,
            1.0,
        );
        assert!(patterned.samples().len() < 250_000);
        assert!(patterned.samples().iter().any(|p| p.y.is_nan()));
        for pair in patterned
            .samples()
            .windows(2)
            .filter(|p| p[0].y.is_finite() && p[1].y.is_finite())
        {
            assert!(pair[0].x == pair[1].x || (pair[0].y - pair[1].y).abs() < 1e-9);
        }
    }
}
