//! Two recent detail levels; prepared vertices stay bounded by viewport reduction budgets.
use crate::{
    line_draw::{Connection, LinePattern},
    time_series::SeriesData,
};
use std::collections::VecDeque;
use zgui::canvas::Lod;
pub(super) type Prepared = Vec<(String, SeriesData, SeriesData, Connection)>;
type DrawKey = (Connection, LinePattern, u64);
pub(super) type Plan = (
    Vec<(String, usize, DrawKey)>,
    u64,
    u64,
    usize,
    Lod,
    (u64, u64, usize),
);
pub(super) fn compatible(completed: &Plan, latest: &Plan) -> bool {
    completed.0 == latest.0
}

pub(super) fn reusable(completed: &Plan, latest: &Plan) -> bool {
    if completed == latest {
        return true;
    }
    if completed.0 != latest.0 || completed.4 != latest.4 || completed.5 != latest.5 {
        return false;
    }
    // Pixel-spaced patterns must be rebuilt when either axis or plot size changes.
    if completed
        .0
        .iter()
        .any(|(_, _, (_, pattern, _))| !matches!(pattern, LinePattern::Solid | LinePattern::None))
    {
        return false;
    }
    let old = crate::Range::new(f64::from_bits(completed.1), f64::from_bits(completed.2));
    let next = crate::Range::new(f64::from_bits(latest.1), f64::from_bits(latest.2));
    let coverage = crate::series_samples::pan_range(old);
    let density = old.span() / effective_columns(completed.3);
    let requested = next.span() / effective_columns(latest.3);
    next.min >= coverage.min && next.max <= coverage.max && requested >= density * (1.0 - 1e-12)
}

fn effective_columns(columns: usize) -> f64 {
    // pan_viewport prepares three view widths, subject to viewport's 8192-column cap.
    columns.clamp(1, 8192 / 3) as f64
}
pub(super) fn detail_plan(request: &Plan) -> Plan {
    let mut detail = request.clone();
    if request
        .0
        .iter()
        .all(|(_, _, (_, pattern, _))| matches!(pattern, LinePattern::Solid | LinePattern::None))
    {
        // Prepare twice the requested density so small zoom-ins retain full requested detail.
        detail.3 = detail.3.saturating_mul(2).min(8192);
    }
    detail
}
#[derive(Default)]
pub(super) struct Viewports(VecDeque<(Plan, Prepared)>);
impl Viewports {
    pub(super) fn find(&self, request: &Plan) -> Option<(Plan, Prepared)> {
        self.0
            .iter()
            .find(|(plan, _)| reusable(plan, request))
            .cloned()
    }
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
    pub(super) fn remember(&mut self, plan: Plan, data: Prepared) {
        self.0.retain(|(old, _)| old.0 == plan.0 && old != &plan);
        self.0.push_front((plan, data));
        self.0.truncate(2);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Plan {
        (
            vec![(
                "signal".into(),
                123,
                (Connection::Linear, LinePattern::Solid, 0),
            )],
            100.0_f64.to_bits(),
            200.0_f64.to_bits(),
            800,
            Lod::Columns,
            (0, 0, 0),
        )
    }
    #[test]
    fn small_zoom_sequence_rebuilds_only_when_cached_density_is_exhausted() {
        let mut request = request();
        let mut cached = detail_plan(&request);
        let mut preparations = 1;
        for step in 1..=120 {
            let span = 100.0 * 0.98_f64.powi(step);
            request.1 = (150.0 - span / 2.0).to_bits();
            request.2 = (150.0 + span / 2.0).to_bits();
            if !reusable(&cached, &request) {
                cached = detail_plan(&request);
                preparations += 1;
            }
        }
        eprintln!("120 small zoom steps: {preparations} preparations including initial view");
        assert_eq!(preparations, 4);
    }
    #[test]
    fn detail_density_supports_small_zooms_but_refreshes_before_detail_is_lost() {
        let request = request();
        let detail = detail_plan(&request);
        let mut zoom = request.clone();
        zoom.1 = 125.0_f64.to_bits();
        zoom.2 = 175.0_f64.to_bits();
        assert!(reusable(&detail, &zoom));
        zoom.2 = 150.0_f64.to_bits();
        assert!(!reusable(&detail, &zoom));
        let mut wide = request.clone();
        wide.3 = 6000;
        let wide_detail = detail_plan(&wide);
        assert!(reusable(&wide_detail, &wide));
        wide.2 = 175.0_f64.to_bits();
        assert!(
            !reusable(&wide_detail, &wide),
            "Column cap cannot promise detail that was never prepared"
        );
        let mut patterned = request;
        patterned.0[0].2.1 = LinePattern::Dashed;
        assert_eq!(detail_plan(&patterned), patterned);
    }
    #[test]
    fn viewport_history_is_bounded_and_drops_replaced_sources() {
        let mut cache = Viewports::default();
        for index in 0..3 {
            let mut plan = request();
            plan.1 = (100.0 + index as f64).to_bits();
            cache.remember(plan, vec![]);
        }
        assert_eq!(cache.0.len(), 2);
        assert!(cache.find(&request()).is_some());
        let mut changed = request();
        changed.0[0].1 = 456;
        cache.remember(changed, vec![]);
        assert_eq!(cache.0.len(), 1);
        assert!(cache.find(&request()).is_none());
        cache.clear();
        assert!(cache.0.is_empty());
    }
}
