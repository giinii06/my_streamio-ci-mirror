//! FFmpeg/VideoToolbox capability detection. Listed encoders do not prove
//! that the GPU can sustain realtime encode; live benchmarks are mandatory.

use anyhow::{bail, Context, Result};
use std::{path::Path, process::Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoEncoder {
    Libx264,
    H264VideoToolbox,
}

impl VideoEncoder {
    pub fn name(self) -> &'static str {
        match self {
            Self::Libx264 => "libx264",
            Self::H264VideoToolbox => "h264_videotoolbox",
        }
    }
}

pub fn choose_encoder(is_macos: bool, prefer_hardware: bool, encoders: &str) -> VideoEncoder {
    if is_macos
        && prefer_hardware
        && encoders.lines().any(|line| {
            line.split_whitespace()
                .any(|word| word == "h264_videotoolbox")
        })
    {
        VideoEncoder::H264VideoToolbox
    } else {
        VideoEncoder::Libx264
    }
}

/// Uses a local trusted FFmpeg executable; do not pass an arbitrary executable
/// path supplied by an unauthenticated API request.
pub fn probe_ffmpeg_encoder(ffmpeg: &Path, prefer_hardware: bool) -> Result<VideoEncoder> {
    let result = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()
        .context("could not query FFmpeg encoders")?;
    if !result.status.success() {
        bail!("FFmpeg encoder probe failed");
    }
    let encoders = String::from_utf8_lossy(&result.stdout);
    Ok(choose_encoder(
        cfg!(target_os = "macos"),
        prefer_hardware,
        &encoders,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_hardware_only_on_mac_when_encoder_is_listed() {
        let sample = " V..... libx264\n V..... h264_videotoolbox\n";
        assert_eq!(
            choose_encoder(true, true, sample),
            VideoEncoder::H264VideoToolbox
        );
        assert_eq!(choose_encoder(false, true, sample), VideoEncoder::Libx264);
        assert_eq!(choose_encoder(true, false, sample), VideoEncoder::Libx264);
        assert_eq!(
            choose_encoder(true, true, " V..... libx264"),
            VideoEncoder::Libx264
        );
    }

    #[test]
    fn does_not_match_similarly_named_encoder() {
        assert_eq!(
            choose_encoder(true, true, " V..... h264_videotoolbox_fake"),
            VideoEncoder::Libx264
        );
    }
}
