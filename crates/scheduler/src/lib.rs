//! A conservative, generation-aware scheduler for torrent piece requests.
//! This crate deliberately does not invent an inaccurate time-to-byte mapping.
//! A media index (e.g. from a real MP4 demuxer) must provide the ranges first.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: u64,
    /// Exclusive end.
    pub end: u64,
}

impl ByteRange {
    pub fn new(start: u64, end: u64) -> Result<Self, &'static str> {
        if start >= end {
            return Err("byte range must be nonempty");
        }
        Ok(Self { start, end })
    }

    pub fn len(&self) -> u64 {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexedSpan {
    pub time_start_ms: u64,
    pub time_end_ms: u64,
    /// Includes all keyframe/preroll bytes needed to decode this span.
    pub bytes: ByteRange,
}

#[derive(Clone, Debug)]
pub struct MediaIndex {
    spans: Vec<IndexedSpan>,
}

impl MediaIndex {
    pub fn new(spans: Vec<IndexedSpan>, file_length: u64) -> Result<Self, &'static str> {
        if spans.is_empty()
            || spans.iter().any(|span| {
                span.time_start_ms >= span.time_end_ms
                    || span.bytes.start >= span.bytes.end
                    || span.bytes.end > file_length
            })
        {
            return Err("invalid media index");
        }
        if spans
            .windows(2)
            .any(|pair| pair[0].time_end_ms > pair[1].time_start_ms)
        {
            return Err("overlapping or unordered time spans");
        }
        Ok(Self { spans })
    }

    /// Returns byte spans provided by the real demuxer; does not estimate
    /// byte offsets using a runtime percentage or constant bitrate.
    pub fn required_ranges(&self, start_ms: u64, end_ms: u64) -> Vec<ByteRange> {
        if start_ms >= end_ms {
            return vec![];
        }
        self.spans
            .iter()
            .filter(|span| span.time_start_ms < end_ms && span.time_end_ms > start_ms)
            .map(|span| span.bytes)
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PieceDemand {
    pub index: u64,
    pub generation: u64,
    pub deadline_ms: u64,
}

pub fn map_to_pieces(
    file_start_in_torrent: u64,
    range: ByteRange,
    piece_len: u64,
    generation: u64,
    deadline_ms: u64,
) -> Result<Vec<PieceDemand>, &'static str> {
    if piece_len == 0 {
        return Err("torrent piece length must be positive");
    }
    let first = file_start_in_torrent
        .checked_add(range.start)
        .ok_or("file offset overflow")?;
    let end = file_start_in_torrent
        .checked_add(range.end)
        .ok_or("file offset overflow")?;
    if first >= end {
        return Err("empty demand");
    }
    let start_piece = first / piece_len;
    let end_piece = (end - 1) / piece_len;
    if end_piece - start_piece > 4096 {
        return Err("piece request window exceeds safety cap");
    }
    Ok((start_piece..=end_piece)
        .map(|index| PieceDemand {
            index,
            generation,
            deadline_ms,
        })
        .collect())
}

/// Parser for one RFC 7233-style "bytes=start-end" request.
/// Multiple/suffix ranges are explicitly unsupported rather than silently misread.
pub fn parse_http_range(header: &str, length: u64) -> Result<ByteRange, &'static str> {
    let data = header
        .strip_prefix("bytes=")
        .ok_or("unsupported range unit")?;
    if data.contains(',') {
        return Err("multipart byte ranges are unsupported");
    }
    let (start, end) = data.split_once('-').ok_or("invalid range")?;
    if start.is_empty() || length == 0 {
        return Err("suffix byte ranges unsupported");
    }
    let start = start.parse::<u64>().map_err(|_| "invalid range start")?;
    let last_inclusive = if end.is_empty() {
        length - 1
    } else {
        end.parse::<u64>().map_err(|_| "invalid range end")?
    };
    if start >= length || start > last_inclusive {
        return Err("unsatisfiable range");
    }
    ByteRange::new(start, (last_inclusive.min(length - 1)) + 1)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Created,
    Buffering,
    Playing,
    Stalled,
    Seeking,
    Ended,
    Failed,
}

#[derive(Clone, Debug)]
pub struct PlaybackCursor {
    pub generation: u64,
    pub time_ms: u64,
    pub state: PlaybackState,
}

impl Default for PlaybackCursor {
    fn default() -> Self {
        Self {
            generation: 0,
            time_ms: 0,
            state: PlaybackState::Created,
        }
    }
}

impl PlaybackCursor {
    pub fn seek(&mut self, position_ms: u64) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.time_ms = position_ms;
        self.state = PlaybackState::Seeking;
        self.generation
    }

    pub fn accepts_result(&self, generation: u64) -> bool {
        self.generation == generation && self.state != PlaybackState::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_file_boundary_and_inclusive_last_piece() {
        let range = ByteRange::new(3, 17).unwrap();
        let demands = map_to_pieces(14, range, 8, 5, 2000).unwrap();
        assert_eq!(
            demands.iter().map(|p| p.index).collect::<Vec<_>>(),
            vec![2, 3]
        );
        assert!(demands.iter().all(|p| p.generation == 5));
    }

    #[test]
    fn exact_end_does_not_request_next_piece() {
        let demands = map_to_pieces(0, ByteRange::new(0, 16).unwrap(), 8, 0, 100).unwrap();
        assert_eq!(demands.len(), 2);
    }

    #[test]
    fn no_linear_time_to_byte_assumption() {
        let index = MediaIndex::new(
            vec![
                IndexedSpan {
                    time_start_ms: 0,
                    time_end_ms: 1000,
                    bytes: ByteRange::new(0, 90).unwrap(),
                },
                IndexedSpan {
                    time_start_ms: 1000,
                    time_end_ms: 2000,
                    bytes: ByteRange::new(90, 1000).unwrap(),
                },
            ],
            1000,
        )
        .unwrap();
        assert_eq!(
            index.required_ranges(1100, 1200),
            vec![ByteRange::new(90, 1000).unwrap()]
        );
    }

    #[test]
    fn http_range_inclusive_end_and_bounds() {
        assert_eq!(
            parse_http_range("bytes=10-19", 100).unwrap(),
            ByteRange::new(10, 20).unwrap()
        );
        assert_eq!(
            parse_http_range("bytes=95-", 100).unwrap(),
            ByteRange::new(95, 100).unwrap()
        );
        assert!(parse_http_range("bytes=-5", 100).is_err());
        assert!(parse_http_range("bytes=3-2", 100).is_err());
        assert!(parse_http_range("bytes=1-4,5-8", 100).is_err());
    }

    #[test]
    fn seek_invalidates_stale_completed_segments() {
        let mut cursor = PlaybackCursor::default();
        assert!(cursor.accepts_result(0));
        assert_eq!(cursor.seek(42_000), 1);
        assert!(!cursor.accepts_result(0));
        assert!(cursor.accepts_result(1));
    }
}
