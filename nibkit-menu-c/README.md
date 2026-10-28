# nibkit-menu-c

nibkit for C and C++ apps: a static library and its header, `include/nibkit_menu.h`. An emulator
written in C++ (DSperate) shows the same menu as DSlot and inkdot through it:

- **The menu** (nibkit's `menu`): it fills a page a call at a time, passes on the buttons and
  touches, and draws the RGBA picture it gets back on the bottom screen; the top screen while
  paused, or on a shelf of save states, the same way.
- **The Connections page:** Wi-Fi and Bluetooth on or off and the paired devices to connect, as
  DSlot shows them, for a pause menu: `nib_connections_new`, `_fill`, `_act` (nibOS; NULL on the
  stock OS).
- **Toasts:** a short notice over a game that is playing, in DSlot's style (a heading coloured
  by its kind, a title with its points, a line under it, a picture such as an achievement's
  badge at the left), drawn into a picture of its own size with alpha and the corner to lay it
  at, for a frame of any size: `nib_toast_begin` (or `_unlock`), `_header`, `_detail`,
  `_points`, `_badge`, `_render`; and `nib_toasts_*`, the queue that shows them one at a time
  for their kind's time, an unlock waiting a moment for its badge.
- **RetroAchievements' indicators:** `nib_indicators_*`, the progress and challenges up while
  rcheevos says, at the bottom left: the progress a panel in the toasts' style, each challenge
  its badge alone, see-through, in a pulsing gold ring (`nib_toast_progress`,
  `nib_toast_challenge` say what each is);
  `nib_toast_badge_rect`, `_pulse`, `_stack` for an app that lays its own.
- **Versions:** `NIBKIT_MENU_VERSION` in the header, `nib_menu_version()` in the library.
- **What the pages show:** DSlot's settings file and the emulator's options as its description
  declares them (values, "Use NDS (2x)", stepping, the command-line words), and the System page's
  brightness, clock, battery and volume.

```bash
cargo build --release                 # target/release/libnibkit_menu.a
c++ -I include app.cpp target/release/libnibkit_menu.a -lpthread -ldl -lm
```

Built for nibOS by default; `--no-default-features --features stock` for the stock OS. The
layout, colours, face (Roboto Condensed, SIL OFL 1.1: the library embeds it, so ship [OFL.txt](OFL.txt) with it) and button rules are nibkit's: see its
`menu` module.
