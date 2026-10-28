# Changelog

nibkit, nibkit-menu-c and nib-cheevos, which are released together. Until 0.1.0 nibkit and
nibkit-menu-c lived in nibOS's repository (`sdk/nibkit`, `sdk/nibkit-menu-c`), where their
history starts.

## 0.2.1

- **nib-cheevos** `libretro` (feature `libretro`): Rust bindings for rcheevos' `rc_libretro.c`, for
  a program that runs libretro cores. `Regions::new(console_id, map, memory)` lays the console's
  memory out in the service's address space over the core's own, from its memory map
  (`RETRO_ENVIRONMENT_SET_MEMORY_MAPS`, as `MemoryDescriptor`s) or, without one, from what
  `retro_get_memory_data` and `retro_get_memory_size` answer (`CoreMemory`), with rcheevos'
  console table and its quirks for each; it reads in place (`Regions::read`, and it is a
  `Memory` for `Client::frame`), `backed()` and `regions()` say what it found.
  `disallowed_settings(library_name).allows(key, value)` and `system_allowed(library_name,
  console_id)` are rcheevos' rules for a core's options and consoles; `verbose(true)` logs how it
  maps a console. The feature builds against a copy of `libretro.h` (the API's header, MIT) in
  `nib-cheevos/libretro/` unless `LIBRETRO_INCLUDE_DIR` names the program's own, so a program
  needs nothing set to use it. Tests: the structs' layouts against C's, a map and the memory ids
  for SNES, NES, Mega Drive, Game Gear, PC Engine, Neo Geo Pocket and PlayStation, the rules
  for Snes9x and PCSX ReARMed.
- **nib-cheevos** `chd` (feature `chd`): CHD disc images are hashed (`hash::file` on a `.chd`, or
  on a playlist whose first disc is one), through a CD reader of rhash's shape over the `chd`
  crate (chd-rs, BSD-3-Clause), which decompresses the image's hunks: the tracks from the
  image's `CHT2`/`CHTR` metadata, padded as CHD pads them, sectors numbered as on the disc
  (pregaps stored with a track included), a sector's user data as rhash's own reader gives it.
  Checked on the RG DS+: Gran Turismo's `.chd` hashes to the service's hash for the game
  (13e08c49...). Without the feature a CHD is `Error::Unsupported`, as before. The shim has
  `nib_rc_hash_disc`, rhash with a program's own CD reader.
- A patch version: everything is added; nothing in 0.2.0 changes. nibkit-menu-c:
  `NIBKIT_MENU_VERSION` 201, the same calls.

## 0.2.0

- **nib-cheevos**, a new crate (MIT): RetroAchievements for every nibOS emulator and DSlot, one
  implementation through rcheevos 12.4 (vendored, once, in `nib-cheevos/rcheevos`; it was
  inkdot's `inkdot-cheevos`, which it replaces). `client` (feature `client`): the `rc_client`
  session driven once a frame, the console's memory read through the `Memory` trait, typed
  `Event`s, `Settings { hardcore, encore, unofficial }`, the set's progress for save states, a
  `curl` transport on a thread that tells rcheevos to retry anything but a request `curl`
  could never make; `badges`, the shared badge cache filled on a thread. `hash` (features
  `hash`, `zip`): rhash for a file or bytes, for every console by its `Roms/` folder name, one
  table to rcheevos' console ids (`CONSOLES`), discs from a cue sheet or playlist, a zip as its
  first entry with one of the console's extensions (`zip_member`); not CHD, nor Game & Watch
  (rhash has neither). `api` (feature `api`): signing in, a game's sets
  (`achievementsets`) and the player's unlocks, built and read by rcheevos' `rc_api_*`, every
  answer checked the same way (a refused or expired token is `Error::Refused`, whichever
  request), by `curl`. `launch`: `RA_TOKEN_FILE`, `RA_USERNAME`, `RA_HARDCORE`, the token file
  (two lines, 0600, written whole), `Launch::from_env()`. `ua`: `user_agent(product, version,
  core)` with rcheevos' clause. `paths`: the hosts, the badge cache (an absolute
  `XDG_CACHE_HOME` only), `WARNING_ID` and `is_warning`. The feature `libretro` builds rcheevos'
  `rc_libretro.c` too, against a `libretro.h` the program has (`LIBRETRO_INCLUDE_DIR`).
- `dslot`: an emulator's description says where it earns RetroAchievements, `[achievements]
  systems = [...]` (some of its `[systems]`; another is an error) and `hardcore` (default
  false): `Emulator::achievements`, `Emulator::earns(system)`, and `Emulators::earning()`, the
  systems the installed emulators earn in. DSlot passes the sign-in only to those.
- A minor version: a crate is new, and `Emulator` has a field more. nibkit-menu-c:
  `NIBKIT_MENU_VERSION` 200, the same calls.

## 0.1.4

- `menu::indicators`: a challenge is now just its badge, not a panel: an icon `ICON` (56) units
  square with rounded corners, in the gold ring that pulses, the whole icon `ICON_OPACITY`
  (0.8) opaque so it is less in the way; no heading, no title. The icons sit in a row under the
  progress panel, the newest at the right, those that don't fit across left out, the oldest
  first; one whose badge doesn't come shows a dark square in its ring. A challenge for the
  achievement in progress is still shown in the progress panel, headed "Challenge", ringed. The
  progress panel is as it was. nibkit-menu-c: `NIBKIT_MENU_VERSION` 104, the same calls.

## 0.1.3

- `menu::indicators` (feature `menu-raster`): RetroAchievements' progress and challenge
  indicators, the same in every app. `Indicators` keeps them while rcheevos says they are up
  (`progress(id, Notice)`, `hide_progress`, `challenge(id, Notice)`, `end_challenge`, `clear`,
  `badge(name, ..)`): the progress a panel in the toast's style headed "Progress"; each
  challenge a panel of its own headed "Challenge" in gold, its badge at 56 units in a gold ring
  that pulses slowly (1.8 s); a challenge for the achievement whose progress is shown one panel,
  not two, the progress under its title. Stacked at the bottom left, opposite the toasts: the
  progress at the top, the challenges under it, the newest last, the oldest challenges left out
  first when they don't fit; one awaiting its badge waits for it up to `BADGE_WAIT`. `tick(now)`
  once a frame says when the picture changes; `draw(fonts, frame)` gives a `Stack` (one picture
  and its place), its panels drawn only when what they say or the frame changes, the rings
  painted again `PULSE_RATE` (16) times a second (about 0.1 ms; the panels 1.4 ms, a Mac).
- `menu::toast`: `Kind::Progress` and `Kind::Challenge` (`Said`/`Notice::progress(title,
  measured)`, `::challenge(title)`, headed `PROGRESS` and `CHALLENGE`); `Toast::badge_rect`;
  `pulse(rgba, size, badge, scale, strength)`, a gold ring painted into a drawn panel's pixels,
  and `pulse_strength(since)`; `stack(frame, scale, sizes, pinned)`, where indicator panels go.
  A badge is never drawn under `BADGE_LEAST` (56 units): a toast of two lines is 2 units taller,
  its lines centred beside it. nibkit-menu-c: `NIB_TOAST_PROGRESS`, `NIB_TOAST_CHALLENGE`,
  `nib_toast_progress`, `_challenge`, `_badge_rect`, `_pulse`, `_pulse_strength`, `_stack`, and
  `nib_indicators_*`; `NIBKIT_MENU_VERSION` 103.

## 0.1.2

- `menu::toast` (feature `menu-raster`): the toast over a game that is playing, in DSlot's
  style, and the way toasts come and go, the same in every app. A `Queue` of `Notice`s
  (owned; `Notice::new(kind, title)`, `Notice::unlock(title, detail, points)` with the
  "Achievement unlocked" heading, and setters for the heading, detail, points, badge and time):
  `push`, then `tick(now)` once a frame for the `Shown` one, one at a time and in turn, each
  for its `Kind::shows()` (an unlock 4 s, anything else 2.5 s); an unlock whose badge is on its
  way (`awaits_badge(name)`) waits for `Queue::badge(name, ..)` up to `BADGE_WAIT` (1.5 s) at
  the head of the queue. `Queue::draw(fonts, scale)` draws the shown toast's picture once and
  keeps it; `Shown::serial` says when it changed. `draw(fonts, &Said, scale)` draws one toast
  from borrowed words (`Said::new(kind, title)` and setters, `Said::unlock`) into a `Toast`
  (RGBA with straight alpha, its own size), as wide as its longest line within two thirds of
  the screen, long lines ending in an ellipsis; `lay` puts it over a `Canvas`, `place` and
  `Toast::place` say where on any frame: the bottom right, `MARGIN` (16 units) in. `Kind` and
  `Said` are `#[non_exhaustive]`, so either can grow. nibkit-menu-c's `nib_toast_*` and
  `nib_toasts_*`, and inkdot's and DSperate's notices, are this one.
- nibkit-menu-c: the toast and its queue for C and C++, in the `nib_menu_*` style, so they can
  grow: `nib_toast_begin(t, kind, title)` or `nib_toast_unlock(t, title, detail, points)`,
  then `_header`, `_detail`, `_points`, `_badge` (copied), `_await_badge`, `_shows`, and
  `nib_toast_render(t, frame_w, frame_h, ..)` giving the picture and where on the frame it
  goes; `nib_toasts_*`, the queue (`_push`, `_badge`, `_tick(now_ms)`, `_render`,
  `_shown_id`, `_clear`). `NIBKIT_MENU_VERSION` and `nib_menu_version()`,
  `nib_menu_scale_for(w, h)`. `nib_menu_*` pictures are laid by their alpha, as nibkit's are.
- `menu::scale_for(w, h)`: the pixels to a unit for a frame of any size (the 1024 × 768 screen
  fitted in it), the one scale every app's drawing uses; a scale that isn't a number draws at
  1, a tiny one at 0.05. `colour::EDGE`, a toast's edge.
- `menu::raster`: `Canvas::blank(w, h, scale)`, a clear picture of its own size. A `Picture`
  is laid over what is under it by its alpha (a transparent pixel was drawn black). Rounded
  boxes and frames sample only the pixels their edge crosses: a states shelf draws in 1.7 ms
  instead of 13.7, a menu page in 0.78 instead of 0.92 (a Mac, release build).

## 0.1.1

- `paths::rom_roots` (nibOS) leaves the TF2 card's folders out when there is no card in the
  slot: a path under an unmounted mount point is an empty folder at best, and under the
  automount nibOS used to have, each touch of it waited the device timeout out (5 s) with the
  slot empty, so a shelf took minutes to fill. nibOS now mounts the card from udev when it
  appears.
- `paths::wait_for_cards(timeout)`: waits for a card the kernel has seen but udev hasn't
  mounted yet, for a launcher listing games at power-on; returns at once otherwise (and on
  the stock OS and desktops).

## 0.1.0

- `dslot`: `[games]` in a description names a file listing the only games it plays of a
  system (name, title, model, year, series, tab-separated): `Emulator::takes`,
  `Emulators::choose_for`, `Emulators::unit` and `Unit`, for an emulator that plays some of a
  system's games (inktick, Game & Watch).
- `device::set_wifi` and `wifi` use nibOS's Wi-Fi switch, `nib wifi on|off|status` (the Wi-Fi
  radio alone, by rfkill; Bluetooth carries on), falling back to `nib net status` on an older
  nibOS. `device::network_up` (`nib net up`) brings BlueZ up for Bluetooth where the network
  mode left it down, without turning Wi-Fi on.
- `menu::glyphs` and `Value::Status`: a row's state in words and then small pictures (a
  network's signal as the status bar's fan of arcs, bright and dim, a lock, a check; a
  device's battery as the battery icon), drawn by `glyphs::render` at the text's height, so
  every app's (and `menu-raster`'s) are the same shapes. The Connections page shows them:
  `device_value` is a `Value` now, `network_value` new. `net::radios::Net::secured`.
- `net::radios` (feature `net-control`): a worker thread for a menu's Wi-Fi and Bluetooth pages:
  a snapshot of both read every couple of seconds, one thing done at a time (scan, join,
  leave, forget, power, look, pair, connect, disconnect, remove), pairing's questions answered
  through `Radios::reply`. From DSlot, which now uses it.
- `menu::connections` (features `menu`, `net-control`): the Connections page any app's pause
  menu can include: Wi-Fi and Bluetooth on or off, and the paired devices connected or
  disconnected with one press, with their battery, as DSlot's pages show them.
  `Connections::start`, `refresh`, `title`, `rows`, `act`; `HINT`; `device_value`.
- `menu::keyboard`: the on-screen keyboard, for a passphrase, a PIN or a search: letters,
  symbols, or a number pad for digits; laid out as `Draw`s like the menu; the D-pad and A to
  type, B to delete, Y to show a secret, L shift, R symbols, Start done, Select cancel, or a tap
  on a key.
- `net::wifi` and `net::bluetooth` (feature `net-control`, zbus): Wi-Fi through iwd's D-Bus API
  (status, scan, the networks in range with security and signal, joining with a passphrase
  through an agent, disconnecting, known networks, forgetting) and Bluetooth through BlueZ's
  (power, the devices with their kind, battery and signal, discovery, pairing with an agent
  that asks the app (PIN, passkey, confirmation), trust, connect, disconnect, remove).
  `net::Error`: unsupported (the stock OS, a desktop), off, or failed and why in a person's
  words. Example: `radios`.
- `dslot::apps` also lists the apps installed with the system (`platform::SYSTEM_APPS`:
  `/usr/share/dslot/apps` on nibOS), for a package's app, which can't write to the cards.
- `net`: the radios for a status bar: Wi-Fi off, connecting or connected (with signal bars),
  Bluetooth off, on or connected (how many, and whether one is a gamepad). Read from sysfs,
  `iw` and BlueZ's `Powered`, never hard: `net::Monitor` reads them on a thread every few
  seconds. Unknown on the stock OS and a desktop.
- `desktop` (feature, and the choice on any target but Linux): an app made for the handheld
  builds and runs on a computer: windows for the screens (`screens::desktop_windows`, the mouse
  on the bottom one as touch with `Rect::panel_point`), a keyboard map (`input::keyboard`,
  `input::Button`), the platform's own app folders, games in nibOS's data folder or
  `NIBKIT_ROMS`, and the handheld's controls `None`. Apps may say `runs_on = ["desktop"]`.
- `menu`: a states shelf's resume point (`Card::resume`, before the numbered slots made with
  `Card::slot`): set apart on the left and drawn differently, with a RESUME tag; A loads it, X
  and Y do nothing on it, and the slot counter leaves it out. `Shelf::first_slot` is where the
  shelf opens. `layout::card_box` and `card_hit` now take the shelf and give card indices.
  nibkit-menu-c: `nib_menu_resume_card`, `nib_menu_first_slot`.
- Ready for crates.io: metadata, the files a package carries, documentation on everything
  public, Rust 1.87.

- First version, from DSlot's code: screens and their placement, vblank waits, face-button
  labels, the touch panel, per-screen brightness, volume and Wi-Fi, audio advice, app folders,
  layered settings (system / console / game), and DSlot's description files (feature
  `dslot`). nibOS and the stock OS as features.
- `power`: the power menu's request (SIGUSR1); `device::sleep`, `restart`, `can_sleep`.
- `backlight::dim`: both screens dimmed for a while, their levels put back exactly (and not
  saved) when it is dropped: for a screensaver.
- `backlight::step`: a 5% step that lands on multiples of 5 (39 goes to 40 or 35).
- `device::watch_volume`: the default output's volume changed by anyone, from PipeWire's change
  events (`pactl subscribe`).
- `input::Repeat`: a held button repeating (first after a delay, then steadily, faster once
  held 1.5 s), never storing up repeats it couldn't keep up with.
- `clock`: a 12-hour or 24-hour clock for every app (`~/.config/nibos/prefs.toml`, else the
  locale's habit), and the time formatted that way.
- `menu` (feature): the menu every app shows on the touch screen, as DSlot draws it: a page of
  rows under tabs, or a shelf of cards (save states), laid out as `Draw`s in DSlot's sizes and
  colours; the buttons and touches answered as DSlot's menu does; with `menu-raster`, drawn into
  an RGBA picture in Roboto Condensed (SIL OFL 1.1, embedded in `assets/` with its `OFL.txt`; first given here as Apache-2.0, which older Roboto releases were). For emulators' in-game
  menus, so every one looks and works as DSlot.
