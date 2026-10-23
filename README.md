# nibkit

What an app on a nibOS handheld (the Anbernic RG DS+ today) needs to know and do: where its two
screens are, the buttons by what is printed on them, touch, each screen's brightness, the
volume, where its files go, and settings at three levels. A Rust library, MIT, with no graphics
library of its own: use it with SDL, as [`examples/hello.rs`](examples/hello.rs) and
[`../template`](../template) do, or anything else.

The same app builds for **nibOS** (feature `nibos`, the default), the RG DS+'s **stock OS**
(feature `stock`) or a **desktop** computer (feature `desktop`; chosen on its own on macOS and
Windows), so it can be built, tried and played anywhere. Where a system lacks something (each
screen's brightness, a volume an app may set, the network switch), nibkit says so with `None`
instead of guessing.

```toml
[dependencies]
nibkit = "0.1"                                                         # nibOS; a desktop on macOS
# nibkit = { version = "0.1", features = ["desktop"] }                 # a Linux desktop
# nibkit = { version = "0.1", default-features = false, features = ["stock"] }  # the stock OS
```

## Features

| Feature | |
|---|---|
| `nibos` (default) | Built for nibOS: its panels, nibos-hw's gamepad and backlight links, PipeWire, the `nib` command. |
| `stock` | Built for the RG DS+'s stock OS (Weston across both panels, its own helper keeping volume and brightness). |
| `desktop` | Built for a computer: no panels (an app opens windows: `screens::desktop_windows`, the mouse on the bottom one as touch: `Rect::panel_point`), a keyboard map (`input::keyboard`), the platform's own app folders, the handheld's controls `None`. Chosen anyway on any target but Linux. |
| `dslot` | For an emulator or app DSlot starts: description files, the apps folder, the saves convention, DSlot's settings file. |
| `menu` | DSlot's menu for any app's touch screen: what goes where, and the buttons and touches. No dependencies. |
| `net-control` | Joining Wi-Fi and pairing Bluetooth: `net::wifi`, `net::bluetooth` (zbus). |
| `menu-raster` | `menu`, drawn into an RGBA picture in Roboto Condensed (embedded, SIL OFL 1.1: `assets/OFL.txt`). |

Rust 1.87 or later.

## On a desktop

Both screens show in windows (one each, or one with both stacked as the handheld holds them),
the keyboard stands in for the buttons (the arrows, X and Z for A and B, S and A for X and Y, Q
and W for L and R, Return for Start, Escape for Back, which opens a game's pause menu, and Home
for Menu, which saves and quits) as does any gamepad SDL knows, and the
mouse on the bottom screen is a finger. Settings, data and cache go in the platform's own
folders (`~/Library/Application Support/<app>` on macOS, the XDG folders on Linux); games in
`Roms` in nibOS's data folder there, or wherever `NIBKIT_ROMS` says. Brightness, the battery,
the speakers' tuning, the network and powering off are the handheld's: `None`, or nothing done
(an app never powers off the computer).

## What is in it

| Module | |
|---|---|
| `screens` | `place(display bounds, swap)`: which SDL display (or which half of one) is the top screen and which the bottom. nibOS: a display per panel, the bottom (touch) one on the left. Stock OS: one display across both, the bottom one on the right. Size, refresh rate. |
| `vblank` | `Vblank::open()?.wait(Screen::Top)`: the panel's next vertical blank, and when it was (DRM WAIT_VBLANK, no DRM master needed, so it works beside SDL). |
| `input` | `printed(sdl_face)`: SDL names face buttons by position on an Xbox pad, the DS+ has Nintendo's labels. The gamepad's name, mapping files to load (stock OS), Menu arriving on release (nibOS), Back's keycodes. |
| `touch` | `Panel::open()`, `poll()`: the touch panel from evdev, one finger, 0..1 on the bottom screen. |
| `backlight` | `get(Screen)`, `set(Screen, percent)`, `save()`, `set_off(Screen, bool)`, `watch()` (changes made by the shortcut, `nib` or other apps): each screen's brightness above nibOS's floor, with its perceptual curve. `None` on the stock OS. |
| `battery` | `read()` (charge, state, plugged in, `time_to_full()`), `protect()`, `set_protect(bool)`, `watch()`: the battery by the RK817's count, and battery protect (stop charging at about 80%, by a lower charge voltage). `None` on the stock OS. |
| `device` | `volume()`, `set_volume()` (PipeWire, through `pactl`), `wifi()`, `set_wifi()` (`nib net`), `sleep()`, `restart()`, `power_off()`. |
| `net` | The radios for a status bar: `radios()` reads Wi-Fi (off, connecting, connected with signal bars: sysfs, `iw`) and Bluetooth (off, on, connected: how many devices, whether one is a gamepad: sysfs, BlueZ's `Powered`); `Monitor::start(every)` reads them on a thread of its own, `now()` gives the last. `Unknown` on the stock OS and a desktop. |
| `net::wifi`, `net::bluetooth` (feature `net-control`) | Wi-Fi through iwd (`Wifi::open()`, `status()`, `scan()`, `networks()`, `connect(network, passphrase)`, `known()`, `forget()`) and Bluetooth through BlueZ (`Bluetooth::open()`, `set_powered()`, `devices()`, `discover()`, `pair(device, answer)`, `connect()`, `disconnect()`, `remove()`), over D-Bus. Calls block: use a thread. |
| `net::radios` (feature `net-control`) | The worker behind a menu's Wi-Fi and Bluetooth pages: `Radios::start()`, `now()` (a snapshot), `ask(Do)`, `reply()` to a pairing question. |
| `clock` | `hour12()`: nibOS's 12- or 24-hour preference (`~/.config/nibos/prefs.toml`, else the locale's habit); `format(h, m, hour12)`. |
| `power` | The power menu's request (SIGUSR1 on a long press of power, nibOS): `watch()`, then `take()` once a frame. **A program DSlot starts saves its auto-resume state and exits on SIGTERM**: that is how a game is put away for the menu. |
| `audio` | `ADVICE`: the sample rate and buffer size that suit the sound server: PipeWire at 48 kHz on nibOS; on the stock OS PulseAudio behind an ALSA shim that wants big buffers. |
| `paths` | `AppDirs::new("app")`: config, data and cache folders (XDG on nibOS; beside the program on the stock OS). `rom_roots()` (each card's `Roms`, then slot CFW's `Games`; `slot_copy()` tells a ROM in both), `saves_root()`, `saves_dir()`. |
| `settings` | `Settings`: values at the system, a console and a game, each winning over the one before, in one TOML file; `Def` says what a setting takes, its levels and default. |
| `menu::keyboard`, `menu::keys` (feature `menu`) | Two touch keyboards: `keyboard` types a string (a passphrase, a name: `Typed::Done`), `keys` is a terminal's keyboard giving Linux key codes pressed and released, with Shift, Ctrl, Alt and Fn, for a uinput device (nibos-keys, the console's). Both `draws()` like the menu. |
| `dslot` (feature `dslot`) | For an emulator or app that DSlot starts: its description file's types, parsing and checks, where descriptions go, the placeholders and saves-folder convention. |

## A plain app

[`examples/hello.rs`](examples/hello.rs): windows on both screens wherever the system puts
them, A by its label, a dot under a finger, the top screen's brightness on the D-pad, a colour
remembered in the app's settings.

```sh
cargo run --example hello                                         # nibOS, or a desktop (macOS)
cargo run --example hello --features desktop                      # a Linux desktop
cargo run --example brightness                                    # nibOS: brightness as it changes
cargo run --example battery                                       # nibOS: the battery and battery protect
cargo run --example hello --no-default-features --features stock  # built for the stock OS
```

On the device, run it with the screens free: `sdk/run.sh` stops DSlot first and starts it again
after.

## Settings

```rust
use nibkit::settings::{Def, Kind, Levels, Scope, Settings};
let mut settings = Settings::load(nibkit::paths::AppDirs::new("inkdot").config.join("settings.toml"));
let palette = Def {
    key: "palette".into(),
    label: "Palette".into(),
    kind: Kind::choice(&[("green", "Green"), ("grey", "Grey")]),
    levels: Levels::ALL,
    default: Some("green".into()),
};
settings.set(Scope::Console("GB".into()), "palette", "grey".into());
let chain = Scope::chain("GB", "Tetris (World)");      // the game, its console, the system
let value = settings.value(&chain, &palette);           // "grey", from GB
let from = settings.source(&chain, &palette);           // Some(Scope::Console("GB"))
settings.save()?;
```

The file has `[system]`, `[console.GB]` and `[game."GB/Tetris (World)"]` tables; a key with a
dot goes in a table of its own (`dsperate.hires3d` is `hires3d` in `[console.NDS.dsperate]`),
and `[state]` holds whatever the app keeps between runs. DSlot keeps its own settings this way.

## For DSlot (feature `dslot`)

An emulator describes itself to DSlot in `/usr/share/dslot/emulators/<id>.toml`: which systems
it plays and the command for each, its options (which DSlot shows as settings per console and
per game, and passes when they are set) and actions on a game's files. `nibkit::dslot` reads
and checks those files the way DSlot does: `Emulators::load(&description_dirs())`. The format
and how DSlot starts a program (it lets go of both screens first, takes them back when the
program exits): `docs/emulators.md` in the DSlot repository.

Apps (`dslot::apps`) are folders with an `app.toml`, in `Roms/APPS` on either card and, on
nibOS, in `/usr/share/dslot/apps` for apps a package installs (`platform::SYSTEM_APPS`):
`docs/apps.md` in the DSlot repository.

## Building and testing

```sh
cargo test --features dslot,menu-raster                                 # nibOS (a desktop on macOS)
cargo test --features desktop,dslot,menu-raster                         # a desktop
cargo test --no-default-features --features stock,dslot,menu-raster     # the stock OS
cargo clippy --all-targets --features dslot,menu-raster -- -D warnings
```

Every test runs on a desktop, Linux or macOS: none needs the handheld.

The example needs SDL2 (`sdl2-compat` on nibOS; Homebrew's `sdl2` on a Mac).
