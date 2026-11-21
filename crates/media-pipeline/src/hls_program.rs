//! A conservative **offline** reference program assembler for multiple
//! independently encoded source clips. One FFmpeg process encodes all video
//! intervals to one H.264/AAC MPEG-TS HLS timeline, with continuous audio
//! coming from the same primary file throughout.
//!
//! Inputs must be locally available and digest checked. Torrent-backed callers
//! must first establish that bytes are verified by their torrent metadata;
//! this offline builder alone does not prove BitTorrent provenance.

use anyhow::{anyhow, bail, Context, Result};
use my_streamio_timeline::Alignment;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    ffi::OsString,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_SLICES: usize = 32;
const MAX_DURATION_MS: u64 = 15 * 60_000;

#[derive(Clone, Debug)]
pub struct MediaInput {
    pub path: PathBuf,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct ProgramSlice {
    pub video: MediaInput,
    pub canonical_start_ms: u64,
    pub source_start_ms: u64,
    pub duration_ms: u64,
    /// Required for a non-primary rendition; canonical time is the primary
    /// video's reference timeline, not the alternate's byte position.
    pub alignment: Option<Alignment>,
}

#[derive(Clone, Debug)]
pub struct ProgramSpec {
    pub id: String,
    pub primary_video: MediaInput,
    pub primary_audio: MediaInput,
    pub primary_audio_start_ms: u64,
    pub slices: Vec<ProgramSlice>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub hls_segment_seconds: u32,
}

#[derive(Clone, Debug)]
pub struct HlsProgramOutput {
    pub directory: PathBuf,
    pub playlist: PathBuf,
    pub segments: usize,
}

impl ProgramSpec {
    pub fn validate(&self) -> Result<u64> {
        if self.id.is_empty() || self.id.len() > 80 ||
            !self.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            bail!("program id must be 1..80 safe ASCII letters, numbers or dashes");
        }
        if self.slices.is_empty() || self.slices.len() > MAX_SLICES {
            bail!("program requires 1..32 bounded video intervals");
        }
        if self.width < 16 || self.width > 3840 || self.height < 16 || self.height > 2160 ||
            self.width % 2 != 0 || self.height % 2 != 0 ||
            !(16..=60).contains(&self.fps) ||
            !(2..=10).contains(&self.hls_segment_seconds) {
            bail!("invalid output video/HLS profile");
        }
        let mut end = 0_u64;
        for slice in &self.slices {
            if slice.canonical_start_ms != end ||
                slice.duration_ms < 1000 || slice.duration_ms > 10_000 ||
                slice.duration_ms % 1000 != 0 {
                bail!("interval must be contiguous and 1..10 whole seconds");
            }
            end = end.checked_add(slice.duration_ms).context("timeline overflow")?;
            if end > MAX_DURATION_MS {
                bail!("offline program exceeds prototype time budget");
            }
            let source_end_ms = slice.source_start_ms
                .checked_add(slice.duration_ms).context("source time overflow")?;
            let primary = slice.video.sha256 == self.primary_video.sha256;
            if !primary {
                let map = slice.alignment.as_ref().context("alternate source requires timeline proof")?;
                if !map.covers(slice.source_start_ms as f64, source_end_ms as f64) {
                    bail!("alternate source alignment does not cover required interval");
                }
                let start = map.reference_time(slice.source_start_ms as f64)
                    .context("no mapped start frame")?;
                let mapped_end = map.reference_time(source_end_ms as f64)
                    .context("no mapped end frame")?;
                if (start - slice.canonical_start_ms as f64).abs() > 42.0 ||
                    (mapped_end - end as f64).abs() > 42.0 {
                    bail!("alternate segment is not aligned with the reference timeline");
                }
            } else if slice.canonical_start_ms != slice.source_start_ms {
                bail!("primary video segment has an unexpected source offset");
            }
        }
        self.primary_audio_start_ms.checked_add(end).context("audio duration overflow")?;
        Ok(end)
    }

    fn filter_graph(&self, total_ms: u64) -> String {
        let mut parts = Vec::with_capacity(self.slices.len() + 2);
        for index in 0..self.slices.len() {
            parts.push(format!(
                "[{index}:v:0]fps={},scale={}:{}:flags=lanczos,format=yuv420p,setpts=PTS-STARTPTS[v{index}]",
                self.fps, self.width, self.height,
            ));
        }
        let concat = (0..self.slices.len()).map(|index| format!("[v{index}]"))
            .collect::<String>();
        parts.push(format!(
            "{concat}concat=n={}:v=1:a=0[v]",
            self.slices.len()
        ));
        parts.push(format!(
            "[{}:a:0]aresample=async=1:first_pts=0,atrim=0:{:.3},asetpts=PTS-STARTPTS[a]",
            self.slices.len(), total_ms as f64 / 1000.0
        ));
        parts.join(";")
    }

    fn ffmpeg_args(&self, stage: &Path, total_ms: u64) -> Vec<OsString> {
        let mut args: Vec<OsString> = [
            "-hide_banner", "-nostdin", "-loglevel", "error", "-y",
        ].into_iter().map(Into::into).collect();
        for slice in &self.slices {
            args.extend([
                OsString::from("-ss"),
                OsString::from(format!("{:.3}", slice.source_start_ms as f64 / 1000.0)),
                OsString::from("-t"),
                OsString::from(format!("{:.3}", slice.duration_ms as f64 / 1000.0)),
                OsString::from("-i"),
                slice.video.path.clone().into_os_string(),
            ]);
        }
        args.extend([
            OsString::from("-ss"),
            OsString::from(format!("{:.3}", self.primary_audio_start_ms as f64 / 1000.0)),
            OsString::from("-t"),
            OsString::from(format!("{:.3}", total_ms as f64 / 1000.0)),
            OsString::from("-i"),
            self.primary_audio.path.clone().into_os_string(),
            OsString::from("-filter_complex"),
            OsString::from(self.filter_graph(total_ms)),
        ]);
        let gop = self.fps * self.hls_segment_seconds;
        let gop_string = gop.to_string();
        let segment_seconds = self.hls_segment_seconds.to_string();
        let force = format!("expr:gte(t,n_forced*{})", self.hls_segment_seconds);
        for value in [
            "-map", "[v]", "-map", "[a]", "-c:v", "libx264", "-preset", "veryfast",
            "-pix_fmt", "yuv420p", "-g", &gop_string, "-keyint_min", &gop_string,
            "-sc_threshold", "0", "-force_key_frames", &force,
            "-c:a", "aac", "-ar", "48000", "-ac", "2", "-b:a", "128k",
            "-f", "hls", "-hls_time", &segment_seconds, "-hls_playlist_type", "vod",
            "-hls_flags", "independent_segments", "-hls_segment_filename",
        ] {
            args.push(value.into());
        }
        args.push(stage.join("seg%05d.ts").into_os_string());
        args.push(stage.join("playlist.m3u8").into_os_string());
        args
    }
}

fn verify_digest(root: &Path, input: &MediaInput) -> Result<PathBuf> {
    let canonical = input.path.canonicalize().context("media input missing")?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        bail!("media input must be an ordinary file inside allowed storage root");
    }
    let mut file = File::open(&canonical)?;
    let mut sha = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let size = file.read(&mut buffer).context("reading video input")?;
        if size == 0 {
            break;
        }
        sha.update(&buffer[..size]);
    }
    let calculated: [u8; 32] = sha.finalize().into();
    if calculated != input.sha256 {
        bail!("media input failed full-file digest verification");
    }
    Ok(canonical)
}

fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<()> {
    let mut child = command.spawn().context("FFmpeg or FFprobe not available")?;
    let start = Instant::now();
    loop {
        if let Some(exit) = child.try_wait().context("media process failed")? {
            if exit.success() {
                return Ok(());
            }
            bail!("media process exited unsuccessfully");
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("media process timed out");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Builds HLS only from fully available checked files. No output is made
/// public until FFmpeg completes and ffprobe verifies the finished playlist.
/// The complete directory is atomically moved into the published location.
pub fn render_hls_program(
    spec: &ProgramSpec,
    approved_root: &Path,
    ffmpeg: &Path,
    ffprobe: &Path,
) -> Result<HlsProgramOutput> {
    let total_ms = spec.validate()?;
    let root = approved_root.canonicalize().context("storage root missing")?;

    let mut seen = HashMap::<PathBuf, [u8; 32]>::new();
    for input in std::iter::once(&spec.primary_video)
        .chain(std::iter::once(&spec.primary_audio))
        .chain(spec.slices.iter().map(|slice| &slice.video))
    {
        let checked = verify_digest(&root, input)?;
        if seen.insert(checked, input.sha256).is_some_and(|before| before != input.sha256) {
            bail!("conflicting digests were supplied for the same input path");
        }
    }

    let parent = root.join("hls-programs");
    fs::create_dir_all(&parent)?;
    if !parent.canonicalize()?.starts_with(&root) {
        bail!("HLS output directory is outside allowed storage");
    }
    let public = parent.join(&spec.id);
    if public.exists() {
        bail!("HLS program already published");
    }
    let stage = parent.join(format!(".{}.partial", spec.id));
    fs::create_dir(&stage).context("program already rendering or stale staging directory")?;

    let result = (|| -> Result<usize> {
        let log_path = stage.join("ffmpeg.log");
        let log = File::create(&log_path)?;
        let mut worker = Command::new(ffmpeg);
        worker.args(spec.ffmpeg_args(&stage, total_ms))
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::from(log));
        run_with_timeout(worker, Duration::from_secs(120))?;

        let manifest = stage.join("playlist.m3u8");
        let playlist = fs::read_to_string(&manifest).context("FFmpeg did not create playlist")?;
        if !playlist.starts_with("#EXTM3U\n") || !playlist.contains("#EXT-X-ENDLIST") {
            bail!("HLS playlist is incomplete");
        }
        let segments: Vec<_> = playlist.lines().filter(|line| line.ends_with(".ts")).collect();
        if segments.is_empty() {
            bail!("FFmpeg produced no HLS media segments");
        }
        for name in &segments {
            if name.contains('/') || name.contains('\\') ||
                !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.') ||
                !stage.join(name).is_file() {
                bail!("invalid or missing HLS segment");
            }
        }

        let mut probe = Command::new(ffprobe);
        probe.args(["-v", "error", "-show_entries", "stream=codec_name", "-of",
                    "csv=p=0", "-i"]).arg(&manifest);
        probe.stdin(Stdio::null()).stdout(Stdio::from(File::create(stage.join("probe.txt"))?))
            .stderr(Stdio::null());
        run_with_timeout(probe, Duration::from_secs(15))?;
        let names = fs::read_to_string(stage.join("probe.txt"))?;
        if !names.lines().any(|line| line.trim() == "h264")
            || !names.lines().any(|line| line.trim() == "aac") {
            bail!("HLS output did not contain matching H.264 and AAC streams");
        }
        Ok(segments.len())
    })();

    match result {
        Ok(segments) => {
            fs::rename(&stage, &public).context("atomic HLS program publish failed")?;
            Ok(HlsProgramOutput {
                directory: public.clone(),
                playlist: public.join("playlist.m3u8"),
                segments,
            })
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&stage);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ProgramSpec {
        let a = MediaInput { path: PathBuf::from("/trusted/a.mp4"), sha256: [1;32] };
        let b = MediaInput { path: PathBuf::from("/trusted/b.mp4"), sha256: [2;32] };
        ProgramSpec {
            id: "sample-2026".to_owned(),
            primary_video: a.clone(),
            primary_audio: a.clone(),
            primary_audio_start_ms: 0,
            slices: vec![
                ProgramSlice {video:a.clone(), canonical_start_ms:0, source_start_ms:0,
                              duration_ms:3000, alignment:None},
                ProgramSlice {video:b, canonical_start_ms:3000, source_start_ms:3000,
                              duration_ms:3000, alignment:Some(Alignment {
                                  slope:1.0, offset_ms:0.0, source_start_ms:0.0,
                                  source_end_ms:12000.0, max_residual_ms:0.0, evidence_count:3,
                              })},
                ProgramSlice {video:a, canonical_start_ms:6000, source_start_ms:6000,
                              duration_ms:3000, alignment:None},
            ],
            width:640, height:360, fps:24, hls_segment_seconds:3,
        }
    }

    #[test]
    fn video_switch_uses_single_continuous_primary_audio_track() {
        let spec = fixture();
        assert_eq!(spec.validate().unwrap(), 9000);
        let graph = spec.filter_graph(9000);
        assert!(graph.contains("concat=n=3:v=1:a=0[v]"));
        assert!(graph.contains("[3:a:0]aresample"));
        let args = spec.ffmpeg_args(Path::new("/out"), 9000);
        assert!(args.iter().any(|arg| arg == "independent_segments"));
        assert!(args.iter().any(|arg| arg == "libx264"));
    }

    #[test]
    fn rejects_alternate_without_validated_timeline_mapping() {
        let mut spec = fixture();
        spec.slices[1].alignment = None;
        assert!(spec.validate().is_err());
    }

    #[test]
    fn rejects_timeline_edit_and_noncontiguous_segments() {
        let mut spec = fixture();
        spec.slices[1].alignment.as_mut().unwrap().offset_ms = 2000.0;
        assert!(spec.validate().is_err());
        spec = fixture();
        spec.slices[1].canonical_start_ms = 3100;
        assert!(spec.validate().is_err());
    }

    #[test]
    fn output_paths_are_not_derived_from_arbitrary_user_input() {
        let mut spec = fixture();
        spec.id = "../another-program".into();
        assert!(spec.validate().is_err());
        spec.id = "safe".into();
        spec.width = 639;
        assert!(spec.validate().is_err());
    }
}
