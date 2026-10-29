# nib-cheevos

RetroAchievements for nibOS: one implementation for every emulator and for DSlot, the launcher,
through [rcheevos](https://github.com/RetroAchievements/rcheevos), the service's own library
(vendored under `rcheevos/`, MIT, never edited). The network is `curl`'s: nib-cheevos has no TLS
of its own. Nothing here is English meant for the player but the service's own titles and
messages: each app words what happens itself.

| Module | Feature | |
|---|---|---|
| `client` | `client` | An emulator's session, driven once per emulated frame: signed in with the launcher's token, the game's set loaded by its hash and evaluated against the machine's memory (the `Memory` trait, in the service's address space for the console), what happens as typed `Event`s (unlocks, progress and challenge indicators, leaderboards, the game completed, the service failing), the set's progress kept in save states, `Settings { hardcore, encore, unofficial }`. Requests go out on a thread of their own; a request `curl` could never make is not retried, anything else is, holding an unlock until it is sent. |
| `badges` | `client` | The badges' pictures, fetched into the cache every app shares. |
| `hash` | `hash`, `zip`, `chd` | A game's identity, by rhash, for every console nibOS plays, named as the `Roms/` folders are (`NDS`, `GBA`, `GB`, `GBC`, `SFC`, `FC`, `FDS`, `MD`, `SMS`, `GG`, `SG1000`, `LYNX`, `NGP`, `PCE`, `PCECD`, `PS`, `WS`, `GW`) with rcheevos' console id for each; a file or bytes; a disc from its cue sheet or playlist; with `zip`, a zip is its first entry with one of the console's extensions; with `chd`, a CHD disc image, read by a CD reader over the `chd` crate (BSD-3-Clause). |
| `api` | `api` | What a launcher asks outside a game: signing in, a game's sets by its hash, the player's unlocks, a badge; built and read by rcheevos' `rc_api_*`, every answer checked the same way (a refused or expired token is `Error::Refused`, whichever request). |
| `libretro` | `libretro` | For a program that runs libretro cores, by rcheevos' `rc_libretro.c`: `Regions`, the console's memory in the service's address space over the core's own (its memory map, `RETRO_ENVIRONMENT_SET_MEMORY_MAPS`, or else its memory ids), with rcheevos' quirks for each console, read in place and a `Memory` for the session; `disallowed_settings(core).allows(key, value)` and `system_allowed(core, console)`, rcheevos' rules for a core's options and consoles. |
| `launch` | | The sign-in from DSlot to the game: `RA_TOKEN_FILE`, `RA_USERNAME`, `RA_HARDCORE`, the token file (two lines, the username then the token, 0600, written whole or not at all), `Launch::from_env()`. |
| `ua` | | The User-Agent: `user_agent("inkdot", "0.1.0", None)` is `inkdot/0.1.0 (nibOS; linux aarch64) rcheevos/12.4`. |
| `paths` | | The service's hosts, the badge cache (`$XDG_CACHE_HOME/retroachievements/badges` for an absolute `XDG_CACHE_HOME`, else `~/.cache/...`), `WARNING_ID` and `is_warning` for the service's "Unknown Emulator" entries. |

`client`, `hash`, `zip` and `api` are on by default; `chd` (CHD disc images) and `libretro` are
asked for. An emulator takes `client` and `hash`, a launcher `zip` (with `hash`) and `api`, and
either `chd` where it plays discs:

```toml
nib-cheevos = { git = "https://github.com/abradburne/nibkit", tag = "v0.2.2", default-features = false, features = ["client", "hash"] }
nib-cheevos = { git = "https://github.com/abradburne/nibkit", tag = "v0.2.2", default-features = false, features = ["zip", "api"] }
```

An emulator built on libretro cores adds `libretro`:

```toml
nib-cheevos = { git = "https://github.com/abradburne/nibkit", tag = "v0.2.2", default-features = false, features = ["client", "hash", "zip", "libretro"] }
```

rcheevos is C: building needs a C compiler. With the feature `libretro`, rcheevos'
`rc_libretro.c` is built too, against the copy of `libretro.h` (the libretro API's header, MIT, its
licence at its top) in `libretro/`, or the one in the folder `LIBRETRO_INCLUDE_DIR` names.

## Not yet

- Game & Watch units: rhash 12.4 has no rule for them.
- Hardcore: the session supports it, but the service hasn't approved nibOS's emulators for it;
  DSlot doesn't offer it yet.

MIT, as are rcheevos (`rcheevos/LICENSE`) and `libretro/libretro.h` (its licence at its top).
