//! Builds the vendored rcheevos (rcheevos/README.md says which files and why) and the C shim
//! over it into one static library. With the feature `libretro`, rcheevos' `rc_libretro.c` too,
//! against the libretro.h in the folder `LIBRETRO_INCLUDE_DIR` names, or the copy in
//! `libretro/` (rcheevos doesn't ship one).

fn main() {
    let rc = "rcheevos";
    let mut build = cc::Build::new();
    build
        .include(format!("{rc}/include"))
        .include(format!("{rc}/src"))
        .include(format!("{rc}/src/rapi"))
        .include(format!("{rc}/src/rhash"))
        .file("csrc/shim.c")
        .file(format!("{rc}/src/rc_client.c"))
        .file(format!("{rc}/src/rc_util.c"))
        .file(format!("{rc}/src/rc_compat.c"))
        .file(format!("{rc}/src/rc_version.c"));
    // Whole directories: their translation units reference each other freely (trigger ->
    // condset -> condition -> operand -> memref; hash -> hash_rom, hash_disc, cdreader), so a
    // subset would break on an update. But rhash's encrypted-image and zip hashing, which the
    // defines below leave out of hash.c (and the AES they need).
    let left_out = ["aes.c", "hash_encrypted.c", "hash_zip.c"];
    for dir in ["rapi", "rcheevos", "rhash"] {
        let mut files: Vec<_> = std::fs::read_dir(format!("{rc}/src/{dir}"))
            .expect("the vendored rcheevos")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "c"))
            .filter(|p| {
                !p.file_name()
                    .is_some_and(|n| left_out.iter().any(|o| n == *o))
            })
            .collect();
        files.sort();
        for f in files {
            build.file(f);
        }
    }
    if std::env::var_os("CARGO_FEATURE_LIBRETRO").is_some() {
        // The program's own libretro.h when it names one, else the copy here (libretro/, the
        // API's header, MIT): rc_libretro.c reads only its memory map and memory ids, which
        // every version since the map's has laid out the same.
        println!("cargo:rerun-if-env-changed=LIBRETRO_INCLUDE_DIR");
        let dir = std::env::var_os("LIBRETRO_INCLUDE_DIR").unwrap_or_else(|| "libretro".into());
        build
            .include(dir)
            .file(format!("{rc}/src/rc_libretro.c"))
            .file("csrc/libretro.c");
    }
    // Encrypted images (3DS) and zip hashing (arcade sets, MS-DOS) are for consoles nibOS
    // doesn't play; a zipped game is unpacked here before it is hashed (src/hash.rs). Lua is
    // not part of the achievement language the service uses. Discs are hashed: the PC Engine
    // CD's and the PlayStation's, by rhash's own CD reader.
    for def in ["RC_HASH_NO_ENCRYPTED", "RC_HASH_NO_ZIP", "RC_DISABLE_LUA"] {
        build.define(def, None);
    }
    // rcheevos is upstream's, warnings and all; the shim is held to the usual standard.
    build.warnings(false).compile("nib_rcheevos");
    println!("cargo:rerun-if-changed=csrc");
    println!("cargo:rerun-if-changed=rcheevos/src");
    println!("cargo:rerun-if-changed=rcheevos/include");
    println!("cargo:rerun-if-changed=libretro");
    println!("cargo:rerun-if-changed=build.rs");
}
