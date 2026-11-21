//! Opt-in network test: REAL BitTorrent seed sessions, real magnet metadata,
//! verified pieces and the exact production HTTP Range handlers.
//! Enabled only by cargo test --features network-fixture.
//! No public trackers, downloaded test assets or prepopulated receiver files.

use super::{app_router, ProgramRoot};
use librqbit::{CreateTorrentOptions, Session};
use my_streamio_torrent_adapter::TorrentBackend;
use reqwest::{header, Client, StatusCode};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, time::timeout};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "my-streamio-two-swarms-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated fixture root");
        Self(path)
    }

    fn root(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_real_swarms_deliver_piece_verified_http_seek_bytes() {
    timeout(Duration::from_secs(300), async {
        let scratch = Scratch::new();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let sources = scratch.root().join("seed");
        fs::create_dir_all(&sources).expect("fixture media directory");

        let generated = Command::new("sh")
            .arg("tests/fixtures/generate.sh")
            .arg(&sources)
            .current_dir(&repo)
            .status()
            .expect("launch synthetic FFmpeg generator");
        assert!(generated.success(), "FFmpeg did not create the two media files");

        let primary = sources.join("reference.mp4");
        let alternative = sources.join("alternate.mp4");
        let raw_a = fs::read(&primary).expect("H.264 fixture");
        let raw_b = fs::read(&alternative).expect("HEVC fixture");
        assert!(raw_a.len() > 65_536 && raw_b.len() > 65_536);
        assert_ne!(Sha256::digest(&raw_a), Sha256::digest(&raw_b));

        // A separate real BitTorrent session seeds both files. It is NOT
        // the receiver and cannot reuse its storage directory.
        let seeder = Session::new(sources.clone())
            .await
            .expect("start actual BitTorrent seeder");
        let peer_port = seeder
            .listen_addr()
            .expect("seeder TCP listener")
            .port();
        assert_ne!(peer_port, 0, "seeder must have a real listening TCP port");

        let (t1, _seed_a) = seeder
            .create_and_serve_torrent(
                &primary,
                CreateTorrentOptions {
                    piece_length: Some(65_536),
                    ..Default::default()
                },
            )
            .await
            .expect("seed primary video torrent");
        let (t2, _seed_b) = seeder
            .create_and_serve_torrent(
                &alternative,
                CreateTorrentOptions {
                    piece_length: Some(131_072),
                    ..Default::default()
                },
            )
            .await
            .expect("seed alternate video torrent");

        let a_magnet = t1.as_magnet().to_string();
        let b_magnet = t2.as_magnet().to_string();
        assert_ne!(t1.info_hash(), t2.info_hash(), "must be distinct swarms");
        assert!(a_magnet.starts_with("magnet:?xt=urn:btih:"));
        assert!(b_magnet.starts_with("magnet:?xt=urn:btih:"));
        assert!(!a_magnet.contains("&tr=") && !b_magnet.contains("&tr="));

        let receiver = scratch.root().join("receiver");
        fs::create_dir(&receiver).expect("receiver root");
        assert!(!receiver.join("reference.mp4").exists());
        assert!(!receiver.join("alternate.mp4").exists());

        let backend = TorrentBackend::open(receiver)
            .await
            .expect("independent receiving session");

        // In the opt-in feature, the only extra peer is pinned to localhost.
        // Normal release code never exposes this peer-injection method.
        let first = backend
            .add_magnet_from_fixture_peer(&a_magnet, peer_port)
            .await
            .expect("resolve first magnet metadata over BEP9");
        let second = backend
            .add_magnet_from_fixture_peer(&b_magnet, peer_port)
            .await
            .expect("resolve second magnet metadata over BEP9");
        let first_id = first["id"].as_u64().expect("first torrent id");
        let second_id = second["id"].as_u64().expect("second torrent id");
        assert_ne!(first_id, second_id);
        assert!(backend
            .add_magnet_from_fixture_peer("not-a-magnet", peer_port)
            .await
            .is_err());
        assert!(backend
            .add_magnet_from_fixture_peer(&a_magnet, 0)
            .await
            .is_err());

        let root = ProgramRoot(scratch.root().canonicalize().expect("canonical root"));
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("HTTP listener");
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let server = tokio::spawn(async move {
            axum::serve(listener, app_router(backend, root))
                .await
                .expect("test HTTP server");
        });
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("HTTP test client");

        let bad = client
            .post(format!("{base}/api/v1/sources"))
            .json(&serde_json::json!({"magnet":"not-a-magnet"}))
            .send().await.expect("bad magnet response");
        assert_eq!(bad.status(), StatusCode::UNPROCESSABLE_ENTITY);

        for (torrent_id, expected) in [(first_id, raw_a), (second_id, raw_b)] {
            let files = client
                .get(format!("{base}/api/v1/sources/{torrent_id}/files"))
                .send().await.expect("source metadata response");
            assert_eq!(files.status(), StatusCode::OK);
            let files: serde_json::Value = files.json().await.expect("source file list");
            assert_eq!(files["files"][0]["length"].as_u64(), Some(expected.len() as u64));
            let url = format!("{base}/api/v1/sources/{torrent_id}/files/0/stream");

            // All bytes must originate from lib's verified piece reader.
            // Different Range requests simulate progressive playback/seek/EOF.
            let mut received = Vec::with_capacity(expected.len());
            let mut cursor = 0usize;
            while cursor < expected.len() {
                let end = (cursor + 262_143).min(expected.len() - 1);
                let response = client.get(&url)
                    .header(header::RANGE, format!("bytes={cursor}-{end}"))
                    .send().await.expect("verified piece HTTP request");
                assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
                assert_eq!(
                    response.headers().get(header::CONTENT_RANGE).unwrap().to_str().unwrap(),
                    format!("bytes {cursor}-{end}/{}", expected.len())
                );
                let body = response.bytes().await.expect("Range body");
                assert_eq!(&body[..], &expected[cursor..=end]);
                received.extend_from_slice(&body);
                cursor = end + 1;
            }
            assert_eq!(Sha256::digest(&received), Sha256::digest(&expected));

            // A noncontiguous seek backwards after progressive reads.
            let seek_end = expected.len().min(8192) - 1;
            let response = client.get(&url)
                .header(header::RANGE, format!("bytes=0-{seek_end}"))
                .send().await.expect("backward seek");
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(&response.bytes().await.unwrap()[..], &expected[..=seek_end]);

            let response = client.get(&url)
                .header(header::RANGE, format!("bytes={}-", expected.len()))
                .send().await.expect("EOF range");
            assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        }

        server.abort();
        seeder.stop().await;
        println!("PASS: two distinct real v1 swarms -> verified HTTP byte ranges, seek, EOF");
    })
    .await
    .expect("two-swarm network fixture exceeded bounded deadline");
}
