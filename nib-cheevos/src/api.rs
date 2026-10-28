//! The service's requests a launcher makes on its own, outside a game: signing in, a game's
//! sets by its hash, and the player's unlocks in it, for a shelf to show. Each request is built,
//! and each answer read, by rcheevos' own `rc_api_*` functions (`rc_api_init_login_request`,
//! `rc_api_init_fetch_game_sets_request`, `rc_api_init_fetch_user_unlocks_request` and their
//! `rc_api_process_*`), so they are spelt and checked exactly as an emulator's session does;
//! nothing here knows a URL or a JSON field.
//!
//! [`Service`] makes them by `curl`, one at a time, waiting for each: call it on a thread of its
//! own. The parts are public too ([`login_request`], [`read_login`]...), for a test that plays
//! the service, or a program with a transport of its own.
//!
//! Every answer is checked the same way: a refusal of the sign-in (a wrong password, a token
//! revoked or expired: the service's codes `invalid_credentials`, `expired_token`,
//! `access_denied`, or an HTTP 401 or 403) is [`Error::Refused`], whichever request it answers;
//! the network down is [`Error::NoNetwork`]; anything else [`Error::Failed`], in the service's
//! own words.

use crate::ffi::{self, string};
use crate::launch::Credentials;
use crate::paths::{badge_url, is_warning};
pub use crate::transport::Request;
use crate::transport::{self, Failure};
use std::ffi::{c_char, c_int, c_void, CString};
use std::path::{Path, PathBuf};

/// Why a request came to nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The service refused the sign-in: a wrong password, an unknown user, a token no longer
    /// good. The service's own words.
    Refused(String),
    /// The service couldn't be reached: no network, or it didn't answer in time.
    NoNetwork,
    /// Something else went wrong (an odd answer, the service failing, `curl` missing). What was
    /// said.
    Failed(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Refused(why) => write!(f, "refused: {why}"),
            Error::NoNetwork => write!(f, "the service couldn't be reached"),
            Error::Failed(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<Failure> for Error {
    fn from(f: Failure) -> Error {
        if f.offline {
            Error::NoNetwork
        } else {
            Error::Failed(f.why)
        }
    }
}

/// What the service says of the player when they sign in.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Login {
    /// The username as the service spells it.
    pub username: String,
    /// The token to keep: it signs the player in from then on, without the password.
    pub token: String,
    /// The name the service shows for them.
    pub display_name: String,
    /// Hardcore points.
    pub score: u32,
    /// Softcore points.
    pub softcore: u32,
    /// Unread messages on the site.
    pub messages: u32,
}

impl std::fmt::Debug for Login {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Login")
            .field("username", &self.username)
            .field("token", &"<redacted>")
            .field("display_name", &self.display_name)
            .field("score", &self.score)
            .field("softcore", &self.softcore)
            .field("messages", &self.messages)
            .finish()
    }
}

/// A game, as the service knows it by a hash: its sets' achievements together.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Game {
    /// The service's id.
    pub id: u32,
    /// rcheevos' console id.
    pub console_id: u32,
    /// Its title at the service.
    pub title: String,
    /// The name of its icon on the media host (`Images/<name>.png`).
    pub image_name: String,
    /// Every achievement of every set the service gave for the hash (the core set, and the
    /// bonus sets that go with it), in its order; never the service's warnings
    /// ([`crate::paths::is_warning`]).
    pub achievements: Vec<Achievement>,
}

impl Game {
    /// The game ids the player's unlocks are asked for by ([`Service::unlocks`]): one per set,
    /// each once.
    pub fn unlock_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = Vec::new();
        for a in &self.achievements {
            let id = if a.set_game_id == 0 {
                self.id
            } else {
                a.set_game_id
            };
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.is_empty() {
            ids.push(self.id);
        }
        ids
    }
}

/// One achievement of a game's sets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Achievement {
    /// The service's id.
    pub id: u32,
    /// Its name.
    pub title: String,
    /// What earns it.
    pub description: String,
    /// What it is worth.
    pub points: u32,
    /// The service's name for its badge ("12345"): `<name>.png` on the media host, and in the
    /// badge cache.
    pub badge_name: String,
    /// Where its badge is.
    pub badge_url: String,
    /// Published (the service's "core" category), as against unofficial.
    pub official: bool,
    /// The set it belongs to, and the game id that set's unlocks are asked for by.
    pub set_id: u32,
    /// See [`Achievement::set_id`].
    pub set_game_id: u32,
}

/// What a sign-in signs in with.
#[derive(Clone, Copy)]
pub enum Secret<'a> {
    /// The password, typed: sent once, kept nowhere.
    Password(&'a str),
    /// The token kept from an earlier sign-in.
    Token(&'a str),
}

/// The `login2` request: signing `username` in with a password or a token.
pub fn login_request(username: &str, secret: Secret<'_>) -> Result<Request, Error> {
    let user = c(username)?;
    let (password, token) = match secret {
        Secret::Password(p) => (Some(c(p)?), None),
        Secret::Token(t) => (None, Some(c(t)?)),
    };
    let ptr = |s: &Option<CString>| s.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    build(|built, ud| {
        // SAFETY: the strings are NUL-terminated (or null) and valid for the call; `built` and
        // `ud` are valid for the call.
        unsafe { ffi::nib_rc_api_login(user.as_ptr(), ptr(&password), ptr(&token), built, ud) }
    })
}

/// The `achievementsets` request: the game a hash is, and its sets.
pub fn game_request(credentials: &Credentials, hash: &str) -> Result<Request, Error> {
    let (user, token, hash) = (c(&credentials.username)?, c(&credentials.token)?, c(hash)?);
    build(|built, ud| {
        // SAFETY: as login_request.
        unsafe {
            ffi::nib_rc_api_game_sets(user.as_ptr(), token.as_ptr(), hash.as_ptr(), built, ud)
        }
    })
}

/// The `unlocks` request: the achievements the player has of the game with this id, in
/// hardcore or in softcore.
pub fn unlocks_request(
    credentials: &Credentials,
    game_id: u32,
    hardcore: bool,
) -> Result<Request, Error> {
    let (user, token) = (c(&credentials.username)?, c(&credentials.token)?);
    build(|built, ud| {
        // SAFETY: as login_request.
        unsafe {
            ffi::nib_rc_api_unlocks(
                user.as_ptr(),
                token.as_ptr(),
                game_id,
                hardcore.into(),
                built,
                ud,
            )
        }
    })
}

/// A sign-in's answer, read.
pub fn read_login(http_status: i32, body: &[u8]) -> Result<Login, Error> {
    let mut out: Option<Result<Login, Error>> = None;
    extern "C" fn done(login: *const ffi::Login, ud: *mut c_void) {
        // SAFETY: the shim's struct and its strings are valid for the call; `ud` is `out`,
        // lent for the call.
        let (l, out) = unsafe { (&*login, &mut *ud.cast::<Option<Result<Login, Error>>>()) };
        // SAFETY: as above.
        let read = unsafe {
            checked(&l.status).map(|()| Login {
                username: string(l.username),
                token: string(l.token),
                display_name: string(l.display_name),
                score: l.score,
                softcore: l.score_softcore,
                messages: l.unread_messages,
            })
        };
        *out = Some(read);
    }
    with_body(body, |body, len| {
        // SAFETY: the body is valid for `len` bytes and NUL-terminated past them; the callback
        // and `out` are valid for the call.
        unsafe { ffi::nib_rc_api_read_login(http_status, body, len, done, (&raw mut out).cast()) }
    });
    let login = out.unwrap_or_else(|| Err(Error::Failed("no answer read".into())))?;
    if login.username.is_empty() || login.token.is_empty() {
        return Err(Error::Failed(
            "the service's answer had no token in it".into(),
        ));
    }
    Ok(login)
}

/// A game's sets' answer, read: the game, or `None` when the service doesn't know the hash.
pub fn read_game(http_status: i32, body: &[u8]) -> Result<Option<Game>, Error> {
    struct Reading {
        game: Option<Result<Game, Error>>,
    }
    let mut r = Reading { game: None };
    extern "C" fn game(g: *const ffi::Game, ud: *mut c_void) {
        // SAFETY: the shim's struct and its strings are valid for the call; `ud` is `r`, lent
        // for the call.
        let (g, r) = unsafe { (&*g, &mut *ud.cast::<Reading>()) };
        // SAFETY: as above.
        let read = unsafe {
            checked(&g.status).map(|()| Game {
                id: g.id,
                console_id: g.console_id,
                title: string(g.title).trim().to_owned(),
                image_name: string(g.image_name),
                achievements: Vec::new(),
            })
        };
        r.game = Some(read);
    }
    extern "C" fn achievement(a: *const ffi::SetAchievement, ud: *mut c_void) {
        // SAFETY: as game().
        let (a, r) = unsafe { (&*a, &mut *ud.cast::<Reading>()) };
        let Some(Ok(game)) = r.game.as_mut() else {
            return;
        };
        if is_warning(a.id) {
            return;
        }
        // SAFETY: the shim's strings are valid for the call.
        let (title, description, badge_name, url) = unsafe {
            (
                string(a.title),
                string(a.description),
                string(a.badge_name),
                string(a.badge_url),
            )
        };
        let badge_url = if url.is_empty() && !badge_name.is_empty() {
            crate::paths::badge_url(&badge_name)
        } else {
            url
        };
        game.achievements.push(Achievement {
            id: a.id,
            title: title.trim().to_owned(),
            description: description.trim().to_owned(),
            points: a.points,
            badge_name,
            badge_url,
            official: a.category == 3,
            set_id: a.set_id,
            set_game_id: a.set_game_id,
        });
    }
    with_body(body, |body, len| {
        // SAFETY: as read_login; `r` is valid for the call.
        unsafe {
            ffi::nib_rc_api_read_game_sets(
                http_status,
                body,
                len,
                game,
                achievement,
                (&raw mut r).cast(),
            )
        }
    });
    match r.game {
        Some(Ok(g)) if g.id == 0 => Ok(None),
        Some(Ok(g)) => Ok(Some(g)),
        Some(Err(Error::Failed(why) | Error::Refused(why))) if unknown_game(&why) => Ok(None),
        Some(Err(e)) => Err(e),
        None => Err(Error::Failed("no answer read".into())),
    }
}

/// The player's unlocks' answer, read: the achievements' ids.
pub fn read_unlocks(http_status: i32, body: &[u8]) -> Result<Vec<u32>, Error> {
    let mut out: Option<Result<Vec<u32>, Error>> = None;
    extern "C" fn done(u: *const ffi::Unlocks, ud: *mut c_void) {
        // SAFETY: the shim's struct is valid for the call, its `ids` for `count`; `ud` is
        // `out`, lent for the call.
        let (u, out) = unsafe { (&*u, &mut *ud.cast::<Option<Result<Vec<u32>, Error>>>()) };
        // SAFETY: as above.
        let read = unsafe {
            checked(&u.status).map(|()| {
                if u.ids.is_null() || u.count == 0 {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(u.ids, u.count as usize).to_vec()
                }
            })
        };
        *out = Some(read);
    }
    with_body(body, |body, len| {
        // SAFETY: as read_login.
        unsafe { ffi::nib_rc_api_read_unlocks(http_status, body, len, done, (&raw mut out).cast()) }
    });
    out.unwrap_or_else(|| Err(Error::Failed("no answer read".into())))
}

/// The service, by `curl`, as a program that names itself `user_agent`
/// ([`crate::ua::user_agent`]).
#[derive(Clone, Debug)]
pub struct Service {
    curl: Option<PathBuf>,
    user_agent: String,
}

impl Service {
    /// The service, through the `curl` on `PATH` (or `/usr/bin/curl`).
    pub fn new(user_agent: impl Into<String>) -> Service {
        Service {
            curl: transport::curl_path(),
            user_agent: user_agent.into(),
        }
    }

    /// Whether there is a `curl` to make requests with.
    pub fn available(&self) -> bool {
        self.curl.is_some()
    }

    /// Makes a request and waits for the answer: its HTTP status and body.
    pub fn ask(&self, request: &Request) -> Result<(i32, Vec<u8>), Error> {
        let curl = self
            .curl
            .as_deref()
            .ok_or_else(|| Error::Failed("no curl to reach the service with".into()))?;
        Ok(transport::perform(curl, request, &self.user_agent)?)
    }

    /// Signs in (`login2`): what the service says of the player, and the token to keep.
    pub fn login(&self, username: &str, secret: Secret<'_>) -> Result<Login, Error> {
        let (status, body) = self.ask(&login_request(username, secret)?)?;
        read_login(status, &body)
    }

    /// The game a hash is, with its sets' achievements; `None` when the service doesn't know
    /// the hash.
    pub fn game(&self, credentials: &Credentials, hash: &str) -> Result<Option<Game>, Error> {
        let (status, body) = self.ask(&game_request(credentials, hash)?)?;
        read_game(status, &body)
    }

    /// The achievements the player has of the game with this id, in hardcore or in softcore.
    pub fn unlocks(
        &self,
        credentials: &Credentials,
        game_id: u32,
        hardcore: bool,
    ) -> Result<Vec<u32>, Error> {
        let (status, body) = self.ask(&unlocks_request(credentials, game_id, hardcore)?)?;
        read_unlocks(status, &body)
    }

    /// The game a hash is and every achievement of it the player has, softcore or hardcore,
    /// in every set; `None` when the service doesn't know the hash.
    pub fn game_and_unlocks(
        &self,
        credentials: &Credentials,
        hash: &str,
    ) -> Result<Option<(Game, Vec<u32>)>, Error> {
        let Some(game) = self.game(credentials, hash)? else {
            return Ok(None);
        };
        let mut unlocked = Vec::new();
        for id in game.unlock_ids() {
            for hardcore in [false, true] {
                for a in self.unlocks(credentials, id, hardcore)? {
                    if !unlocked.contains(&a) {
                        unlocked.push(a);
                    }
                }
            }
        }
        Ok(Some((game, unlocked)))
    }

    /// The badge `name` fetched from the media host into the file `to`, whole or not at all.
    pub fn badge(&self, name: &str, to: &Path) -> Result<(), Error> {
        let curl = self
            .curl
            .as_deref()
            .ok_or_else(|| Error::Failed("no curl to reach the service with".into()))?;
        Ok(transport::fetch(
            curl,
            &badge_url(name),
            to,
            &self.user_agent,
        )?)
    }
}

/// A string for the shim.
fn c(s: &str) -> Result<CString, Error> {
    CString::new(s).map_err(|_| Error::Failed("a NUL in a request".into()))
}

/// The request the shim builds through `call(built, ud)`.
fn build(call: impl FnOnce(ffi::RequestFn, *mut c_void) -> c_int) -> Result<Request, Error> {
    let mut out: Option<Request> = None;
    extern "C" fn built(
        url: *const c_char,
        post: *const c_char,
        content_type: *const c_char,
        _ticket: *mut c_void,
        ud: *mut c_void,
    ) {
        // SAFETY: the strings are valid for the call; `ud` is `out`, lent for the call.
        let (out, url, post) = unsafe {
            (
                &mut *ud.cast::<Option<Request>>(),
                string(url),
                (!post.is_null()).then(|| (string(content_type), string(post))),
            )
        };
        *out = Some(Request { url, post });
    }
    let result = call(built, (&raw mut out).cast());
    match out {
        Some(request) if result == ffi::OK => Ok(request),
        _ => Err(Error::Failed("rcheevos would not build the request".into())),
    }
}

/// Lends `body`, with a NUL after it (rcheevos reads it as a C string as well as by its
/// length), to `call(body, length)`.
fn with_body(body: &[u8], call: impl FnOnce(*const c_char, usize)) {
    let mut b = Vec::with_capacity(body.len() + 1);
    b.extend_from_slice(body);
    b.push(0);
    call(b.as_ptr().cast(), body.len());
}

/// What an answer's status comes to: fine, or the error to give.
///
/// # Safety
///
/// The status's strings are valid for the call.
unsafe fn checked(status: &ffi::Status) -> Result<(), Error> {
    // SAFETY: as the caller promises.
    let (message, code) = unsafe { (string(status.error_message), string(status.error_code)) };
    let why = || {
        let tidy = plain(&message);
        if tidy.is_empty() {
            if code.is_empty() {
                "The service said no".to_owned()
            } else {
                code.clone()
            }
        } else {
            tidy
        }
    };
    match status.result {
        ffi::OK => Ok(()),
        ffi::BAD_TOKEN => Err(Error::Refused(why())),
        ffi::UNKNOWN_GAME => Err(Error::Failed(if message.is_empty() {
            "Unknown game".into()
        } else {
            why()
        })),
        ffi::NO_ANSWER if message.is_empty() => {
            Err(Error::Failed("the service didn't answer".into()))
        }
        _ => Err(Error::Failed(why())),
    }
}

/// The service's error messages, without their trailing exhortations.
fn plain(why: &str) -> String {
    why.trim()
        .trim_end_matches("Please try again.")
        .trim()
        .to_owned()
}

/// The service's ways of saying it has no set for a hash.
fn unknown_game(why: &str) -> bool {
    let why = why.to_ascii_lowercase();
    why.contains("unknown game") || why.contains("not found") || why.contains("no game")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alan() -> Credentials {
        Credentials {
            username: "Alan".into(),
            token: "t/k".into(),
        }
    }

    /// The form a request posts, as `key=value` pairs, the values decoded.
    fn form(r: &Request) -> Vec<(String, String)> {
        let (content_type, form) = r.post.clone().expect("a POST");
        assert_eq!(content_type, "application/x-www-form-urlencoded");
        form.split('&')
            .map(|kv| {
                let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
                (k.to_owned(), decode(v))
            })
            .collect()
    }

    /// `%XX` back to its byte.
    fn decode(v: &str) -> String {
        let mut out = Vec::new();
        let mut bytes = v.bytes();
        while let Some(b) = bytes.next() {
            if b == b'%' {
                let hex: String = bytes.by_ref().take(2).map(char::from).collect();
                out.push(u8::from_str_radix(&hex, 16).unwrap());
            } else {
                out.push(b);
            }
        }
        String::from_utf8(out).unwrap()
    }

    fn has(r: &Request, key: &str, value: &str) -> bool {
        form(r).iter().any(|(k, v)| k == key && v == value)
    }

    /// rcheevos spells the requests: the service's address, the request's name, who, and the
    /// secret in the form, never in the address.
    #[test]
    fn requests_are_rcheevos_own() {
        let r = login_request("Alan", Secret::Password("p&w")).unwrap();
        assert_eq!(r.url, "https://retroachievements.org/dorequest.php");
        assert!(
            has(&r, "r", "login2") && has(&r, "u", "Alan") && has(&r, "p", "p&w"),
            "{r:?}"
        );
        assert!(!form(&r).iter().any(|(k, _)| k == "t"));
        let r = login_request("Alan", Secret::Token("t/k")).unwrap();
        assert!(has(&r, "t", "t/k") && !form(&r).iter().any(|(k, _)| k == "p"));
        let r = game_request(&alan(), "abc123").unwrap();
        assert!(
            has(&r, "r", "achievementsets") && has(&r, "m", "abc123") && has(&r, "t", "t/k"),
            "{r:?}"
        );
        let r = unlocks_request(&alan(), 42, true).unwrap();
        assert!(
            has(&r, "r", "unlocks") && has(&r, "g", "42") && has(&r, "h", "1"),
            "{r:?}"
        );
        assert!(!r.url.contains("t/k") && !r.url.contains("t%2Fk"));
        assert!(login_request("Al\0an", Secret::Token("t")).is_err());
    }

    #[test]
    fn a_sign_in_is_read() {
        let body = br#"{"Success":true,"User":"Alan","Token":"abc123","Score":1234,"SoftcoreScore":5,"Messages":2,"AvatarUrl":"x"}"#;
        let login = read_login(200, body).unwrap();
        assert_eq!(
            (login.username.as_str(), login.token.as_str()),
            ("Alan", "abc123")
        );
        assert_eq!((login.score, login.softcore, login.messages), (1234, 5, 2));
        assert!(
            !format!("{login:?}").contains("abc123"),
            "the token never printed"
        );
        assert!(matches!(
            read_login(200, br#"{"Success":true,"User":"x"}"#),
            Err(Error::Failed(_))
        ));
        assert!(matches!(read_login(200, b"<html>"), Err(Error::Failed(_))));
    }

    const SETS: &[u8] = br#"{"Success":true,"GameId":12345,"Title":"Advance Wars: Days of Ruin","ConsoleId":18,
        "ImageIconUrl":"https://media.retroachievements.org/Images/000001.png",
        "RichPresenceGameId":12345,"RichPresencePatch":"","Sets":[
        {"AchievementSetId":7,"GameId":12345,"Title":"Advance Wars: Days of Ruin","Type":"core",
         "ImageIconUrl":"https://media.retroachievements.org/Images/000001.png","Achievements":[
          {"ID":1,"Title":"First Blood","Description":"Win a battle","Flags":3,"Points":5,
           "MemAddr":"0xH0001=1","Author":"t","BadgeName":"00001","Created":1,"Modified":1,"Type":"",
           "Rarity":1.0,"RarityHardcore":1.0,
           "BadgeURL":"https://media.retroachievements.org/Badge/00001.png",
           "BadgeLockedURL":"https://media.retroachievements.org/Badge/00001_lock.png"},
          {"ID":2,"Title":"Test only","Description":"Unofficial","Flags":5,"Points":1,
           "MemAddr":"0xH0001=1","Author":"t","BadgeName":"00002","Created":1,"Modified":1,"Type":"",
           "Rarity":1.0,"RarityHardcore":1.0,"BadgeURL":"","BadgeLockedURL":""},
          {"ID":101000001,"Title":"Warning: Unknown Emulator","Description":"Hardcore unlocks cannot be earned using this emulator.",
           "Flags":3,"Points":0,"MemAddr":"0=1","Author":"t","BadgeName":"00000","Created":1,"Modified":1,"Type":"",
           "Rarity":1.0,"RarityHardcore":1.0,"BadgeURL":"","BadgeLockedURL":""}],"Leaderboards":[]},
        {"AchievementSetId":8,"GameId":23456,"Title":"Bonus","Type":"bonus",
         "ImageIconUrl":"https://media.retroachievements.org/Images/000002.png","Achievements":[
          {"ID":3,"Title":" Veteran ","Description":"","Flags":3,"Points":25,
           "MemAddr":"0xH0001=1","Author":"t","BadgeName":"00003","Created":1,"Modified":1,"Type":"",
           "Rarity":1.0,"RarityHardcore":1.0,"BadgeURL":"https://media.retroachievements.org/Badge/00003.png",
           "BadgeLockedURL":""}],"Leaderboards":[]}]}"#;

    /// A game's sets: every set's achievements, unofficial ones marked, the warnings left out,
    /// and the game ids its unlocks are asked by.
    #[test]
    fn a_game_with_its_sets() {
        let game = read_game(200, SETS).unwrap().unwrap();
        assert_eq!(
            (game.id, game.console_id, game.title.as_str()),
            (12345, 18, "Advance Wars: Days of Ruin")
        );
        assert_eq!(game.image_name, "000001");
        let ids: Vec<_> = game.achievements.iter().map(|a| a.id).collect();
        assert_eq!(ids, [1, 2, 3]);
        let official: Vec<_> = game.achievements.iter().map(|a| a.official).collect();
        assert_eq!(official, [true, false, true]);
        let veteran = &game.achievements[2];
        assert_eq!(
            (
                veteran.title.as_str(),
                veteran.points,
                veteran.badge_name.as_str()
            ),
            ("Veteran", 25, "00003")
        );
        assert_eq!((veteran.set_id, veteran.set_game_id), (8, 23456));
        assert_eq!(
            game.achievements[1].badge_url, "https://media.retroachievements.org/Badge/00002.png",
            "no URL given: the media host's"
        );
        assert_eq!(game.unlock_ids(), [12345, 23456]);
    }

    #[test]
    fn a_hash_the_service_doesnt_know_is_an_answer() {
        let not_found =
            br#"{"Success":false,"Error":"Unknown game","Code":"not_found","Status":404}"#;
        assert_eq!(read_game(404, not_found), Ok(None));
        assert_eq!(
            read_game(200, br#"{"Success":false,"Error":"Unknown game"}"#),
            Ok(None)
        );
        assert_eq!(
            read_game(
                200,
                br#"{"Success":true,"GameId":0,"Title":"","ConsoleId":0,"Sets":[]}"#
            ),
            Ok(None)
        );
    }

    #[test]
    fn unlocks_are_read() {
        let ids = read_unlocks(
            200,
            br#"{"Success":true,"UserUnlocks":[3,99],"GameID":12345,"HardcoreMode":false}"#,
        )
        .unwrap();
        assert_eq!(ids, [3, 99]);
        assert_eq!(
            read_unlocks(200, br#"{"Success":true,"UserUnlocks":[]}"#),
            Ok(vec![])
        );
    }

    /// A revoked token is the same refusal whichever request it answers; the service failing
    /// otherwise is no refusal; the network failing is told apart.
    #[test]
    fn every_answer_is_checked_the_same_way() {
        let revoked = br#"{"Success":false,"Status":401,"Code":"invalid_credentials","Error":"Invalid user\/password combination. Please try again."}"#;
        let refused = Error::Refused("Invalid user/password combination.".into());
        assert_eq!(read_login(401, revoked), Err(refused.clone()));
        assert_eq!(read_game(401, revoked), Err(refused.clone()));
        assert_eq!(read_unlocks(401, revoked), Err(refused));
        assert_eq!(
            read_unlocks(
                200,
                br#"{"Success":false,"Code":"expired_token","Error":"Expired"}"#
            ),
            Err(Error::Refused("Expired".into()))
        );
        // A 401 without a code it knows is a refusal still.
        assert!(matches!(
            read_unlocks(401, br#"{"Success":false,"Error":"Nope"}"#),
            Err(Error::Refused(_))
        ));
        assert_eq!(
            read_game(
                200,
                br#"{"Success":false,"Error":"Credentials invalid (0)"}"#
            ),
            Err(Error::Failed("Credentials invalid (0)".into()))
        );
        assert!(matches!(
            read_unlocks(200, b"not json"),
            Err(Error::Failed(_))
        ));
        // An HTML page from a proxy in front of the service: its title.
        assert_eq!(
            read_unlocks(
                502,
                b"<html><head><title>502 Bad Gateway</title></head></html>"
            ),
            Err(Error::Failed("502 Bad Gateway".into()))
        );
        assert_eq!(
            read_unlocks(503, b""),
            Err(Error::Failed("the service didn't answer".into()))
        );
        let offline = Failure {
            retry: true,
            offline: true,
            why: "x".into(),
        };
        assert_eq!(Error::from(offline), Error::NoNetwork);
    }
}
