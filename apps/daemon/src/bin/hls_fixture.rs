//! Run only on locally synthesized fixture media; not a torrent source API.
use my_streamio_media_pipeline::hls_program::{
    render_hls_program, MediaInput, ProgramSlice, ProgramSpec,
};
use my_streamio_timeline::Alignment;
use sha2::{Digest, Sha256};
use std::{
    env,
    error::Error,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

fn digest_file(path: &Path) -> Result<[u8; 32], Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env::args().nth(1).ok_or(
        "usage: cargo run --bin hls_fixture -- <generated-fixture-directory>",
    )?);
    let root = root.canonicalize()?;
    let reference = root.join("reference.mp4");
    let alternate = root.join("alternate.mp4");
    let primary = MediaInput {
        sha256: digest_file(&reference)?,
        path: reference,
    };
    let other = MediaInput {
        sha256: digest_file(&alternate)?,
        path: alternate,
    };
    let ground_truth = Alignment {
        slope: 1.0,
        offset_ms: 0.0,
        source_start_ms: 0.0,
        source_end_ms: 20_000.0,
        max_residual_ms: 0.0,
        evidence_count: 3,
    };
    let program = ProgramSpec {
        id: "integration-switch".to_string(),
        primary_video: primary.clone(),
        primary_audio: primary.clone(),
        primary_audio_start_ms: 0,
        slices: vec![
            ProgramSlice {
                video: primary.clone(),
                canonical_start_ms: 0,
                source_start_ms: 0,
                duration_ms: 3_000,
                alignment: None,
            },
            ProgramSlice {
                video: other,
                canonical_start_ms: 3_000,
                source_start_ms: 3_000,
                duration_ms: 3_000,
                alignment: Some(ground_truth),
            },
            ProgramSlice {
                video: primary,
                canonical_start_ms: 6_000,
                source_start_ms: 6_000,
                duration_ms: 3_000,
                alignment: None,
            },
        ],
        width: 640,
        height: 360,
        fps: 24,
        hls_segment_seconds: 3,
    };

    let result = render_hls_program(
        &program,
        &root,
        Path::new("ffmpeg"),
        Path::new("ffprobe"),
    )?;
    writeln!(
        std::io::stdout(),
        "Built verified local test HLS: {} ({} segments)",
        result.playlist.display(),
        result.segments,
    )?;
    Ok(())
}
