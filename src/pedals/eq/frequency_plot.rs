//! Helpers for displaying the graphic EQ's live frequency plot.
//!
//! The processor sends the spectrum as dBFS magnitudes (see
//! [`FrequencyAnalyser`](crate::dsp_algorithms::frequency_analysis::FrequencyAnalyser)), so
//! the display only has to map that window onto the plot's axis and smooth it over time.

use eframe::egui::Color32;
use egui_plot::PlotPoint;

use crate::dsp_algorithms::frequency_analysis::SPECTRUM_FLOOR_DB;

/// The colour of the live frequency plot, used for its curve and its translucent fill and
/// for the tint of its toggle button.
pub const LIVE_FREQUENCY_COLOR: Color32 = Color32::from_rgb(220, 100, 100);

/// How quickly the displayed curve rises towards a louder payload, in milliseconds.
const ATTACK_MS: f64 = 50.0;

/// How quickly it falls towards a quieter one, in milliseconds. Much slower than the
/// attack, so quiet bands settle down instead of flickering with every frame.
const RELEASE_MS: f64 = 350.0;

/// Map a dBFS magnitude onto the plot's y axis, stretching the whole
/// `SPECTRUM_FLOOR_DB..0 dB` window over the full height of the graph: full scale reaches
/// `full_scale_y` and the floor reaches its negation, so silence sits flat on the bottom
/// edge instead of somewhere inside the plot. Values outside the window are clamped and
/// NaN falls to the floor, so no payload can place a point outside the graph.
pub fn live_plot_y(db: f64, full_scale_y: f64) -> f64 {
    if db.is_nan() {
        return -full_scale_y;
    }

    let fraction = ((db - SPECTRUM_FLOOR_DB) / -SPECTRUM_FLOOR_DB).clamp(0.0, 1.0);

    full_scale_y * (fraction * 2.0 - 1.0)
}

/// Convert a deserialized payload of dBFS magnitudes into plot points for the graph.
pub fn payload_to_plot_points(plot_points: &mut [PlotPoint], full_scale_y: f64) {
    for point in plot_points.iter_mut() {
        point.y = live_plot_y(point.y, full_scale_y);
    }
}

/// Move the displayed curve towards the newest payload, using the fast attack when a band
/// gets louder and the slow release when it gets quieter, and report whether the payload
/// could be smoothed into.
///
/// A payload holding a non-finite point is dropped completely, leaving the curve that is
/// already on screen alone rather than blending a NaN into it. One with a different number
/// of points can't be blended at all, so it replaces the curve instead of leaving the
/// display stuck on the old shape.
pub fn update_live_frequency_plot(
    displayed: &mut Vec<PlotPoint>,
    target: &[PlotPoint],
    elapsed_ms: f64,
) -> bool {
    if target.is_empty()
        || !target
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite())
    {
        return false;
    }

    if target.len() != displayed.len() {
        displayed.clear();
        displayed.extend_from_slice(target);

        return false;
    }

    let elapsed_ms = elapsed_ms.max(0.0);
    let attack = (elapsed_ms / ATTACK_MS).min(1.0);
    let release = (elapsed_ms / RELEASE_MS).min(1.0);

    for (displayed, target) in displayed.iter_mut().zip(target.iter()) {
        let factor = if target.y > displayed.y {
            attack
        } else {
            release
        };

        displayed.y += (target.y - displayed.y) * factor;
    }

    true
}

/// The y the live frequency plot's fill should reach down to, for the graph it is drawn in.
///
/// The graph centres its y axis on zero, so it is `full_scale_y` tall on each side of it only
/// while every curve fits: the EQ's response is the sum of its bands, so a few boosted ones
/// reach past `full_scale_y` and the graph grows to match. Filling down to `-full_scale_y`
/// would then leave the shading hanging above the bottom edge, so the fill follows the graph
/// itself, which is its tallest curve mirrored below zero. Non-finite levels are ignored, so
/// the fill can only be moved by levels that can actually be drawn.
pub fn live_plot_fill_bottom(
    response_plot: &[PlotPoint],
    live_plot: &[PlotPoint],
    full_scale_y: f64,
) -> f64 {
    let tallest = response_plot
        .iter()
        .chain(live_plot)
        .map(|point| point.y.abs())
        .filter(|y| y.is_finite())
        .fold(full_scale_y, f64::max);

    -tallest
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_SCALE: f64 = 15.0;

    fn points(values: &[f64]) -> Vec<PlotPoint> {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| PlotPoint::new(index as f64, *value))
            .collect()
    }

    fn all_finite(plot_points: &[PlotPoint]) -> bool {
        plot_points
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite())
    }

    #[test]
    fn a_broken_payload_never_reaches_the_plot() {
        // Non-finite magnitudes are mapped onto the edges of the graph ...
        let mut payload = points(&[f64::NAN, f64::INFINITY, f64::NEG_INFINITY]);
        payload_to_plot_points(&mut payload, FULL_SCALE);

        assert!(all_finite(&payload));
        assert_eq!(payload[0].y, -FULL_SCALE);
        assert_eq!(payload[1].y, FULL_SCALE);
        assert_eq!(payload[2].y, -FULL_SCALE);

        // ... and a payload holding one is dropped completely, so the curve on screen is
        // left alone rather than being smoothed towards, or replaced by, a NaN
        let mut displayed = points(&[1.0, 2.0]);

        assert!(!update_live_frequency_plot(
            &mut displayed,
            &points(&[f64::NAN, 3.0]),
            100.0
        ));
        assert_eq!(displayed, points(&[1.0, 2.0]));
        assert!(all_finite(&displayed));

        // An empty payload is ignored as well, while one with a different number of points
        // replaces the curve instead of leaving it behind a rebuilt analyser
        assert!(!update_live_frequency_plot(&mut displayed, &[], 100.0));
        assert_eq!(displayed, points(&[1.0, 2.0]));

        assert!(!update_live_frequency_plot(
            &mut displayed,
            &points(&[4.0, 5.0, 6.0]),
            100.0
        ));
        assert_eq!(displayed, points(&[4.0, 5.0, 6.0]));
    }

    #[test]
    fn the_db_window_fills_the_whole_graph_height() {
        // Full scale and the floor reach the very edges of the graph, with the middle of
        // the window sitting on the unity line
        assert_eq!(live_plot_y(0.0, FULL_SCALE), FULL_SCALE);
        assert_eq!(live_plot_y(SPECTRUM_FLOOR_DB, FULL_SCALE), -FULL_SCALE);
        assert_eq!(live_plot_y(SPECTRUM_FLOOR_DB / 2.0, FULL_SCALE), 0.0);

        // The mapping is monotonic, so bands keep their order on screen
        let window = [SPECTRUM_FLOOR_DB, -48.0, -36.0, -24.0, -12.0, 0.0];

        assert!(window
            .windows(2)
            .all(|pair| live_plot_y(pair[0], FULL_SCALE) < live_plot_y(pair[1], FULL_SCALE)));

        // Levels outside the window are clamped to an edge, so a loud payload can't push
        // the curve off the top of the graph and a silent one can't fall out of the bottom
        assert_eq!(live_plot_y(12.0, FULL_SCALE), FULL_SCALE);
        assert_eq!(live_plot_y(-120.0, FULL_SCALE), -FULL_SCALE);
    }

    #[test]
    fn the_attack_is_faster_than_the_release() {
        let mut displayed = points(&[-FULL_SCALE]);

        // A step up covers most of the distance within a single 33 ms frame ...
        assert!(update_live_frequency_plot(
            &mut displayed,
            &points(&[FULL_SCALE]),
            33.0
        ));
        let risen = displayed[0].y;
        assert!(risen > 0.0 && risen < FULL_SCALE);

        // ... while the same step down only crawls back, keeping the peak on screen long
        // enough to see
        assert!(update_live_frequency_plot(
            &mut displayed,
            &points(&[-FULL_SCALE]),
            33.0
        ));
        let fallen = displayed[0].y;
        assert!(fallen < risen);
        assert!(risen - fallen < risen + FULL_SCALE);

        // It never overshoots the payload, and two frames arriving at the same instant are
        // a no-op rather than a jump
        assert!(update_live_frequency_plot(
            &mut displayed,
            &points(&[0.0]),
            0.0
        ));
        assert_eq!(displayed[0].y, fallen);
        assert!(all_finite(&displayed));
    }

    #[test]
    fn the_fill_reaches_the_bottom_of_the_graph() {
        // A response that fits inside the mapped window leaves the graph exactly that tall,
        // so the fill reaches the floor the window maps onto the bottom edge
        assert_eq!(
            live_plot_fill_bottom(
                &points(&[0.0, -3.0, 0.5]),
                &points(&[-FULL_SCALE; 4]),
                FULL_SCALE
            ),
            -FULL_SCALE
        );

        // One reaching past it grows the graph and takes its bottom edge with it, so the
        // fill still touches the bottom of the graph instead of stopping at the mapped floor
        assert_eq!(
            live_plot_fill_bottom(&points(&[21.5, -3.0]), &points(&[0.0]), FULL_SCALE),
            -21.5
        );

        // A response that can't be drawn at all leaves the graph to the window, as does a
        // live curve that never rises off the floor
        assert_eq!(
            live_plot_fill_bottom(&points(&[f64::NAN, f64::NEG_INFINITY]), &[], FULL_SCALE),
            -FULL_SCALE
        );
        assert_eq!(
            live_plot_fill_bottom(&[], &points(&[-FULL_SCALE; 8]), FULL_SCALE),
            -FULL_SCALE
        );
    }
}
