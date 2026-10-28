//! CHD disc images (MAME's compressed hunks of data, the usual form of a PlayStation or PC
//! Engine CD game) for rhash (feature `chd`): a CD reader of rhash's shape, `rc_hash_cdreader_t`,
//! over the `chd` crate, which decompresses the image's hunks. rhash then hashes a CHD as it
//! hashes the same disc from its cue sheet: the same tracks, the same sectors, the same hash.
//!
//! A CD in a CHD is its frames, 2352 bytes of sector and 96 of subcode each, the tracks one
//! after another, each padded to a multiple of 4 frames; each track is described by a metadata
//! entry (`CHT2`, or `CHTR` in older images): its number, its kind (`MODE1_RAW`, `MODE2_RAW`,
//! `AUDIO`...), its frames, and the pregap stored with it, if any (`PGTYPE` starting with `V`).
//! Sectors are numbered as on the disc: the first track's first sector (after a pregap stored
//! with it) is 0, and every track follows the one before, its stored pregap included, so the
//! sector numbers a disc's file system gives (a PlayStation's) are the ones read here. A sector
//! read is its user data (2048 bytes of a data sector, after its sync and header), as rhash's own
//! reader gives it.
//!
//! Not here: a CHD that needs a parent image (a delta), and GD-ROMs (no console nibOS plays).

use chd::metadata::{KnownMetadata, Metadata};
use chd::Chd;
use std::ffi::{c_char, c_void, CStr};
use std::fs::File;
use std::io::BufReader;

/// A CD frame in a CHD: the sector and its subcode.
const FRAME: usize = 2352 + 96;
/// Every track's frames are padded to a multiple of this in a CHD.
const PADDING: u32 = 4;
/// rhash's special track numbers (`RC_HASH_CDTRACK_*`).
const FIRST_DATA: u32 = u32::MAX;
const LAST: u32 = u32::MAX - 1;
const LARGEST: u32 = u32::MAX - 2;

/// A track as the image's metadata describes it, and where it is.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TrackInfo {
    number: u32,
    kind: String,
    /// Its frames in the image, the stored pregap included.
    frames: u32,
    /// The pregap's frames stored before the track's own (0 when it isn't stored).
    pregap: u32,
    /// Its first frame in the image (after the tracks before it, padded).
    first_frame: u32,
    /// Its first sector's number on the disc (its stored pregap's first, if any).
    first_sector: u32,
}

impl TrackInfo {
    fn audio(&self) -> bool {
        self.kind == "AUDIO"
    }

    /// Where a sector's user data starts in its 2352 bytes, and how much of it there is.
    fn data(&self) -> (usize, usize) {
        match self.kind.as_str() {
            "MODE1_RAW" => (16, 2048),
            "MODE2_RAW" => (24, 2048),
            "MODE2" | "MODE2_FORM_MIX" => (8, 2048),
            "AUDIO" => (0, 2352),
            // MODE1, MODE2_FORM1 (2048 bytes) and MODE2_FORM2 (2324) are stored cooked.
            _ => (0, 2048),
        }
    }
}

/// The tracks of an image from its metadata entries' text (`TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE
/// FRAMES:223485 PREGAP:0 PGTYPE:MODE2_RAW PGSUB:RW POSTGAP:0`), in order, laid out.
fn layout(entries: &[String]) -> Vec<TrackInfo> {
    let mut tracks: Vec<TrackInfo> = entries
        .iter()
        .filter_map(|text| {
            let field = |name: &str| {
                text.split_whitespace()
                    .find_map(|kv| kv.strip_prefix(name)?.strip_prefix(':'))
            };
            let number = field("TRACK")?.parse().ok()?;
            let frames = field("FRAMES")?.parse().ok()?;
            let stored = field("PGTYPE").is_some_and(|t| t.starts_with('V'));
            let pregap = if stored {
                field("PREGAP").and_then(|p| p.parse().ok()).unwrap_or(0)
            } else {
                0
            };
            Some(TrackInfo {
                number,
                kind: field("TYPE")?.to_owned(),
                frames,
                pregap: pregap.min(frames),
                first_frame: 0,
                first_sector: 0,
            })
        })
        .collect();
    tracks.sort_by_key(|t| t.number);
    let (mut frame, mut sector) = (0u32, 0u32);
    // The first track's stored pregap is before sector 0.
    let before = tracks.first().map_or(0, |t| t.pregap);
    for t in &mut tracks {
        t.first_frame = frame;
        t.first_sector = sector.wrapping_sub(before);
        frame = frame.saturating_add(t.frames.div_ceil(PADDING) * PADDING);
        sector = sector.saturating_add(t.frames);
    }
    tracks
}

/// The track rhash asks for: a number, or one of its special ones.
fn pick(tracks: &[TrackInfo], track: u32) -> Option<&TrackInfo> {
    match track {
        FIRST_DATA => tracks.iter().find(|t| !t.audio()),
        LAST => tracks.last(),
        LARGEST => tracks.iter().max_by_key(|t| t.frames),
        n => tracks.iter().find(|t| t.number == n),
    }
}

/// An open track: the image, the track, and the last hunk decompressed.
struct Track {
    chd: Chd<BufReader<File>>,
    info: TrackInfo,
    frames_per_hunk: u32,
    hunk: Option<u32>,
    data: Vec<u8>,
    compressed: Vec<u8>,
}

impl Track {
    fn open(path: &str, track: u32) -> Option<Track> {
        let file = BufReader::new(File::open(path).ok()?);
        let mut chd = Chd::open(file, None).ok()?;
        let entries: Vec<String> = Vec::<Metadata>::try_from(chd.metadata_refs())
            .ok()?
            .into_iter()
            .filter(|m| {
                m.metatag == KnownMetadata::CdRomTrack2 as u32
                    || m.metatag == KnownMetadata::CdRomTrack as u32
            })
            .map(|m| {
                String::from_utf8_lossy(&m.value)
                    .trim_end_matches('\0')
                    .to_owned()
            })
            .collect();
        let info = pick(&layout(&entries), track)?.clone();
        let hunk_bytes = chd.header().hunk_size() as usize;
        let frames_per_hunk = (hunk_bytes / FRAME) as u32;
        if frames_per_hunk == 0 {
            return None;
        }
        Some(Track {
            data: vec![0; hunk_bytes],
            chd,
            info,
            frames_per_hunk,
            hunk: None,
            compressed: Vec::new(),
        })
    }

    /// Sector `sector`'s 2352 bytes, or `None` past the track or when its hunk won't read.
    fn sector(&mut self, sector: u32) -> Option<&[u8]> {
        let within = sector.checked_sub(self.info.first_sector)?;
        if within >= self.info.frames {
            return None;
        }
        let frame = self.info.first_frame + within;
        let hunk = frame / self.frames_per_hunk;
        if self.hunk != Some(hunk) {
            self.hunk = None;
            self.chd
                .hunk(hunk)
                .ok()?
                .read_hunk_in(&mut self.compressed, &mut self.data)
                .ok()?;
            self.hunk = Some(hunk);
        }
        let at = (frame % self.frames_per_hunk) as usize * FRAME;
        self.data.get(at..at + 2352)
    }

    /// Up to `buf.len()` bytes of user data from sector `sector` on: how many.
    fn read(&mut self, mut sector: u32, buf: &mut [u8]) -> usize {
        let (offset, size) = self.info.data();
        let mut done = 0;
        while done < buf.len() {
            let Some(raw) = self.sector(sector) else {
                break;
            };
            let n = size.min(buf.len() - done);
            buf[done..done + n].copy_from_slice(&raw[offset..offset + n]);
            done += n;
            sector += 1;
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
    std::panic::catch_unwind(|| Track::open(&path, track))
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
    // SAFETY: `track` is a Box<Track> open_track made and close_track hasn't freed; rhash
    // gives `bytes` writable bytes at `buffer`, one call at a time.
    let (track, buf) = unsafe {
        (
            &mut *track.cast::<Track>(),
            std::slice::from_raw_parts_mut(buffer.cast::<u8>(), bytes),
        )
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| track.read(sector, buf))).unwrap_or(0)
}

extern "C" fn close_track(track: *mut c_void) {
    if !track.is_null() {
        // SAFETY: a Box<Track> open_track made, freed here once.
        drop(unsafe { Box::from_raw(track.cast::<Track>()) });
    }
}

extern "C" fn first_track_sector(track: *mut c_void) -> u32 {
    if track.is_null() {
        return 0;
    }
    // SAFETY: as read_sector.
    let info = unsafe { &(*track.cast::<Track>()).info };
    info.first_sector.wrapping_add(info.pregap)
}

/// The reader for the shim.
pub(crate) const READER: crate::ffi::CdReader = crate::ffi::CdReader {
    open_track,
    read_sector,
    close_track,
    first_track_sector,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// A PlayStation disc: one data track, nothing stored before it; a PC Engine CD's: audio,
    /// then data with its two-second pregap stored, then audio again.
    #[test]
    fn tracks_are_laid_out_as_on_the_disc() {
        let ps = layout(&[
            "TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE FRAMES:1001 PREGAP:0 PGTYPE:MODE2_RAW PGSUB:RW POSTGAP:0".into(),
        ]);
        assert_eq!(ps[0].first_sector, 0);
        assert_eq!(ps[0].data(), (24, 2048));
        assert_eq!(pick(&ps, FIRST_DATA).unwrap().number, 1);
        let pce = layout(&[
            "TRACK:2 TYPE:MODE1_RAW SUBTYPE:NONE FRAMES:3150 PREGAP:150 PGTYPE:VMODE1_RAW PGSUB:RW POSTGAP:0".into(),
            "TRACK:1 TYPE:AUDIO SUBTYPE:NONE FRAMES:1503 PREGAP:0 PGTYPE:MODE1 PGSUB:RW POSTGAP:0".into(),
            "TRACK:3 TYPE:AUDIO SUBTYPE:NONE FRAMES:9000 PREGAP:150 PGTYPE:VAUDIO PGSUB:RW POSTGAP:0".into(),
        ]);
        assert_eq!(pce.iter().map(|t| t.number).collect::<Vec<_>>(), [1, 2, 3]);
        // Track 2 starts after track 1's 1503 frames padded to 1504.
        assert_eq!((pce[1].first_frame, pce[1].first_sector), (1504, 1503));
        assert_eq!(
            (pce[2].first_frame, pce[2].first_sector),
            (1504 + 3152, 1503 + 3150)
        );
        let data = pick(&pce, FIRST_DATA).unwrap();
        assert_eq!(data.number, 2);
        assert_eq!(data.pregap, 150, "its index 1 is 150 sectors in");
        assert_eq!(pick(&pce, LAST).unwrap().number, 3);
        assert_eq!(pick(&pce, LARGEST).unwrap().number, 3);
        assert!(pick(&pce, 4).is_none());
        // An older image's CHTR: no pregap fields.
        let old = layout(&["TRACK:1 TYPE:MODE1 SUBTYPE:NONE FRAMES:20".into()]);
        assert_eq!(
            (old[0].frames, old[0].pregap, old[0].data()),
            (20, 0, (0, 2048))
        );
        assert!(layout(&["garbage".into()]).is_empty());
    }

    #[test]
    fn a_file_that_isnt_a_chd_opens_no_track() {
        let path = std::env::temp_dir().join(format!("nib-cheevos-notchd-{}", std::process::id()));
        std::fs::write(&path, b"not a chd").unwrap();
        assert!(Track::open(path.to_str().unwrap(), 1).is_none());
        assert!(Track::open("/nonexistent.chd", 1).is_none());
        let _ = std::fs::remove_file(path);
    }
}
