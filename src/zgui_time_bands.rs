//! Retained full-height time bands for a plot background.

use crate::Range;
use std::sync::Arc;
use zgui::{
    canvas::{Brush, Series, zgui_color::Color},
    elements::canvas::CanvasHandle,
};

/// Disjoint intervals sharing one paint, in chronological order.
#[derive(Clone, PartialEq)]
pub struct TimeBandBatch {
    /// Finite start/end times with `start < end`, sorted without overlap.
    pub intervals: Arc<[[f64; 2]]>,
    /// RGB fill and boundary colour.
    pub color: [u8; 3],
    /// Fill opacity in `0..=1`.
    pub opacity: f32,
    /// Draw a one-pixel boundary at each interval's actual start.
    pub boundaries: bool,
}

/// A background that retains interval buffers across pans and zooms.
///
/// Nearby views share padded interval buffers. Uniform visible states keep an
/// unchanged canvas, so panning within one state requests no background repaint.
/// Different batches paint in input order; intervals within a batch must not overlap.
#[derive(Default)]
pub struct RetainedTimeBands {
    handle: CanvasHandle,
    batches: Vec<TimeBandBatch>,
    prepared: Option<(Range, f64, f64)>,
    uniform: Option<Vec<Option<usize>>>,
}

impl RetainedTimeBands {
    /// The canvas to mount and register for export.
    pub fn handle(&self) -> &CanvasHandle {
        &self.handle
    }

    /// Replace source intervals or paints. Equal sources retain their buffers.
    pub fn replace(&mut self, batches: Vec<TimeBandBatch>) {
        if self.batches != batches {
            self.batches = batches;
            self.prepared = None;
            self.uniform = None;
        }
    }

    /// Show `range` across a canvas of `width` by `height` CSS pixels.
    ///
    /// Fill and boundary widths stay in pixels as the X view changes. Only data
    /// replacement, resize, or leaving prepared coverage rebuilds the buffers.
    pub fn show(&mut self, range: Range, width: f64, height: f64) {
        if ![range.min, range.max, width, height]
            .iter()
            .all(|v| v.is_finite())
            || range.span() <= 0.
            || width <= 0.
            || height <= 0.
        {
            return;
        }
        let uniform = self
            .batches
            .iter()
            .map(|batch| {
                let index = batch.intervals.partition_point(|v| v[1] <= range.min);
                match batch.intervals.get(index) {
                    None => Some(None),
                    Some(v) if v[0] >= range.max => Some(None),
                    Some(v) if v[0] < range.min && v[1] >= range.max => Some(Some(index)),
                    _ => None,
                }
            })
            .collect::<Option<Vec<_>>>();
        if let Some(signature) = uniform {
            if self.uniform.as_ref() == Some(&signature)
                && self
                    .prepared
                    .is_some_and(|(_, w, h)| w == width && h == height)
            {
                return;
            }
            self.handle.draw(|scene| {
                scene.clear();
                for (batch, visible) in self.batches.iter().zip(&signature) {
                    if visible.is_some() && batch.opacity > 0. {
                        line(
                            scene,
                            vec![[0., height as f32 / 2.], [width as f32, height as f32 / 2.]],
                            color(batch, batch.opacity),
                            height,
                        );
                    }
                }
            });
            self.handle.set_transform(kurbo::Affine::IDENTITY);
            self.uniform = Some(signature);
            self.prepared = Some((range, width, height));
            return;
        }
        let reuse = self.uniform.is_none()
            && self.prepared.is_some_and(|(coverage, w, h)| {
                w == width && h == height && coverage.min <= range.min && coverage.max >= range.max
            });
        if !reuse {
            let coverage = range.expand(1., 0.);
            self.handle.draw(|scene| {
                scene.clear();
                // Fills precede boundaries, matching the background layer's paint order.
                for batch in &self.batches {
                    if batch.opacity <= 0. {
                        continue;
                    }
                    let mut points = Vec::new();
                    for [start, end] in visible(batch, coverage) {
                        points.extend([
                            [
                                (start.max(coverage.min) - coverage.min) as f32,
                                height as f32 / 2.,
                            ],
                            [
                                (end.min(coverage.max) - coverage.min) as f32,
                                height as f32 / 2.,
                            ],
                            [f32::NAN, f32::NAN],
                        ]);
                    }
                    line(scene, points, color(batch, batch.opacity), height);
                }
                for batch in &self.batches {
                    if !batch.boundaries || batch.opacity <= 0. {
                        continue;
                    }
                    let mut points = Vec::new();
                    for [start, _] in visible(batch, coverage) {
                        if *start < coverage.min {
                            continue;
                        }
                        let x = (start - coverage.min) as f32;
                        points.extend([[x, 0.], [x, height as f32], [f32::NAN, f32::NAN]]);
                    }
                    line(scene, points, color(batch, 0.35), 1.);
                }
            });
            self.prepared = Some((coverage, width, height));
            self.uniform = None;
        }
        let origin = self.prepared.unwrap().0.min;
        let scale = width / range.span();
        self.handle.set_transform(kurbo::Affine::new([
            scale,
            0.,
            0.,
            1.,
            (origin - range.min) * scale,
            0.,
        ]));
    }
}

fn visible(batch: &TimeBandBatch, range: Range) -> impl Iterator<Item = &[f64; 2]> {
    let start = batch.intervals.partition_point(|v| v[1] <= range.min);
    batch.intervals[start..]
        .iter()
        .take_while(move |v| v[0] < range.max)
}

fn color(batch: &TimeBandBatch, alpha: f32) -> Color {
    let [r, g, b] = batch.color;
    Color::srgb_u8(r, g, b, 255).with_alpha(alpha)
}

fn line(scene: &mut zgui::canvas::CanvasScene, points: Vec<[f32; 2]>, color: Color, width: f64) {
    if points.is_empty() {
        return;
    }
    scene.push_series(Series::Line {
        data: points.into(),
        to_canvas: kurbo::Affine::IDENTITY,
        stroke: kurbo::Stroke::new(width).with_caps(kurbo::Cap::Butt),
        brush: Brush::Solid(color),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bands(intervals: &[[f64; 2]]) -> RetainedTimeBands {
        let mut out = RetainedTimeBands::default();
        out.replace(vec![TimeBandBatch {
            intervals: intervals.into(),
            color: [20, 60, 90],
            opacity: 0.2,
            boundaries: true,
        }]);
        out
    }
    #[test]
    fn pans_and_zooms_retain_buffers_and_pixel_widths() {
        let mut bands = bands(&[[0., 10.], [12., 20.]]);
        bands.show((5., 15.).into(), 800., 300.);
        let handle = bands.handle().scene().clone();
        let revision = handle.revision();
        let scene = zgui::canvas::resolve(handle.token()).unwrap();
        let data = match &scene.lock().unwrap().series()[0].series {
            Series::Line { data, .. } => data.clone(),
            _ => unreachable!(),
        };
        for range in [(6., 16.), (7., 12.), (3., 18.)] {
            bands.show(range.into(), 800., 300.);
        }
        assert_eq!(handle.revision(), revision);
        let held = scene.lock().unwrap();
        let Series::Line {
            data: after,
            stroke,
            ..
        } = &held.series()[0].series
        else {
            panic!()
        };
        assert!(Arc::ptr_eq(&data, after));
        assert_eq!(stroke.width, 300.);
        let Series::Line { stroke, .. } = &held.series()[1].series else {
            panic!()
        };
        assert_eq!(stroke.width, 1.);
        drop(held);
        bands.show((100., 110.).into(), 800., 300.);
        assert!(handle.revision() > revision);
    }
    #[test]
    fn unchanged_uniform_state_requests_no_background_repaint() {
        let mut bands = bands(&[[0., 100.]]);
        bands.show((10., 20.).into(), 800., 300.);
        let handle = bands.handle().scene().clone();
        let before = (handle.revision(), handle.view());
        for range in [(20., 30.), (40., 60.), (1., 99.)] {
            bands.show(range.into(), 800., 300.);
        }
        assert_eq!((handle.revision(), handle.view()), before);
        bands.show((95., 105.).into(), 800., 300.);
        assert!(handle.revision() > before.0);
    }
    #[test]
    #[cfg(feature = "raster")]
    fn exports_match_clipped_rectangles_through_pan_zoom_gaps_and_overlap() {
        use kurbo::Shape;
        use zgui::canvas::{CanvasScene, ShapeBuilder};
        let sources = vec![
            TimeBandBatch {
                intervals: vec![[0., 4.], [6., 10.]].into(),
                color: [255, 0, 0],
                opacity: 0.25,
                boundaries: false,
            },
            TimeBandBatch {
                intervals: vec![[2., 8.]].into(),
                color: [0, 0, 255],
                opacity: 0.5,
                boundaries: false,
            },
        ];
        let mut retained = RetainedTimeBands::default();
        retained.replace(sources.clone());
        for (range, width, height) in [
            ((0., 10.), 100., 40.),
            ((1., 11.), 100., 40.),
            ((3., 8.), 100., 40.),
            ((6.5, 7.5), 100., 40.),
            ((20., 30.), 100., 40.),
            ((0., 10.), 200., 80.),
        ] {
            let range: Range = range.into();
            retained.show(range, width, height);
            let description = crate::PlotDescription {
                width,
                height,
                background: "#ffffff".into(),
                rects: vec![],
                text: vec![],
            };
            let mut reference = CanvasScene::default();
            for batch in &sources {
                for [start, end] in batch.intervals.iter() {
                    let left = (start.max(range.min) - range.min) / range.span() * width;
                    let right = (end.min(range.max) - range.min) / range.span() * width;
                    if right > left {
                        reference.push(
                            ShapeBuilder::new(
                                kurbo::Rect::new(left, 0., right, height).to_path(0.1),
                            )
                            .fill(Brush::Solid(color(batch, batch.opacity)))
                            .build(),
                        );
                    }
                }
            }
            let canvas = zgui::canvas::resolve(retained.handle().scene().token()).unwrap();
            let held = canvas.lock().unwrap();
            let raster = |scene: &CanvasScene| {
                let svg = crate::zgui_svg::scenes_to_svg(&[scene], &description).unwrap();
                let tree =
                    resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
                let mut pixels =
                    resvg::tiny_skia::Pixmap::new(width as u32, height as u32).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixels.as_mut(),
                );
                pixels
            };
            let actual = raster(&held);
            let expected = raster(&reference);
            assert!(
                actual
                    .data()
                    .iter()
                    .zip(expected.data())
                    .all(|(a, b)| a.abs_diff(*b) <= 1),
                "range {range:?}, size {width}x{height}"
            );
        }
    }
}
