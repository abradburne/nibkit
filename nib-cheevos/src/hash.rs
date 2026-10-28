//! The identity RetroAchievements gives a game: a hash of its ROM (or disc), by rcheevos' own
//! rhash, so that DSlot's shelf and the emulator that plays the game always agree with each
//! other and with the service. Each console has its rule (the whole file for a Game Boy game;
//! for a DS game its header, boot code and icon; for an NES game the file without its iNES
//! header; for a PlayStation disc its boot executable): rhash knows them, nothing here repeats
//! them.
//!
//! Consoles are named as nibOS's `Roms/` folders are ([`CONSOLES`]: `NDS`, `GBA`, `GB`, `GBC`,
//! `SFC`, `FC`, `FDS`, `MD`, `SMS`, `GG`, `SG1000`, `LYNX`, `NGP`, `PCE`, `PCECD`, `PS`, `WS`,
//! `GW`), and each is one of rcheevos' `RC_CONSOLE_*` ids.
//!
//! A zipped game is the ROM inside it: the first entry, in the zip's order, with one of the
//! console's extensions ([`zip_member`]), unpacked into memory (at most rhash's 64 MiB) and hashed
//! as that file (with the feature `zip`; without it a zip is an [`Error::Unsupported`]). Discs are hashed from their cue sheet (and the tracks it names), a playlist
//! (`.m3u`, its first disc) or an image rhash reads itself; never zipped.
//!
//! Not yet: CHD disc images (rhash has no CHD reader of its own; one would be given to it as a
//! custom CD reader, `rc_hash_cdreader_t`), and Game & Watch units, which rhash 12.4 has no rule
//! for. Both are an [`Error::Unsupported`].

use crate::ffi;
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::Path;
use std::sync::Mutex;

/// The most rhash hashes of a file (its `MAX_BUFFER_SIZE`): no more of a zipped ROM is
/// unpacked.
#[cfg(feature = "zip")]
const MOST: u64 = 64 << 20;

/// rhash reads a buffer as a file through a reader that is one static of its own (a DS ROM's
/// pieces): one hash at a time, whatever the thread.
static RHASH: Mutex<()> = Mutex::new(());

/// A console as nibOS names it, and as RetroAchievements does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Console {
    /// Its `Roms/` folder: `NDS`, `GBA`, `SFC`...
    pub system: &'static str,
    /// rcheevos' `RC_CONSOLE_*` id: the service's console id.
    pub id: u32,
    /// Its name.
    pub name: &'static str,
    /// The extensions its games have (lower case, no zip).
    pub extensions: &'static [&'static str],
    /// Its games are discs: hashed from a cue sheet or a playlist, never zipped.
    pub disc: bool,
}

const fn cart(
    system: &'static str,
    id: u32,
    name: &'static str,
    extensions: &'static [&'static str],
) -> Console {
    Console {
        system,
        id,
        name,
        extensions,
        disc: false,
    }
}

/// Every console nibOS plays, by its folder name, with rcheevos' id for it
/// (`include/rc_consoles.h`). A Game Boy game may be either kind, in either folder: many Color
/// games run on the first Game Boy too, and the hash is the same.
pub const CONSOLES: &[Console] = &[
    cart("NDS", 18, "Nintendo DS", &["nds"]),
    cart("GBA", 5, "Game Boy Advance", &["gba"]),
    cart("GB", 4, "Game Boy", &["gb", "gbc"]),
    cart("GBC", 6, "Game Boy Color", &["gbc", "gb"]),
    cart("SFC", 3, "Super Nintendo", &["sfc", "smc"]),
    cart("FC", 7, "NES / Famicom", &["nes", "unf", "unif"]),
    cart("FDS", 81, "Famicom Disk System", &["fds"]),
    cart(
        "MD",
        1,
        "Mega Drive / Genesis",
        &["md", "gen", "bin", "smd"],
    ),
    cart("SMS", 11, "Master System", &["sms"]),
    cart("GG", 15, "Game Gear", &["gg"]),
    cart("SG1000", 33, "SG-1000", &["sg"]),
    cart("LYNX", 13, "Atari Lynx", &["lnx"]),
    cart("NGP", 14, "Neo Geo Pocket", &["ngp", "ngc"]),
    cart("PCE", 8, "PC Engine", &["pce"]),
    Console {
        system: "PCECD",
        id: 76,
        name: "PC Engine CD",
        extensions: &["cue", "chd", "m3u"],
        disc: true,
    },
    Console {
        system: "PS",
        id: 12,
        name: "PlayStation",
        extensions: &["cue", "chd", "m3u", "pbp"],
        disc: true,
    },
    cart("WS", 53, "WonderSwan", &["ws", "wsc"]),
    cart("GW", 60, "Game & Watch", &["mgw"]),
];

/// The console with this folder name (any case).
pub fn console(system: &str) -> Option<&'static Console> {
    CONSOLES
        .iter()
        .find(|c| c.system.eq_ignore_ascii_case(system))
}

/// rcheevos' id for the console with this folder name.
pub fn console_id(system: &str) -> Option<u32> {
    console(system).map(|c| c.id)
}

/// Why a game has no hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No console is named so.
    UnknownSystem(String),
    /// Not hashed here yet: a CHD image, a zipped disc, a Game & Watch unit.
    Unsupported(String),
    /// The file, or the zip, couldn't be read.
    Read(String),
    /// A zip without a ROM of the console's in it.
    NoRom,
    /// rhash couldn't make a hash of it: not shaped like the console's game (what rhash said).
    Rhash(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownSystem(s) => write!(f, "no console is named {s}"),
            Error::Unsupported(why) => write!(f, "not hashed yet: {why}"),
            Error::Read(why) => write!(f, "{why}"),
            Error::NoRom => write!(f, "no ROM in the zip"),
            Error::Rhash(why) if why.is_empty() => write!(f, "rcheevos could not hash it"),
            Error::Rhash(why) => write!(f, "rcheevos could not hash it: {why}"),
        }
    }
}

impl std::error::Error for Error {}

/// The hash of the game at `path` for the console with folder name `system`: 32 lowercase
/// hex digits. A zip is its ROM ([`zip_member`]).
pub fn file(system: &str, path: &Path) -> Result<String, Error> {
    let console = console(system).ok_or_else(|| Error::UnknownSystem(system.to_owned()))?;
    let ext = extension(path);
    if console.id == 60 {
        return Err(Error::Unsupported(
            "rcheevos has no rule for Game & Watch units".into(),
        ));
    }
    if ext == "chd" {
        return Err(Error::Unsupported("CHD images".into()));
    }
    if ext == "zip" {
        if console.disc {
            return Err(Error::Unsupported("a zipped disc".into()));
        }
        return zipped(console, path);
    }
    let c_path = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| Error::Read(format!("{}: a NUL in the path", path.display())))?;
    rhash(|out, log, ud| {
        // SAFETY: `out` has 33 bytes; the path is NUL-terminated and valid for the call; the
        // log callback and its `ud` are valid for the call.
        unsafe { ffi::nib_rc_hash_file(out, console.id, c_path.as_ptr(), log, 0, ud) }
    })
}

/// The hash of a game's bytes for the console with folder name `system`; `name` is its file's
/// name (its extension may say what kind of file it was). For a game already in memory: an
/// emulator hashes the ROM it loaded.
pub fn buffer(system: &str, name: &str, data: &[u8]) -> Result<String, Error> {
    let console = console(system).ok_or_else(|| Error::UnknownSystem(system.to_owned()))?;
    if console.disc || console.id == 60 {
        return Err(Error::Unsupported(format!(
            "{} games from memory",
            console.name
        )));
    }
    if data.is_empty() {
        return Err(Error::Rhash("no bytes".into()));
    }
    let c_name = CString::new(name.replace('\0', "")).unwrap_or_default();
    rhash(|out, log, ud| {
        // SAFETY: `out` has 33 bytes; the name is NUL-terminated and `data` is valid for its
        // length, both for the call; the log callback and its `ud` are valid for the call.
        unsafe {
            ffi::nib_rc_hash_buffer(
                out,
                console.id,
                c_name.as_ptr(),
                data.as_ptr(),
                data.len(),
                log,
                0,
                ud,
            )
        }
    })
}

/// The entry of a zip that is the game, by the entries' names in the zip's order: the first
/// with one of the console's extensions (a readme, a folder or a picture beside it is passed
/// over). `None` when there is none.
pub fn zip_member<'a>(system: &str, names: impl IntoIterator<Item = &'a str>) -> Option<usize> {
    let console = console(system)?;
    names.into_iter().position(|name| {
        !name.ends_with('/')
            && console
                .extensions
                .contains(&extension(Path::new(name)).as_str())
    })
}

/// A zipped game's hash: its ROM unpacked into memory and hashed as that file.
#[cfg(feature = "zip")]
fn zipped(console: &Console, path: &Path) -> Result<String, Error> {
    use std::io::Read;
    let read = |e: &dyn std::fmt::Display| Error::Read(format!("{}: {e}", path.display()));
    let file = std::fs::File::open(path).map_err(|e| read(&e))?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|e| read(&e))?;
    // In the order they were written (the central directory's).
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.name_for_index(i).unwrap_or_default().to_owned())
        .collect();
    let at = zip_member(console.system, names.iter().map(String::as_str)).ok_or(Error::NoRom)?;
    let mut entry = zip.by_index(at).map_err(|e| read(&e))?;
    let name = entry.name().to_owned();
    let mut data = Vec::with_capacity(entry.size().min(MOST) as usize);
    (&mut entry)
        .take(MOST)
        .read_to_end(&mut data)
        .map_err(|e| read(&e))?;
    buffer(console.system, &name, &data)
}

/// Without the zip reader, a zipped game isn't hashed.
#[cfg(not(feature = "zip"))]
fn zipped(_: &Console, _: &Path) -> Result<String, Error> {
    Err(Error::Unsupported(
        "zipped games (nib-cheevos' feature `zip`)".into(),
    ))
}

/// A path's extension, lower case; empty without one.
fn extension(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Runs one rhash call, `call(out, log, ud)`, one at a time: the hash, or what rhash said
/// when it made none.
fn rhash(
    call: impl FnOnce(*mut c_char, Option<ffi::LogFn>, *mut c_void) -> c_int,
) -> Result<String, Error> {
    let mut out = [0 as c_char; 33];
    let mut said = String::new();
    let ok = {
        let _one = RHASH.lock().unwrap_or_else(|e| e.into_inner());
        call(
            out.as_mut_ptr(),
            Some(heard),
            (&mut said as *mut String).cast(),
        )
    };
    if ok == 0 {
        return Err(Error::Rhash(said));
    }
    // SAFETY: the shim NUL-terminates `out` within its 33 bytes.
    Ok(unsafe { ffi::string(out.as_ptr()) })
}

/// rhash's error messages, kept: the last one says why.
extern "C" fn heard(message: *const c_char, ud: *mut c_void) {
    // SAFETY: `ud` is the `String` rhash() lent for the call, only reached from this call;
    // the message is valid for the call.
    let (said, message) = unsafe { (&mut *ud.cast::<String>(), ffi::string(message)) };
    *said = message;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The MD5 of some bytes, by rhash's own rule for a console that hashes the whole file.
    fn md5(data: &[u8]) -> String {
        buffer("GBA", "x.gba", data).unwrap()
    }

    fn temp(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nib-cheevos-hash-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(feature = "zip")]
    fn zip_of(entries: &[(&str, &[u8])], deflate: bool) -> Vec<u8> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let method = if deflate {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        let options = zip::write::SimpleFileOptions::default().compression_method(method);
        for (name, data) in entries {
            w.start_file(*name, options).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn every_folder_is_a_console_of_rcheevos() {
        let systems: Vec<&str> = CONSOLES.iter().map(|c| c.system).collect();
        assert_eq!(
            systems,
            [
                "NDS", "GBA", "GB", "GBC", "SFC", "FC", "FDS", "MD", "SMS", "GG", "SG1000", "LYNX",
                "NGP", "PCE", "PCECD", "PS", "WS", "GW"
            ]
        );
        // include/rc_consoles.h's numbers.
        let ids: Vec<u32> = CONSOLES.iter().map(|c| c.id).collect();
        assert_eq!(
            ids,
            [18, 5, 4, 6, 3, 7, 81, 1, 11, 15, 33, 13, 14, 8, 76, 12, 53, 60]
        );
        assert_eq!(console_id("sfc"), Some(3));
        assert_eq!(console_id("PORTS"), None);
        assert!(console("PS").unwrap().disc && !console("PCE").unwrap().disc);
    }

    /// RFC 1321's vector: the whole-file consoles hash a ROM to its MD5, the Game Boy's in
    /// either folder alike.
    #[test]
    fn whole_file_consoles_hash_the_md5() {
        let abc = "900150983cd24fb0d6963f7d28e17f72";
        for system in ["GB", "GBC", "GBA", "MD", "SMS", "GG", "SG1000", "NGP", "WS"] {
            assert_eq!(buffer(system, "x", b"abc").as_deref(), Ok(abc), "{system}");
        }
    }

    /// Headers a copier or a dumper put in front of the ROM are not the game: rhash leaves them
    /// out, so a headered and a bare dump are the same game.
    #[test]
    fn headers_are_left_out() {
        let rom: Vec<u8> = (0..0x2000u32).map(|i| (i * 7 % 251) as u8).collect();
        let bare = md5(&rom);
        let with = |head: &[u8], len: usize| {
            let mut v = head.to_vec();
            v.resize(len, 0xAA);
            v.extend_from_slice(&rom);
            v
        };
        // iNES and fwNES: 16 bytes.
        assert_eq!(buffer("FC", "x.nes", &with(b"NES\x1a", 16)).unwrap(), bare);
        assert_eq!(buffer("FDS", "x.fds", &with(b"FDS\x1a", 16)).unwrap(), bare);
        assert_eq!(buffer("FC", "x.nes", &rom).unwrap(), bare);
        // A Lynx cart's 64-byte LYNX header.
        assert_eq!(buffer("LYNX", "x.lnx", &with(b"LYNX\0", 64)).unwrap(), bare);
        // A copier's 512 bytes on a SNES or PC Engine ROM (a size 512 over 8 KiB's multiple).
        assert_eq!(buffer("SFC", "x.sfc", &with(b"", 512)).unwrap(), bare);
        assert_eq!(buffer("SFC", "x.sfc", &rom).unwrap(), bare);
        assert_eq!(buffer("PCE", "x.pce", &with(b"", 512)).unwrap(), bare);
        // A Game Boy keeps everything.
        assert_ne!(buffer("GB", "x.gb", &with(b"", 512)).unwrap(), bare);
    }

    /// A DS ROM of the smallest shape: header, ARM9 then ARM7 code, the icon block last.
    fn ds_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x4000 + 0x100 + 0x80 + 0xA00];
        rom[0x20..0x24].copy_from_slice(&0x4000u32.to_le_bytes());
        rom[0x2C..0x30].copy_from_slice(&0x100u32.to_le_bytes());
        rom[0x30..0x34].copy_from_slice(&0x4100u32.to_le_bytes());
        rom[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        rom[0x68..0x6C].copy_from_slice(&0x4180u32.to_le_bytes());
        // Something other than zeros past the header, which holds the offsets.
        for (i, b) in rom.iter_mut().enumerate().skip(0x160) {
            *b ^= (i % 251) as u8;
        }
        rom
    }

    /// A DS game is its header, its two pieces of boot code and its icon block, in that order:
    /// what lies between and after them doesn't count.
    #[test]
    fn a_ds_game_is_its_header_boot_code_and_icon() {
        let rom = ds_rom();
        let mut pieces = rom[..0x160].to_vec();
        pieces.extend_from_slice(&rom[0x4000..0x4100]);
        pieces.extend_from_slice(&rom[0x4100..0x4180]);
        pieces.extend_from_slice(&rom[0x4180..0x4180 + 0xA00]);
        let want = md5(&pieces);
        assert_eq!(buffer("NDS", "x.nds", &rom).unwrap(), want);
        let mut longer = rom.clone();
        longer.extend_from_slice(&[7; 4096]);
        longer[0x3000] ^= 0xFF;
        assert_eq!(buffer("NDS", "x.nds", &longer).unwrap(), want);
        // And the same from the file, which rhash reads piece by piece.
        let dir = temp("ds");
        let path = dir.join("Game.nds");
        std::fs::write(&path, &longer).unwrap();
        assert_eq!(file("NDS", &path).unwrap(), want);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A zip is its first entry with one of the console's extensions, stored or deflated, and
    /// hashes as the file itself does.
    #[cfg(feature = "zip")]
    #[test]
    fn a_zipped_rom_is_its_first_rom_entry() {
        let dir = temp("zip");
        let rom = ds_rom();
        let plain = dir.join("Game.nds");
        std::fs::write(&plain, &rom).unwrap();
        let want = file("NDS", &plain).unwrap();
        for deflate in [false, true] {
            let zipped = dir.join(format!("Game {deflate}.zip"));
            let bytes = zip_of(
                &[
                    ("readme.txt", b"not this"),
                    ("art/", b""),
                    ("Game (USA).NDS", &rom),
                    ("Game (Europe).nds", b"nor this"),
                ],
                deflate,
            );
            std::fs::write(&zipped, bytes).unwrap();
            assert_eq!(file("NDS", &zipped).unwrap(), want, "deflate {deflate}");
        }
        // Nothing of the console's in it.
        let other = dir.join("Other.zip");
        std::fs::write(&other, zip_of(&[("Game.gba", b"abc")], false)).unwrap();
        assert_eq!(file("NDS", &other), Err(Error::NoRom));
        assert_eq!(
            file("GBA", &other).as_deref(),
            Ok("900150983cd24fb0d6963f7d28e17f72")
        );
        // Not a zip at all.
        let bad = dir.join("Bad.zip");
        std::fs::write(&bad, b"PK? no").unwrap();
        assert!(matches!(file("GBA", &bad), Err(Error::Read(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_zip_rule() {
        let names = ["readme.txt", "x.gb", "y.gbc"];
        assert_eq!(
            zip_member("GBC", names),
            Some(1),
            "a GB ROM in the GBC folder"
        );
        assert_eq!(zip_member("GBA", names), None);
        assert_eq!(zip_member("NDS", ["d/", "d/x.NDS"]), Some(1));
        assert_eq!(zip_member("NOPE", ["x.nds"]), None);
    }

    /// A PlayStation disc from its cue sheet: rhash reads the track it names with its own CD
    /// reader, finds SYSTEM.CNF and hashes the boot executable it names (here none: no hash,
    /// but rhash said why). What isn't done yet says so.
    #[test]
    fn discs_are_read_from_their_cue_sheet_and_chd_is_not_yet() {
        let dir = temp("disc");
        let bin = dir.join("Game.bin");
        std::fs::write(&bin, vec![0u8; 2352 * 32]).unwrap();
        let cue = dir.join("Game.cue");
        std::fs::write(
            &cue,
            "FILE \"Game.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
        )
        .unwrap();
        match file("PS", &cue) {
            Err(Error::Rhash(why)) => assert!(!why.is_empty(), "rhash says why"),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            file("PS", &dir.join("Game.chd")),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            file("PCECD", &dir.join("Game.zip")),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            buffer("PS", "x.cue", b"x"),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            file("GW", &dir.join("gnw_ball.zip")),
            Err(Error::Unsupported(_))
        ));
        assert_eq!(
            file("PORTS", &cue),
            Err(Error::UnknownSystem("PORTS".into()))
        );
        assert!(matches!(
            file("GBA", &dir.join("gone.gba")),
            Err(Error::Rhash(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Hashes from many threads at once agree: rhash's static reader is one at a time.
    #[test]
    fn hashing_on_many_threads_at_once() {
        let rom = ds_rom();
        let want = buffer("NDS", "x.nds", &rom).unwrap();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let rom = rom.clone();
                std::thread::spawn(move || {
                    (0..20).all(|_| buffer("NDS", "x.nds", &rom).unwrap() == md5_ds(&rom))
                })
            })
            .collect();
        fn md5_ds(rom: &[u8]) -> String {
            buffer("NDS", "y.nds", rom).unwrap()
        }
        assert!(threads.into_iter().all(|t| t.join().unwrap()));
        assert_eq!(md5_ds(&rom), want);
    }
}
