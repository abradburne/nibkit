# Changelog

## 0.1.0 (unreleased)

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
