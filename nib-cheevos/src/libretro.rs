//! RetroAchievements for a program that runs libretro cores (feature `libretro`), through
//! rcheevos' own `rc_libretro.c`, so that every console's memory comes out as the service lays
//! it out, with the quirks of each core rcheevos knows about, and nothing here repeats them:
//!
//! - [`Regions`]: the console's memory in the service's address space ([`crate::Memory`]),
//!   built from the core's memory map (`RETRO_ENVIRONMENT_SET_MEMORY_MAPS`, its descriptors as
//!   [`MemoryDescriptor`]s) or, for a core that gives none, from what `retro_get_memory_data` and
//!   `retro_get_memory_size` answer ([`CoreMemory`]). rcheevos' console table
//!   (`src/rcheevos/consoleinfo.c`) says which of the core's addresses each of the service's is.
//! - [`Disallowed`]: the core options rcheevos says a core must not have while achievements are
//!   earned (a PAL region, a graphics layer turned off, an underclocked CPU), by the core's
//!   `library_name`.
//! - [`system_allowed`]: whether a core may earn achievements for a console at all (a core that
//!   emulates one console badly is ruled out for it).
//!
//! The core's memory is read in place, never copied: the pointers in a [`Regions`] are the
//! core's, and good only while the core keeps that memory where it said (until the game is
//! unloaded, or the core gives a new map). A program builds them again when the map or the
//! memory the core reports changes (a core may show its battery RAM only after a frame).

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_uint, CStr, CString};

/// The most regions rcheevos keeps for one console (`RC_LIBRETRO_MAX_MEMORY_REGIONS`).
const MOST: usize = 32;

/// `libretro.h`'s `RETRO_MEMORY_SYSTEM_RAM`, `RETRO_MEMORY_SAVE_RAM`, `RETRO_MEMORY_VIDEO_RAM`:
/// the ids `retro_get_memory_data` takes, and rcheevos asks for.
const SAVE_RAM: u32 = 0;
const SYSTEM_RAM: u32 = 2;
const VIDEO_RAM: u32 = 3;

/// A libretro core's memory descriptor, laid out as `libretro.h`'s `struct
/// retro_memory_descriptor` (a test checks it against C's): one stretch of the console's
/// address space and where the core keeps it. A program copies the core's into these.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryDescriptor {
    /// `RETRO_MEMDESC_*`.
    pub flags: u64,
    /// The core's memory: what the descriptor maps, at `offset`. Null when it is not backed.
    pub ptr: *mut u8,
    /// Where in `ptr` the mapped bytes start.
    pub offset: usize,
    /// The first address it maps.
    pub start: usize,
    /// The address bits that must match `start`'s for an address to be in it; 0 to match by
    /// `start` and `len`.
    pub select: usize,
    /// The address bits that go to no memory (mirrors).
    pub disconnect: usize,
    /// How many bytes it maps.
    pub len: usize,
    /// The address space's name, for a core that has several; rcheevos doesn't read it, nor
    /// does nib-cheevos (pass null).
    pub addrspace: *const c_char,
}

impl Default for MemoryDescriptor {
    fn default() -> MemoryDescriptor {
        MemoryDescriptor {
            flags: 0,
            ptr: std::ptr::null_mut(),
            offset: 0,
            start: 0,
            select: 0,
            disconnect: 0,
            len: 0,
            addrspace: std::ptr::null(),
        }
    }
}

/// One of the core's memories as `retro_get_memory_data` and `retro_get_memory_size` give it:
/// where it is and how big; null and 0 when the core has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    /// The core's bytes.
    pub data: *mut u8,
    /// How many.
    pub size: usize,
}

impl Default for Block {
    fn default() -> Block {
        Block {
            data: std::ptr::null_mut(),
            size: 0,
        }
    }
}

/// What a core says of its memories by id (`RETRO_MEMORY_*`), for a core without a memory map,
/// and for consoles whose regions rcheevos takes from them anyway.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoreMemory {
    /// `RETRO_MEMORY_SYSTEM_RAM`.
    pub system_ram: Block,
    /// `RETRO_MEMORY_SAVE_RAM`.
    pub save_ram: Block,
    /// `RETRO_MEMORY_VIDEO_RAM`.
    pub video_ram: Block,
}

impl CoreMemory {
    fn get(&self, id: u32) -> Block {
        match id {
            SYSTEM_RAM => self.system_ram,
            SAVE_RAM => self.save_ram,
            VIDEO_RAM => self.video_ram,
            _ => Block::default(),
        }
    }
}

/// `rc_libretro_memory_regions_t`.
#[repr(C)]
struct RawRegions {
    data: [*mut u8; MOST],
    size: [usize; MOST],
    total_size: usize,
    count: u32,
}

/// `rc_libretro_core_memory_info_t`.
#[repr(C)]
struct RawCoreMemory {
    data: *mut u8,
    size: usize,
}

/// `struct retro_memory_map`.
#[repr(C)]
struct RawMap {
    descriptors: *const MemoryDescriptor,
    num_descriptors: c_uint,
}

/// `rc_disallowed_setting_t`.
#[repr(C)]
struct RawSetting {
    setting: *const c_char,
    value: *const c_char,
}

type CoreMemoryFn = extern "C" fn(id: u32, info: *mut RawCoreMemory);
type MessageFn = extern "C" fn(message: *const c_char);

extern "C" {
    fn rc_libretro_memory_init(
        regions: *mut RawRegions,
        mmap: *const RawMap,
        get_core_memory_info: CoreMemoryFn,
        console_id: u32,
    ) -> c_int;
    fn rc_libretro_memory_read(
        regions: *const RawRegions,
        address: u32,
        buffer: *mut u8,
        num_bytes: u32,
    ) -> u32;
    fn rc_libretro_get_disallowed_settings(library_name: *const c_char) -> *const RawSetting;
    fn rc_libretro_is_setting_allowed(
        disallowed: *const RawSetting,
        setting: *const c_char,
        value: *const c_char,
    ) -> c_int;
    fn rc_libretro_is_system_allowed(library_name: *const c_char, console_id: u32) -> c_int;
    fn rc_libretro_init_verbose_message_callback(callback: Option<MessageFn>);
}

thread_local! {
    /// The core's memories while [`Regions::new`] runs: rcheevos asks for them through a
    /// callback that has no pointer of its own.
    static ASKED: Cell<CoreMemory> = Cell::new(CoreMemory::default());
}

extern "C" fn core_memory(id: u32, info: *mut RawCoreMemory) {
    if info.is_null() {
        return;
    }
    let block = ASKED.with(|a| a.get().get(id));
    // SAFETY: rcheevos passes a valid, writable info struct for the call.
    unsafe {
        *info = RawCoreMemory {
            data: block.data,
            size: if block.data.is_null() { 0 } else { block.size },
        }
    };
}

extern "C" fn said(message: *const c_char) {
    if !message.is_null() {
        // SAFETY: rcheevos passes a NUL-terminated message valid for the call.
        let line = unsafe { CStr::from_ptr(message) }.to_string_lossy();
        eprintln!("cheevos: rc_libretro: {line}");
    }
}

/// rcheevos' account of each region it maps, on stderr (`cheevos: rc_libretro: Registered
/// 0x2000 bytes of SYSTEM RAM at $000000 (descriptor 1, offset 0x000000)`), or not. Off unless
/// turned on; for every thread.
pub fn verbose(on: bool) {
    // SAFETY: stores a function pointer (or null) rcheevos calls with a C string.
    unsafe { rc_libretro_init_verbose_message_callback(on.then_some(said as MessageFn)) };
}

/// A console's memory in the service's address space, over the core's own (see the module's
/// documentation): what [`crate::Client::frame`] reads.
pub struct Regions {
    raw: Box<RawRegions>,
    backed: bool,
}

impl Regions {
    /// The regions of the console with rcheevos id `console_id` (`crate::hash::console_id`),
    /// from the core's memory map `map` when it gave one (not empty), else from `memory`
    /// (which rcheevos may also ask for with a map). Addresses neither backs read as unbacked.
    ///
    /// # Safety
    ///
    /// Every non-null pointer in `map` and `memory` points to at least the bytes it is said to
    /// have (`offset + len` for a descriptor, `size` for a block), and stays valid, unmoved,
    /// for as long as the `Regions` is read: until the core unloads the game, changes its map
    /// or moves that memory. The memory is only read, on this thread, while the core isn't
    /// running (between frames).
    pub unsafe fn new(console_id: u32, map: &[MemoryDescriptor], memory: CoreMemory) -> Regions {
        let raw_map = RawMap {
            descriptors: map.as_ptr(),
            num_descriptors: map.len().min(u32::MAX as usize) as c_uint,
        };
        let mut raw = Box::new(RawRegions {
            data: [std::ptr::null_mut(); MOST],
            size: [0; MOST],
            total_size: 0,
            count: 0,
        });
        let previous = ASKED.with(|a| a.replace(memory));
        // SAFETY: `raw` is writable; the map and its descriptors are valid for the call (the
        // descriptors as the caller promises); the callback answers from ASKED, set for this
        // call on this thread.
        let backed = unsafe {
            rc_libretro_memory_init(
                &mut *raw,
                if map.is_empty() {
                    std::ptr::null()
                } else {
                    &raw_map
                },
                core_memory,
                console_id,
            )
        } != 0;
        ASKED.with(|a| a.set(previous));
        Regions { raw, backed }
    }

    /// Whether any of it is backed by the core's memory: nothing can be earned without.
    pub fn backed(&self) -> bool {
        self.backed
    }

    /// The service's address space for the console, in bytes, backed or not.
    pub fn size(&self) -> usize {
        self.raw.total_size
    }

    /// The regions in the service's order: each one's size, and whether the core backs it.
    /// Neighbouring regions rcheevos could join are one.
    pub fn regions(&self) -> Vec<(usize, bool)> {
        let n = (self.raw.count as usize).min(MOST);
        (0..n)
            .map(|i| (self.raw.size[i], !self.raw.data[i].is_null()))
            .collect()
    }

    /// Reads `buf.len()` bytes at the service's `address`: how many are backed, from the start
    /// (the rest read as zero).
    pub fn read(&self, address: u32, buf: &mut [u8]) -> u32 {
        let n = buf.len().min(u32::MAX as usize) as u32;
        // SAFETY: `buf` is writable for `n` bytes; the regions' pointers are the core's, valid
        // as `new`'s caller promised.
        let got = unsafe { rc_libretro_memory_read(&*self.raw, address, buf.as_mut_ptr(), n) };
        let got = got.min(n);
        buf[got as usize..].fill(0);
        got
    }
}

#[cfg(feature = "client")]
impl crate::Memory for Regions {
    fn read(&self, address: u32, buf: &mut [u8]) -> u32 {
        Regions::read(self, address, buf)
    }
}

/// The core options a core must not have set while achievements are earned, by rcheevos' list
/// for it (empty for a core it has none for).
pub struct Disallowed {
    list: *const RawSetting,
}

// SAFETY: the list is rcheevos' static, never written: it may be read from any thread.
unsafe impl Send for Disallowed {}
// SAFETY: as Send.
unsafe impl Sync for Disallowed {}

/// The settings rcheevos rules out for the core whose `retro_system_info::library_name` is
/// `library_name` ("Snes9x", "PCSX-ReARMed").
pub fn disallowed_settings(library_name: &str) -> Disallowed {
    let list = CString::new(library_name).map_or(std::ptr::null(), |name| {
        // SAFETY: a NUL-terminated name, valid for the call; the list is rcheevos' static.
        unsafe { rc_libretro_get_disallowed_settings(name.as_ptr()) }
    });
    Disallowed { list }
}

impl Disallowed {
    /// Whether rcheevos has no rule for the core.
    pub fn is_empty(&self) -> bool {
        self.list.is_null()
    }

    /// Whether option `key` may have `value` (rcheevos matches some keys by a prefix and some
    /// values by a pattern: "pal", "<100", "!disabled").
    pub fn allows(&self, key: &str, value: &str) -> bool {
        if self.list.is_null() {
            return true;
        }
        let (Ok(key), Ok(value)) = (CString::new(key), CString::new(value)) else {
            return true;
        };
        // SAFETY: the list is rcheevos' static, ending in a null setting; the strings are
        // NUL-terminated and valid for the call.
        unsafe { rc_libretro_is_setting_allowed(self.list, key.as_ptr(), value.as_ptr()) != 0 }
    }
}

/// Whether the core named `library_name` may earn achievements for the console with rcheevos id
/// `console_id` at all.
pub fn system_allowed(library_name: &str, console_id: u32) -> bool {
    let Ok(name) = CString::new(library_name) else {
        return true;
    };
    // SAFETY: a NUL-terminated name, valid for the call.
    unsafe { rc_libretro_is_system_allowed(name.as_ptr(), console_id) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern "C" {
        fn nib_rl_sizeof_regions() -> usize;
        fn nib_rl_sizeof_core_memory() -> usize;
        fn nib_rl_sizeof_descriptor() -> usize;
        fn nib_rl_sizeof_map() -> usize;
        fn nib_rl_sizeof_setting() -> usize;
    }

    #[test]
    fn the_mirrors_are_laid_out_as_c_lays_them_out() {
        use std::mem::size_of;
        // SAFETY: functions returning sizeof, no arguments.
        unsafe {
            assert_eq!(size_of::<RawRegions>(), nib_rl_sizeof_regions());
            assert_eq!(size_of::<RawCoreMemory>(), nib_rl_sizeof_core_memory());
            assert_eq!(size_of::<MemoryDescriptor>(), nib_rl_sizeof_descriptor());
            assert_eq!(size_of::<RawMap>(), nib_rl_sizeof_map());
            assert_eq!(size_of::<RawSetting>(), nib_rl_sizeof_setting());
        }
    }

    const SNES: u32 = 3;
    const NES: u32 = 7;
    const MEGA_DRIVE: u32 = 1;
    const GAME_GEAR: u32 = 15;
    const PC_ENGINE: u32 = 8;
    const NEO_GEO_POCKET: u32 = 14;
    const PLAYSTATION: u32 = 12;
    const GAME_BOY: u32 = 4;

    fn block(v: &mut [u8]) -> Block {
        Block {
            data: v.as_mut_ptr(),
            size: v.len(),
        }
    }

    /// A Super Nintendo core's map: work RAM at $7E0000, the cartridge's RAM where rcheevos
    /// looks for it, $1000000 (past the console's own addresses: a cartridge's RAM is in a
    /// different place for each kind of ROM). The service's $000000 is work RAM, $020000 the
    /// cartridge's RAM.
    #[test]
    fn a_super_nintendo_map_puts_work_ram_first_and_cartridge_ram_after() {
        let mut wram: Vec<u8> = (0..0x20000u32).map(|i| (i % 251) as u8).collect();
        let mut sram = vec![0xA5u8; 0x2000];
        let map = [
            MemoryDescriptor {
                ptr: wram.as_mut_ptr(),
                start: 0x7E0000,
                len: 0x20000,
                ..MemoryDescriptor::default()
            },
            MemoryDescriptor {
                ptr: sram.as_mut_ptr(),
                start: 0x1000000,
                len: 0x2000,
                ..MemoryDescriptor::default()
            },
        ];
        // SAFETY: the vectors outlive the regions and aren't moved.
        let r = unsafe { Regions::new(SNES, &map, CoreMemory::default()) };
        assert!(r.backed());
        let mut b = [0u8; 4];
        assert_eq!(r.read(0x1234, &mut b), 4);
        assert_eq!(b[0], wram[0x1234]);
        assert_eq!(r.read(0x020000, &mut b), 4);
        assert_eq!(b, [0xA5; 4]);
        // Past the 8 KiB the cartridge has: unbacked, read as zero.
        assert_eq!(r.read(0x022000, &mut b), 0);
        assert_eq!(b, [0; 4]);
        // A read across the end of work RAM into the cartridge's.
        assert_eq!(r.read(0x01FFFE, &mut b), 4);
        assert_eq!(b, [wram[0x1FFFE], wram[0x1FFFF], 0xA5, 0xA5]);
    }

    /// An NES core without a map: rcheevos takes the 2 KiB of system RAM for $0000 (and its
    /// mirrors), the cartridge's RAM for $6000.
    #[test]
    fn an_nes_core_without_a_map_is_read_through_its_memory_ids() {
        let mut ram: Vec<u8> = (0..0x800u32).map(|i| i as u8).collect();
        let mut sram = vec![0x5Au8; 0x2000];
        let memory = CoreMemory {
            system_ram: block(&mut ram),
            save_ram: block(&mut sram),
            ..CoreMemory::default()
        };
        // SAFETY: the vectors outlive the regions and aren't moved.
        let r = unsafe { Regions::new(NES, &[], memory) };
        assert!(r.backed());
        let mut b = [0u8; 2];
        assert_eq!(r.read(0x0010, &mut b), 2);
        assert_eq!(b, [0x10, 0x11]);
        assert_eq!(r.read(0x6000, &mut b), 2);
        assert_eq!(b, [0x5A, 0x5A]);
        // The PPU's registers aren't the core's memory.
        assert_eq!(r.read(0x2000, &mut b), 0);
        assert_eq!(r.size(), 0x10000);
    }

    /// The Mega Drive's 64 KiB of work RAM, then the cartridge's RAM; Game Gear's 8 KiB, then its
    /// cartridge's; a PC Engine's 8 KiB; a Neo Geo Pocket's 16 KiB: each from its core's ids.
    #[test]
    fn other_cartridge_consoles_take_their_ram_from_the_memory_ids() {
        for (console, ram_size, save_at) in [
            (MEGA_DRIVE, 0x10000, Some(0x10000)),
            (GAME_GEAR, 0x2000, Some(0x2000)),
            (PC_ENGINE, 0x2000, None),
            (NEO_GEO_POCKET, 0x4000, None),
        ] {
            let mut ram = vec![1u8; ram_size];
            let mut sram = vec![2u8; 0x100];
            let memory = CoreMemory {
                system_ram: block(&mut ram),
                save_ram: block(&mut sram),
                ..CoreMemory::default()
            };
            // SAFETY: the vectors outlive the regions and aren't moved.
            let r = unsafe { Regions::new(console, &[], memory) };
            let mut b = [0u8; 1];
            assert_eq!(r.read(0, &mut b), 1, "console {console}");
            assert_eq!(b, [1]);
            assert_eq!(r.read(ram_size as u32 - 1, &mut b), 1, "console {console}");
            if let Some(at) = save_at {
                assert_eq!(r.read(at, &mut b), 1, "console {console}");
                assert_eq!(b, [2]);
                // The cartridge RAM's region is bigger than the 256 bytes the core has.
                assert_eq!(r.read(at + 0x100, &mut b), 0, "console {console}");
            }
        }
    }

    /// The PlayStation's 2 MiB of RAM as a core maps it (kernel and user RAM in one, and the
    /// scratchpad), rcheevos' three regions over it.
    #[test]
    fn a_playstation_map_has_its_scratchpad_after_the_ram() {
        let mut ram = vec![0u8; 0x200000];
        ram[0x10000] = 0x77;
        let mut scratch = vec![0x33u8; 0x400];
        let map = [
            MemoryDescriptor {
                flags: 1 << 2,
                ptr: ram.as_mut_ptr(),
                start: 0x00000000,
                select: 0xFFE00000,
                len: 0x200000,
                ..MemoryDescriptor::default()
            },
            MemoryDescriptor {
                ptr: scratch.as_mut_ptr(),
                start: 0x1F800000,
                len: 0x400,
                ..MemoryDescriptor::default()
            },
        ];
        // SAFETY: the vectors outlive the regions and aren't moved.
        let r = unsafe { Regions::new(PLAYSTATION, &map, CoreMemory::default()) };
        let mut b = [0u8; 1];
        assert_eq!(r.read(0x10000, &mut b), 1);
        assert_eq!(b, [0x77]);
        assert_eq!(r.read(0x200000, &mut b), 1);
        assert_eq!(b, [0x33]);
        assert_eq!(r.size(), 0x200400);
    }

    #[test]
    fn a_core_with_no_memory_backs_nothing() {
        // SAFETY: no pointers.
        let r = unsafe { Regions::new(SNES, &[], CoreMemory::default()) };
        assert!(!r.backed());
        let mut b = [9u8; 3];
        assert_eq!(r.read(0, &mut b), 0);
        assert_eq!(b, [0; 3]);
        assert!(r.regions().iter().all(|(_, backed)| !backed));
    }

    #[test]
    fn rcheevos_rules_out_a_pal_snes9x_and_a_slow_playstation() {
        let snes9x = disallowed_settings("Snes9x");
        assert!(!snes9x.is_empty());
        assert!(!snes9x.allows("snes9x_region", "pal"));
        assert!(snes9x.allows("snes9x_region", "ntsc"));
        assert!(!snes9x.allows("snes9x_layer_1", "disabled"));
        assert!(snes9x.allows("snes9x_layer_1", "enabled"));
        let pcsx = disallowed_settings("PCSX-ReARMed");
        assert!(!pcsx.allows("pcsx_rearmed_psxclock", "50"));
        assert!(pcsx.allows("pcsx_rearmed_psxclock", "57"));
        assert!(pcsx.allows("pcsx_rearmed_psxclock", "auto"));
        let none = disallowed_settings("Nestopia");
        assert!(none.is_empty());
        assert!(none.allows("nestopia_favored_system", "pal"));
    }

    #[test]
    fn a_core_may_be_ruled_out_for_a_console() {
        assert!(!system_allowed("Mesen-S", GAME_BOY));
        assert!(system_allowed("Mesen-S", SNES));
        assert!(system_allowed("Snes9x", SNES));
    }
}
