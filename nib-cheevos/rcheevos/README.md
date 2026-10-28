# Vendored rcheevos

Upstream: <https://github.com/RetroAchievements/rcheevos>, branch `master`, commit
`2ad0b8672f68a48148620164510b963039e49eb1` (2026-07-23), version 12.4.0, MIT (`LICENSE`).
Taken verbatim: `include/`, `src/`, `LICENSE`; the two `.natvis` files (Visual Studio debugger
views) are left out. Nothing here is modified: the achievement logic has to agree with the
service's, and a local change to the condition evaluator or the hash would be a bug that never
shows locally. What nib-cheevos needs done differently is done in `../csrc/shim.c`, through the
callbacks rcheevos provides. (Until nibkit 0.2.0 this copy was inkdot's, in
`crates/inkdot-cheevos/rcheevos`; it is the same commit.)

## What is compiled

`../build.rs` builds, with `RC_HASH_NO_ENCRYPTED`, `RC_HASH_NO_ZIP` and `RC_DISABLE_LUA`:

| files | why |
|---|---|
| `src/rc_client.c` | the session: sign-in, the game's set, unlocks |
| `src/rapi/*.c` | the service's requests and answers (the session's, and the launcher's own) |
| `src/rcheevos/*.c` | the achievement runtime (the whole directory: its units reference each other) |
| `src/rhash/*.c` but `aes.c`, `hash_encrypted.c`, `hash_zip.c` | a game's identity: cartridges, and discs through rhash's own CD reader (`cdreader.c`) |
| `src/rc_util.c`, `rc_compat.c`, `rc_version.c` | what those need |
| `src/rc_libretro.c`, only with the feature `libretro` | a libretro core's memory map and the settings rcheevos allows it; needs `libretro.h`, which is not here |

Not built: the encrypted-image hashing (3DS) and the zip hashing (arcade sets, MS-DOS), which
are for consoles nibOS doesn't play (a zipped game is unpacked by `../src/hash.rs` before it is
hashed), `rc_client_raintegration.c` (Windows authoring toolkit) and `rc_client_external.c`.
