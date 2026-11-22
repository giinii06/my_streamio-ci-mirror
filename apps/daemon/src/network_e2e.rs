//! Gated network fixture: two real independently seeded synthetic MP4 torrents
//! -> BEP9 metadata + SHA1 verified pieces -> production Axum Range handlers.
//! Enable with cargo test -p my-streamio-daemon --features network-fixture.

use super::{app_router, ProgramRoot};
use my_streamio_torrent_adapter::{FixtureSeeder, TorrentBackend};
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
            .expect("monotonic fixture identifier")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "my-streamio-net-e2e-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create temporary fixture root");
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct HttpResult {
    status: u16,
    headers: String,
    body: Vec<u8>,
}

fn local_http(
    scratch: &Scratch,
    url: &str,
    range: Option<String>,
    json: Option<&str>,
) -> HttpResult {
    let headers_path = scratch.path().join("curl-headers.txt");
    let body_path = scratch.path().join("curl-body.bin");
    let mut command = Command::new("curl");
    command.args([
        "--fail-with-body",
        "--silent",
        "--show-error",
        "--max-time",
        "45",
        "--output",
    ]);
    command.arg(&body_path);
    command.arg("--dump-header").arg(&headers_path);
    command.args(["--write-out", "%{http_code}"]);
    if let Some(ref range) = range {
        command.arg("--header").arg(format!("Range: {range}"));
    }
    if let Some(body) = json {
        command.args(["--request", "POST", "--header", "Content-Type: application/json"]);
        command.arg("--data").arg(body);
    }
    command.arg(url);
    let output = command.output().expect("curl executable is required");
    // curl --fail-with-body returns exit code 22 for 4xx, so allow it if
    // and only if a deliberate negative HTTP test returned a valid status.
    let status = std::str::from_utf8(&output.stdout)
        .expect("curl status is utf8")
        .parse::<u16>()
        .expect("curl returned an HTTP status");
    assert!(
        output.status.success() || (output.status.code() == Some(22) && status >= 400),
        "transport/network failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    HttpResult {
        status,
        headers: fs::read_to_string(&headers_path).expect("HTTP response headers"),
        body: fs::read(&body_path).expect("HTTP response body"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_real_swarms_deliver_piece_verified_http_seek_bytes() {
    timeout(Duration::from_secs(300), async {
        let scratch = Scratch::new();
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source_dir = scratch.path().join("seed");
        fs::create_dir(&source_dir).expect("seed source directory");

        let generated = Command::new("sh")
            .arg("tests/fixtures/generate.sh")
            .arg(&source_dir)
            .current_dir(repo)
            .status()
            .expect("launch fixture generator");
        assert!(generated.success(), "synthetic H.264/HEVC creation failed");

        let primary = source_dir.join("reference.mp4");
        let alternate = source_dir.join("alternate.mp4");
        let content_a = fs::read(&primary).expect("read synthetic reference");
        let content_b = fs::read(&alternate).expect("read synthetic alternative");
        assert!(content_a.len() > 65_536 && content_b.len() > 65_536);
        assert_ne!(Sha256::digest(&content_a), Sha256::digest(&content_b));

        let seeder = FixtureSeeder::start(&primary, &alternate)
            .await
            .expect("create two real BitTorrent seeds");
        assert!(seeder.primary_magnet.starts_with("magnet:?xt=urn:btih:"));
        assert!(seeder.alternate_magnet.starts_with("magnet:?xt=urn:btih:"));
        assert_ne!(seeder.primary_magnet, seeder.alternate_magnet);
        let port = seeder.peer_port;

        // Receiver is not preloaded with any source media. Only downloaded
        // and SHA1-verified BitTorrent pieces are exposed by the HTTP handler.
        let recv_dir = scratch.path().join("receiver");
        fs::create_dir(&recv_dir).expect("receiver storage root");
        assert!(!recv_dir.join("reference.mp4").exists());
        assert!(!recv_dir.join("alternate.mp4").exists());

        let backend = TorrentBackend::open(recv_dir)
            .await
            .expect("start an isolated BitTorrent receiver");
        // Test-only, instance-scoped peer injection: the PUBLIC API still
        // accepts only valid magnets, and the released binary cannot set it.
        assert!(backend.install_fixture_loopback_peer(0).is_err());
        backend
            .install_fixture_loopback_peer(port)
            .expect("install fixed local seed port");

        let program_root = ProgramRoot(
            scratch.path().canonicalize().expect("canonicalize test root"),
        );
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind loopback HTTP");
        let base = format!(
            "http://127.0.0.1:{}",
            listener.local_addr().unwrap().port()
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app_router(backend, program_root))
                .await
                .expect("in-process real HTTP server");
        });

        let bad = local_http(
            &scratch,
            &format!("{base}/api/v1/sources"),
            None,
            Some(r#"{"magnet":"not-a-magnet"}"#),
        );
        assert_eq!(bad.status, 422);

        // Use the actual production POST handler; the isolated test feature
        // changes only where the downloader finds its first real TCP peer.
        let primary_json =
            serde_json::json!({"magnet": seeder.primary_magnet.clone()}).to_string();
        let alternate_json =
            serde_json::json!({"magnet": seeder.alternate_magnet.clone()}).to_string();
        let first_response = local_http(
            &scratch,
            &format!("{base}/api/v1/sources"),
            None,
            Some(&primary_json),
        );
        assert_eq!(first_response.status, 200, "first magnet admission");
        let second_response = local_http(
            &scratch,
            &format!("{base}/api/v1/sources"),
            None,
            Some(&alternate_json),
        );
        assert_eq!(second_response.status, 200, "second magnet admission");
        let first: serde_json::Value =
            serde_json::from_slice(&first_response.body).expect("primary source response");
        let second: serde_json::Value =
            serde_json::from_slice(&second_response.body).expect("alternate source response");
        let first_id = first["id"].as_u64().expect("first torrent ID");
        let second_id = second["id"].as_u64().expect("second torrent ID");
        assert_ne!(first_id, second_id, "independent torrent admissions");

        for (id, expected, codec) in [(first_id, content_a, "h264"), (second_id, content_b, "hevc")] {
            let metadata = local_http(
                &scratch,
                &format!("{base}/api/v1/sources/{id}/files"),
                None,
                None,
            );
            assert_eq!(metadata.status, 200);
            let files: serde_json::Value =
                serde_json::from_slice(&metadata.body).expect("torrent metadata JSON");
            assert_eq!(
                files["files"][0]["length"].as_u64(),
                Some(expected.len() as u64)
            );
            let stream_url = format!("{base}/api/v1/sources/{id}/files/0/stream");
            let mut assembled = Vec::with_capacity(expected.len());
            let mut start = 0usize;
            while start < expected.len() {
                let end = (start + 262_143).min(expected.len() - 1);
                let response = local_http(
                    &scratch,
                    &stream_url,
                    Some(format!("bytes={start}-{end}")),
                    None,
                );
                assert_eq!(response.status, 206);
                let wanted = format!("bytes {start}-{end}/{}", expected.len());
                assert!(
                    response.headers.lines().any(|line| {
                        line.split_once(':').is_some_and(|(name, value)| {
                            name.eq_ignore_ascii_case("content-range") && value.trim() == wanted
                        })
                    }),
                    "server did not expose exact Content-Range {wanted}"
                );
                assert_eq!(response.body, expected[start..=end]);
                assembled.extend_from_slice(&response.body);
                start = end + 1;
            }
            assert_eq!(Sha256::digest(&assembled), Sha256::digest(&expected));

            // A byte-perfect transfer is not yet proof of playable media:
            // independently identify and completely decode the bytes that
            // travelled through both real BitTorrent and the HTTP API.
            let reconstructed = scratch.path().join(format!("received-{id}.mp4"));
            fs::write(&reconstructed, &assembled)
                .expect("persist reconstructed peer-delivered MP4");
            let probe = Command::new("ffprobe")
                .args([
                    "-v", "error",
                    "-select_streams", "v:0",
                    "-show_entries", "stream=codec_name",
                    "-of", "default=noprint_wrappers=1:nokey=1",
                ])
                .arg(&reconstructed)
                .output()
                .expect("launch FFprobe against downloaded video");
            assert!(
                probe.status.success(),
                "downloaded video metadata probe failed: {}",
                String::from_utf8_lossy(&probe.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&probe.stdout).trim(), codec);
            let decode = Command::new("ffmpeg")
                .args(["-hide_banner", "-v", "error", "-nostdin", "-xerror", "-i"])
                .arg(&reconstructed)
                .args(["-map", "0", "-f", "null", "-"])
                .output()
                .expect("launch full decode of peer-delivered video and audio");
            assert!(
                decode.status.success(),
                "full downloaded media decode failed: {}",
                String::from_utf8_lossy(&decode.stderr)
            );

            let back = local_http(
                &scratch,
                &stream_url,
                Some("bytes=0-8191".to_string()),
                None,
            );
            assert_eq!(back.status, 206);
            assert_eq!(back.body, expected[..8192]);

            let eof = local_http(
                &scratch,
                &stream_url,
                Some(format!("bytes={}-", expected.len())),
                None,
            );
            assert_eq!(eof.status, 416);
        }

        // This receiver has its own empty storage and resolves only torrent
        // metadata. The paused flag prevents payload pieces from arriving.
        let offline_backend = TorrentBackend::open(scratch.path().join("offline-receiver"))
            .await
            .expect("open independent empty receiver");
        let offline_source = offline_backend
            .add_magnet_from_fixture_peer(&seeder.primary_magnet, port, true)
            .await
            .expect("resolve peer metadata without downloading pieces");
        let offline_id = offline_source["id"]
            .as_u64()
            .expect("paused receiver torrent id") as usize;
        let offline_files = offline_backend
            .files(offline_id)
            .expect("paused receiver must have metadata");
        assert_eq!(
            offline_files[0].length,
            fs::metadata(&primary).expect("read primary fixture metadata").len()
        );

        let offline_listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind offline receiver HTTP listener");
        let offline_base = format!(
            "http://127.0.0.1:{}",
            offline_listener.local_addr().unwrap().port()
        );
        let offline_root = ProgramRoot(
            scratch.path().canonicalize().expect("canonicalize offline root"),
        );
        let offline_server = tokio::spawn(async move {
            axum::serve(offline_listener, app_router(offline_backend, offline_root))
                .await
                .expect("serve offline receiver API");
        });

        // With the only seeder gone, no valid pieces can become available.
        seeder.stop().await;
        let unavailable = local_http(
            &scratch,
            &format!("{offline_base}/api/v1/sources/{offline_id}/files/0/stream"),
            Some("bytes=0-1023".to_string()),
            None,
        );
        assert_eq!(unavailable.status, 503, "missing peer/pieces must fail closed");
        assert_eq!(unavailable.body, b"Verified bytes not ready");
        assert!(
            !unavailable
                .headers
                .to_ascii_lowercase()
                .contains("content-range:"),
            "failed streaming must not advertise playable media ranges"
        );
        offline_server.abort();
        server.abort();
        println!("E2E PASS: two real swarms, decoded media, and unavailable-peer fail-closed");
    })
    .await
    .expect("controlled two-swarm network test exceeded 300-second limit");
}
