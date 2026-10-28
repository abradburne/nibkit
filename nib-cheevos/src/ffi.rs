//! The C shim's API (`csrc/shim.h`), declared for Rust: the shim's own structs and numbers,
//! never rcheevos'. [`tests`] checks the structs' sizes against the shim's.

#![allow(dead_code)]

use std::ffi::{c_char, c_int, c_void};

/// A client (`struct nib_rc`): opaque.
#[repr(C)]
pub struct NibRc {
    _private: [u8; 0],
}

/// `NIB_RC_EVENT_*`.
pub const EVENT_UNLOCKED: u32 = 1;
pub const EVENT_LEADERBOARD_STARTED: u32 = 2;
pub const EVENT_LEADERBOARD_FAILED: u32 = 3;
pub const EVENT_LEADERBOARD_SUBMITTED: u32 = 4;
pub const EVENT_CHALLENGE_SHOW: u32 = 5;
pub const EVENT_CHALLENGE_HIDE: u32 = 6;
pub const EVENT_PROGRESS_SHOW: u32 = 7;
pub const EVENT_PROGRESS_HIDE: u32 = 8;
pub const EVENT_PROGRESS_UPDATE: u32 = 9;
pub const EVENT_TRACKER_SHOW: u32 = 10;
pub const EVENT_TRACKER_HIDE: u32 = 11;
pub const EVENT_TRACKER_UPDATE: u32 = 12;
pub const EVENT_RESET: u32 = 13;
pub const EVENT_GAME_COMPLETED: u32 = 14;
pub const EVENT_SUBSET_COMPLETED: u32 = 15;
pub const EVENT_SERVER_ERROR: u32 = 16;
pub const EVENT_DISCONNECTED: u32 = 17;
pub const EVENT_RECONNECTED: u32 = 18;

/// `NIB_RC_*` results.
pub const OK: c_int = 0;
pub const FAILED: c_int = 1;
pub const UNKNOWN_GAME: c_int = 2;
pub const BAD_TOKEN: c_int = 3;
pub const NO_SIGN_IN: c_int = 4;
pub const NO_ANSWER: c_int = 5;

/// `NIB_RC_NOT_REACHED`, rcheevos' `RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR`.
pub const NOT_REACHED: c_int = -2;
/// `NIB_RC_CANNOT_ASK`, rcheevos' `RC_API_SERVER_RESPONSE_CLIENT_ERROR`.
pub const CANNOT_ASK: c_int = -1;

/// `struct nib_rc_achievement`.
#[repr(C)]
pub struct Achievement {
    pub id: u32,
    pub points: u32,
    pub title: *const c_char,
    pub description: *const c_char,
    pub badge_name: *const c_char,
    pub badge_url: *const c_char,
    pub measured_progress: *const c_char,
    pub measured_percent: f32,
}

/// `struct nib_rc_leaderboard`.
#[repr(C)]
pub struct Leaderboard {
    pub id: u32,
    pub title: *const c_char,
    pub description: *const c_char,
    pub tracker_value: *const c_char,
}

/// `struct nib_rc_tracker`.
#[repr(C)]
pub struct Tracker {
    pub id: u32,
    pub display: *const c_char,
}

/// `struct nib_rc_event`.
#[repr(C)]
pub struct Event {
    pub kind: u32,
    pub achievement: *const Achievement,
    pub leaderboard: *const Leaderboard,
    pub tracker: *const Tracker,
    pub subset_title: *const c_char,
    pub error_api: *const c_char,
    pub error_message: *const c_char,
}

pub type ReadFn =
    extern "C" fn(address: u32, buffer: *mut u8, num_bytes: u32, ud: *mut c_void) -> u32;
pub type RequestFn = extern "C" fn(
    url: *const c_char,
    post_data: *const c_char,
    content_type: *const c_char,
    ticket: *mut c_void,
    ud: *mut c_void,
);
pub type EventFn = extern "C" fn(event: *const Event, ud: *mut c_void);
pub type DoneFn = extern "C" fn(result: c_int, error_message: *const c_char, ud: *mut c_void);
pub type LogFn = extern "C" fn(message: *const c_char, ud: *mut c_void);

/// `struct nib_rc_cdreader`: a disc reader for rhash (rcheevos' `rc_hash_cdreader_t`).
#[repr(C)]
pub struct CdReader {
    pub open_track: extern "C" fn(path: *const c_char, track: u32) -> *mut c_void,
    pub read_sector:
        extern "C" fn(track: *mut c_void, sector: u32, buffer: *mut c_void, bytes: usize) -> usize,
    pub close_track: extern "C" fn(track: *mut c_void),
    pub first_track_sector: extern "C" fn(track: *mut c_void) -> u32,
}

/// `struct nib_rc_callbacks`.
#[repr(C)]
pub struct Callbacks {
    pub read: ReadFn,
    pub request: RequestFn,
    pub event: EventFn,
    pub login_done: DoneFn,
    pub load_done: DoneFn,
    pub log: Option<LogFn>,
}

/// `struct nib_rc_settings`.
#[repr(C)]
pub struct Settings {
    pub hardcore: c_int,
    pub encore: c_int,
    pub unofficial: c_int,
}

/// `struct nib_rc_summary`.
#[repr(C)]
#[derive(Default)]
pub struct Summary {
    pub total: u32,
    pub unlocked: u32,
    pub unsupported: u32,
    pub points: u32,
    pub points_earned: u32,
    pub unofficial: u32,
}

/// `struct nib_rc_status`.
#[repr(C)]
pub struct Status {
    pub result: c_int,
    pub error_message: *const c_char,
    pub error_code: *const c_char,
    pub http_status: c_int,
}

/// `struct nib_rc_login`.
#[repr(C)]
pub struct Login {
    pub status: Status,
    pub username: *const c_char,
    pub token: *const c_char,
    pub display_name: *const c_char,
    pub score: u32,
    pub score_softcore: u32,
    pub unread_messages: u32,
}

/// `struct nib_rc_game`.
#[repr(C)]
pub struct Game {
    pub status: Status,
    pub id: u32,
    pub console_id: u32,
    pub title: *const c_char,
    pub image_name: *const c_char,
    pub image_url: *const c_char,
    pub num_sets: u32,
}

/// `struct nib_rc_set_achievement`.
#[repr(C)]
pub struct SetAchievement {
    pub set_id: u32,
    pub set_game_id: u32,
    pub set_type: u32,
    pub set_title: *const c_char,
    pub id: u32,
    pub points: u32,
    pub category: u32,
    pub kind: u32,
    pub title: *const c_char,
    pub description: *const c_char,
    pub badge_name: *const c_char,
    pub badge_url: *const c_char,
    pub badge_locked_url: *const c_char,
    pub rarity: f32,
    pub rarity_hardcore: f32,
}

/// `struct nib_rc_unlocks`.
#[repr(C)]
pub struct Unlocks {
    pub status: Status,
    pub ids: *const u32,
    pub count: u32,
}

pub type LoginFn = extern "C" fn(login: *const Login, ud: *mut c_void);
pub type GameFn = extern "C" fn(game: *const Game, ud: *mut c_void);
pub type SetAchievementFn = extern "C" fn(achievement: *const SetAchievement, ud: *mut c_void);
pub type UnlocksFn = extern "C" fn(unlocks: *const Unlocks, ud: *mut c_void);

extern "C" {
    pub fn nib_rc_create(
        callbacks: *const Callbacks,
        settings: *const Settings,
        ud: *mut c_void,
    ) -> *mut NibRc;
    pub fn nib_rc_destroy(rc: *mut NibRc);
    pub fn nib_rc_respond(
        ticket: *mut c_void,
        http_status: c_int,
        body: *const c_char,
        body_length: usize,
    );
    pub fn nib_rc_discard(ticket: *mut c_void);
    pub fn nib_rc_login_with_token(rc: *mut NibRc, username: *const c_char, token: *const c_char);
    pub fn nib_rc_load_game(rc: *mut NibRc, hash: *const c_char);
    pub fn nib_rc_unload_game(rc: *mut NibRc);
    pub fn nib_rc_do_frame(rc: *mut NibRc);
    pub fn nib_rc_idle(rc: *mut NibRc);
    pub fn nib_rc_reset(rc: *mut NibRc);
    pub fn nib_rc_progress_size(rc: *mut NibRc) -> usize;
    pub fn nib_rc_save_progress(rc: *mut NibRc, buffer: *mut u8, size: usize) -> c_int;
    pub fn nib_rc_load_progress(rc: *mut NibRc, buffer: *const u8, size: usize) -> c_int;
    pub fn nib_rc_display_name(rc: *const NibRc) -> *const c_char;
    pub fn nib_rc_game_title(rc: *const NibRc) -> *const c_char;
    pub fn nib_rc_game_id(rc: *const NibRc) -> u32;
    pub fn nib_rc_get_summary(rc: *const NibRc, out: *mut Summary);
    pub fn nib_rc_hardcore(rc: *const NibRc) -> c_int;

    pub fn nib_rc_hash_file(
        out: *mut c_char,
        console_id: u32,
        path: *const c_char,
        log: Option<LogFn>,
        verbose: c_int,
        ud: *mut c_void,
    ) -> c_int;
    pub fn nib_rc_hash_disc(
        out: *mut c_char,
        console_id: u32,
        path: *const c_char,
        reader: *const CdReader,
        log: Option<LogFn>,
        verbose: c_int,
        ud: *mut c_void,
    ) -> c_int;
    pub fn nib_rc_hash_buffer(
        out: *mut c_char,
        console_id: u32,
        name: *const c_char,
        data: *const u8,
        size: usize,
        log: Option<LogFn>,
        verbose: c_int,
        ud: *mut c_void,
    ) -> c_int;

    pub fn nib_rc_api_login(
        username: *const c_char,
        password: *const c_char,
        token: *const c_char,
        built: RequestFn,
        ud: *mut c_void,
    ) -> c_int;
    pub fn nib_rc_api_game_sets(
        username: *const c_char,
        token: *const c_char,
        hash: *const c_char,
        built: RequestFn,
        ud: *mut c_void,
    ) -> c_int;
    pub fn nib_rc_api_unlocks(
        username: *const c_char,
        token: *const c_char,
        game_id: u32,
        hardcore: c_int,
        built: RequestFn,
        ud: *mut c_void,
    ) -> c_int;
    pub fn nib_rc_api_read_login(
        http_status: c_int,
        body: *const c_char,
        body_length: usize,
        done: LoginFn,
        ud: *mut c_void,
    );
    pub fn nib_rc_api_read_game_sets(
        http_status: c_int,
        body: *const c_char,
        body_length: usize,
        game: GameFn,
        achievement: SetAchievementFn,
        ud: *mut c_void,
    );
    pub fn nib_rc_api_read_unlocks(
        http_status: c_int,
        body: *const c_char,
        body_length: usize,
        done: UnlocksFn,
        ud: *mut c_void,
    );

    pub fn nib_rc_user_agent_clause(out: *mut c_char, size: usize) -> usize;
    pub fn nib_rc_version() -> *const c_char;
    #[cfg(test)]
    pub fn nib_rc_sizes(out: *mut usize);
}

/// A C string from the shim, copied; empty for null.
///
/// # Safety
///
/// `p` is null or a NUL-terminated string valid for the call.
pub unsafe fn string(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: as the caller promises.
    unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn the_structs_are_the_shims() {
        let mut sizes = [0usize; 12];
        // SAFETY: the shim writes twelve sizes.
        unsafe { nib_rc_sizes(sizes.as_mut_ptr()) };
        assert_eq!(
            sizes,
            [
                size_of::<Event>(),
                size_of::<Achievement>(),
                size_of::<Leaderboard>(),
                size_of::<Tracker>(),
                size_of::<Callbacks>(),
                size_of::<Settings>(),
                size_of::<Summary>(),
                size_of::<Status>(),
                size_of::<Login>(),
                size_of::<Game>(),
                size_of::<SetAchievement>(),
                size_of::<Unlocks>(),
            ]
        );
    }
}
