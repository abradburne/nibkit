# nibkit-menu-c

nibkit's `menu` (DSlot's menu, for an app's touch screen) for C and C++ apps: a static library
and its header, `include/nibkit_menu.h`. An emulator written in C++ (DSperate) shows the same
menu as DSlot and inkdot through it: it fills a page a call at a time, passes on the buttons and
touches, and draws the RGBA picture it gets back on the bottom screen.

```bash
cargo build --release                 # target/release/libnibkit_menu.a
c++ -I include app.cpp target/release/libnibkit_menu.a -lpthread -ldl -lm
```

Built for nibOS by default; `--no-default-features --features stock` for the stock OS. The
layout, colours, face (Roboto Condensed, Apache-2.0) and button rules are nibkit's: see its
`menu` module.
