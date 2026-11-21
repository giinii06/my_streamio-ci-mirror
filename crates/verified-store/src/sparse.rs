//! Bounded in-memory reuse of independently piece-verified **single-file v1**
//! torrent bytes. A target swarm is NOT assumed to contain the same file:
//! clients must first prove a whole-file identity, then rehash each target
//! piece against the target torrent's authenticated piece hashes.

use crate::{PieceHash, VerifiedObject};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProvenFileIdentity {
    pub digest: [u8; 32],
    pub length: u64,
}

impl ProvenFileIdentity {
    /// The only proof this module currently accepts is one created from a
    /// fully verified v1 single-file object. BEP52 file roots need a distinct
    /// verified Merkle-proof constructor in a future phase.
    pub fn from_whole_file(file: &VerifiedObject) -> Self {
        Self {
            digest: file.digest(),
            length: file.len() as u64,
        }
    }
}

pub struct SparseVerifiedFile {
    identity: ProvenFileIdentity,
    spans: BTreeMap<u64, Vec<u8>>,
    bytes_in_memory: usize,
    memory_limit: usize,
}

impl SparseVerifiedFile {
    pub fn new(identity: ProvenFileIdentity, memory_limit: usize) -> Result<Self, &'static str> {
        if identity.length == 0 || memory_limit == 0 {
            return Err("empty file identity or zero memory budget");
        }
        Ok(Self {
            identity,
            spans: BTreeMap::new(),
            bytes_in_memory: 0,
            memory_limit,
        })
    }

    pub fn identity(&self) -> ProvenFileIdentity {
        self.identity
    }

    pub fn bytes_in_memory(&self) -> usize {
        self.bytes_in_memory
    }

    /// Return false for an exact, already stored piece and true when inserted.
    /// For v1 multifile torrents, hashes may span neighboring files; those
    /// pieces MUST NOT be sent here without reconstructing the full torrent
    /// piece first. This implementation deliberately requires file-offset-0.
    pub fn insert_verified_source_v1_piece(
        &mut self,
        piece_index: u64,
        source_piece_length: u64,
        data: Vec<u8>,
        expected_sha1: PieceHash,
    ) -> Result<bool, &'static str> {
        if source_piece_length == 0 {
            return Err("source piece length must be positive");
        }
        let start = piece_index
            .checked_mul(source_piece_length)
            .ok_or("source piece offset overflow")?;
        if start >= self.identity.length {
            return Err("source piece outside proven file");
        }
        let full_length = source_piece_length.min(self.identity.length - start);
        if data.len() as u64 != full_length {
            return Err("incomplete source piece must not be stored");
        }
        let actual: PieceHash = Sha1::digest(&data).into();
        if actual != expected_sha1 {
            return Err("source torrent piece SHA-1 mismatch");
        }
        let end = start.checked_add(full_length).ok_or("source piece end overflow")?;
        for (&offset, other) in self.spans.range(..end) {
            let other_end = offset.saturating_add(other.len() as u64);
            if other_end > start {
                if offset == start && other.as_slice() == data.as_slice() {
                    return Ok(false);
                }
                return Err("conflicting or overlapping verified source spans");
            }
        }
        let future_bytes = self.bytes_in_memory.checked_add(data.len())
            .ok_or("sparse memory accounting overflow")?;
        if future_bytes > self.memory_limit {
            return Err("sparse verified cache memory budget exceeded");
        }
        self.spans.insert(start, data);
        self.bytes_in_memory = future_bytes;
        Ok(true)
    }

    /// Only returns bytes if all requested target-piece bytes are present AND
    /// the target's own SHA-1 piece hash matches. A missing span is Ok(None)
    /// rather than an unverified partial read or zero-filled output.
    pub fn read_target_v1_piece(
        &self,
        index: u64,
        target_piece_length: u64,
        target_sha1: PieceHash,
    ) -> Result<Option<Vec<u8>>, &'static str> {
        if target_piece_length == 0 {
            return Err("target piece length must be positive");
        }
        let start = index.checked_mul(target_piece_length)
            .ok_or("target piece offset overflow")?;
        if start >= self.identity.length {
            return Err("target piece outside proven file");
        }
        let end = start.checked_add(target_piece_length)
            .ok_or("target piece end overflow")?.min(self.identity.length);
        let mut position = start;
        let mut result = Vec::new();
        while position < end {
            let Some((&offset, bytes)) = self.spans.range(..=position).next_back() else {
                return Ok(None);
            };
            let span_end = offset.saturating_add(bytes.len() as u64);
            if position >= span_end {
                return Ok(None);
            }
            let from = usize::try_from(position - offset)
                .map_err(|_| "sparse read offset overflow")?;
            let take = usize::try_from((end - position).min(span_end - position))
                .map_err(|_| "sparse read length overflow")?;
            result.extend_from_slice(&bytes[from..from + take]);
            position += take as u64;
        }
        let actual: PieceHash = Sha1::digest(&result).into();
        if actual != target_sha1 {
            return Err("target torrent piece SHA-1 mismatch");
        }
        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VerifiedObject;

    fn hash(data: &[u8]) -> PieceHash {
        Sha1::digest(data).into()
    }

    fn fixture() -> (Vec<u8>, ProvenFileIdentity) {
        let data = b"abcdefghij".to_vec();
        let source_hashes: Vec<PieceHash> = data.chunks(4).map(hash).collect();
        let object = VerifiedObject::from_single_file_v1(data.clone(), 4, &source_hashes).unwrap();
        (data, ProvenFileIdentity::from_whole_file(&object))
    }

    #[test]
    fn identical_file_can_supply_alternate_piece_boundaries() {
        let (data, proof) = fixture();
        let mut sparse = SparseVerifiedFile::new(proof, 64).unwrap();
        for (index, chunk) in data.chunks(4).enumerate() {
            assert!(sparse.insert_verified_source_v1_piece(
                index as u64, 4, chunk.to_vec(), hash(chunk)
            ).unwrap());
        }
        assert_eq!(sparse.bytes_in_memory(), 10);
        assert_eq!(sparse.read_target_v1_piece(0, 6, hash(&data[..6])).unwrap(), Some(data[..6].to_vec()));
        assert_eq!(sparse.read_target_v1_piece(1, 6, hash(&data[6..])).unwrap(), Some(data[6..].to_vec()));
    }

    #[test]
    fn gaps_do_not_return_false_verified_bytes() {
        let (data, proof) = fixture();
        let mut sparse = SparseVerifiedFile::new(proof, 64).unwrap();
        sparse.insert_verified_source_v1_piece(0, 4, data[..4].to_vec(), hash(&data[..4])).unwrap();
        assert_eq!(sparse.read_target_v1_piece(0, 6, hash(&data[..6])).unwrap(), None);
        assert!(sparse.read_target_v1_piece(0, 6, [0;20]).unwrap().is_none());
    }

    #[test]
    fn corrupt_pieces_overlaps_and_target_hash_mismatches_fail() {
        let (data, proof) = fixture();
        let mut sparse = SparseVerifiedFile::new(proof, 64).unwrap();
        assert!(sparse.insert_verified_source_v1_piece(0, 4, data[..4].to_vec(), [0;20]).is_err());
        assert!(sparse.insert_verified_source_v1_piece(0, 4, data[..4].to_vec(), hash(&data[..4])).unwrap());
        assert!(!sparse.insert_verified_source_v1_piece(0, 4, data[..4].to_vec(), hash(&data[..4])).unwrap());
        assert!(sparse.insert_verified_source_v1_piece(0, 6, data[..6].to_vec(), hash(&data[..6])).is_err());
        for (index, chunk) in data.chunks(4).enumerate().skip(1) {
            sparse.insert_verified_source_v1_piece(index as u64, 4, chunk.to_vec(), hash(chunk)).unwrap();
        }
        assert!(sparse.read_target_v1_piece(0, 6, [0;20]).is_err());
    }

    #[test]
    fn enforces_memory_limit_and_validates_piece_boundaries() {
        let (data, proof) = fixture();
        let mut sparse = SparseVerifiedFile::new(proof, 5).unwrap();
        sparse.insert_verified_source_v1_piece(0, 4, data[..4].to_vec(), hash(&data[..4])).unwrap();
        assert!(sparse.insert_verified_source_v1_piece(1, 4, data[4..8].to_vec(), hash(&data[4..8])).is_err());
        assert_eq!(sparse.bytes_in_memory(), 4);
        assert!(sparse.insert_verified_source_v1_piece(2, 4, data[8..].to_vec(), hash(&data[8..])).is_err());
        assert!(sparse.read_target_v1_piece(u64::MAX, 2, [0;20]).is_err());
    }
}
