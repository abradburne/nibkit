//! The DS+'s buttons by what is printed on them. Its face buttons are in Nintendo's layout (A on
//! the right, B at the bottom, X at the top, Y on the left), and SDL's game controller API names
//! buttons by position on an Xbox pad, so SDL's "A" is the DS+'s B. [`printed`] turns SDL's
//! name for a face button into the DS+'s label:
//!
//! ```
//! use nibkit::input::{printed, Face};
//! // SDL reported its "B" (the right-hand button): the DS+'s A.
//! assert_eq!(printed(Face::B), Face::A);
//! ```

use crate::platform;

/// A face button.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Face {
    A,
    B,
    X,
    Y,
}

/// The DS+'s label on the face button SDL calls `sdl`.
pub fn printed(sdl: Face) -> Face {
    platform::printed(sdl)
}

/// SDL's name for the face button labelled `label`.
pub fn sdl_name(label: Face) -> Face {
    [Face::A, Face::B, Face::X, Face::Y]
        .into_iter()
        .find(|&f| printed(f) == label)
        .unwrap_or(label)
}

/// The gamepad's name, where it is known: on nibOS, nibos-input's merged gamepad.
pub const GAMEPAD_NAME: Option<&str> = platform::GAMEPAD_NAME;

/// Gamepad mapping files to load (SDL_GameControllerAddMappingsFromFile) before opening the
/// gamepad: the stock emulators' on the stock OS; none on nibOS.
pub const CONTROLLER_MAPPINGS: &[&str] = platform::CONTROLLER_MAPPINGS;

/// Menu (SDL's "Guide") arrives as a press and a release together when it is let go of: nibOS
/// holds it back in case it is the start of a shortcut (Menu + volume: brightness).
pub const MENU_ON_RELEASE: bool = platform::MENU_ON_RELEASE;

/// Back is a key of its own, not a gamepad button. Through sdl2-compat it arrives as SDL2's
/// "Sleep" keycode; a translating sdl2-compat would say "AC Back". Bind both.
pub const BACK_KEYCODES: &[&str] = &["Sleep", "AC Back"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        // A and B are swapped on both systems; X and Y on the stock OS only.
        assert_eq!(printed(Face::B), Face::A);
        assert_eq!(printed(Face::A), Face::B);
        for f in [Face::A, Face::B, Face::X, Face::Y] {
            assert_eq!(printed(sdl_name(f)), f);
        }
    }
}
