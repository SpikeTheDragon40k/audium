//! Partial content hash for detecting moved or renamed files: blake3 over
//! the file size, the first 64 KiB and the last 64 KiB. Never the whole file.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub const CHUNK: u64 = 64 * 1024;

pub fn partial_hash(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());

    let mut buf = vec![0u8; CHUNK as usize];
    let head = read_up_to(&mut file, &mut buf)?;
    hasher.update(&buf[..head]);
    if size > CHUNK {
        let tail_start = size.saturating_sub(CHUNK).max(CHUNK);
        file.seek(SeekFrom::Start(tail_start))?;
        let tail = read_up_to(&mut file, &mut buf)?;
        hasher.update(&buf[..tail]);
    }
    Ok(hasher.finalize().as_bytes()[..16].to_vec())
}

fn read_up_to(file: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_content_same_hash_regardless_of_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.path().join("a.mp3"), &data).expect("write");
        std::fs::write(dir.path().join("b.mp3"), &data).expect("write");
        let mut other = data.clone();
        *other.last_mut().expect("non-empty") ^= 1;
        std::fs::write(dir.path().join("c.mp3"), &other).expect("write");
        std::fs::write(dir.path().join("small.mp3"), b"tiny").expect("write");

        let h = |n: &str| partial_hash(&dir.path().join(n)).expect("hash");
        assert_eq!(h("a.mp3"), h("b.mp3"));
        assert_ne!(h("a.mp3"), h("c.mp3"), "tail change is detected");
        assert_eq!(h("small.mp3").len(), 16);
        assert!(partial_hash(&dir.path().join("missing")).is_err());
    }
}
