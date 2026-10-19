//! The bottom screen's touch panel (a Goodix GT9xx), read straight from evdev: one finger, as
//! down, moves and up, 0..1 across and down the screen. The stock DS emulator reads it this way
//! too; SDL's finger events work as well on nibOS, but its mouse emulation of them does not
//! know which window they belong to with two screens.
//!
//! ```no_run
//! use nibkit::touch::{Panel, Pointer};
//! let mut panel = Panel::open(); // None until its driver is loaded: look again later
//! let mut pointers = Vec::new();
//! if let Some(panel) = panel.as_mut() {
//!     panel.poll(&mut pointers); // non-blocking; once a frame
//! }
//! for p in pointers.iter().map(|p| p.scaled(1024.0, 768.0)) {
//!     if let Pointer::Down(x, y) = p { println!("touch at {x}, {y}"); }
//! }
//! ```

/// A finger on the touch screen: 0..1 across and down from the top left, or scaled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    Down(f32, f32),
    Move(f32, f32),
    Up,
}

impl Pointer {
    /// From the panel's 0..1 across and down to a `w` by `h` screen.
    pub fn scaled(self, w: f32, h: f32) -> Pointer {
        match self {
            Pointer::Down(x, y) => Pointer::Down(x * w, y * h),
            Pointer::Move(x, y) => Pointer::Move(x * w, y * h),
            Pointer::Up => Pointer::Up,
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::Panel;

#[cfg(not(target_os = "linux"))]
pub struct Panel;

#[cfg(not(target_os = "linux"))]
impl Panel {
    pub fn open() -> Option<Panel> {
        None
    }
    pub fn poll(&mut self, _out: &mut Vec<Pointer>) {}
}

#[cfg(target_os = "linux")]
mod linux {
    use super::Pointer;
    use std::ffi::CString;
    use std::os::raw::{c_int, c_ulong};

    const EV_SYN: u16 = 0;
    const EV_KEY: u16 = 1;
    const EV_ABS: u16 = 3;
    const BTN_TOUCH: u16 = 0x14a;
    const ABS_X: u16 = 0x00;
    const ABS_Y: u16 = 0x01;
    const ABS_MT_POSITION_X: u16 = 0x35;
    const ABS_MT_POSITION_Y: u16 = 0x36;
    const ABS_MT_TRACKING_ID: u16 = 0x39;

    const fn ior(nr: u32, size: u32) -> c_ulong {
        ((2u32 << 30) | (size << 16) | (0x45 << 8) | nr) as c_ulong
    }

    #[repr(C)]
    #[derive(Default)]
    struct AbsInfo {
        value: i32,
        minimum: i32,
        maximum: i32,
        fuzz: i32,
        flat: i32,
        resolution: i32,
    }

    #[repr(C)]
    struct InputEvent {
        time: libc::timeval,
        kind: u16,
        code: u16,
        value: i32,
    }

    pub struct Panel {
        fd: c_int,
        x: (i32, i32),
        y: (i32, i32),
        raw: (i32, i32),
        down: bool,
        was_down: bool,
        moved: bool,
    }

    impl Panel {
        /// Opens `NIBKIT_TOUCH_DEV` (or `DSLOT_TOUCH_DEV`) if set, otherwise the first Goodix
        /// panel under /dev/input. `None` when there is none, or its driver has yet to load.
        pub fn open() -> Option<Panel> {
            if let Ok(dev) =
                std::env::var("NIBKIT_TOUCH_DEV").or_else(|_| std::env::var("DSLOT_TOUCH_DEV"))
            {
                return Panel::try_open(&dev, true);
            }
            // Picked by name from sysfs first: closing an evdev node waits for an RCU grace
            // period (7-23 ms each on nibOS), so opening every one to ask costs ~0.25 s.
            let named = (0..32).find(|i| {
                std::fs::read_to_string(format!("/sys/class/input/event{i}/device/name")).is_ok_and(
                    |n| n.to_lowercase().contains("goodix") || n.to_lowercase().contains("gt9"),
                )
            });
            // None yet: its driver may still be loading, and the caller looks again.
            named.and_then(|i| Panel::try_open(&format!("/dev/input/event{i}"), false))
        }

        fn try_open(path: &str, any_name: bool) -> Option<Panel> {
            let cpath = CString::new(path).ok()?;
            // SAFETY: a NUL-terminated path.
            let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
            if fd < 0 {
                return None;
            }
            let mut name = [0u8; 128];
            // SAFETY: EVIOCGNAME(128) writes at most 128 bytes, the buffer's size.
            unsafe { libc::ioctl(fd, ior(0x06, 128) as _, name.as_mut_ptr()) };
            let name = String::from_utf8_lossy(&name).to_lowercase();
            let range = |code: u16| {
                let mut info = AbsInfo::default();
                // SAFETY: EVIOCGABS fills a struct input_absinfo, which AbsInfo is.
                let ok =
                    unsafe { libc::ioctl(fd, ior(0x40 + code as u32, 24) as _, &mut info) } >= 0;
                (ok && info.maximum > info.minimum).then_some((info.minimum, info.maximum))
            };
            let x = range(ABS_MT_POSITION_X).or_else(|| range(ABS_X));
            let y = range(ABS_MT_POSITION_Y).or_else(|| range(ABS_Y));
            match (
                any_name || name.contains("gt9") || name.contains("goodix"),
                x,
                y,
            ) {
                (true, Some(x), Some(y)) => {
                    eprintln!(
                        "nibkit: touch panel {path} \"{}\" x {x:?} y {y:?}",
                        name.trim_end_matches('\0')
                    );
                    Some(Panel {
                        fd,
                        x,
                        y,
                        raw: (0, 0),
                        down: false,
                        was_down: false,
                        moved: false,
                    })
                }
                _ => {
                    // SAFETY: our own descriptor, not used again.
                    unsafe { libc::close(fd) };
                    None
                }
            }
        }

        /// Drain pending events. Positions are normalised to 0..1 across the panel.
        pub fn poll(&mut self, out: &mut Vec<Pointer>) {
            let mut ev = std::mem::MaybeUninit::<InputEvent>::uninit();
            let size = std::mem::size_of::<InputEvent>();
            loop {
                // SAFETY: reads at most one event into space for one.
                let n = unsafe { libc::read(self.fd, ev.as_mut_ptr().cast(), size) };
                if n != size as isize {
                    break;
                }
                // SAFETY: a whole event was read into it.
                let ev = unsafe { ev.assume_init_ref() };
                match (ev.kind, ev.code) {
                    (EV_ABS, ABS_MT_POSITION_X | ABS_X) => {
                        (self.raw.0, self.moved) = (ev.value, true)
                    }
                    (EV_ABS, ABS_MT_POSITION_Y | ABS_Y) => {
                        (self.raw.1, self.moved) = (ev.value, true)
                    }
                    (EV_ABS, ABS_MT_TRACKING_ID) => self.down = ev.value >= 0,
                    (EV_KEY, BTN_TOUCH) => self.down = ev.value != 0,
                    (EV_SYN, 0) => self.report(out),
                    _ => {}
                }
            }
        }

        fn report(&mut self, out: &mut Vec<Pointer>) {
            let norm =
                |v: i32, (lo, hi): (i32, i32)| ((v - lo) as f32 / (hi - lo) as f32).clamp(0.0, 1.0);
            let (x, y) = (norm(self.raw.0, self.x), norm(self.raw.1, self.y));
            match (self.was_down, self.down) {
                (false, true) => out.push(Pointer::Down(x, y)),
                (true, true) if self.moved => out.push(Pointer::Move(x, y)),
                (true, false) => out.push(Pointer::Up),
                _ => {}
            }
            self.was_down = self.down;
            self.moved = false;
        }
    }

    impl Drop for Panel {
        fn drop(&mut self) {
            // SAFETY: our own descriptor, not used again.
            unsafe { libc::close(self.fd) };
        }
    }
}
