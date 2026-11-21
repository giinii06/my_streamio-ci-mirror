//! Multi-swarm session budgeting, not protocol-level peer aggregation.
//! Each infohash retains its own independent peer pool.

use my_streamio_domain::SwarmKey;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwarmState {
    Dormant,
    Discovering,
    Connected,
    Supplying,
    CoolingDown,
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSnapshot {
    pub discovered: usize,
    pub connected: usize,
    pub useful: usize,
    pub verified_bytes_per_sec: u64,
    pub needed_range_available_percent: u8,
}

impl PeerSnapshot {
    pub fn valid(&self) -> bool {
        self.needed_range_available_percent <= 100
            && self.useful <= self.connected
            && self.connected <= self.discovered
    }

    /// Higher priority for peers supplying relevant verified data. Never equate
    /// a discovered endpoint with a contributing peer.
    pub fn utility(&self) -> u128 {
        u128::from(self.verified_bytes_per_sec) * u128::from(self.needed_range_available_percent)
            / 100
            + self.useful as u128 * 1024
    }
}

#[derive(Clone, Debug)]
pub struct SwarmEntry {
    pub key: SwarmKey,
    pub state: SwarmState,
    pub stats: PeerSnapshot,
    pub last_active_epoch: u64,
}

pub struct SwarmRegistry {
    entries: HashMap<SwarmKey, SwarmEntry>,
    max_registered: usize,
    max_active: usize,
}

impl SwarmRegistry {
    pub fn new(max_registered: usize, max_active: usize) -> Result<Self, &'static str> {
        if max_registered == 0 || max_active == 0 || max_active > max_registered {
            return Err("invalid swarm budgets");
        }
        Ok(Self {
            entries: HashMap::new(),
            max_registered,
            max_active,
        })
    }

    /// Idempotent: multiple magnet URLs with one infohash do not create
    /// multiple torrent sessions.
    pub fn register(&mut self, key: SwarmKey) -> Result<bool, &'static str> {
        if self.entries.contains_key(&key) {
            return Ok(false);
        }
        if self.entries.len() >= self.max_registered {
            return Err("swarm registry is at capacity");
        }
        self.entries.insert(
            key.clone(),
            SwarmEntry {
                key,
                state: SwarmState::Dormant,
                stats: PeerSnapshot::default(),
                last_active_epoch: 0,
            },
        );
        Ok(true)
    }

    /// A failed source admission must free its reserved slot.
    pub fn remove(&mut self, key: &SwarmKey) -> bool {
        self.entries.remove(key).is_some()
    }

    pub fn update(
        &mut self,
        key: &SwarmKey,
        state: SwarmState,
        stats: PeerSnapshot,
    ) -> Result<(), &'static str> {
        if !stats.valid() {
            return Err("invalid peer counts or range availability");
        }
        let entry = self.entries.get_mut(key).ok_or("unknown swarm")?;
        entry.state = state;
        entry.stats = stats;
        Ok(())
    }

    /// Deterministic for a given epoch; failed swarms excluded. Includes
    /// already connected but temporarily idle swarms for resilience.
    pub fn choose_active(&mut self, epoch: u64) -> Vec<SwarmKey> {
        let mut entries: Vec<_> = self
            .entries
            .values()
            .filter(|entry| entry.state != SwarmState::Failed)
            .collect();

        entries.sort_by(|a, b| {
            b.stats
                .utility()
                .cmp(&a.stats.utility())
                .then(a.last_active_epoch.cmp(&b.last_active_epoch))
                .then(a.key.as_hex().cmp(&b.key.as_hex()))
        });

        let chosen: Vec<_> = entries
            .iter()
            .take(self.max_active)
            .map(|entry| entry.key.clone())
            .collect();

        let chosen_set: HashSet<_> = chosen.iter().collect();
        for entry in self.entries.values_mut() {
            if chosen_set.contains(&entry.key) {
                entry.last_active_epoch = epoch;
            } else if matches!(entry.state, SwarmState::Connected | SwarmState::Supplying) {
                entry.state = SwarmState::CoolingDown;
            }
        }
        chosen
    }

    pub fn snapshot(&self) -> Vec<SwarmEntry> {
        let mut rows: Vec<_> = self.entries.values().cloned().collect();
        rows.sort_by_key(|entry| entry.key.as_hex());
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: u8) -> SwarmKey {
        SwarmKey::V1([byte; 20])
    }

    fn peer_stats(goodput: u64, useful: usize) -> PeerSnapshot {
        PeerSnapshot {
            discovered: 100,
            connected: 20,
            useful,
            verified_bytes_per_sec: goodput,
            needed_range_available_percent: 100,
        }
    }

    #[test]
    fn deduplicates_magnet_infohash_and_respects_capacity() {
        let mut r = SwarmRegistry::new(2, 1).unwrap();
        assert!(r.register(hash(1)).unwrap());
        assert!(!r.register(hash(1)).unwrap());
        assert!(r.register(hash(2)).unwrap());
        assert!(r.register(hash(3)).is_err());
    }

    #[test]
    fn failed_registration_releases_capacity() {
        let mut registry = SwarmRegistry::new(1, 1).unwrap();
        assert!(registry.register(hash(1)).unwrap());
        assert!(registry.register(hash(2)).is_err());
        assert!(registry.remove(&hash(1)));
        assert!(!registry.remove(&hash(1)));
        assert!(registry.register(hash(2)).unwrap());
    }

    #[test]
    fn prioritizes_actual_goodput_over_discovered_peers() {
        let mut r = SwarmRegistry::new(3, 1).unwrap();
        r.register(hash(1)).unwrap();
        r.register(hash(2)).unwrap();
        r.update(&hash(1), SwarmState::Connected, peer_stats(0, 0))
            .unwrap();
        r.update(&hash(2), SwarmState::Supplying, peer_stats(2_000_000, 2))
            .unwrap();
        assert_eq!(r.choose_active(1), vec![hash(2)]);
    }

    #[test]
    fn rejects_impossible_peer_statistics() {
        let mut r = SwarmRegistry::new(1, 1).unwrap();
        r.register(hash(1)).unwrap();
        assert!(r
            .update(
                &hash(1),
                SwarmState::Supplying,
                PeerSnapshot {
                    discovered: 5,
                    connected: 6,
                    ..PeerSnapshot::default()
                }
            )
            .is_err());
    }

    #[test]
    fn does_not_activate_failed_swarm() {
        let mut r = SwarmRegistry::new(2, 2).unwrap();
        r.register(hash(1)).unwrap();
        r.register(hash(2)).unwrap();
        r.update(&hash(1), SwarmState::Failed, peer_stats(100_000, 5))
            .unwrap();
        assert_eq!(r.choose_active(9), vec![hash(2)]);
    }
}
