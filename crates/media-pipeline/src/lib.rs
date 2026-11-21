//! Safely turn an already verified media file into an immutable video segment.
//! This MVP worker produces H.264/1080p/24fps video only. It does NOT splice
//! audio, prove scene identity, or transform an incomplete torrent file itself.

pub mod hardware;
pub mod planner;

use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const MAX_SEGMENT_MS: u64 = 10_000;
const TARGET_WIDTH: u16 = 1920;
const TARGET_HEIGHT: u16 = 1080;
const TARGET_FPS: u16 = 24;

#[derive(Clone, Debug)]
pub struct SegmentJob {
    pub id: String,
    /// Path inside approved storage root.
    pub source: PathBuf,
    pub start_ms: u64,
    pub duration_ms: u64,
}

impl SegmentJob {
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty()
            || self.id.len() > 80
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            bail!("invalid segment id");
        }
        if self.duration_ms == 0 || self.duration_ms > MAX_SEGMENT_MS {
            bail!("segment duration outside allowed bounds");
        }
        self.start_ms
            .checked_add(self.duration_ms)
            .context("segment time overflow")?;
        Ok(())
    }

    pub fn ffmpeg_args(&self, tmp_output: &Path) -> Result<Vec<String>> {
        self.validate()?;
        Ok(vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-nostdin".into(),
            "-y".into(),
            "-ss".into(),
            format!("{:.3}", self.start_ms as f64 / 1000.0),
            "-i".into(),
            self.source.display().to_string(),
            "-t".into(),
            format!("{:.3}", self.duration_ms as f64 / 1000.0),
            "-map".into(),
            "0:v:0".into(),
            "-an".into(),
            "-vf".into(),
            format!(
                "scale={TARGET_WIDTH}:{TARGET_HEIGHT}:flags=lanczos,fps={TARGET_FPS},setpts=PTS-STARTPTS"
            ),
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            "veryfast".into(),
            "-pix_fmt".into(),
            "yuv420p".into(),
            "-g".into(),
            "48".into(),
            "-sc_threshold".into(),
            "0".into(),
            "-movflags".into(),
            "+faststart".into(),
            "-f".into(),
            "mp4".into(),
            tmp_output.display().to_string(),
        ])
    }
}

/// Execute FFmpeg with direct OS argument passing (no shell), within an
/// approved storage root. Segments are published only after ffprobe succeeds.
pub fn render_video_segment(
    job: &SegmentJob,
    storage_root: &Path,
    ffmpeg: &Path,
    ffprobe: &Path,
) -> Result<PathBuf> {
    job.validate()?;

    let root = storage_root
        .canonicalize()
        .context("storage root must exist")?;
    let source = job.source.canonicalize().context("source must exist")?;
    if !source.starts_with(&root) || !source.is_file() {
        bail!("source must be a regular file within approved storage");
    }

    let output_dir = root.join("segments");
    fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join(format!("{}.mp4", job.id));
    if output_path.exists() {
        bail!("segment has already been published");
    }

    let lock_dir = output_dir.join(format!(".{}.partial", job.id));
    fs::create_dir(&lock_dir).context("segment already in progress or stale lock")?;
    let tmp_path = lock_dir.join("video.mp4");

    let worker = SegmentJob {
        id: job.id.clone(),
        source,
        start_ms: job.start_ms,
        duration_ms: job.duration_ms,
    };

    let result = (|| -> Result<()> {
        let output = Command::new(ffmpeg)
            .args(worker.ffmpeg_args(&tmp_path)?)
            .output()
            .context("FFmpeg process could not be started")?;
        if !output.status.success() {
            bail!("FFmpeg returned nonzero status");
        }
        if !tmp_path.is_file() || tmp_path.metadata()?.len() == 0 {
            bail!("FFmpeg did not produce a nonempty segment");
        }
        let probe = Command::new(ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=codec_name",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(&tmp_path)
            .output()
            .context("ffprobe not available")?;
        if !probe.status.success() || String::from_utf8_lossy(&probe.stdout).trim() != "h264" {
            bail!("segment verification failed");
        }
        fs::rename(&tmp_path, &output_path).context("atomic segment publication failed")?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_dir_all(&lock_dir);
        return Err(error);
    }
    fs::remove_dir(&lock_dir)?;
    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> SegmentJob {
        SegmentJob {
            id: "seg-1".to_string(),
            source: PathBuf::from("/approved/source.mp4"),
            start_ms: 2_000,
            duration_ms: 4_000,
        }
    }

    #[test]
    fn keeps_all_user_supplied_paths_as_process_arguments() {
        let args = job().ffmpeg_args(Path::new("/tmp/segment.mp4")).unwrap();
        assert_eq!(args[args.len() - 1], "/tmp/segment.mp4");
        assert!(args
            .windows(2)
            .any(|pair| pair == ["-i", "/approved/source.mp4"]));
        assert!(args.contains(&"-an".to_owned()));
    }

    #[test]
    fn prevents_path_like_ids_and_unbounded_work() {
        let mut test = job();
        test.id = "../other".into();
        assert!(test.validate().is_err());
        test.id = "valid".into();
        test.duration_ms = 0;
        assert!(test.validate().is_err());
        test.duration_ms = 60_000;
        assert!(test.validate().is_err());
        test.duration_ms = 4_000;
        test.start_ms = u64::MAX;
        assert!(test.validate().is_err());
    }
}
