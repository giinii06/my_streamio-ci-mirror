//! Strict, in-memory whole-file identity proof and target piece re-verification.
//! Initial v1/single-file scope: multi-file boundary, sparse piece data and BEP52
//! Merkle verification must be implemented before using this for those torrents.

use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc};

pub type FileDigest = [u8; 32];
pub type PieceHash = [u8; 20];

#[derive(Clone)]
pub struct VerifiedObject {
    digest: FileDigest,
    data: Arc<[u8]>,
}

impl VerifiedObject {
    /// A complete file is trusted only after checking the source's exact piece
    /// hashes. This entry point supports single-file BEP3 torrents only.
    pub fn from_single_file_v1(
        bytes: Vec<u8>,
        piece_size: usize,
        hashes: &[PieceHash],
    ) -> Result<Self, &'static str> {
        if bytes.is_empty() || piece_size == 0 {
            return Err("empty file or invalid piece size");
        }
        let expected_count = 1 + (bytes.len() - 1) / piece_size;
        if hashes.len() != expected_count {
            return Err("piece hash count mismatch");
        }
        for (slice, hash) in bytes.chunks(piece_size).zip(hashes.iter()) {
            let computed: PieceHash = Sha1::digest(slice).into();
            if slice.is_empty() || computed != *hash {
                return Err("source piece failed SHA-1 verification");
            }
        }
        let digest = Sha256::digest(&bytes).into();
        Ok(Self {
            digest,
            data: Arc::from(bytes),
        })
    }

    pub fn digest(&self) -> FileDigest {
        self.digest
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Revalidate against the *target torrent* piece hash (different Infohash
    /// or piece length). A successful source proof alone is not sufficient.
    pub fn verified_target_piece(
        &self,
        index: usize,
        piece_size: usize,
        target_hash: PieceHash,
    ) -> Result<Vec<u8>, &'static str> {
        if piece_size == 0 {
            return Err("invalid target piece size");
        }
        let start = index.checked_mul(piece_size).ok_or("piece offset overflow")?;
        if start >= self.len() {
            return Err("piece outside file");
        }
        let end = start.saturating_add(piece_size).min(self.len());
        let bytes = &self.data[start..end];
        let computed: PieceHash = Sha1::digest(bytes).into();
        if computed != target_hash {
            return Err("target piece SHA-1 mismatch");
        }
        Ok(bytes.to_vec())
    }
}

#[derive(Default)]
pub struct VerifiedStore {
    objects: HashMap<FileDigest, VerifiedObject>,
}

impl VerifiedStore {
    pub fn insert(&mut self, object: VerifiedObject) -> FileDigest {
        let digest = object.digest();
        self.objects.entry(digest).or_insert(object);
        digest
    }

    pub fn get(&self, digest: &FileDigest) -> Option<&VerifiedObject> {
        self.objects.get(digest)
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece_hashes(data: &[u8], piece_len: usize) -> Vec<PieceHash> {
        data.chunks(piece_len)
            .map(|c| Sha1::digest(c).into())
            .collect()
    }

    #[test]
    fn accepts_source_and_rechecks_different_target_piece_size() {
        let bytes = b"ABCDEFGHIJ".to_vec();
        let source = VerifiedObject::from_single_file_v1(
            bytes.clone(),
            4,
            &piece_hashes(&bytes, 4),
        )
        .unwrap();
        let alternate_hashes = piece_hashes(&bytes, 6);
        assert_eq!(source.verified_target_piece(0, 6, alternate_hashes[0]).unwrap().as_slice(), b"ABCDEF");
        assert_eq!(source.verified_target_piece(1, 6, alternate_hashes[1]).unwrap().as_slice(), b"GHIJ");
        let mut store = VerifiedStore::default();
        let id = store.insert(source.clone());
        store.insert(source);
        assert_eq!(store.len(), 1);
        assert_eq!(store.get(&id).unwrap().len(), 10);
    }

    #[test]
    fn refuses_unverified_source_and_target_corruption() {
        let data = b"abcdefghij".to_vec();
        let mut source_hashes = piece_hashes(&data, 4);
        source_hashes[1][0] ^= 1;
        assert!(VerifiedObject::from_single_file_v1(data.clone(), 4, &source_hashes).is_err());

        let source =
            VerifiedObject::from_single_file_v1(data.clone(), 4, &piece_hashes(&data, 4))
                .unwrap();
        assert!(source.verified_target_piece(0, 6, [0; 20]).is_err());
        assert!(source.verified_target_piece(2, 6, [0; 20]).is_err());
    }

    #[test]
    fn rejects_overflow_and_empty_files() {
        assert!(VerifiedObject::from_single_file_v1(vec![], 4, &[]).is_err());
        let data = b"x".to_vec();
        let object =
            VerifiedObject::from_single_file_v1(data.clone(), 1, &piece_hashes(&data, 1))
                .unwrap();
        assert!(object.verified_target_piece(usize::MAX, 4, [0; 20]).is_err());
    }
}
