//! Conservative affine timeline alignment from independently verified
//! video/audio match anchors. Actual perceptual fingerprint extraction is
//! separate; these fit checks must not be replaced with duration matching.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub source_ms: f64,
    pub reference_ms: f64,
    /// Similarity score from an independently verified fingerprint matcher.
    pub confidence: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alignment {
    pub slope: f64,
    pub offset_ms: f64,
    pub source_start_ms: f64,
    pub source_end_ms: f64,
    pub max_residual_ms: f64,
    pub evidence_count: usize,
}

impl Alignment {
    pub fn reference_time(&self, source_ms: f64) -> Option<f64> {
        if !source_ms.is_finite()
            || source_ms < self.source_start_ms
            || source_ms > self.source_end_ms
        {
            return None;
        }
        Some(self.slope * source_ms + self.offset_ms)
    }

    pub fn covers(&self, source_start: f64, source_end: f64) -> bool {
        source_start < source_end
            && source_start >= self.source_start_ms
            && source_end <= self.source_end_ms
    }
}

/// Fits an affine map ONLY when all high-confidence independent anchor matches
/// agree. Deliberately rejects cuts, missing sections and ambiguous repetitions.
/// A later piecewise alignment algorithm can split at real edit boundaries.
pub fn align(anchors: &[Anchor], max_residual_ms: f64) -> Result<Alignment, &'static str> {
    if anchors.len() < 3 || max_residual_ms <= 0.0 || !max_residual_ms.is_finite() {
        return Err("insufficient evidence or invalid tolerance");
    }
    let mut samples = anchors.to_vec();
    if samples.iter().any(|p| {
        !p.source_ms.is_finite()
            || !p.reference_ms.is_finite()
            || !p.confidence.is_finite()
            || !(0.9..=1.0).contains(&p.confidence)
    }) {
        return Err("low confidence or invalid fingerprint evidence");
    }
    samples.sort_by(|a, b| a.source_ms.total_cmp(&b.source_ms));

    if samples.windows(2).any(|pair| {
        pair[1].source_ms - pair[0].source_ms < 1000.0
            || pair[1].reference_ms <= pair[0].reference_ms
    }) {
        return Err("non-monotonic or insufficiently separated anchors");
    }

    // Theil-Sen slope uses pairwise median instead of fitting to one noisy edge.
    let mut slopes = Vec::new();
    for i in 0..samples.len() {
        for j in i + 1..samples.len() {
            let delta_src = samples[j].source_ms - samples[i].source_ms;
            slopes.push((samples[j].reference_ms - samples[i].reference_ms) / delta_src);
        }
    }
    let slope = median(&mut slopes);
    if !(0.95..=1.05).contains(&slope) {
        return Err("unsupported timebase drift");
    }

    let mut intercepts: Vec<f64> = samples
        .iter()
        .map(|p| p.reference_ms - slope * p.source_ms)
        .collect();
    let offset_ms = median(&mut intercepts);
    let worst_error = samples
        .iter()
        .map(|p| (p.reference_ms - (slope * p.source_ms + offset_ms)).abs())
        .fold(0.0, f64::max);
    if worst_error > max_residual_ms {
        return Err("timeline mismatch or edit cut detected");
    }
    Ok(Alignment {
        slope,
        offset_ms,
        source_start_ms: samples[0].source_ms,
        source_end_ms: samples.last().expect("nonempty samples").source_ms,
        max_residual_ms: worst_error,
        evidence_count: samples.len(),
    })
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(source_ms: f64, reference_ms: f64) -> Anchor {
        Anchor { source_ms, reference_ms, confidence: 0.99 }
    }

    #[test]
    fn aligns_offset_and_rate_drift_only_inside_verified_coverage() {
        let anchors = vec![a(0.0, 350.0), a(10000.0, 10360.0), a(20000.0, 20370.0)];
        let map = align(&anchors, 20.0).unwrap();
        assert!((map.slope - 1.001).abs() < 0.0001);
        assert_eq!(map.reference_time(0.0), Some(350.0));
        assert_eq!(map.reference_time(25_000.0), None);
        assert!(!map.covers(15_000.0, 25_000.0));
    }

    #[test]
    fn refuses_edit_cut_and_too_few_samples() {
        let cut = vec![a(0.0, 0.0), a(10000.0, 10000.0), a(20000.0, 25000.0)];
        assert!(align(&cut, 40.0).is_err());
        assert!(align(&cut[..2], 40.0).is_err());
    }

    #[test]
    fn refuses_repeated_or_low_confidence_evidence() {
        let repeated = vec![a(0.0, 0.0), a(10000.0, 10000.0), a(20000.0, 9000.0)];
        assert!(align(&repeated, 50.0).is_err());
        let mut evidence = vec![a(0.0, 0.0), a(10000.0, 10000.0), a(20000.0, 20000.0)];
        evidence[1].confidence = 0.5;
        assert!(align(&evidence, 50.0).is_err());
    }

    #[test]
    fn rejects_unbounded_or_nonfinite_time() {
        let evidence = vec![a(0.0, 0.0), a(10000.0, 10000.0), a(20000.0, 20000.0)];
        let map = align(&evidence, 2.0).unwrap();
        assert!(map.reference_time(f64::NAN).is_none());
        assert!(align(&evidence, f64::INFINITY).is_err());
    }
}
