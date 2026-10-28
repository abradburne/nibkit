/* SPDX-License-Identifier: MIT
 *
 * nib-cheevos' face of rcheevos: a small C API over rc_client, rhash and rapi in structs of the
 * shim's own, so the Rust side never mirrors an rcheevos struct layout (which may change between
 * rcheevos versions) and a new kind of event adds a field here rather than changing a callback.
 * Numbers are the shim's own too (NIB_RC_*), mapped from rcheevos' in shim.c. Every client
 * function is called on one thread, the emulation thread, except nib_rc_discard, which only
 * frees. The API functions hold no state and may be called on any thread; the hash functions
 * too, but one at a time (rhash reads a buffer through a static of its own).
 *
 * Every struct below is appended to, never reordered; nib_rc_sizes says how big this build's
 * are, so the Rust side can check its declarations against them.
 */
#ifndef NIB_RC_SHIM_H
#define NIB_RC_SHIM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct nib_rc nib_rc;

/* What happened (rc_client's RC_CLIENT_EVENT_*, by the shim's own numbers). */
enum {
  NIB_RC_EVENT_UNLOCKED = 1,              /* achievement */
  NIB_RC_EVENT_LEADERBOARD_STARTED = 2,   /* leaderboard */
  NIB_RC_EVENT_LEADERBOARD_FAILED = 3,    /* leaderboard */
  NIB_RC_EVENT_LEADERBOARD_SUBMITTED = 4, /* leaderboard, its value in tracker_value */
  NIB_RC_EVENT_CHALLENGE_SHOW = 5,        /* achievement */
  NIB_RC_EVENT_CHALLENGE_HIDE = 6,        /* achievement */
  NIB_RC_EVENT_PROGRESS_SHOW = 7,         /* achievement, with its measured progress */
  NIB_RC_EVENT_PROGRESS_HIDE = 8,         /* nothing */
  NIB_RC_EVENT_PROGRESS_UPDATE = 9,       /* achievement, with its measured progress */
  NIB_RC_EVENT_TRACKER_SHOW = 10,         /* tracker */
  NIB_RC_EVENT_TRACKER_HIDE = 11,         /* tracker */
  NIB_RC_EVENT_TRACKER_UPDATE = 12,       /* tracker */
  NIB_RC_EVENT_RESET = 13,                /* the machine should be reset (hardcore was turned on) */
  NIB_RC_EVENT_GAME_COMPLETED = 14,       /* every achievement of the game is earned */
  NIB_RC_EVENT_SUBSET_COMPLETED = 15,     /* subset */
  NIB_RC_EVENT_SERVER_ERROR = 16,         /* error_api, error_message; not retried */
  NIB_RC_EVENT_DISCONNECTED = 17,         /* an unlock could not be sent and is pending */
  NIB_RC_EVENT_RECONNECTED = 18           /* every pending unlock is sent */
};

/* The outcome of a sign-in, a game load or a request to the service. */
enum {
  NIB_RC_OK = 0,
  NIB_RC_FAILED = 1,       /* anything else; the message says what */
  NIB_RC_UNKNOWN_GAME = 2, /* the service does not know the hash */
  NIB_RC_BAD_TOKEN = 3,    /* the password or token was refused, or the token has expired */
  NIB_RC_NO_SIGN_IN = 4,   /* a game load that needs the sign-in that failed */
  NIB_RC_NO_ANSWER = 5     /* the service was not reached (the transport's message says why) */
};

typedef struct nib_rc_achievement {
  uint32_t id;
  uint32_t points;
  const char* title;
  const char* description;
  /* The service's name for its badge picture ("12345"), and where it is; "" when none. */
  const char* badge_name;
  const char* badge_url;
  /* How far a measured achievement is ("3/10", "45%"), "" when it isn't measured, and the same
   * as a fraction of 1. */
  const char* measured_progress;
  float measured_percent;
} nib_rc_achievement;

typedef struct nib_rc_leaderboard {
  uint32_t id;
  const char* title;
  const char* description;
  /* The value an attempt ended on, formatted (SUBMITTED). */
  const char* tracker_value;
} nib_rc_leaderboard;

typedef struct nib_rc_tracker {
  uint32_t id;
  /* The value to show, formatted. */
  const char* display;
} nib_rc_tracker;

/* One event. The pointers are NULL when the event is not about such a thing; the strings are
 * never NULL in a struct that is there ("" for none). Everything is valid for the call only. */
typedef struct nib_rc_event {
  uint32_t type;
  const nib_rc_achievement* achievement;
  const nib_rc_leaderboard* leaderboard;
  const nib_rc_tracker* tracker;
  const char* subset_title;  /* SUBSET_COMPLETED */
  const char* error_api;     /* SERVER_ERROR: the request that failed */
  const char* error_message; /* SERVER_ERROR */
} nib_rc_event;

/* Reads num_bytes of the console's memory at an address in RetroAchievements' address space
 * for the console; returns how many were backed (unbacked bytes read as zero). */
typedef uint32_t (*nib_rc_read_fn)(uint32_t address, uint8_t* buffer, uint32_t num_bytes, void* ud);
/* A request for the service: post_data and content_type are NULL for a GET. For the client's
 * requests, `ticket` must be given back to nib_rc_respond (with the answer) or nib_rc_discard,
 * once; a request the API functions build has none (NULL). */
typedef void (*nib_rc_request_fn)(const char* url, const char* post_data, const char* content_type,
                                  void* ticket, void* ud);
typedef void (*nib_rc_event_fn)(const nib_rc_event* event, void* ud);
/* The outcome of a sign-in or a game load: a NIB_RC_* result and, when it failed, why. */
typedef void (*nib_rc_done_fn)(int result, const char* error_message, void* ud);
typedef void (*nib_rc_log_fn)(const char* message, void* ud);

typedef struct nib_rc_callbacks {
  nib_rc_read_fn read;
  nib_rc_request_fn request;
  nib_rc_event_fn event;
  nib_rc_done_fn login_done;
  nib_rc_done_fn load_done;
  nib_rc_log_fn log; /* NULL: rcheevos' log is dropped */
} nib_rc_callbacks;

/* How the game is played, set before a game loads. */
typedef struct nib_rc_settings {
  int hardcore;   /* no save states, rewind, slow motion or cheats; the service ranks it */
  int encore;     /* earned achievements can be earned again (not sent again) */
  int unofficial; /* the set's unpublished achievements are evaluated too */
} nib_rc_settings;

/* A client, memory read only inside nib_rc_do_frame / nib_rc_idle. NULL when rc_client could
 * not be made. */
nib_rc* nib_rc_create(const nib_rc_callbacks* callbacks, const nib_rc_settings* settings, void* ud);
void nib_rc_destroy(nib_rc* rc);

/* The answer to a request: calls rcheevos back with it and frees the ticket. A negative
 * http_status is rcheevos' RC_API_SERVER_RESPONSE_*: -2 the service was never reached and
 * rcheevos should retry (the body says why), -1 the request could not be made at all and
 * retrying it would not help. */
void nib_rc_respond(void* ticket, int http_status, const char* body, size_t body_length);
/* Frees a ticket without answering it (after the client is gone). */
void nib_rc_discard(void* ticket);
enum {
  NIB_RC_NOT_REACHED = -2, /* RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR */
  NIB_RC_CANNOT_ASK = -1   /* RC_API_SERVER_RESPONSE_CLIENT_ERROR */
};

void nib_rc_login_with_token(nib_rc* rc, const char* username, const char* token);
void nib_rc_load_game(nib_rc* rc, const char* hash);
void nib_rc_unload_game(nib_rc* rc);
void nib_rc_do_frame(nib_rc* rc);
void nib_rc_idle(nib_rc* rc);
/* The game was reset: the runtime's progress (hit counts) starts over. */
void nib_rc_reset(nib_rc* rc);

/* The runtime's progress, for a save state: how many bytes it takes (0 when no game is
 * loaded), and writing it into `buffer` (NIB_RC_OK, or not when it does not fit or no game is
 * loaded). */
size_t nib_rc_progress_size(nib_rc* rc);
int nib_rc_save_progress(nib_rc* rc, uint8_t* buffer, size_t size);
/* Restores progress from a save state; NULL (a state without any) starts every achievement
 * over, as a reset does. NIB_RC_OK, or not when no game is loaded or the bytes are not
 * progress (then everything starts over). */
int nib_rc_load_progress(nib_rc* rc, const uint8_t* buffer, size_t size);

/* The signed-in player's name as the service shows it; NULL when not signed in. Valid until
 * the next call into the client. */
const char* nib_rc_display_name(const nib_rc* rc);
/* The loaded game's title and id; NULL / 0 when none is loaded. */
const char* nib_rc_game_title(const nib_rc* rc);
uint32_t nib_rc_game_id(const nib_rc* rc);
/* The loaded game's achievements (core set): how many, how many the player has, how many read
 * memory this build does not back, how many are unofficial, and the points in all and
 * earned. */
typedef struct nib_rc_summary {
  uint32_t total;
  uint32_t unlocked;
  uint32_t unsupported;
  uint32_t points;
  uint32_t points_earned;
  uint32_t unofficial;
} nib_rc_summary;
void nib_rc_get_summary(const nib_rc* rc, nib_rc_summary* out);
/* Whether hardcore is on (rcheevos may turn it off itself, e.g. for an unknown game). */
int nib_rc_hardcore(const nib_rc* rc);

/* --- rhash --------------------------------------------------------------------------------- */

/* The RetroAchievements identity of a game for the console (an RC_CONSOLE_* id), as rhash
 * computes it: 32 lowercase hex characters and a NUL in `out`. Returns 0 when rhash cannot
 * hash it (an unsupported console, a file it cannot read or that isn't shaped like a game);
 * `log` (may be NULL) hears rhash's error messages, and with `verbose` its other messages too.
 *
 * From a file: `path` is read as rhash reads it, which for a disc is its cue sheet (and the
 * tracks it names), a playlist (.m3u) or an image rhash's own reader opens. From a buffer:
 * the bytes are the game, `name` (its file name) only says what kind of file it was. */
int nib_rc_hash_file(char out[33], uint32_t console_id, const char* path, nib_rc_log_fn log,
                     int verbose, void* ud);
int nib_rc_hash_buffer(char out[33], uint32_t console_id, const char* name, const uint8_t* data,
                       size_t size, nib_rc_log_fn log, int verbose, void* ud);

/* A disc image rhash's own CD reader doesn't open (CHD): the program's reader, rhash's
 * rc_hash_cdreader_t's four calls with their meaning (src/chd.rs has one). */
typedef struct nib_rc_cdreader {
  void* (*open_track)(const char* path, uint32_t track);
  size_t (*read_sector)(void* track, uint32_t sector, void* buffer, size_t requested_bytes);
  void (*close_track)(void* track);
  uint32_t (*first_track_sector)(void* track);
} nib_rc_cdreader;

/* As nib_rc_hash_file, the disc at `path` read through `reader`. */
int nib_rc_hash_disc(char out[33], uint32_t console_id, const char* path,
                     const nib_rc_cdreader* reader, nib_rc_log_fn log, int verbose, void* ud);

/* --- rapi: requests built and answers read by rcheevos ------------------------------------ */

/* What an answer of the service came to: a NIB_RC_* result, and the service's (or the
 * transport's) message and error code when it failed ("" for none). */
typedef struct nib_rc_status {
  int result;
  const char* error_message;
  const char* error_code;
  int http_status;
} nib_rc_status;

/* A sign-in's answer (login2). */
typedef struct nib_rc_login {
  nib_rc_status status;
  const char* username; /* as the service spells it */
  const char* token;
  const char* display_name;
  uint32_t score;
  uint32_t score_softcore;
  uint32_t unread_messages;
} nib_rc_login;

/* A game's sets' answer (achievementsets): the game, then each achievement of each set. */
typedef struct nib_rc_game {
  nib_rc_status status;
  uint32_t id; /* 0: the service doesn't know the hash */
  uint32_t console_id;
  const char* title;
  const char* image_name;
  const char* image_url;
  uint32_t num_sets;
} nib_rc_game;

typedef struct nib_rc_set_achievement {
  /* The set it belongs to: its id, the game id unlocks are asked for by, its type (0 core,
   * 1 bonus, 2 specialty, 3 exclusive) and its title. */
  uint32_t set_id;
  uint32_t set_game_id;
  uint32_t set_type;
  const char* set_title;
  uint32_t id;
  uint32_t points;
  /* 3 core (published), 5 unofficial. */
  uint32_t category;
  /* 0 standard, 1 missable, 2 progression, 3 win. */
  uint32_t type;
  const char* title;
  const char* description;
  const char* badge_name;
  const char* badge_url;
  const char* badge_locked_url;
  float rarity;
  float rarity_hardcore;
} nib_rc_set_achievement;

/* The player's unlocks in a game (unlocks). */
typedef struct nib_rc_unlocks {
  nib_rc_status status;
  const uint32_t* ids;
  uint32_t count;
} nib_rc_unlocks;

typedef void (*nib_rc_login_fn)(const nib_rc_login* login, void* ud);
typedef void (*nib_rc_game_fn)(const nib_rc_game* game, void* ud);
typedef void (*nib_rc_set_achievement_fn)(const nib_rc_set_achievement* achievement, void* ud);
typedef void (*nib_rc_unlocks_fn)(const nib_rc_unlocks* unlocks, void* ud);

/* Builds a request and hands it to `built` (ticket NULL), valid for the call; returns a
 * NIB_RC_* result (NIB_RC_FAILED when rcheevos would not build it: a parameter missing). A
 * sign-in takes the password, or else the token. */
int nib_rc_api_login(const char* username, const char* password, const char* token,
                     nib_rc_request_fn built, void* ud);
int nib_rc_api_game_sets(const char* username, const char* token, const char* hash,
                         nib_rc_request_fn built, void* ud);
int nib_rc_api_unlocks(const char* username, const char* token, uint32_t game_id, int hardcore,
                       nib_rc_request_fn built, void* ud);

/* Reads an answer (http_status as nib_rc_respond takes it) and calls back with what it says,
 * valid for the call: the game first, then each of its sets' achievements. */
void nib_rc_api_read_login(int http_status, const char* body, size_t body_length,
                           nib_rc_login_fn done, void* ud);
void nib_rc_api_read_game_sets(int http_status, const char* body, size_t body_length,
                               nib_rc_game_fn game, nib_rc_set_achievement_fn achievement, void* ud);
void nib_rc_api_read_unlocks(int http_status, const char* body, size_t body_length,
                             nib_rc_unlocks_fn done, void* ud);

/* --- the rest ------------------------------------------------------------------------------ */

/* rcheevos' part of the User-Agent ("rcheevos/12.4.0") into `out`, NUL-terminated; its length. */
size_t nib_rc_user_agent_clause(char* out, size_t size);

/* rcheevos' version, "12.4.0". */
const char* nib_rc_version(void);

/* The sizes of this build's structs, in this order: event, achievement, leaderboard, tracker,
 * callbacks, settings, summary, status, login, game, set_achievement, unlocks. */
void nib_rc_sizes(size_t out[12]);

#ifdef __cplusplus
}
#endif

#endif
