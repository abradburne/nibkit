/* SPDX-License-Identifier: MIT */
#include "shim.h"

#include <stdlib.h>
#include <string.h>

#include "rc_api_request.h"
#include "rc_api_runtime.h"
#include "rc_api_user.h"
#include "rc_client.h"
#include "rc_consoles.h"
#include "rc_error.h"
#include "rc_hash.h"
#include "rc_version.h"

/* rapi's own JSON reader, for the error code an answer's own reader doesn't look for. */
#include "rc_api_common.h"

/* The two answers the transport gives that are not HTTP statuses are rcheevos' own. */
_Static_assert(NIB_RC_NOT_REACHED == RC_API_SERVER_RESPONSE_RETRYABLE_CLIENT_ERROR,
               "NIB_RC_NOT_REACHED is rcheevos' retryable client error");
_Static_assert(NIB_RC_CANNOT_ASK == RC_API_SERVER_RESPONSE_CLIENT_ERROR,
               "NIB_RC_CANNOT_ASK is rcheevos' client error");

struct nib_rc {
  rc_client_t* client;
  nib_rc_callbacks cb;
  void* ud;
};

/* A request in flight: what rcheevos wants called with the answer. */
typedef struct ticket {
  rc_client_server_callback_t callback;
  void* callback_data;
} ticket;

static nib_rc* self_of(const rc_client_t* client) {
  return (nib_rc*)rc_client_get_userdata(client);
}

static const char* or_empty(const char* s) {
  return s ? s : "";
}

static uint32_t read_memory(uint32_t address, uint8_t* buffer, uint32_t num_bytes, rc_client_t* client) {
  nib_rc* rc = self_of(client);
  if (!rc) {
    memset(buffer, 0, num_bytes);
    return 0;
  }
  return rc->cb.read(address, buffer, num_bytes, rc->ud);
}

static void server_call(const rc_api_request_t* request, rc_client_server_callback_t callback,
                        void* callback_data, rc_client_t* client) {
  nib_rc* rc = self_of(client);
  if (!rc || !request) return;
  ticket* t = (ticket*)calloc(1, sizeof *t);
  if (!t) return;
  t->callback = callback;
  t->callback_data = callback_data;
  rc->cb.request(request->url, request->post_data, request->content_type, t, rc->ud);
}

/* rcheevos' event type as the shim's; 0 for one the shim does not pass on (the scoreboard,
 * which comes only in hardcore and needs a struct of its own). */
static uint32_t event_type(uint32_t type) {
  switch (type) {
    case RC_CLIENT_EVENT_ACHIEVEMENT_TRIGGERED: return NIB_RC_EVENT_UNLOCKED;
    case RC_CLIENT_EVENT_LEADERBOARD_STARTED: return NIB_RC_EVENT_LEADERBOARD_STARTED;
    case RC_CLIENT_EVENT_LEADERBOARD_FAILED: return NIB_RC_EVENT_LEADERBOARD_FAILED;
    case RC_CLIENT_EVENT_LEADERBOARD_SUBMITTED: return NIB_RC_EVENT_LEADERBOARD_SUBMITTED;
    case RC_CLIENT_EVENT_ACHIEVEMENT_CHALLENGE_INDICATOR_SHOW: return NIB_RC_EVENT_CHALLENGE_SHOW;
    case RC_CLIENT_EVENT_ACHIEVEMENT_CHALLENGE_INDICATOR_HIDE: return NIB_RC_EVENT_CHALLENGE_HIDE;
    case RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_SHOW: return NIB_RC_EVENT_PROGRESS_SHOW;
    case RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_HIDE: return NIB_RC_EVENT_PROGRESS_HIDE;
    case RC_CLIENT_EVENT_ACHIEVEMENT_PROGRESS_INDICATOR_UPDATE: return NIB_RC_EVENT_PROGRESS_UPDATE;
    case RC_CLIENT_EVENT_LEADERBOARD_TRACKER_SHOW: return NIB_RC_EVENT_TRACKER_SHOW;
    case RC_CLIENT_EVENT_LEADERBOARD_TRACKER_HIDE: return NIB_RC_EVENT_TRACKER_HIDE;
    case RC_CLIENT_EVENT_LEADERBOARD_TRACKER_UPDATE: return NIB_RC_EVENT_TRACKER_UPDATE;
    case RC_CLIENT_EVENT_RESET: return NIB_RC_EVENT_RESET;
    case RC_CLIENT_EVENT_GAME_COMPLETED: return NIB_RC_EVENT_GAME_COMPLETED;
    case RC_CLIENT_EVENT_SUBSET_COMPLETED: return NIB_RC_EVENT_SUBSET_COMPLETED;
    case RC_CLIENT_EVENT_SERVER_ERROR: return NIB_RC_EVENT_SERVER_ERROR;
    case RC_CLIENT_EVENT_DISCONNECTED: return NIB_RC_EVENT_DISCONNECTED;
    case RC_CLIENT_EVENT_RECONNECTED: return NIB_RC_EVENT_RECONNECTED;
    default: return 0;
  }
}

static void on_event(const rc_client_event_t* event, rc_client_t* client) {
  nib_rc* rc = self_of(client);
  if (!rc || !event) return;
  nib_rc_event e;
  nib_rc_achievement a;
  nib_rc_leaderboard l;
  nib_rc_tracker t;
  memset(&e, 0, sizeof e);
  e.type = event_type(event->type);
  if (!e.type) return;
  e.subset_title = event->subset ? or_empty(event->subset->title) : "";
  e.error_api = event->server_error ? or_empty(event->server_error->api) : "";
  e.error_message = event->server_error ? or_empty(event->server_error->error_message) : "";
  if (event->achievement) {
    const rc_client_achievement_t* x = event->achievement;
    memset(&a, 0, sizeof a);
    a.id = x->id;
    a.points = x->points;
    a.title = or_empty(x->title);
    a.description = or_empty(x->description);
    a.badge_name = x->badge_name;
    a.badge_url = or_empty(x->badge_url);
    a.measured_progress = x->measured_progress;
    a.measured_percent = x->measured_percent;
    e.achievement = &a;
  }
  if (event->leaderboard) {
    const rc_client_leaderboard_t* x = event->leaderboard;
    memset(&l, 0, sizeof l);
    l.id = x->id;
    l.title = or_empty(x->title);
    l.description = or_empty(x->description);
    l.tracker_value = or_empty(x->tracker_value);
    e.leaderboard = &l;
  }
  if (event->leaderboard_tracker) {
    memset(&t, 0, sizeof t);
    t.id = event->leaderboard_tracker->id;
    t.display = event->leaderboard_tracker->display;
    e.tracker = &t;
  }
  rc->cb.event(&e, rc->ud);
}

static void on_log(const char* message, const rc_client_t* client) {
  nib_rc* rc = self_of(client);
  if (rc && rc->cb.log) rc->cb.log(or_empty(message), rc->ud);
}

/* An rcheevos result as the shim's. */
static int outcome(int result) {
  switch (result) {
    case RC_OK: return NIB_RC_OK;
    case RC_NO_GAME_LOADED:
    case RC_NOT_FOUND: return NIB_RC_UNKNOWN_GAME;
    case RC_INVALID_CREDENTIALS:
    case RC_EXPIRED_TOKEN:
    case RC_ACCESS_DENIED: return NIB_RC_BAD_TOKEN;
    case RC_LOGIN_REQUIRED: return NIB_RC_NO_SIGN_IN;
    case RC_NO_RESPONSE: return NIB_RC_NO_ANSWER;
    default: return NIB_RC_FAILED;
  }
}

static void login_done(int result, const char* error_message, rc_client_t* client, void* userdata) {
  nib_rc* rc = self_of(client);
  (void)userdata;
  if (rc) rc->cb.login_done(outcome(result), error_message, rc->ud);
}

static void load_done(int result, const char* error_message, rc_client_t* client, void* userdata) {
  nib_rc* rc = self_of(client);
  (void)userdata;
  if (rc) rc->cb.load_done(outcome(result), error_message, rc->ud);
}

nib_rc* nib_rc_create(const nib_rc_callbacks* callbacks, const nib_rc_settings* settings, void* ud) {
  nib_rc* rc = (nib_rc*)calloc(1, sizeof *rc);
  if (!rc) return NULL;
  rc->cb = *callbacks;
  rc->ud = ud;
  rc->client = rc_client_create(read_memory, server_call);
  if (!rc->client) {
    free(rc);
    return NULL;
  }
  rc_client_set_userdata(rc->client, rc);
  /* rc_client defaults hardcore on; the caller says. */
  rc_client_set_hardcore_enabled(rc->client, settings->hardcore != 0);
  rc_client_set_encore_mode_enabled(rc->client, settings->encore != 0);
  rc_client_set_unofficial_enabled(rc->client, settings->unofficial != 0);
  /* The console's memory is only read inside do_frame / idle, on the emulation thread. */
  rc_client_set_allow_background_memory_reads(rc->client, 0);
  rc_client_set_event_handler(rc->client, on_event);
  if (rc->cb.log) rc_client_enable_logging(rc->client, RC_CLIENT_LOG_LEVEL_VERBOSE, on_log);
  return rc;
}

void nib_rc_destroy(nib_rc* rc) {
  if (!rc) return;
  rc_client_destroy(rc->client);
  free(rc);
}

void nib_rc_respond(void* ticket_ptr, int http_status, const char* body, size_t body_length) {
  ticket* t = (ticket*)ticket_ptr;
  if (!t) return;
  rc_api_server_response_t response;
  memset(&response, 0, sizeof response);
  response.body = body ? body : "";
  response.body_length = body ? body_length : 0;
  response.http_status_code = http_status;
  if (t->callback) t->callback(&response, t->callback_data);
  free(t);
}

void nib_rc_discard(void* ticket_ptr) {
  free(ticket_ptr);
}

void nib_rc_login_with_token(nib_rc* rc, const char* username, const char* token) {
  rc_client_begin_login_with_token(rc->client, username, token, login_done, NULL);
}

void nib_rc_load_game(nib_rc* rc, const char* hash) {
  rc_client_begin_load_game(rc->client, hash, load_done, NULL);
}

void nib_rc_unload_game(nib_rc* rc) {
  rc_client_unload_game(rc->client);
}

void nib_rc_do_frame(nib_rc* rc) {
  rc_client_do_frame(rc->client);
}

void nib_rc_idle(nib_rc* rc) {
  rc_client_idle(rc->client);
}

void nib_rc_reset(nib_rc* rc) {
  rc_client_reset(rc->client);
}

size_t nib_rc_progress_size(nib_rc* rc) {
  if (!rc_client_is_game_loaded(rc->client)) return 0;
  return rc_client_progress_size(rc->client);
}

int nib_rc_save_progress(nib_rc* rc, uint8_t* buffer, size_t size) {
  if (!rc_client_is_game_loaded(rc->client) || !buffer) return NIB_RC_FAILED;
  return rc_client_serialize_progress_sized(rc->client, buffer, size) == RC_OK ? NIB_RC_OK : NIB_RC_FAILED;
}

int nib_rc_load_progress(nib_rc* rc, const uint8_t* buffer, size_t size) {
  if (!rc_client_is_game_loaded(rc->client)) return NIB_RC_FAILED;
  if (buffer && rc_client_deserialize_progress_sized(rc->client, buffer, size) == RC_OK) return NIB_RC_OK;
  /* None, or not progress: everything starts over (rcheevos' own answer to NULL). */
  rc_client_deserialize_progress_sized(rc->client, NULL, 0);
  return buffer ? NIB_RC_FAILED : NIB_RC_OK;
}

const char* nib_rc_display_name(const nib_rc* rc) {
  const rc_client_user_t* u = rc_client_get_user_info(rc->client);
  return u ? u->display_name : NULL;
}

const char* nib_rc_game_title(const nib_rc* rc) {
  const rc_client_game_t* g = rc_client_get_game_info(rc->client);
  return g ? g->title : NULL;
}

uint32_t nib_rc_game_id(const nib_rc* rc) {
  const rc_client_game_t* g = rc_client_get_game_info(rc->client);
  return g ? g->id : 0;
}

void nib_rc_get_summary(const nib_rc* rc, nib_rc_summary* out) {
  memset(out, 0, sizeof *out);
  rc_client_user_game_summary_t s;
  memset(&s, 0, sizeof s);
  rc_client_get_user_game_summary(rc->client, &s);
  out->total = s.num_core_achievements;
  out->unlocked = s.num_unlocked_achievements;
  out->unsupported = s.num_unsupported_achievements;
  out->points = s.points_core;
  out->points_earned = s.points_unlocked;
  out->unofficial = s.num_unofficial_achievements;
}

int nib_rc_hardcore(const nib_rc* rc) {
  return rc_client_get_hardcore_enabled(rc->client);
}

/* --- rhash --------------------------------------------------------------------------------- */

/* Where rhash's messages go: the caller's log, and whether it wants the verbose ones. */
typedef struct hash_log {
  nib_rc_log_fn log;
  void* ud;
} hash_log;

static void hash_message(const char* message, const rc_hash_iterator_t* iterator) {
  const hash_log* to = (const hash_log*)iterator->userdata;
  if (to && to->log) to->log(or_empty(message), to->ud);
}

static int hash_with(char out[33], uint32_t console_id, rc_hash_iterator_t* it, nib_rc_log_fn log,
                     int verbose, void* ud) {
  hash_log to;
  to.log = log;
  to.ud = ud;
  it->userdata = &to;
  it->callbacks.error_message = log ? hash_message : NULL;
  it->callbacks.verbose_message = (log && verbose) ? hash_message : NULL;
  out[0] = '\0';
  const int ok = rc_hash_generate(out, console_id, it);
  rc_hash_destroy_iterator(it);
  if (!ok || out[0] == '\0') {
    out[0] = '\0';
    return 0;
  }
  return 1;
}

int nib_rc_hash_file(char out[33], uint32_t console_id, const char* path, nib_rc_log_fn log,
                     int verbose, void* ud) {
  rc_hash_iterator_t it;
  if (!path) return 0;
  /* No buffer: rhash reads the file itself, with its own file and CD readers. */
  rc_hash_initialize_iterator(&it, path, NULL, 0);
  return hash_with(out, console_id, &it, log, verbose, ud);
}

int nib_rc_hash_buffer(char out[33], uint32_t console_id, const char* name, const uint8_t* data,
                       size_t size, nib_rc_log_fn log, int verbose, void* ud) {
  rc_hash_iterator_t it;
  /* rhash takes no buffer of nothing: an empty one would be read as "no buffer, a file". */
  if (!data || size == 0) return 0;
  /* The name only says what kind of file the bytes were; they come from the buffer. */
  rc_hash_initialize_iterator(&it, name ? name : "game", data, size);
  return hash_with(out, console_id, &it, log, verbose, ud);
}

/* --- rapi ---------------------------------------------------------------------------------- */

static int built(int result, rc_api_request_t* request, nib_rc_request_fn to, void* ud) {
  if (result == RC_OK) to(request->url, request->post_data, request->content_type, NULL, ud);
  rc_api_destroy_request(request);
  return result == RC_OK ? NIB_RC_OK : NIB_RC_FAILED;
}

int nib_rc_api_login(const char* username, const char* password, const char* token,
                     nib_rc_request_fn to, void* ud) {
  rc_api_login_request_t params;
  rc_api_request_t request;
  memset(&params, 0, sizeof params);
  memset(&request, 0, sizeof request);
  params.username = username;
  params.password = password;
  params.api_token = token;
  return built(rc_api_init_login_request(&request, &params), &request, to, ud);
}

int nib_rc_api_game_sets(const char* username, const char* token, const char* hash,
                         nib_rc_request_fn to, void* ud) {
  rc_api_fetch_game_sets_request_t params;
  rc_api_request_t request;
  memset(&params, 0, sizeof params);
  memset(&request, 0, sizeof request);
  params.username = username;
  params.api_token = token;
  params.game_hash = hash;
  return built(rc_api_init_fetch_game_sets_request(&request, &params), &request, to, ud);
}

int nib_rc_api_unlocks(const char* username, const char* token, uint32_t game_id, int hardcore,
                       nib_rc_request_fn to, void* ud) {
  rc_api_fetch_user_unlocks_request_t params;
  rc_api_request_t request;
  memset(&params, 0, sizeof params);
  memset(&request, 0, sizeof request);
  params.username = username;
  params.api_token = token;
  params.game_id = game_id;
  params.hardcore = hardcore ? 1 : 0;
  return built(rc_api_init_fetch_user_unlocks_request(&request, &params), &request, to, ud);
}

/* The service's error code in a failed answer, and rcheevos' result for it, read by rapi's
 * own reader: the readers of some answers (unlocks) don't look for one, though the service
 * says a token was refused there too. Codes longer than `code` holds are none rcheevos knows. */
typedef struct code_read {
  int result;
  char code[32];
} code_read;

static code_read code_of(const rc_api_server_response_t* server_response) {
  code_read out;
  rc_api_response_t response;
  rc_json_field_t fields[] = {
    RC_JSON_NEW_FIELD("Success"),
    RC_JSON_NEW_FIELD("Error"),
    RC_JSON_NEW_FIELD("Code")
  };
  memset(&out, 0, sizeof out);
  memset(&response, 0, sizeof response);
  rc_buffer_init(&response.buffer);
  out.result = rc_json_parse_server_response(&response, server_response, fields, sizeof(fields) / sizeof(fields[0]));
  if (response.error_code) {
    strncpy(out.code, response.error_code, sizeof out.code - 1);
  } else {
    out.result = RC_OK;
  }
  rc_buffer_destroy(&response.buffer);
  return out;
}

/* What an answer came to: rcheevos' result, or, when it read the answer but the service said
 * no without a code it knows, a failure; a refusal by its HTTP status alone (401, 403) is a
 * refusal all the same. `code` keeps the error code read for it, when the answer's own reader
 * found none. */
static void status_of(nib_rc_status* out, int result, const rc_api_response_t* response,
                      const rc_api_server_response_t* server_response, code_read* code) {
  out->http_status = server_response->http_status_code;
  out->error_message = or_empty(response->error_message);
  out->error_code = or_empty(response->error_code);
  out->result = outcome(result);
  if (out->result == NIB_RC_OK && !response->succeeded) out->result = NIB_RC_FAILED;
  if (out->result == NIB_RC_FAILED && !response->error_code) {
    *code = code_of(server_response);
    if (code->code[0]) {
      out->error_code = code->code;
      out->result = outcome(code->result);
      if (out->result == NIB_RC_OK) out->result = NIB_RC_FAILED;
    }
  }
  if (out->result == NIB_RC_FAILED && (out->http_status == 401 || out->http_status == 403))
    out->result = NIB_RC_BAD_TOKEN;
}

static rc_api_server_response_t server_response(int http_status, const char* body, size_t body_length) {
  rc_api_server_response_t r;
  memset(&r, 0, sizeof r);
  r.body = body ? body : "";
  r.body_length = body ? body_length : 0;
  r.http_status_code = http_status;
  return r;
}

void nib_rc_api_read_login(int http_status, const char* body, size_t body_length,
                           nib_rc_login_fn done, void* ud) {
  rc_api_login_response_t response;
  nib_rc_login out;
  const rc_api_server_response_t r = server_response(http_status, body, body_length);
  const int result = rc_api_process_login_server_response(&response, &r);
  code_read code;
  memset(&out, 0, sizeof out);
  status_of(&out.status, result, &response.response, &r, &code);
  out.username = or_empty(response.username);
  out.token = or_empty(response.api_token);
  out.display_name = or_empty(response.display_name);
  out.score = response.score;
  out.score_softcore = response.score_softcore;
  out.unread_messages = response.num_unread_messages;
  done(&out, ud);
  rc_api_destroy_login_response(&response);
}

void nib_rc_api_read_game_sets(int http_status, const char* body, size_t body_length,
                               nib_rc_game_fn game, nib_rc_set_achievement_fn achievement, void* ud) {
  rc_api_fetch_game_sets_response_t response;
  nib_rc_game out;
  uint32_t s, i;
  const rc_api_server_response_t r = server_response(http_status, body, body_length);
  const int result = rc_api_process_fetch_game_sets_server_response(&response, &r);
  code_read code;
  memset(&out, 0, sizeof out);
  status_of(&out.status, result, &response.response, &r, &code);
  out.id = response.id;
  out.console_id = response.console_id;
  out.title = or_empty(response.title);
  out.image_name = or_empty(response.image_name);
  out.image_url = or_empty(response.image_url);
  out.num_sets = out.status.result == NIB_RC_OK ? response.num_sets : 0;
  game(&out, ud);
  for (s = 0; s < out.num_sets; ++s) {
    const rc_api_achievement_set_definition_t* set = &response.sets[s];
    for (i = 0; i < set->num_achievements; ++i) {
      const rc_api_achievement_definition_t* x = &set->achievements[i];
      nib_rc_set_achievement a;
      memset(&a, 0, sizeof a);
      a.set_id = set->id;
      a.set_game_id = set->game_id;
      a.set_type = set->type;
      a.set_title = or_empty(set->title);
      a.id = x->id;
      a.points = x->points;
      a.category = x->category;
      a.type = x->type;
      a.title = or_empty(x->title);
      a.description = or_empty(x->description);
      a.badge_name = or_empty(x->badge_name);
      a.badge_url = or_empty(x->badge_url);
      a.badge_locked_url = or_empty(x->badge_locked_url);
      a.rarity = x->rarity;
      a.rarity_hardcore = x->rarity_hardcore;
      achievement(&a, ud);
    }
  }
  rc_api_destroy_fetch_game_sets_response(&response);
}

void nib_rc_api_read_unlocks(int http_status, const char* body, size_t body_length,
                             nib_rc_unlocks_fn done, void* ud) {
  rc_api_fetch_user_unlocks_response_t response;
  nib_rc_unlocks out;
  const rc_api_server_response_t r = server_response(http_status, body, body_length);
  const int result = rc_api_process_fetch_user_unlocks_server_response(&response, &r);
  code_read code;
  memset(&out, 0, sizeof out);
  status_of(&out.status, result, &response.response, &r, &code);
  if (out.status.result == NIB_RC_OK) {
    out.ids = response.achievement_ids;
    out.count = response.num_achievement_ids;
  }
  done(&out, ud);
  rc_api_destroy_fetch_user_unlocks_response(&response);
}

/* --- the rest ------------------------------------------------------------------------------ */

size_t nib_rc_user_agent_clause(char* out, size_t size) {
  if (!out || size == 0) return 0;
  /* rcheevos asks nothing of the client for its clause but an external one, which this build
   * has none of. */
  return rc_client_get_user_agent_clause(NULL, out, size);
}

const char* nib_rc_version(void) {
  return RCHEEVOS_VERSION_STRING;
}

void nib_rc_sizes(size_t out[12]) {
  out[0] = sizeof(nib_rc_event);
  out[1] = sizeof(nib_rc_achievement);
  out[2] = sizeof(nib_rc_leaderboard);
  out[3] = sizeof(nib_rc_tracker);
  out[4] = sizeof(nib_rc_callbacks);
  out[5] = sizeof(nib_rc_settings);
  out[6] = sizeof(nib_rc_summary);
  out[7] = sizeof(nib_rc_status);
  out[8] = sizeof(nib_rc_login);
  out[9] = sizeof(nib_rc_game);
  out[10] = sizeof(nib_rc_set_achievement);
  out[11] = sizeof(nib_rc_unlocks);
}
