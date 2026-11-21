//! Network-facing torrent adapter. The engine owns peer discovery and piece verification.
//! Applications must never treat arbitrary local file reads as torrent-verified bytes.

use anyhow::{anyhow, bail, Context, Result};
use librqbit::{api::TorrentIdOrHash, AddTorrent, AddTorrentOptions, Api, Session};
use my_streamio_domain::SwarmKey;
use my_streamio_discovery::{PeerSnapshot, SwarmRegistry, SwarmState};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::SeekFrom,
    net::IpAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use url::Url;

pub const MAX_MAGNET_BYTES: usize = 8192;
const MAX_TRACKERS: usize = 32;

#[derive(Clone)]
pub struct TorrentBackend {
    api: Api,
    peer_samples: Arc<Mutex<HashMap<usize, PeerWindow>>>,
    admissions: Arc<Mutex<SwarmRegistry>>,
    keys_by_torrent_id: Arc<Mutex<HashMap<usize, SwarmKey>>>,
}

#[derive(Clone, Copy, Debug, Default)]
struct PeerCounter {
    fetched_bytes: u64,
    checked_pieces: u64,
}

struct PeerWindow {
    at: Instant,
    counters: HashMap<String, PeerCounter>,
}

/// All values are scoped to one infohash. Peer endpoint addresses are never
/// returned to the browser. Downloaded bytes are NOT equated with verified bytes.
#[derive(Clone, Debug, serde::Serialize)]
pub struct PeerOverview {
    pub connected_peers: usize,
    pub contributing_peers: usize,
    pub fetched_bytes_per_sec: u64,
    pub checked_pieces_in_window: u64,
    pub window_ms: u64,
    pub has_baseline: bool,
}

fn summarize_peer_window(
    previous: Option<(u64, &HashMap<String, PeerCounter>)>,
    current: &HashMap<String, PeerCounter>,
) -> PeerOverview {
    let mut overview = PeerOverview {
        connected_peers: current.len(),
        contributing_peers: 0,
        fetched_bytes_per_sec: 0,
        checked_pieces_in_window: 0,
        window_ms: previous.map_or(0, |(duration, _)| duration),
        has_baseline: previous.is_some(),
    };

    if let Some((duration_ms, old)) = previous {
        let mut fetched = 0_u64;
        for (peer, new) in current {
            // Unknown/reconnected peers need a second observation before
            // they can count as useful, otherwise old counters skew rates.
            if let Some(before) = old.get(peer) {
                let checked = new.checked_pieces.saturating_sub(before.checked_pieces);
                if checked != 0 {
                    overview.contributing_peers += 1;
                    overview.checked_pieces_in_window =
                        overview.checked_pieces_in_window.saturating_add(checked);
                }
                fetched = fetched.saturating_add(
                    new.fetched_bytes.saturating_sub(before.fetched_bytes),
                );
            }
        }
        if duration_ms > 0 {
            overview.fetched_bytes_per_sec =
                ((u128::from(fetched) * 1000) / u128::from(duration_ms))
                    .min(u128::from(u64::MAX)) as u64;
        }
    }
    overview
}

impl TorrentBackend {
    pub async fn open(storage_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&storage_dir).context("creating torrent storage directory")?;
        let session = Session::new(storage_dir)
            .await
            .context("starting torrent session")?;
        let max_sources = match std::env::var("STREAMIO_MAX_SWARMS") {
            Ok(raw) => raw.parse::<usize>().context("invalid STREAMIO_MAX_SWARMS")?,
            Err(std::env::VarError::NotPresent) => 32,
            Err(error) => return Err(error.into()),
        };
        if !(1..=128).contains(&max_sources) {
            bail!("STREAMIO_MAX_SWARMS must be in 1..=128");
        }
        let admissions = SwarmRegistry::new(max_sources, max_sources.min(8))
            .map_err(|error| anyhow!(error))?;
        Ok(Self {
            api: Api::new(session, None),
            peer_samples: Arc::new(Mutex::new(HashMap::new())),
            admissions: Arc::new(Mutex::new(admissions)),
            keys_by_torrent_id: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn add_magnet(&self, magnet: &str) -> Result<Value> {
        self.add_magnet_with_options(magnet, None).await
    }

    /// An explicit loopback-only peer override for hermetic in-process integration
    /// tests. This method is NOT included in normal or release builds.
    #[cfg(feature = "network-fixture")]
    pub async fn add_magnet_from_fixture_peer(
        &self,
        magnet: &str,
        fixture_port: u16,
    ) -> Result<Value> {
        if fixture_port == 0 {
            bail!("fixture peer must use a nonzero TCP port");
        }
        let peer = std::net::SocketAddr::from(([127, 0, 0, 1], fixture_port));
        self.add_magnet_with_options(
            magnet,
            Some(AddTorrentOptions {
                initial_peers: Some(vec![peer]),
                disable_trackers: true,
                ..Default::default()
            }),
        )
        .await
    }

    async fn add_magnet_with_options(
        &self,
        magnet: &str,
        options: Option<AddTorrentOptions>,
    ) -> Result<Value> {
        let key = validate_magnet(magnet)?;
        {
            let mut registry = self.admissions.lock()
                .map_err(|_| anyhow!("swarm registry poisoned"))?;
            if !registry.register(key.clone()).map_err(|reason| anyhow!(reason))? {
                bail!("this infohash is already registered or in progress");
            }
        }

        // A failed magnet fetch must not exhaust the source budget. Never
        // hold the synchronous admission mutex across an async network call.
        let request = self
            .api
            .api_add_torrent(AddTorrent::from_url(magnet), options)
            .await;
        match request {
            Ok(response) => {
                if let Some(torrent_id) = response.id {
                    let mut id_map = self.keys_by_torrent_id.lock()
                        .map_err(|_| anyhow!("swarm-to-torrent mapping poisoned"))?;
                    id_map.insert(torrent_id, key.clone());
                } else {
                    let mut registry = self.admissions.lock()
                        .map_err(|_| anyhow!("swarm registry poisoned"))?;
                    registry.remove(&key);
                }
                Ok(serde_json::json!({
                    "id": response.id,
                    "info_hash": response.details.info_hash,
                    "name": response.details.name
                }))
            }
            Err(error) => {
                let mut registry = self.admissions.lock()
                    .map_err(|_| anyhow!("swarm registry poisoned"))?;
                registry.remove(&key);
                Err(anyhow!(error).context("librqbit add_torrent failed"))
            }
        }
    }

    pub fn swarms(&self) -> Result<Value> {
        let registry = self.admissions.lock()
            .map_err(|_| anyhow!("swarm registry poisoned"))?;
        let rows: Vec<Value> = registry.snapshot().into_iter()
            .map(|entry| serde_json::json!({
                "info_hash": entry.key.as_hex(),
                "state": entry.state,
                "connected_peers": entry.stats.connected,
                "useful_peers": entry.stats.useful
            }))
            .collect();
        Ok(serde_json::json!({"swarms": rows}))
    }

    pub fn list_torrents(&self) -> Result<Value> {
        Ok(serde_json::json!({
            "torrents": self.api.api_torrent_list().torrents.into_iter().map(|source| {
                serde_json::json!({
                    "id": source.id,
                    "info_hash": source.info_hash,
                    "name": source.name
                })
            }).collect::<Vec<Value>>()
        }))
    }

    /// Snapshot each connected peer's counters and calculate their change
    /// since the previous API request. The first sample has no rate estimate.
    pub fn peer_overview(&self, torrent_id: usize) -> Result<PeerOverview> {
        let snapshot = self.api.api_peer_stats(
            TorrentIdOrHash::Id(torrent_id),
            Default::default(),
        ).context("live peer statistics unavailable")?;

        let now = Instant::now();
        let counters: HashMap<String, PeerCounter> = snapshot.peers.into_iter()
            .map(|(endpoint, stats)| {
                (endpoint, PeerCounter {
                    fetched_bytes: stats.counters.fetched_bytes,
                    checked_pieces: u64::from(stats.counters.downloaded_and_checked_pieces),
                })
            }).collect();

        let mut history = self.peer_samples.lock()
            .map_err(|_| anyhow!("peer statistics store poisoned"))?;
        let previous = history.get(&torrent_id);
        let old = previous.map(|entry| (
            now.duration_since(entry.at).as_millis().min(u128::from(u64::MAX)) as u64,
            &entry.counters,
        ));
        let result = summarize_peer_window(old, &counters);
        history.insert(torrent_id, PeerWindow { at: now, counters });
        drop(history);

        let key = {
            let map = self.keys_by_torrent_id.lock()
                .map_err(|_| anyhow!("swarm-to-torrent mapping poisoned"))?;
            map.get(&torrent_id).cloned()
        };
        if let Some(key) = key {
            let mut registry = self.admissions.lock()
                .map_err(|_| anyhow!("swarm registry poisoned"))?;
            let state = if result.contributing_peers > 0 {
                SwarmState::Supplying
            } else if result.connected_peers > 0 {
                SwarmState::Connected
            } else {
                SwarmState::Discovering
            };
            // This API does not expose total discovered endpoints or verified
            // bytes per second. Connected is only a lower bound on discovery;
            // fetched bytes must never be misreported as verified goodput.
            let stats = PeerSnapshot {
                discovered: result.connected_peers,
                connected: result.connected_peers,
                useful: result.contributing_peers,
                verified_bytes_per_sec: 0,
                needed_range_available_percent: 0,
            };
            registry.update(&key, state, stats).map_err(|reason| anyhow!(reason))?;
        }
        Ok(result)
    }

    /// Return file metadata without disclosing host storage paths.
    pub fn files(&self, torrent_id: usize) -> Result<Vec<PlayableFile>> {
        let details = self
            .api
            .api_torrent_details(TorrentIdOrHash::Id(torrent_id))
            .context("torrent not available")?;
        let files = details.files.context("torrent metadata unavailable")?;
        Ok(files
            .into_iter()
            .enumerate()
            .map(|(id, file)| PlayableFile {
                id,
                name: file.name,
                length: file.length,
                included: file.included,
            })
            .collect())
    }

    /// Read only bytes that librqbit has verified against the torrent pieces.
    /// Never open incomplete torrent files through std::fs::File.
    pub async fn read_verified_range(
        &self,
        torrent_id: usize,
        file_id: usize,
        offset: u64,
        count: usize,
    ) -> Result<Vec<u8>> {
        if count == 0 || count > MAX_HTTP_CHUNK {
            bail!("requested range exceeds cap");
        }
        let mut stream = self
            .api
            .api_stream(TorrentIdOrHash::Id(torrent_id), file_id)
            .await
            .context("cannot open torrent file stream")?;
        let read = async {
            stream.seek(SeekFrom::Start(offset)).await?;
            let mut bytes = vec![0; count];
            stream.read_exact(&mut bytes).await?;
            Ok::<Vec<u8>, std::io::Error>(bytes)
        };
        tokio::time::timeout(Duration::from_secs(20), read)
            .await
            .context("timed out waiting for verified pieces")?
            .context("torrent stream ended early")
    }
}

pub const MAX_HTTP_CHUNK: usize = 1024 * 1024;

#[derive(Clone, Debug, serde::Serialize)]
pub struct PlayableFile {
    pub id: usize,
    pub name: String,
    pub length: u64,
    pub included: bool,
}

/// Validates a magnet before letting the native torrent engine contact any tracker.
/// DNS-rebinding / remote tracker policy will require additional defenses before public use.
pub fn validate_magnet(raw: &str) -> Result<SwarmKey> {
    if raw.len() > MAX_MAGNET_BYTES {
        bail!("magnet exceeds maximum length");
    }

    let url = Url::parse(raw).context("invalid magnet URI")?;
    if url.scheme() != "magnet" {
        bail!("only magnet URI input is accepted");
    }
    let mut identity = None;
    let mut trackers = 0usize;
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "xt" => {
                let value = value.as_ref();
                let parsed = if let Some(hash) = value.strip_prefix("urn:btih:") {
                    Some(parse_btih(hash)?)
                } else if let Some(hash) = value.strip_prefix("urn:btmh:") {
                    Some(SwarmKey::parse_btmh(hash).map_err(|e| anyhow!(e))?)
                } else {
                    None
                };
                if let Some(next) = parsed {
                    if identity.as_ref().is_some_and(|current| current != &next) {
                        bail!("magnet includes conflicting infohash values");
                    }
                    identity = Some(next);
                }
            }
            "tr" => {
                trackers += 1;
                if trackers > MAX_TRACKERS {
                    bail!("too many tracker URLs");
                }
                validate_tracker(&value)?;
            }
            _ => {}
        }
    }

    identity.ok_or_else(|| anyhow!("missing supported xt=urn:btih or urn:btmh"))
}

fn validate_tracker(raw: &str) -> Result<()> {
    let tracker = Url::parse(raw).context("invalid tracker URL")?;
    if !matches!(tracker.scheme(), "http" | "https" | "udp") {
        bail!("unsupported tracker protocol");
    }
    let host = tracker
        .host_str()
        .ok_or_else(|| anyhow!("tracker has no host"))?;
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        bail!("local tracker not allowed by default");
    }
    if host.parse::<IpAddr>().is_ok() {
        bail!("tracker IP literals are disabled by default");
    }
    Ok(())
}

fn parse_btih(value: &str) -> Result<SwarmKey> {
    if value.len() == 40 {
        return SwarmKey::parse_btih(value).map_err(|e| anyhow!(e));
    }
    if value.len() != 32 {
        bail!("btih hash must be 40 hex or 32 base32 characters");
    }

    let mut decoded = [0u8; 20];
    let mut bit_buffer = 0u32;
    let mut remaining = 0u8;
    let mut written = 0usize;
    for byte in value.bytes() {
        let upper = byte.to_ascii_uppercase();
        let digit = match upper {
            b'A'..=b'Z' => upper - b'A',
            b'2'..=b'7' => upper - b'2' + 26,
            _ => bail!("invalid base32 character"),
        };
        bit_buffer = (bit_buffer << 5) | u32::from(digit);
        remaining += 5;
        if remaining >= 8 {
            remaining -= 8;
            decoded[written] = ((bit_buffer >> remaining) & 0xff) as u8;
            written += 1;
        }
    }
    if written != 20 || remaining != 0 {
        bail!("incorrect base32 hash length");
    }
    Ok(SwarmKey::V1(decoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnet_hex_and_base32_are_same_identity() {
        let a = validate_magnet(&format!("magnet:?xt=urn:btih:{}", "00".repeat(20))).unwrap();
        let b = validate_magnet(&format!("magnet:?xt=urn:btih:{}", "A".repeat(32))).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_conflicting_identity_and_bad_trackers() {
        let magnet = format!(
            "magnet:?xt=urn:btih:{}&xt=urn:btih:{}",
            "aa".repeat(20),
            "bb".repeat(20)
        );
        assert!(validate_magnet(&magnet).is_err());
        let magnet = format!(
            "magnet:?xt=urn:btih:{}&tr=http%3A%2F%2F127.0.0.1%3A8080",
            "aa".repeat(20)
        );
        assert!(validate_magnet(&magnet).is_err());
    }

    #[test]
    fn rejects_malformed_and_oversized_magnets() {
        assert!(validate_magnet("https://example.com/").is_err());
        assert!(validate_magnet("magnet:?dn=movie").is_err());
        assert!(validate_magnet(&"a".repeat(MAX_MAGNET_BYTES + 1)).is_err());
    }


    #[test]
    fn peer_metrics_only_credit_observed_verified_piece_progress() {
        let mut old = HashMap::new();
        old.insert("private-endpoint".to_owned(), PeerCounter {
            fetched_bytes: 10_000, checked_pieces: 5,
        });
        let mut current = HashMap::new();
        current.insert("private-endpoint".to_owned(), PeerCounter {
            fetched_bytes: 24_000, checked_pieces: 7,
        });
        current.insert("just-connected".to_owned(), PeerCounter {
            fetched_bytes: 30_000, checked_pieces: 2,
        });
        let overview = summarize_peer_window(Some((2000, &old)), &current);
        assert_eq!(overview.connected_peers, 2);
        assert_eq!(overview.contributing_peers, 1);
        assert_eq!(overview.checked_pieces_in_window, 2);
        assert_eq!(overview.fetched_bytes_per_sec, 7_000);
        let public = serde_json::to_string(&overview).unwrap();
        assert!(!public.contains("private-endpoint"));
        assert!(!public.contains("just-connected"));
    }

    #[test]
    fn first_peer_sample_does_not_fabricate_activity() {
        let mut current = HashMap::new();
        current.insert("peer".to_owned(), PeerCounter {
            fetched_bytes: 1234, checked_pieces: 2,
        });
        let result = summarize_peer_window(None, &current);
        assert!(!result.has_baseline);
        assert_eq!(result.connected_peers, 1);
        assert_eq!(result.contributing_peers, 0);
        assert_eq!(result.fetched_bytes_per_sec, 0);
    }

    #[test]
    fn preserves_v2_hash_identity() {
        let hash = format!("1220{}", "ac".repeat(32));
        let result = validate_magnet(&format!("magnet:?xt=urn:btmh:{hash}")).unwrap();
        assert!(matches!(result, SwarmKey::V2(_)));
    }
}


/// A deterministic local peer used solely by the opt-in network fixture.
/// It is compiled out of all normal release builds and never offers a
/// browser/HTTP endpoint that can accept arbitrary peer addresses.
#[cfg(feature = "network-fixture")]
pub struct FixtureSeeder {
    session: Arc<Session>,
    pub peer_port: u16,
    pub primary_magnet: String,
    pub alternate_magnet: String,
}

#[cfg(feature = "network-fixture")]
impl FixtureSeeder {
    pub async fn start(primary: &std::path::Path, alternate: &std::path::Path) -> Result<Self> {
        let root = primary.parent().context("fixture primary path has no parent")?;
        if alternate.parent() != Some(root) {
            bail!("two fixture source files must share one isolated seed directory");
        }
        let session = Session::new(root.to_path_buf()).await?;
        let peer_port = session
            .listen_addr()
            .context("BitTorrent seeder has no TCP listener")?
            .port();
        if peer_port == 0 {
            bail!("BitTorrent seeder must have an assigned TCP port");
        }
        let (primary_meta, _a) = session
            .create_and_serve_torrent(primary, librqbit::CreateTorrentOptions {
                piece_length: Some(65_536),
                ..Default::default()
            })
            .await
            .context("seeding first synthetic MP4")?;
        let (alternate_meta, _b) = session
            .create_and_serve_torrent(alternate, librqbit::CreateTorrentOptions {
                piece_length: Some(131_072),
                ..Default::default()
            })
            .await
            .context("seeding second synthetic MP4")?;
        if primary_meta.info_hash() == alternate_meta.info_hash() {
            bail!("two independent synthetic torrents must have distinct infohashes");
        }
        Ok(Self {
            session,
            peer_port,
            primary_magnet: primary_meta.as_magnet().to_string(),
            alternate_magnet: alternate_meta.as_magnet().to_string(),
        })
    }

    pub async fn stop(self) {
        self.session.stop().await;
    }
}
