//! DSlot's settings file, and an emulator's options as its description declares them, for a C
//! or C++ emulator's menu pages: the same rows, values and "Use …" words as DSlot's own pages
//! (DSlot's docs/emulators.md, "One place for each option").
//!
//! Values cross as text: a string as it is, a number in decimal, a toggle `true` or `false`; an
//! option parses them by its kind. A scope is `system`, a console (`NDS`) or a game
//! (`NDS/<ROM name>`, the ROM's file name without its extension).

use crate::{out, text};
use nibkit::dslot::Emulator;
use nibkit::settings::Value;
use nibkit::settings::{Def, Kind, Scope, Settings};
use std::ffi::{c_char, c_int, CString};

/// `NibSettings`: a settings file and the last string handed out.
pub struct NibSettings {
    settings: Settings,
    scratch: CString,
}

/// `NibOptions`: one emulator's options for one system, and the last strings handed out.
pub struct NibOptions {
    emulator: Emulator,
    system: String,
    /// Indexes into the emulator's options of those for `system`.
    shown: Vec<usize>,
    scratch: CString,
    words: Vec<CString>,
}

fn scope_of(s: &str) -> Scope {
    match s {
        "" | "system" => Scope::System,
        s if s.contains('/') => Scope::Game(s.to_owned()),
        s => Scope::Console(s.to_owned()),
    }
}

/// A value as C sees it.
pub(crate) fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Integer(n) => n.to_string(),
        Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// Text from C as one of `def`'s values, if it is one.
pub(crate) fn parse(def: &Def, s: &str) -> Option<Value> {
    let v = match &def.kind {
        Kind::Toggle => Value::Boolean(matches!(s, "true" | "on" | "1")),
        Kind::Number { .. } => Value::Integer(s.trim().parse().ok()?),
        Kind::Choice(choices) => choices.iter().find(|(v, _)| value_text(v) == s)?.0.clone(),
    };
    def.accepts(&v).then_some(v)
}

/// Runs `f` on the handle behind `p`, if it isn't NULL.
///
/// # Safety
/// `p` is NULL or a live handle of this type, not used elsewhere during the call.
unsafe fn with<H, T>(p: *mut H, or: T, f: impl FnOnce(&mut H) -> T) -> T {
    // SAFETY: the caller's promise.
    match unsafe { p.as_mut() } {
        Some(h) => f(h),
        None => or,
    }
}

// ---- Settings ----

/// DSlot's settings file (nibOS); NULL where there is none (the stock OS).
#[no_mangle]
pub extern "C" fn nib_settings_open() -> *mut NibSettings {
    match nibkit::dslot::settings_file() {
        Some(file) => open(file),
        None => std::ptr::null_mut(),
    }
}

fn open(file: std::path::PathBuf) -> *mut NibSettings {
    Box::into_raw(Box::new(NibSettings {
        settings: Settings::load(file),
        scratch: CString::default(),
    }))
}

/// A settings file of the app's own choosing.
///
/// # Safety
/// `path` is NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn nib_settings_open_file(path: *const c_char) -> *mut NibSettings {
    // SAFETY: the caller's promise.
    open(unsafe { text(path) }.into())
}

/// # Safety
/// `s` came from `nib_settings_open*` and isn't used again (NULL is ignored).
#[no_mangle]
pub unsafe extern "C" fn nib_settings_free(s: *mut NibSettings) {
    if !s.is_null() {
        // SAFETY: the caller's promise: ours, from Box::into_raw.
        drop(unsafe { Box::from_raw(s) });
    }
}

/// Reads the file again (after DSlot or another app changed it).
///
/// # Safety
/// `s` is a live handle.
#[no_mangle]
pub unsafe extern "C" fn nib_settings_reload(s: *mut NibSettings) {
    // SAFETY: the caller's promise.
    unsafe {
        with(s, (), |s| {
            s.settings = Settings::load(s.settings.file().to_path_buf())
        })
    }
}

/// Writes the file (atomically). 0 on success, -1 on failure.
///
/// # Safety
/// `s` is a live handle.
#[no_mangle]
pub unsafe extern "C" fn nib_settings_save(s: *mut NibSettings) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { with(s, -1, |s| if s.settings.save().is_ok() { 0 } else { -1 }) }
}

/// The value of `key` (`dsperate.hires3d`) set at `scope` exactly, as text; NULL if unset.
///
/// # Safety
/// `s` is a live handle; the strings are NUL-terminated. The result is valid until the next call
/// on `s`.
#[no_mangle]
pub unsafe extern "C" fn nib_settings_get(
    s: *mut NibSettings,
    scope: *const c_char,
    key: *const c_char,
) -> *const c_char {
    // SAFETY: the caller's promise.
    let (scope, key) = unsafe { (scope_of(&text(scope)), text(key)) };
    // SAFETY: as above.
    unsafe {
        with(s, std::ptr::null(), |s| {
            let v = s.settings.get(&scope, &key).map(value_text);
            out(&mut s.scratch, v)
        })
    }
}

/// Sets `key` at `scope` to a string, a number or a toggle (for keys no option describes; an
/// option's value is better set with `nib_option_set`, which parses it by its kind).
///
/// # Safety
/// As `nib_settings_get`.
#[no_mangle]
pub unsafe extern "C" fn nib_settings_set_text(
    s: *mut NibSettings,
    scope: *const c_char,
    key: *const c_char,
    value: *const c_char,
) {
    // SAFETY: the caller's promise.
    let (scope, key, value) = unsafe { (scope_of(&text(scope)), text(key), text(value)) };
    // SAFETY: as above.
    unsafe { with(s, (), |s| s.settings.set(scope, &key, Value::String(value))) }
}

/// # Safety
/// As `nib_settings_get`.
#[no_mangle]
pub unsafe extern "C" fn nib_settings_set_int(
    s: *mut NibSettings,
    scope: *const c_char,
    key: *const c_char,
    value: i64,
) {
    // SAFETY: the caller's promise.
    let (scope, key) = unsafe { (scope_of(&text(scope)), text(key)) };
    // SAFETY: as above.
    unsafe {
        with(s, (), |s| {
            s.settings.set(scope, &key, Value::Integer(value))
        })
    }
}

/// # Safety
/// As `nib_settings_get`.
#[no_mangle]
pub unsafe extern "C" fn nib_settings_set_bool(
    s: *mut NibSettings,
    scope: *const c_char,
    key: *const c_char,
    value: c_int,
) {
    // SAFETY: the caller's promise.
    let (scope, key) = unsafe { (scope_of(&text(scope)), text(key)) };
    // SAFETY: as above.
    unsafe {
        with(s, (), |s| {
            s.settings.set(scope, &key, Value::Boolean(value != 0))
        })
    }
}

/// Clears `key` at `scope`, so the level above applies there ("Use …").
///
/// # Safety
/// As `nib_settings_get`.
#[no_mangle]
pub unsafe extern "C" fn nib_settings_reset(
    s: *mut NibSettings,
    scope: *const c_char,
    key: *const c_char,
) {
    // SAFETY: the caller's promise.
    let (scope, key) = unsafe { (scope_of(&text(scope)), text(key)) };
    // SAFETY: as above.
    unsafe { with(s, (), |s| s.settings.reset(&scope, &key)) }
}

// ---- Options ----

fn options_handle(emulator: Emulator, system: String) -> *mut NibOptions {
    let shown = (0..emulator.options.len())
        .filter(|&i| emulator.options[i].applies(&system))
        .collect();
    Box::into_raw(Box::new(NibOptions {
        emulator,
        system,
        shown,
        scratch: CString::default(),
        words: Vec::new(),
    }))
}

/// The options of emulator `id` (`dsperate`) for `system` (`NDS`), from the descriptions DSlot
/// reads (the user's own replacing the package's); NULL if it has none installed.
///
/// # Safety
/// The strings are NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn nib_options_load(
    id: *const c_char,
    system: *const c_char,
) -> *mut NibOptions {
    // SAFETY: the caller's promise.
    let (id, system) = unsafe { (text(id), text(system)) };
    // As DSlot reads them: the last of its folders with `<id>.toml` wins (the user's own over
    // the package's).
    let text = nibkit::dslot::description_dirs()
        .iter()
        .rev()
        .find_map(|dir| std::fs::read_to_string(dir.join(format!("{id}.toml"))).ok());
    match text.map(|t| nibkit::dslot::check(&id, &t)) {
        Some(Ok(e)) => options_handle(e, system),
        Some(Err(e)) => {
            eprintln!("nibkit: {id}: {e}");
            std::ptr::null_mut()
        }
        None => std::ptr::null_mut(),
    }
}

/// The same from a description's text (the app's own copy, or a test's); NULL if it isn't valid.
///
/// # Safety
/// The strings are NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn nib_options_from_text(
    id: *const c_char,
    description: *const c_char,
    system: *const c_char,
) -> *mut NibOptions {
    // SAFETY: the caller's promise.
    let (id, description, system) = unsafe { (text(id), text(description), text(system)) };
    match nibkit::dslot::check(&id, &description) {
        Ok(e) => options_handle(e, system),
        Err(e) => {
            eprintln!("nibkit: {id}: {e}");
            std::ptr::null_mut()
        }
    }
}

/// # Safety
/// `o` came from `nib_options_*` and isn't used again (NULL is ignored).
#[no_mangle]
pub unsafe extern "C" fn nib_options_free(o: *mut NibOptions) {
    if !o.is_null() {
        // SAFETY: the caller's promise: ours, from Box::into_raw.
        drop(unsafe { Box::from_raw(o) });
    }
}

impl NibOptions {
    fn def(&self, i: c_int) -> Option<&Def> {
        let at = *self.shown.get(usize::try_from(i).ok()?)?;
        Some(&self.emulator.options[at].def)
    }

    fn option(&self, i: c_int) -> Option<&nibkit::dslot::EmuOption> {
        let at = *self.shown.get(usize::try_from(i).ok()?)?;
        Some(&self.emulator.options[at])
    }
}

/// How many options there are for the system.
///
/// # Safety
/// `o` is a live handle.
#[no_mangle]
pub unsafe extern "C" fn nib_options_count(o: *mut NibOptions) -> c_int {
    // SAFETY: the caller's promise.
    unsafe { with(o, 0, |o| o.shown.len() as c_int) }
}

/// Option `i`'s key in the settings file (`dsperate.hires3d`); NULL past the end.
///
/// # Safety
/// `o` is a live handle. Strings returned by `nib_option_*` are valid until the next call on `o`.
#[no_mangle]
pub unsafe extern "C" fn nib_option_key(o: *mut NibOptions, i: c_int) -> *const c_char {
    // SAFETY: the caller's promise.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let v = o.def(i).map(|d| d.key.clone());
            out(&mut o.scratch, v)
        })
    }
}

/// Option `i`'s label, as its row shows it.
///
/// # Safety
/// As `nib_option_key`.
#[no_mangle]
pub unsafe extern "C" fn nib_option_label(o: *mut NibOptions, i: c_int) -> *const c_char {
    // SAFETY: the caller's promise.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let v = o.def(i).map(|d| d.label.clone());
            out(&mut o.scratch, v)
        })
    }
}

/// 0 a choice, 1 a toggle, 2 a number; -1 past the end.
///
/// # Safety
/// As `nib_option_key`.
#[no_mangle]
pub unsafe extern "C" fn nib_option_kind(o: *mut NibOptions, i: c_int) -> c_int {
    // SAFETY: the caller's promise.
    unsafe {
        with(o, -1, |o| match o.def(i).map(|d| &d.kind) {
            Some(Kind::Choice(_)) => 0,
            Some(Kind::Toggle) => 1,
            Some(Kind::Number { .. }) => 2,
            None => -1,
        })
    }
}

/// A choice's number of values.
///
/// # Safety
/// As `nib_option_key`.
#[no_mangle]
pub unsafe extern "C" fn nib_option_choices(o: *mut NibOptions, i: c_int) -> c_int {
    // SAFETY: the caller's promise.
    unsafe {
        with(o, 0, |o| match o.def(i).map(|d| &d.kind) {
            Some(Kind::Choice(c)) => c.len() as c_int,
            _ => 0,
        })
    }
}

/// A choice's value `j` as text (`label` 0) or its label as shown (`label` 1).
///
/// # Safety
/// As `nib_option_key`.
#[no_mangle]
pub unsafe extern "C" fn nib_option_choice(
    o: *mut NibOptions,
    i: c_int,
    j: c_int,
    label: c_int,
) -> *const c_char {
    // SAFETY: the caller's promise.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let v = match o.def(i).map(|d| &d.kind) {
                Some(Kind::Choice(c)) => usize::try_from(j)
                    .ok()
                    .and_then(|j| c.get(j))
                    .map(|(v, l)| if label != 0 { l.clone() } else { value_text(v) }),
                _ => None,
            };
            out(&mut o.scratch, v)
        })
    }
}

/// A number's range and step; returns its unit ("x", "%", or empty), NULL if it isn't a number.
///
/// # Safety
/// As `nib_option_key`; the out-pointers are NULL or writable.
#[no_mangle]
pub unsafe extern "C" fn nib_option_range(
    o: *mut NibOptions,
    i: c_int,
    min: *mut i64,
    max: *mut i64,
    step: *mut i64,
) -> *const c_char {
    // SAFETY: the caller's promise.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let Some(Kind::Number {
                min: lo,
                max: hi,
                step: st,
                unit,
            }) = o.def(i).map(|d| d.kind.clone())
            else {
                return std::ptr::null();
            };
            for (p, v) in [(min, lo), (max, hi), (step, st)] {
                if let Some(p) = p.as_mut() {
                    *p = v;
                }
            }
            out(&mut o.scratch, Some(unit))
        })
    }
}

/// Option `i`'s default as text; NULL when it has none (the emulator's own: nothing is passed).
///
/// # Safety
/// As `nib_option_key`.
#[no_mangle]
pub unsafe extern "C" fn nib_option_default(o: *mut NibOptions, i: c_int) -> *const c_char {
    // SAFETY: the caller's promise.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let v = o.def(i).and_then(|d| d.default.as_ref()).map(value_text);
            out(&mut o.scratch, v)
        })
    }
}

/// A value as its row shows it ("2x (512x384)", "On").
///
/// # Safety
/// As `nib_option_key`; `value` is NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn nib_option_show(
    o: *mut NibOptions,
    i: c_int,
    value: *const c_char,
) -> *const c_char {
    // SAFETY: the caller's promise.
    let value = unsafe { text(value) };
    // SAFETY: as above.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let v = o.def(i).and_then(|d| Some(d.show(&parse(d, &value)?)));
            out(&mut o.scratch, v)
        })
    }
}

/// The value one step (`dir` -1 or +1) from `from` (NULL: from the default), as text: round a
/// choice, held at a number's ends.
///
/// # Safety
/// As `nib_option_show`; `from` may be NULL.
#[no_mangle]
pub unsafe extern "C" fn nib_option_step(
    o: *mut NibOptions,
    i: c_int,
    from: *const c_char,
    dir: c_int,
) -> *const c_char {
    // SAFETY: the caller's promise.
    let from = (!from.is_null()).then(|| unsafe { text(from) });
    // SAFETY: as above.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let v = o.def(i).map(|d| {
                let from = from.as_deref().and_then(|f| parse(d, f));
                value_text(&d.step(from.as_ref(), dir))
            });
            out(&mut o.scratch, v)
        })
    }
}

/// The chain of levels seen from a page: the game's (`rom` given) or the console's.
fn chain(system: &str, rom: Option<String>) -> Vec<Scope> {
    match rom {
        Some(rom) => Scope::chain(system, &rom).to_vec(),
        None => Scope::console_chain(system).to_vec(),
    }
}

/// Option `i`'s value seen from the game's page (`rom`: the ROM's name without its extension) or
/// the console's (`rom` NULL), as text; NULL when set nowhere and it has no default. `*level`
/// says where it comes from: 3 the game, 2 the console, 1 the system, 0 its default or nowhere.
///
/// # Safety
/// `o` and `s` are live handles; the strings are NUL-terminated (`rom` may be NULL); `level` is
/// NULL or writable. The result is valid until the next call on `o`.
#[no_mangle]
pub unsafe extern "C" fn nib_option_value(
    o: *mut NibOptions,
    i: c_int,
    s: *mut NibSettings,
    rom: *const c_char,
    level: *mut c_int,
) -> *const c_char {
    // SAFETY: the caller's promise.
    let rom = (!rom.is_null()).then(|| unsafe { text(rom) });
    // SAFETY: as above.
    let Some(settings) = (unsafe { s.as_ref() }) else {
        return std::ptr::null();
    };
    // SAFETY: as above.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let Some(def) = o.def(i).cloned() else {
                return std::ptr::null();
            };
            let chain = chain(&o.system, rom);
            let from = match settings.settings.source(&chain, &def) {
                Some(Scope::Game(_)) => 3,
                Some(Scope::Console(_)) => 2,
                Some(Scope::System) => 1,
                None => 0,
            };
            if let Some(l) = level.as_mut() {
                *l = from;
            }
            let v = settings
                .settings
                .value(&chain, &def)
                .map(|v| value_text(&v));
            out(&mut o.scratch, v)
        })
    }
}

/// What option `i`'s row shows when not set at its page's level, in DSlot's words: from the
/// game's page "Use NDS (2x)" when the console sets it; else "Use default (…)" for an option with
/// a default, or "Use DSperate's own" (the emulator's name) for one without, which isn't passed.
///
/// # Safety
/// As `nib_option_value` (without `level`).
#[no_mangle]
pub unsafe extern "C" fn nib_option_inherit(
    o: *mut NibOptions,
    i: c_int,
    s: *mut NibSettings,
    rom: *const c_char,
) -> *const c_char {
    // SAFETY: the caller's promise.
    let for_game = !rom.is_null();
    // SAFETY: as above.
    let Some(settings) = (unsafe { s.as_ref() }) else {
        return std::ptr::null();
    };
    // SAFETY: as above.
    unsafe {
        with(o, std::ptr::null(), |o| {
            let Some(def) = o.def(i).cloned() else {
                return std::ptr::null();
            };
            // The level above this page's: the console's for a game, nothing above a console's
            // (an emulator's options aren't set for the whole system).
            let above = Scope::Console(o.system.clone());
            let shown = |v: &Value| def.show(v).split(" (").next().unwrap_or("").to_owned();
            let set = if for_game {
                settings
                    .settings
                    .get(&above, &def.key)
                    .filter(|v| def.accepts(v))
                    .cloned()
            } else {
                None
            };
            let v = match (set, &def.default) {
                (Some(v), _) => format!("Use {} ({})", o.system, shown(&v)),
                (None, Some(d)) => format!("Use default ({})", shown(d)),
                (None, None) => format!("Use {}'s own", o.emulator.name),
            };
            out(&mut o.scratch, Some(v))
        })
    }
}

/// Sets option `i` at `scope` to `value` (text, parsed by its kind). 0 if set, -1 if `value`
/// isn't one of its values.
///
/// # Safety
/// As `nib_option_value`; `scope` and `value` are NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn nib_option_set(
    o: *mut NibOptions,
    i: c_int,
    s: *mut NibSettings,
    scope: *const c_char,
    value: *const c_char,
) -> c_int {
    // SAFETY: the caller's promise.
    let (scope, value) = unsafe { (scope_of(&text(scope)), text(value)) };
    // SAFETY: as above.
    let Some(settings) = (unsafe { s.as_mut() }) else {
        return -1;
    };
    // SAFETY: as above.
    unsafe {
        with(o, -1, |o| {
            let Some(def) = o.def(i) else { return -1 };
            match parse(def, &value) {
                Some(v) => {
                    settings.settings.set(scope, &def.key, v);
                    0
                }
                None => -1,
            }
        })
    }
}

/// The command-line words `value` adds for option `i`; returns how many, each then from
/// `nib_option_word`.
///
/// # Safety
/// As `nib_option_show`.
#[no_mangle]
pub unsafe extern "C" fn nib_option_words(
    o: *mut NibOptions,
    i: c_int,
    value: *const c_char,
) -> c_int {
    // SAFETY: the caller's promise.
    let value = unsafe { text(value) };
    // SAFETY: as above.
    unsafe {
        with(o, 0, |o| {
            let words = o
                .option(i)
                .and_then(|opt| Some(opt.words(&parse(&opt.def, &value)?)))
                .unwrap_or_default();
            o.words = words
                .into_iter()
                .map(|w| CString::new(w).unwrap_or_default())
                .collect();
            o.words.len() as c_int
        })
    }
}

/// Word `j` of those `nib_option_words` made.
///
/// # Safety
/// `o` is a live handle; the result is valid until the next `nib_option_words` on it.
#[no_mangle]
pub unsafe extern "C" fn nib_option_word(o: *mut NibOptions, j: c_int) -> *const c_char {
    // SAFETY: the caller's promise.
    unsafe {
        with(o, std::ptr::null(), |o| {
            usize::try_from(j)
                .ok()
                .and_then(|j| o.words.get(j))
                .map_or(std::ptr::null(), |w| w.as_ptr())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    fn s(p: *const c_char) -> Option<String> {
        // SAFETY: the tests read what these calls just returned.
        (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }

    const DESCRIPTION: &str = r#"
name = "DSperate"
priority = 30

[systems]
NDS = ["/usr/bin/dsperate", "{options}", "{rom}"]
GBA = ["/usr/bin/dsperate", "--gba", "{options}", "{rom}"]

[[options]]
key = "hires3d"
label = "3D resolution"
systems = ["NDS"]
values = [1, 2, 4]
labels = ["1x (256x192)", "2x (512x384)", "4x (1024x768)"]
args = ["--hires3d", "{value}"]

[[options]]
key = "linear"
label = "Smooth picture"
systems = ["NDS", "GBA"]
kind = "toggle"
args = ["--linear"]
off = ["--no-linear"]
"#;

    #[test]
    fn options_settings_and_their_rows() {
        let dir = std::env::temp_dir().join(format!("nibkit-c-opts-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let file = c(dir.join("settings.toml").to_str().unwrap());
        unsafe {
            let o = nib_options_from_text(
                c("dsperate").as_ptr(),
                c(DESCRIPTION).as_ptr(),
                c("NDS").as_ptr(),
            );
            if o.is_null() {
                // The description format may have moved on: say so rather than pass silently.
                panic!("the test's description no longer parses");
            }
            assert_eq!(nib_options_count(o), 2);
            assert_eq!(s(nib_option_label(o, 0)).as_deref(), Some("3D resolution"));
            assert_eq!(nib_option_kind(o, 0), 0);
            assert_eq!(nib_option_choices(o, 0), 3);
            assert_eq!(s(nib_option_choice(o, 0, 1, 0)).as_deref(), Some("2"));
            assert_eq!(
                s(nib_option_choice(o, 0, 1, 1)).as_deref(),
                Some("2x (512x384)")
            );
            assert_eq!(nib_option_kind(o, 1), 1);
            let st = nib_settings_open_file(file.as_ptr());
            let rom = c("Mole Patrol");
            // Set nowhere: the emulator's own.
            assert_eq!(
                s(nib_option_inherit(o, 0, st, rom.as_ptr())).as_deref(),
                Some("Use DSperate's own")
            );
            assert_eq!(
                nib_option_set(o, 0, st, c("NDS").as_ptr(), c("2").as_ptr()),
                0
            );
            assert_eq!(
                nib_option_set(o, 0, st, c("NDS").as_ptr(), c("3").as_ptr()),
                -1,
                "not one of its values"
            );
            assert_eq!(
                s(nib_option_inherit(o, 0, st, rom.as_ptr())).as_deref(),
                Some("Use NDS (2x)")
            );
            let mut level = -1;
            assert_eq!(
                s(nib_option_value(o, 0, st, rom.as_ptr(), &mut level)).as_deref(),
                Some("2")
            );
            assert_eq!(level, 2, "from the console");
            nib_option_set(o, 0, st, c("NDS/Mole Patrol").as_ptr(), c("4").as_ptr());
            assert_eq!(
                s(nib_option_value(o, 0, st, rom.as_ptr(), &mut level)).as_deref(),
                Some("4")
            );
            assert_eq!(level, 3, "from the game");
            assert_eq!(
                s(nib_settings_get(
                    st,
                    c("NDS/Mole Patrol").as_ptr(),
                    c("dsperate.hires3d").as_ptr()
                ))
                .as_deref(),
                Some("4")
            );
            nib_settings_reset(
                st,
                c("NDS/Mole Patrol").as_ptr(),
                c("dsperate.hires3d").as_ptr(),
            );
            assert_eq!(nib_settings_save(st), 0);
            nib_settings_reload(st);
            assert_eq!(
                s(nib_option_value(o, 0, st, rom.as_ptr(), &mut level)).as_deref(),
                Some("2")
            );
            // Showing, stepping, the command line.
            assert_eq!(
                s(nib_option_show(o, 0, c("2").as_ptr())).as_deref(),
                Some("2x (512x384)")
            );
            assert_eq!(
                s(nib_option_step(o, 0, c("4").as_ptr(), 1)).as_deref(),
                Some("1"),
                "round a choice"
            );
            assert_eq!(nib_option_words(o, 0, c("2").as_ptr()), 2);
            assert_eq!(s(nib_option_word(o, 1)).as_deref(), Some("2"));
            assert_eq!(nib_option_words(o, 1, c("false").as_ptr()), 1);
            assert_eq!(s(nib_option_word(o, 0)).as_deref(), Some("--no-linear"));
            nib_settings_free(st);
            nib_options_free(o);
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
