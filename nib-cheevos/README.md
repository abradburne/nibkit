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
| `hash` | `hash`, `zip` | A game's identity, by rhash, for every console nibOS plays, named as the `Roms/` folders are (`NDS`, `GBA`, `GB`, `GBC`, `SFC`, `FC`, `FDS`, `MD`, `SMS`, `GG`, `SG1000`, `LYNX`, `NGP`, `PCE`, `PCECD`, `PS`, `WS`, `GW`) with rcheevos' console id for each; a file or bytes; a disc from its cue sheet or playlist; with `zip`, a zip is its first entry with one of the console's extensions. |
| `api` | `api` | What a launcher asks outside a game: signing in, a game's sets by its hash, the player's unlocks, a badge; built and read by rcheevos' `rc_api_*`, every answer checked the same way (a refused or expired token is `Error::Refused`, whichever request). |
| `launch` | | The sign-in from DSlot to the game: `RA_TOKEN_FILE`, `RA_USERNAME`, `RA_HARDCORE`, the token file (two lines, the username then the token, 0600, written whole or not at all), `Launch::from_env()`. |
| `ua` | | The User-Agent: `user_agent("inkdot", "0.1.0", None)` is `inkdot/0.1.0 (nibOS; linux aarch64) rcheevos/12.4`. |
| `paths` | | The service's hosts, the badge cache (`$XDG_CACHE_HOME/retroachievements/badges` for an absolute `XDG_CACHE_HOME`, else `~/.cache/...`), `WARNING_ID` and `is_warning` for the service's "Unknown Emulator" entries. |

Every feature is on by default. An emulator takes `client` and `hash`, a launcher `zip` (with
`hash`) and `api`:

```toml
nib-cheevos = { git = "https://github.com/abradburne/nibkit", tag = "v0.2.0", default-features = false, features = ["client", "hash"] }
nib-cheevos = { git = "https://github.com/abradburne/nibkit", tag = "v0.2.0", default-features = false, features = ["zip", "api"] }
```

rcheevos is C: building needs a C compiler. With the feature `libretro`, rcheevos'
`rc_libretro.c` (a libretro core's memory map, the settings and systems rcheevos allows a core) is
built too, against the `libretro.h` in the folder `LIBRETRO_INCLUDE_DIR` names; its functions are
in the library for a program to declare, nib-cheevos has no Rust face for them yet.

## Not yet

- CHD disc images: rhash has no CHD reader; one would be handed to it as a custom CD reader.
- Game & Watch units: rhash 12.4 has no rule for them.
- Hardcore: the session supports it, but the service hasn't approved nibOS's emulators for it;
  DSlot doesn't offer it yet.

MIT, as is rcheevos (`rcheevos/LICENSE`).
