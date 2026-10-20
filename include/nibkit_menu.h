/* nibkit for C and C++: DSlot's menu for an app's touch screen (1024 x 768), and what its pages
 * show: DSlot's settings and an emulator's options, brightness, the clock, the battery, the volume.
 *
 * Fill a page, answer the buttons and touches, draw it:
 *
 *     NibMenu *m = nib_menu_new();
 *     nib_menu_begin(m, "MOLE PATROL", "A  choose   \xc2\xb7   B  resume");
 *     nib_menu_tab(m, "Play", 1);
 *     nib_menu_tab(m, "States", 0);
 *     nib_menu_row(m, "Resume", NIB_VALUE_NONE, NULL, 0);
 *     nib_menu_row(m, "Fast-forward", NIB_VALUE_TEXT, "Off", 0);
 *     nib_menu_status(m, "5:05 PM", 76, 0, 1, 0.0f);
 *     NibEffect e = nib_menu_key(m, NIB_KEY_DOWN);
 *     int w, h;
 *     const uint8_t *rgba = nib_menu_render(m, &w, &h);
 *
 * A page is filled again after anything changes (begin, tabs, rows or cards); the highlighted
 * row, the scroll position and the highlighted card are kept from one filling to the next, so
 * the highlight stays where the player left it. Adding a card makes the page a shelf of cards
 * (save states) instead of rows. Strings are UTF-8 and copied; a picture is copied too.
 *
 * The layout, colours, face and button rules are DSlot's: see nibkit's `menu` module.
 */

#ifndef NIBKIT_MENU_H
#define NIBKIT_MENU_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct NibMenu NibMenu;

/* What a row shows on its right. `text` is the value; `number` is a rating (0-5) or a level
 * (0-100). */
enum {
    NIB_VALUE_NONE = 0,      /* an action: nothing on the right */
    NIB_VALUE_TEXT = 1,      /* set here: white, with < > when highlighted */
    NIB_VALUE_INHERITED = 2, /* "Use GB (Light)": muted */
    NIB_VALUE_MORE = 3,      /* opens a page or list: "2 on >" */
    NIB_VALUE_STARS = 4,     /* number: 0-5 */
    NIB_VALUE_SLIDER = 5,    /* number: 0-100 */
    NIB_VALUE_PROMPT = 6,    /* waiting for its second press: white */
    NIB_VALUE_WARNING = 7    /* the same, for an action that deletes: red */
};

/* The buttons, by what is printed on them. */
enum {
    NIB_KEY_UP = 0,
    NIB_KEY_DOWN = 1,
    NIB_KEY_LEFT = 2,
    NIB_KEY_RIGHT = 3,
    NIB_KEY_A = 4,
    NIB_KEY_B = 5,
    NIB_KEY_X = 6,
    NIB_KEY_Y = 7,
    NIB_KEY_L = 8,
    NIB_KEY_R = 9
};

/* What a button or touch asks of the app. */
enum {
    NIB_EFFECT_NONE = 0,   /* nothing (the highlight may have moved: draw again) */
    NIB_EFFECT_CHANGE = 1, /* step row `index`'s value by `step` (-1 or +1) */
    NIB_EFFECT_CHOOSE = 2, /* act on row `index` */
    NIB_EFFECT_USE = 3,    /* Y on row `index`: back to the level above (clear it here) */
    NIB_EFFECT_PAGE = 4,   /* show tab `index` */
    NIB_EFFECT_BACK = 5,   /* B: back a level, or close the menu */
    NIB_EFFECT_CARD = 6    /* button `key` (A, X, Y) on card `index` */
};

typedef struct {
    int kind;
    int index;
    int step;
    int key;
} NibEffect;

NibMenu *nib_menu_new(void);
void nib_menu_free(NibMenu *m);

/* Starts filling a page: its title (the line under the tabs) and its hint line. */
void nib_menu_begin(NibMenu *m, const char *title, const char *hint);
void nib_menu_tab(NibMenu *m, const char *label, int selected);
/* A heading over the rows that follow: small and faint, never highlighted. */
void nib_menu_heading(NibMenu *m, const char *label);
void nib_menu_row(NibMenu *m, const char *label, int value_kind, const char *text, int number);
/* A card on a shelf: a picture (RGBA, rows top first; NULL for an empty slot) and its label. */
void nib_menu_card(NibMenu *m, const char *label, const uint8_t *rgba, int width, int height);
/* The resume point (written when the game was left), added before the slots: drawn apart from
 * them and differently, its label when it was written ("Today 4:41 PM"). A loads it; X and Y
 * do nothing on it. Open the shelf on nib_menu_first_slot(m), the first numbered slot. */
void nib_menu_resume_card(NibMenu *m, const char *label, const uint8_t *rgba, int width,
                          int height);
int nib_menu_first_slot(const NibMenu *m);
/* The bar: the clock as shown, the battery (-1 if none) and whether it is charging, and the slot
 * in the bar's edge: 1 for the DS card slot, 2 for slot-2 with carts `cart_width` wide (GBA 560). */
void nib_menu_status(NibMenu *m, const char *clock, int battery, int charging, int slot, float cart_width);

/* The highlighted row (counting headings) or card, and moving it. */
int nib_menu_row_at(const NibMenu *m);
void nib_menu_set_row(NibMenu *m, int row);
int nib_menu_card_at(const NibMenu *m);
void nib_menu_set_card(NibMenu *m, int card);

NibEffect nib_menu_key(NibMenu *m, int key);
/* A finger down at (x, y), in screen units (0-1024, 0-768). */
NibEffect nib_menu_touch(NibMenu *m, float x, float y);

/* The page drawn, RGBA, rows top first, `*width` x `*height` (1024 x 768): valid until the next
 * call on `m`. */
const uint8_t *nib_menu_render(NibMenu *m, int *width, int *height);

/* The top screen while the menu is open, in the same kind of buffer as nib_menu_render.
 *
 * Paused: `game` (RGBA, any size, rows top first; NULL for none) scaled by nearest pixel to fill
 * the screen keeping its shape, dimmed, with `title` ("Paused") large and `subtitle` (the game's
 * name and time today) under it. */
const uint8_t *nib_top_paused(NibMenu *m, const uint8_t *game, int gw, int gh,
                              const char *title, const char *subtitle, int *width, int *height);

/* On a shelf of save states: the highlighted one's `picture` large (NULL for an empty slot), its
 * `name` ("Slot 2") beside it, `under` that ("Quick slot", or NULL), then `n` lines of
 * `labels[i]` and `values[i]` ("Saved", "Today 4:41 PM"). */
const uint8_t *nib_top_state(NibMenu *m, const uint8_t *picture, int pw, int ph,
                             const char *name, const char *under,
                             const char *const *labels, const char *const *values, int n,
                             int *width, int *height);

/* ---- DSlot's settings and an emulator's options -------------------------------------------
 *
 * The same file, rows and words as DSlot's own pages (DSlot's docs/emulators.md, "One place for
 * each option"). Values cross as text: a string as it is, a number in decimal, a toggle "true" or
 * "false". A scope is "system", a console ("NDS") or a game ("NDS/<ROM name>", the ROM's file
 * name without its extension). Strings returned are valid until the next call on the same
 * handle. */

typedef struct NibSettings NibSettings;
typedef struct NibOptions NibOptions;

/* DSlot's settings file (nibOS); NULL where there is none (the stock OS). */
NibSettings *nib_settings_open(void);
NibSettings *nib_settings_open_file(const char *path);
void nib_settings_free(NibSettings *s);
/* Reads the file again; writes it (atomically): 0 on success, -1 on failure. */
void nib_settings_reload(NibSettings *s);
int nib_settings_save(NibSettings *s);
/* A key ("dsperate.hires3d") set at a scope exactly, as text; NULL if unset. */
const char *nib_settings_get(NibSettings *s, const char *scope, const char *key);
void nib_settings_set_text(NibSettings *s, const char *scope, const char *key, const char *value);
void nib_settings_set_int(NibSettings *s, const char *scope, const char *key, int64_t value);
void nib_settings_set_bool(NibSettings *s, const char *scope, const char *key, int value);
/* Clears a key at a scope, so the level above applies there ("Use ..."). */
void nib_settings_reset(NibSettings *s, const char *scope, const char *key);

/* Emulator `id`'s ("dsperate") options for `system` ("NDS"), from the descriptions DSlot reads;
 * or from a description's text. NULL if there is none, or it isn't valid. */
NibOptions *nib_options_load(const char *id, const char *system);
NibOptions *nib_options_from_text(const char *id, const char *description, const char *system);
void nib_options_free(NibOptions *o);
int nib_options_count(NibOptions *o);

/* Option `i`: its key in the settings file, its label, its kind (0 a choice, 1 a toggle,
 * 2 a number), a choice's values (`label` 0: the value as text; 1: its label as shown), a
 * number's range and step (returns its unit; NULL if it isn't a number), its default (NULL: the
 * emulator's own, not passed). */
const char *nib_option_key(NibOptions *o, int i);
const char *nib_option_label(NibOptions *o, int i);
int nib_option_kind(NibOptions *o, int i);
int nib_option_choices(NibOptions *o, int i);
const char *nib_option_choice(NibOptions *o, int i, int j, int label);
const char *nib_option_range(NibOptions *o, int i, int64_t *min, int64_t *max, int64_t *step);
const char *nib_option_default(NibOptions *o, int i);

/* A value as its row shows it; the value one step on or back (`dir` +1 or -1) from `from` (NULL:
 * from the default), round a choice and held at a number's ends. */
const char *nib_option_show(NibOptions *o, int i, const char *value);
const char *nib_option_step(NibOptions *o, int i, const char *from, int dir);

/* Option `i`'s value seen from the game's page (`rom`: the ROM's name without its extension) or
 * the console's (`rom` NULL); NULL when set nowhere and with no default. `*level`: 3 the game,
 * 2 the console, 1 the system, 0 its default or nowhere. */
const char *nib_option_value(NibOptions *o, int i, NibSettings *s, const char *rom, int *level);
/* What its row shows when not set at its page's level: "Use NDS (2x)", "Use default (On)" or
 * "Use DSperate's own". */
const char *nib_option_inherit(NibOptions *o, int i, NibSettings *s, const char *rom);
/* Sets option `i` at `scope` (text, parsed by its kind): 0, or -1 if it isn't one of its
 * values. Save with nib_settings_save. */
int nib_option_set(NibOptions *o, int i, NibSettings *s, const char *scope, const char *value);
/* The command-line words a value adds: how many, then each. */
int nib_option_words(NibOptions *o, int i, const char *value);
const char *nib_option_word(NibOptions *o, int j);

/* ---- The System page ------------------------------------------------------------------------
 *
 * -1 where the system doesn't let an app have one (the stock OS keeps brightness and volume). */

/* `screen` 0 the top, 1 the bottom; set at once as a slider moves, save when it is let go of. */
int nib_backlight_get(int screen);
void nib_backlight_set(int screen, int percent);
void nib_backlight_save(void);
/* 12-hour (1) or 24-hour (0), for every app; setting it: 0 on success, -1 on failure. */
int nib_clock_hour12(void);
int nib_clock_set_hour12(int on);
/* The time now as the bar shows it ("17:05", "5:05 PM"); valid until the next call on `m`. */
const char *nib_menu_clock(NibMenu *m);
/* The battery's charge in percent and whether it is charging (`charging` may be NULL). */
int nib_battery(int *charging);
int nib_volume_get(void);
void nib_volume_set(int percent);

#ifdef __cplusplus
}
#endif

#endif
