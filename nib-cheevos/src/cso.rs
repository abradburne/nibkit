//! CSO disc images (a PSP game's UMD, compressed: "CISO") for rhash (feature `cso`): a CD
//! reader of rhash's shape, `rc_hash_cdreader_t`, which inflates the image's blocks. rhash then
//! hashes a CSO as it hashes the same disc as an ISO: its one track of 2048-byte sectors.
//!
//! The format (version 0 and 1): a 24-byte header, `CISO`, the header's size (u32), the
//! uncompressed size (u64), the block size (u32, 2048 for a UMD), the version (u8) and the
//! index's shift (u8), two bytes unused; then one u32 a block and one more, each the block's
//! offset in the file shifted right by the index's shift, its top bit set when the block is
//! stored as it is rather than deflated (raw deflate, no zlib header). A block's bytes run to
//! the next one's offset. All little-endian.
//!
//! Not here: version 2 (blocks that may be LZ4) and ZSO (`ZISO`, LZ4 throughout).

use std::ffi::{c_char, c_void, CStr};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// A UMD's sector.
const SECTOR: usize = 2048;
/// The largest block taken: a UMD's are 2048 bytes; anything far larger isn't a disc image.
const MOST_BLOCK: u32 = 1 << 20;
/// rhash's special track numbers (`RC_HASH_CDTRACK_*`): the first data, last and largest.
const LARGEST: u32 = u32::MAX - 2;

/// The header's facts.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Header {
    total: u64,
    block: u32,
    shift: u8,
}

impl Header {
    fn parse(bytes: &[u8; 24]) -> Option<Header> {
        if &bytes[..4] != b"CISO" || bytes[20] > 1 {
            return None;
        }
        let total = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let block = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
        if block == 0
            || block > MOST_BLOCK
            || !(block as usize).is_multiple_of(SECTOR)
            || total == 0
        {
            return None;
        }
        Some(Header {
            total,
            block,
            shift: bytes[21].min(31),
        })
    }

    fn blocks(&self) -> u64 {
        self.total.div_ceil(u64::from(self.block))
    }
}

/// An open image: the file, its index, and the last block inflated.
struct Image {
    file: File,
    header: Header,
    index: Vec<u32>,
    block: Option<u32>,
    data: Vec<u8>,
    compressed: Vec<u8>,
}

impl Image {
    fn open(path: &str, track: u32) -> Option<Image> {
        // One track: the first, or one of rhash's special numbers for it.
        if track != 1 && track < LARGEST {
            return None;
        }
        let mut file = File::open(path).ok()?;
        let mut head = [0u8; 24];
        file.read_exact(&mut head).ok()?;
        let header = Header::parse(&head)?;
        let entries = usize::try_from(header.blocks().checked_add(1)?).ok()?;
        // At most a UMD's worth of index (a dual-layer disc's 8.5 GB in 2 KiB blocks).
        if entries > 1 << 23 {
            return None;
        }
        let mut raw = vec![0u8; entries * 4];
        file.read_exact(&mut raw).ok()?;
        let index = raw
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Some(Image {
            data: vec![0; header.block as usize],
            file,
            header,
            index,
            block: None,
            compressed: Vec::new(),
        })
    }

    /// Block `n`, inflated, or `None` when it won't read.
    fn load(&mut self, n: u32) -> Option<()> {
        if self.block == Some(n) {
            return Some(());
        }
        self.block = None;
        let (this, next) = (
            *self.index.get(n as usize)?,
            *self.index.get(n as usize + 1)?,
        );
        let at = u64::from(this & 0x7FFF_FFFF) << self.header.shift;
        let end = u64::from(next & 0x7FFF_FFFF) << self.header.shift;
        let stored = this & 0x8000_0000 != 0;
        let block = self.header.block as usize;
        // A stored block is the block's bytes; a deflated one at most a little more.
        let len = usize::try_from(end.checked_sub(at)?)
            .ok()?
            .min(block + block / 2 + 64);
        self.compressed.resize(len, 0);
        self.file.seek(SeekFrom::Start(at)).ok()?;
        self.file.read_exact(&mut self.compressed).ok()?;
        if stored {
            let n = len.min(block);
            self.data[..n].copy_from_slice(&self.compressed[..n]);
            self.data[n..].fill(0);
        } else {
            let out =
                miniz_oxide::inflate::decompress_to_vec_with_limit(&self.compressed, block).ok()?;
            let n = out.len().min(block);
            self.data[..n].copy_from_slice(&out[..n]);
            self.data[n..].fill(0);
        }
        self.block = Some(n);
        Some(())
    }

    /// Up to `buf.len()` bytes from sector `sector` on: how many.
    fn read(&mut self, sector: u32, buf: &mut [u8]) -> usize {
        let mut at = u64::from(sector) * SECTOR as u64;
        let mut done = 0;
        while done < buf.len() && at < self.header.total {
            let block = u64::from(self.header.block);
            let Ok(n) = u32::try_from(at / block) else {
                break;
            };
            if self.load(n).is_none() {
                break;
            }
            let within = (at % block) as usize;
            let left = (self.header.total - at).min(block - within as u64) as usize;
            let take = left.min(buf.len() - done);
            buf[done..done + take].copy_from_slice(&self.data[within..within + take]);
            done += take;
            at += take as u64;
        }
        done
    }
}

extern "C" fn open_track(path: *const c_char, track: u32) -> *mut c_void {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: rhash passes the image's path, NUL-terminated, valid for the call.
    let path = unsafe { CStr::from_ptr(path) }
        .to_string_lossy()
        .into_owned();
    std::panic::catch_unwind(|| Image::open(&path, track))
        .ok()
        .flatten()
        .map_or(std::ptr::null_mut(), |t| Box::into_raw(Box::new(t)).cast())
}

extern "C" fn read_sector(
    track: *mut c_void,
    sector: u32,
    buffer: *mut c_void,
    bytes: usize,
) -> usize {
    if track.is_null() || buffer.is_null() {
        return 0;
    }
    // SAFETY: `track` is a Box<Image> open_track made and close_track hasn't freed; rhash
    // gives `bytes` writable bytes at `buffer`, one call at a time.
    let (image, buf) = unsafe {
        (
            &mut *track.cast::<Image>(),
            std::slice::from_raw_parts_mut(buffer.cast::<u8>(), bytes),
        )
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| image.read(sector, buf))).unwrap_or(0)
}

extern "C" fn close_track(track: *mut c_void) {
    if !track.is_null() {
        // SAFETY: a Box<Image> open_track made, freed here once.
        drop(unsafe { Box::from_raw(track.cast::<Image>()) });
    }
}

extern "C" fn first_track_sector(_: *mut c_void) -> u32 {
    0
}

/// The reader for the shim.
pub(crate) const READER: crate::ffi::CdReader = crate::ffi::CdReader {
    open_track,
    read_sector,
    close_track,
    first_track_sector,
};

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A CSO of `data` in 2048-byte blocks, the odd ones stored, the even ones deflated, with
    /// index shift `shift`.
    pub(crate) fn cso_of(data: &[u8], shift: u8) -> Vec<u8> {
        let blocks = data.len().div_ceil(SECTOR);
        let mut head = Vec::new();
        head.extend_from_slice(b"CISO");
        head.extend_from_slice(&24u32.to_le_bytes());
        head.extend_from_slice(&(data.len() as u64).to_le_bytes());
        head.extend_from_slice(&(SECTOR as u32).to_le_bytes());
        head.extend_from_slice(&[1, shift, 0, 0]);
        let mut body = Vec::new();
        let mut index = Vec::new();
        let start = 24 + (blocks + 1) * 4;
        let pad = |body: &mut Vec<u8>| {
            while !(start + body.len()).is_multiple_of(1 << shift) {
                body.push(0);
            }
        };
        for (i, chunk) in data.chunks(SECTOR).enumerate() {
            pad(&mut body);
            let at = ((start + body.len()) >> shift) as u32;
            if i % 2 == 1 {
                index.push(at | 0x8000_0000);
                let mut block = chunk.to_vec();
                block.resize(SECTOR, 0);
                body.extend_from_slice(&block);
            } else {
                index.push(at);
                body.extend_from_slice(&miniz_oxide::deflate::compress_to_vec(chunk, 6));
            }
        }
        pad(&mut body);
        index.push(((start + body.len()) >> shift) as u32);
        for i in index {
            head.extend_from_slice(&i.to_le_bytes());
        }
        head.extend_from_slice(&body);
        head
    }

    #[test]
    fn sectors_read_back_across_stored_and_deflated_blocks() {
        let data: Vec<u8> = (0..SECTOR * 5 + 100).map(|i| (i * 7 % 251) as u8).collect();
        for shift in [0, 2] {
            let path = std::env::temp_dir().join(format!(
                "nib-cheevos-cso-{shift}-{}.cso",
                std::process::id()
            ));
            std::fs::write(&path, cso_of(&data, shift)).unwrap();
            let mut image = Image::open(path.to_str().unwrap(), 1).unwrap();
            let mut buf = vec![0; SECTOR * 2];
            assert_eq!(image.read(1, &mut buf), SECTOR * 2);
            assert_eq!(buf, data[SECTOR..SECTOR * 3]);
            // The last sector is short: what is there.
            assert_eq!(image.read(5, &mut buf), 100);
            assert_eq!(buf[..100], data[SECTOR * 5..]);
            assert_eq!(image.read(6, &mut buf), 0);
            assert!(Image::open(path.to_str().unwrap(), 2).is_none());
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn a_file_that_isnt_a_cso_opens_nothing() {
        let mut head = [0u8; 24];
        head[..4].copy_from_slice(b"ZISO");
        assert!(Header::parse(&head).is_none());
        head[..4].copy_from_slice(b"CISO");
        assert!(Header::parse(&head).is_none(), "no size, no block");
        head[8] = 1;
        head[16..20].copy_from_slice(&2048u32.to_le_bytes());
        assert!(Header::parse(&head).is_some());
        head[20] = 2;
        assert!(Header::parse(&head).is_none(), "version 2 may be LZ4");
        assert!(Image::open("/nonexistent.cso", 1).is_none());
    }
}
