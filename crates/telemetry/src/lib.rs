//! Bounded, local-only playback telemetry. Cache reuse is not network goodput.

use serde::Serialize;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackEvent {
    StartupDelayMs(u64),
    StallMs(u64),
    SegmentReady,
    SegmentDeadlineMiss,
    SourceSwitch,
    VerifiedNetworkBytes(u64),
    ReusedCacheBytes(u64),
}

#[derive(Default, Clone, Debug, Serialize)]
pub struct MetricsSummary {
    pub startup_p95_ms: Option<u64>,
    pub startup_count: usize,
    pub stall_total_ms: u64,
    pub segment_ready: u64,
    pub segment_deadline_misses: u64,
    pub source_switches: u64,
    pub verified_network_bytes: u64,
    pub reused_cache_bytes: u64,
}

pub struct Metrics {
    startups: VecDeque<u64>,
    max_startups: usize,
    summary: MetricsSummary,
}

impl Metrics {
    pub fn new(max_startups: usize) -> Result<Self, &'static str> {
        if max_startups == 0 || max_startups > 10_000 {
            return Err("startup history size outside allowed bounds");
        }
        Ok(Self {
            startups: VecDeque::with_capacity(max_startups.min(100)),
            max_startups,
            summary: MetricsSummary::default(),
        })
    }

    pub fn record(&mut self, event: PlaybackEvent) {
        match event {
            PlaybackEvent::StartupDelayMs(duration) => {
                if self.startups.len() == self.max_startups {
                    self.startups.pop_front();
                }
                self.startups.push_back(duration);
            }
            PlaybackEvent::StallMs(duration) => {
                self.summary.stall_total_ms = self.summary.stall_total_ms.saturating_add(duration);
            }
            PlaybackEvent::SegmentReady => {
                self.summary.segment_ready = self.summary.segment_ready.saturating_add(1);
            }
            PlaybackEvent::SegmentDeadlineMiss => {
                self.summary.segment_deadline_misses =
                    self.summary.segment_deadline_misses.saturating_add(1);
            }
            PlaybackEvent::SourceSwitch => {
                self.summary.source_switches = self.summary.source_switches.saturating_add(1);
            }
            PlaybackEvent::VerifiedNetworkBytes(amount) => {
                self.summary.verified_network_bytes =
                    self.summary.verified_network_bytes.saturating_add(amount);
            }
            PlaybackEvent::ReusedCacheBytes(amount) => {
                self.summary.reused_cache_bytes =
                    self.summary.reused_cache_bytes.saturating_add(amount);
            }
        }
    }

    pub fn snapshot(&self) -> MetricsSummary {
        let mut snapshot = self.summary.clone();
        snapshot.startup_count = self.startups.len();
        if !self.startups.is_empty() {
            let mut durations: Vec<_> = self.startups.iter().copied().collect();
            durations.sort_unstable();
            let rank = (durations.len() * 95).div_ceil(100);
            snapshot.startup_p95_ms = durations.get(rank.saturating_sub(1)).copied();
        }
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p95_is_observed_data_not_default_zero() {
        let mut metrics = Metrics::new(100).unwrap();
        assert_eq!(metrics.snapshot().startup_p95_ms, None);
        for i in 1..=100 {
            metrics.record(PlaybackEvent::StartupDelayMs(i));
        }
        assert_eq!(metrics.snapshot().startup_p95_ms, Some(95));
    }

    #[test]
    fn tracks_cache_and_network_separately() {
        let mut metrics = Metrics::new(3).unwrap();
        metrics.record(PlaybackEvent::VerifiedNetworkBytes(100));
        metrics.record(PlaybackEvent::ReusedCacheBytes(900));
        metrics.record(PlaybackEvent::SegmentReady);
        metrics.record(PlaybackEvent::SegmentDeadlineMiss);
        metrics.record(PlaybackEvent::SourceSwitch);
        let state = metrics.snapshot();
        assert_eq!(state.verified_network_bytes, 100);
        assert_eq!(state.reused_cache_bytes, 900);
        assert_eq!(state.segment_ready, 1);
        assert_eq!(state.segment_deadline_misses, 1);
        assert_eq!(state.source_switches, 1);
    }

    #[test]
    fn bounded_history_discards_old_startups() {
        let mut metrics = Metrics::new(2).unwrap();
        metrics.record(PlaybackEvent::StartupDelayMs(1000));
        metrics.record(PlaybackEvent::StartupDelayMs(1));
        metrics.record(PlaybackEvent::StartupDelayMs(2));
        assert_eq!(metrics.snapshot().startup_count, 2);
        assert_eq!(metrics.snapshot().startup_p95_ms, Some(2));
    }
}
