/* The sizes of rc_libretro's structs and libretro's memory map as C lays them out, for
 * src/libretro.rs's tests: its #[repr(C)] mirrors must match. Built with the feature
 * `libretro` only (build.rs). */

#include <stddef.h>

#include "rc_libretro.h"

size_t nib_rl_sizeof_regions(void) { return sizeof(rc_libretro_memory_regions_t); }
size_t nib_rl_sizeof_core_memory(void) { return sizeof(rc_libretro_core_memory_info_t); }
size_t nib_rl_sizeof_descriptor(void) { return sizeof(struct retro_memory_descriptor); }
size_t nib_rl_sizeof_map(void) { return sizeof(struct retro_memory_map); }
size_t nib_rl_sizeof_setting(void) { return sizeof(rc_disallowed_setting_t); }
