//! An independent, deadline-aware decision engine for video segment source
//! selection. This never merges compressed bytes from unrelated encodings.
//! Consumers MUST reverify decoded scene alignment and torrent pieces before
//! publishing the chosen segment. A decision is not a proof of ready media.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub source_id: u64,
    /// True only for a source fully aligned over the requested canonical interval.
    pub aligned_for_interval: bool,
    /// True when the codec/color/format can be normalized into the output profile.
    pub convertible: bool,
    pub estimated_download_ms: u64,
    pub estimated_transform_ms: u64,
}

impl Candidate {
    fn ready_in_ms(&self) -> Option<u64> {
        if !self.aligned_for_interval || !self.convertible {
            return None;
        }
        self.estimated_download_ms
            .checked_add(self.estimated_transform_ms)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SegmentDecision {
    Select {
        source_id: u64,
        deadline_margin_ms: u64,
        switch: bool,
        reason: SelectReason,
    },
    Buffer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectReason {
    Initial,
    StayForHysteresis,
    StayNoBetterSource,
    FasterSource,
    CurrentSourceUnavailable,
}

#[derive(Clone, Copy, Debug)]
pub struct SelectionPolicy {
    /// Avoid repeatedly toggling between equal-speed swarms.
    pub minimum_holding_ms: u64,
    /// Require this much ETA improvement before switching a viable source.
    pub minimum_gain_ms: u64,
}

impl Default for SelectionPolicy {
    fn default() -> Self {
        Self {
            minimum_holding_ms: 8_000,
            minimum_gain_ms: 600,
        }
    }
}

pub struct SegmentPlanner {
    policy: SelectionPolicy,
    current_source: Option<u64>,
    last_switch_ms: Option<u64>,
}

impl SegmentPlanner {
    pub fn new(policy: SelectionPolicy) -> Self {
        Self { policy, current_source: None, last_switch_ms: None }
    }

    pub fn active_source(&self) -> Option<u64> {
        self.current_source
    }

    /// All timestamps are measured against a single monotonic clock. This
    /// chooses a feasible candidate, not a decoded or validated output.
    pub fn choose(
        &mut self,
        now_ms: u64,
        deadline_ms: u64,
        candidates: &[Candidate],
    ) -> SegmentDecision {
        let remaining = deadline_ms.saturating_sub(now_ms);
        let mut feasible: Vec<_> = candidates
            .iter()
            .filter_map(|candidate| {
                candidate
                    .ready_in_ms()
                    .filter(|eta| *eta <= remaining)
                    .map(|eta| (candidate.source_id, eta))
            })
            .collect();
        feasible.sort_by_key(|(source_id, eta)| (*eta, *source_id));
        let Some(&(fast_id, fast_eta)) = feasible.first() else {
            return SegmentDecision::Buffer;
        };
        let current_eta = self.current_source.and_then(|id| {
            feasible.iter().find(|(source_id, _)| *source_id == id).map(|(_, eta)| *eta)
        });
        let (selected, eta, reason) = match (self.current_source, current_eta) {
            (None, _) => (fast_id, fast_eta, SelectReason::Initial),
            (Some(_), None) => (fast_id, fast_eta, SelectReason::CurrentSourceUnavailable),
            (Some(current), Some(active_eta)) => {
                let holding = self.last_switch_ms
                    .is_some_and(|last| now_ms.saturating_sub(last) < self.policy.minimum_holding_ms);
                if holding {
                    (current, active_eta, SelectReason::StayForHysteresis)
                } else if fast_id != current
                    && active_eta.saturating_sub(fast_eta) >= self.policy.minimum_gain_ms
                    && fast_eta < active_eta
                {
                    (fast_id, fast_eta, SelectReason::FasterSource)
                } else {
                    (current, active_eta, SelectReason::StayNoBetterSource)
                }
            }
        };
        let switching = self.current_source.is_some_and(|current| current != selected);
        if self.current_source != Some(selected) {
            self.current_source = Some(selected);
            self.last_switch_ms = Some(now_ms);
        }
        SegmentDecision::Select {
            source_id: selected,
            deadline_margin_ms: remaining - eta,
            switch: switching,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: u64, download_ms: u64, transform_ms: u64) -> Candidate {
        Candidate {
            source_id: id,
            aligned_for_interval: true,
            convertible: true,
            estimated_download_ms: download_ms,
            estimated_transform_ms: transform_ms,
        }
    }

    #[test]
    fn joins_distinct_encodings_at_a_timeline_segment_decision_only() {
        let mut planner = SegmentPlanner::new(SelectionPolicy::default());
        let decision = planner.choose(0, 5_000, &[source(1, 3000, 200), source(2, 500, 200)]);
        assert_eq!(decision, SegmentDecision::Select {
            source_id: 2,
            deadline_margin_ms: 4300,
            switch: false,
            reason: SelectReason::Initial,
        });
        assert_eq!(planner.active_source(), Some(2));
    }

    #[test]
    fn rejects_unaligned_and_late_sources() {
        let mut unaligned = source(1, 100, 100);
        unaligned.aligned_for_interval = false;
        let result = SegmentPlanner::new(SelectionPolicy::default())
            .choose(1_000, 2_000, &[unaligned, source(2, 2_000, 100)]);
        assert_eq!(result, SegmentDecision::Buffer);
    }

    #[test]
    fn holds_then_switches_to_strongly_better_candidate() {
        let mut planner = SegmentPlanner::new(SelectionPolicy::default());
        planner.choose(0, 5_000, &[source(1, 100, 100)]);
        let early = planner.choose(3_000, 8_000, &[source(1, 3_000, 300), source(2, 300, 300)]);
        assert_eq!(early, SegmentDecision::Select {
            source_id: 1, deadline_margin_ms: 1700,
            switch: false, reason: SelectReason::StayForHysteresis,
        });
        let later = planner.choose(9_000, 14_000, &[source(1, 3_000, 300), source(2, 300, 300)]);
        assert!(matches!(later, SegmentDecision::Select {
            source_id: 2, switch: true, reason: SelectReason::FasterSource, ..
        }));
    }

    #[test]
    fn fails_over_immediately_when_current_source_becomes_late() {
        let mut planner = SegmentPlanner::new(SelectionPolicy::default());
        planner.choose(0, 5_000, &[source(1, 100, 100)]);
        let decision = planner.choose(300, 3_000, &[source(1, 4_000, 100), source(2, 900, 200)]);
        assert!(matches!(decision, SegmentDecision::Select {
            source_id: 2, switch: true, reason: SelectReason::CurrentSourceUnavailable, ..
        }));
    }

    #[test]
    fn overflow_and_not_convertible_never_receive_deadlines() {
        let mut a = source(1, u64::MAX, 500);
        a.convertible = false;
        assert_eq!(
            SegmentPlanner::new(SelectionPolicy::default()).choose(100, 1_000, &[a]),
            SegmentDecision::Buffer
        );
    }
}
