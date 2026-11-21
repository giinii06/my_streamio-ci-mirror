//! Coarse video fingerprints for alignment of *different encodes* of
//! a legally possessed reference clip. Not a torrent piece mixer.
//!
//! Low-information frames and ambiguous matches are deliberately rejected.
//! The caller must still verify editing differences and check the fit using
//! the crate's `align` function before permitting a source switch.

use anyhow::{anyhow, bail, Context, Result};
use std::{
    collections::HashSet,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::Anchor;

const GRID: usize = 16;
const PIXELS: usize = GRID * GRID;
const SIGNATURE_BYTES: usize = PIXELS / 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameSignature([u8; SIGNATURE_BYTES]);

impl FrameSignature {
    /// Perceptual average hash over a 16x16 grayscale frame. Only coarse
    /// visual similarity is claimed, not same-scene proof.
    pub fn from_grayscale(frame: &[u8]) -> Result<Self> {
        if frame.len() != PIXELS {
            bail!("fingerprint input must contain exactly 256 grayscale pixels");
        }
        let min = frame.iter().copied().min().unwrap_or(0);
        let max = frame.iter().copied().max().unwrap_or(0);
        if max.saturating_sub(min) < 12 {
            bail!("low-information frame: ambiguous black/fade sequence");
        }
        let avg = frame.iter().map(|p| u32::from(*p)).sum::<u32>() / PIXELS as u32;
        let mut bits = [0_u8; SIGNATURE_BYTES];
        for (index, pixel) in frame.iter().enumerate() {
            if u32::from(*pixel) >= avg {
                bits[index / 8] |= 0x80_u8 >> (index % 8);
            }
        }
        Ok(Self(bits))
    }

    pub fn distance(&self, other: &Self) -> u32 {
        self.0.iter().zip(other.0.iter())
            .map(|(a, b)| (a ^ b).count_ones())
            .sum()
    }
}

#[derive(Clone, Debug)]
pub struct VideoFingerprint {
    pub time_ms: u64,
    pub signature: FrameSignature,
}

/// Extract one small decoded video frame from a local (already validated)
/// media file. Fixed FFmpeg arguments are passed directly, never via a shell.
/// An external timeout kills a wedged decoder.
pub fn fingerprint_frame(
    ffmpeg: &Path,
    source: &Path,
    at_ms: u64,
    timeout: Duration,
) -> Result<VideoFingerprint> {
    if timeout.is_zero() || timeout > Duration::from_secs(30) {
        bail!("decoder timeout must be between zero and thirty seconds");
    }
    if !source.is_file() {
        bail!("source is not an ordinary local file");
    }
    let mut child = Command::new(ffmpeg)
        .args([
            "-hide_banner", "-nostdin", "-loglevel", "error", "-ss",
            &format!("{:.3}", at_ms as f64 / 1000.0), "-i",
        ])
        .arg(source)
        .args([
            "-map", "0:v:0", "-frames:v", "1", "-vf",
            "scale=16:16:flags=area,format=gray",
            "-c:v", "rawvideo", "-f", "rawvideo", "pipe:1",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .context("failed to start FFmpeg")?;

    let start = Instant::now();
    let exit = loop {
        if let Some(exit) = child.try_wait().context("cannot poll FFmpeg")? {
            break exit;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("FFmpeg frame extraction timed out");
        }
        thread::sleep(Duration::from_millis(25));
    };
    let output = child.wait_with_output().context("cannot read decoded frame")?;
    if !exit.success() {
        bail!("FFmpeg failed to decode requested video frame");
    }
    if output.stdout.len() != PIXELS {
        bail!("FFmpeg returned an incomplete or unexpectedly sized frame");
    }
    let signature = FrameSignature::from_grayscale(&output.stdout)?;
    Ok(VideoFingerprint { time_ms: at_ms, signature })
}

/// Match sample fingerprints that have a unique, clearly better candidate.
/// Frames occurring multiple times are excluded from alignment evidence.
/// Returns anchor hints, NOT an approval to stitch video.
pub fn find_unique_anchors(
    reference: &[VideoFingerprint],
    other: &[VideoFingerprint],
    max_hamming: u32,
    ambiguity_gap: u32,
) -> Vec<Anchor> {
    if max_hamming > 25 || ambiguity_gap == 0 {
        return vec![];
    }
    let mut matches = vec![];
    let mut already_matched = HashSet::<usize>::new();
    let mut last_reference_ms = None;
    let mut other_sorted: Vec<_> = other.iter().collect();
    other_sorted.sort_by_key(|frame| frame.time_ms);

    for frame in other_sorted {
        let mut ranked: Vec<_> = reference.iter().enumerate()
            .map(|(i, ref_frame)| (i, frame.signature.distance(&ref_frame.signature)))
            .collect();
        ranked.sort_by_key(|(i, distance)| (*distance, *i));
        let Some(&(ref_index, distance)) = ranked.first() else { continue };
        if distance > max_hamming {
            continue;
        }
        if ranked.get(1).is_some_and(|(_, runner_up)| {
            runner_up.saturating_sub(distance) < ambiguity_gap
        }) {
            continue;
        }
        let ref_ms = reference[ref_index].time_ms;
        if last_reference_ms.is_some_and(|prior| ref_ms <= prior)
            || !already_matched.insert(ref_index)
        {
            continue;
        }
        last_reference_ms = Some(ref_ms);
        matches.push(Anchor {
            source_ms: frame.time_ms as f64,
            reference_ms: ref_ms as f64,
            confidence: 1.0 - f64::from(distance) / PIXELS as f64,
        });
    }
    matches
}

/// First-pass offline alignment from multiple independent frame samples.
/// Reject low confidence and edits instead of silently guessing continuity.
pub fn alignment_from_frames(
    reference: &[VideoFingerprint],
    other: &[VideoFingerprint],
) -> Result<crate::Alignment> {
    let anchors = find_unique_anchors(reference, other, 20, 5);
    if anchors.len() < 3 {
        return Err(anyhow!("not enough unique visual matches to establish alignment"));
    }
    crate::align(&anchors, 40.0).map_err(|reason| anyhow!("{reason}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(seed: u8, offset: u64) -> VideoFingerprint {
        let pixels: Vec<u8> = (0..PIXELS)
            .map(|i| ((i as u32 * u32::from(seed) + (i * i) as u32) % 256) as u8)
            .collect();
        VideoFingerprint {
            time_ms: offset,
            signature: FrameSignature::from_grayscale(&pixels).unwrap(),
        }
    }

    #[test]
    fn low_information_and_invalid_frame_lengths_fail_closed() {
        assert!(FrameSignature::from_grayscale(&[0_u8; 256]).is_err());
        assert!(FrameSignature::from_grayscale(&[0_u8; 255]).is_err());
    }

    #[test]
    fn frame_similarity_counts_bit_flips() {
        let original = fingerprint(19, 0);
        let alternate = fingerprint(19, 9000);
        assert_eq!(original.signature.distance(&alternate.signature), 0);
    }

    #[test]
    fn anchors_support_constant_offset_without_mixing_torrent_bytes() {
        let reference = vec![fingerprint(11, 0), fingerprint(19, 10000), fingerprint(31, 20000)];
        let other = vec![fingerprint(11, 350), fingerprint(19, 10350), fingerprint(31, 20350)];
        let aligned = alignment_from_frames(&reference, &other).unwrap();
        assert!((aligned.offset_ms + 350.0).abs() < 1.0);
        assert_eq!(aligned.evidence_count, 3);
    }

    #[test]
    fn repeated_frames_are_not_alignment_evidence() {
        let repeated = fingerprint(17, 0);
        let reference = vec![
            repeated.clone(),
            VideoFingerprint { time_ms: 10_000, signature: repeated.signature },
            fingerprint(29, 20_000),
        ];
        let other = vec![fingerprint(17, 0), fingerprint(29, 20_000)];
        assert!(find_unique_anchors(&reference, &other, 20, 5).len() < 2);
    }

    #[test]
    fn edited_cut_is_rejected_as_unaligned() {
        let reference = vec![fingerprint(11, 0), fingerprint(19, 10000), fingerprint(31, 20000)];
        let changed = vec![fingerprint(11, 0), fingerprint(19, 10000), fingerprint(31, 25000)];
        assert!(alignment_from_frames(&reference, &changed).is_err());
    }
}
