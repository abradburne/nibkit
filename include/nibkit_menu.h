/* nibkit's menu for C and C++: DSlot's menu, for an app's touch screen (1024 x 768).
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

#ifdef __cplusplus
}
#endif

#endif
