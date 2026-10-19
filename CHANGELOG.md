# Changelog

## 0.1.0 (unreleased)

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
