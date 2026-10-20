//! A plain nibOS app on nibkit and SDL2: both screens, the buttons by their labels, touch,
//! the top screen's brightness, and a setting that is remembered. Nothing to do with DSlot.
//!
//!   cargo run --example hello                                    # nibOS, or a desktop
//!   cargo run --example hello --no-default-features --features stock
//!
//! On a desktop both screens are in one window, stacked; the mouse on the bottom one is a
//! finger, and the keyboard stands in for the buttons (`nibkit::input::keyboard`: X is A, the
//! arrows the D-pad, Escape quits).
//!
//! On the device, run it with nothing else on the screens (`sdk/run.sh` stops DSlot and starts
//! it again afterwards). D-pad up / down: the top screen's brightness. A: change the colour,
//! which is remembered. A finger on the bottom screen: a dot. Select + Start: quit.

use nibkit::input::{keyboard, printed, Face};
use nibkit::screens::{self, Screen};
use nibkit::settings::{Scope, Settings, Value};
use nibkit::{backlight, paths::AppDirs, touch};
use sdl2::{controller::Button, event::Event, pixels::Color, rect::Rect, render::WindowCanvas};

const COLOURS: [(&str, Color); 3] = [
    ("teal", Color::RGB(20, 140, 150)),
    ("plum", Color::RGB(120, 60, 130)),
    ("sand", Color::RGB(190, 160, 90)),
];

fn main() -> Result<(), String> {
    // A colour, remembered between runs: the app's settings, at the system level.
    let mut settings = Settings::load(AppDirs::new("hello").config.join("settings.toml"));
    let mut colour = settings
        .get(&Scope::System, "colour")
        .and_then(Value::as_str)
        .unwrap_or("teal")
        .to_owned();

    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let bounds: Vec<screens::Rect> = (0..video.num_video_displays()?)
        .filter_map(|i| video.display_bounds(i).ok())
        .map(|r| screens::Rect::new(r.x(), r.y(), r.width(), r.height()))
        .collect();
    // Where each screen is. On a desktop, one window with the two screens stacked.
    let placement = screens::place(&bounds, false);
    let mut screens: Vec<(Screen, WindowCanvas)> = Vec::new();
    match placement {
        Some(p) => {
            // A window per screen; on the stock OS one across both.
            let wanted = if p.shared {
                vec![Screen::Top]
            } else {
                vec![Screen::Top, Screen::Bottom]
            };
            for screen in wanted {
                let r = p.region(screen).rect;
                let width = if p.shared { r.w * 2 } else { r.w };
                let window = video
                    .window("hello", width, r.h)
                    .position(r.x, r.y)
                    .fullscreen_desktop()
                    .build()
                    .map_err(|e| e.to_string())?;
                let canvas = window
                    .into_canvas()
                    .accelerated()
                    .present_vsync()
                    .build()
                    .map_err(|e| e.to_string())?;
                screens.push((screen, canvas));
            }
        }
        None => {
            let screens::Windows::Stacked { size, .. } = screens::desktop_windows(true, 0.5) else {
                unreachable!("asked for one window");
            };
            let window = video
                .window("hello", size.0, size.1)
                .build()
                .map_err(|e| e.to_string())?;
            let canvas = window
                .into_canvas()
                .present_vsync()
                .build()
                .map_err(|e| e.to_string())?;
            screens.push((Screen::Top, canvas));
        }
    }

    let pads = sdl.game_controller()?;
    for file in nibkit::input::CONTROLLER_MAPPINGS {
        let _ = pads.load_mappings(file);
    }
    let _open: Vec<_> = (0..pads.num_joysticks()?)
        .filter_map(|i| pads.open(i).ok())
        .collect();
    let mut panel = touch::Panel::open();
    let mut pointers = Vec::new();
    let mut dot: Option<(f32, f32)> = None;
    let mut select = false;
    let mut events = sdl.event_pump()?;
    // On a desktop: where each screen is in the one window, and whether the mouse is down.
    let stacked = match screens::desktop_windows(true, 0.5) {
        screens::Windows::Stacked { top, bottom, .. } if placement.is_none() => Some((top, bottom)),
        _ => None,
    };
    let mut mouse_down = false;
    'run: loop {
        for event in events.poll_iter() {
            // The keyboard as the buttons: A changes the colour, Escape (Menu) quits.
            let key = match &event {
                Event::KeyDown {
                    keycode: Some(k), ..
                } => keyboard(&k.name()),
                _ => None,
            };
            match key {
                Some(nibkit::input::Button::Menu) => break 'run,
                Some(nibkit::input::Button::A) => {
                    next_colour(&mut colour, &mut settings);
                    continue;
                }
                _ => {}
            }
            // The mouse on the bottom screen, as a finger.
            if let Some((_, bottom)) = stacked {
                match event {
                    Event::MouseButtonDown { x, y, .. } => {
                        mouse_down = true;
                        dot = bottom.panel_point(x, y).map(as_fraction);
                    }
                    Event::MouseMotion { x, y, .. } if mouse_down => {
                        dot = bottom.panel_point(x, y).map(as_fraction);
                    }
                    Event::MouseButtonUp { .. } => {
                        mouse_down = false;
                        dot = None;
                    }
                    _ => {}
                }
            }
            match event {
                Event::Quit { .. } => break 'run,
                Event::ControllerButtonDown {
                    button: Button::Back,
                    ..
                } => select = true,
                Event::ControllerButtonUp {
                    button: Button::Back,
                    ..
                } => select = false,
                Event::ControllerButtonDown {
                    button: Button::Start,
                    ..
                } if select => break 'run,
                Event::ControllerButtonDown {
                    button: Button::DPadUp | Button::DPadDown,
                    ..
                } => {
                    let up = matches!(
                        event,
                        Event::ControllerButtonDown {
                            button: Button::DPadUp,
                            ..
                        }
                    );
                    if let Some(now) = backlight::get(Screen::Top) {
                        backlight::set(
                            Screen::Top,
                            if up {
                                (now + 10).min(100)
                            } else {
                                now.saturating_sub(10)
                            },
                        );
                        backlight::save();
                    }
                }
                // SDL's names are by position; the DS+'s labels are not.
                Event::ControllerButtonDown { button, .. }
                    if face(button).map(printed) == Some(Face::A) =>
                {
                    next_colour(&mut colour, &mut settings);
                }
                _ => {}
            }
        }
        pointers.clear();
        if let Some(panel) = panel.as_mut() {
            panel.poll(&mut pointers);
        }
        for p in &pointers {
            dot = match *p {
                touch::Pointer::Down(x, y) | touch::Pointer::Move(x, y) => Some((x, y)),
                touch::Pointer::Up => None,
            };
        }
        let fill = COLOURS
            .iter()
            .find(|(n, _)| *n == colour)
            .map_or(COLOURS[0].1, |c| c.1);
        for (screen, canvas) in screens.iter_mut() {
            canvas.set_draw_color(fill);
            canvas.clear();
            let (w, h) = canvas.output_size()?;
            // Where the finger is on this window: the bottom screen's window, or the right half
            // of the stock OS's one across both.
            let spot = match (dot, placement, stacked) {
                (Some((x, y)), Some(p), _) if p.shared => {
                    Some((w as f32 / 2.0 * (1.0 + x), y * h as f32))
                }
                (Some((x, y)), Some(_), _) if *screen == Screen::Bottom => {
                    Some((x * w as f32, y * h as f32))
                }
                // A desktop's one window: the bottom screen's part of it, in its drawing pixels.
                (Some((x, y)), None, Some((_, bottom))) => {
                    let k = w as f32 / bottom.w as f32;
                    Some((
                        (bottom.x as f32 + x * bottom.w as f32) * k,
                        (bottom.y as f32 + y * bottom.h as f32) * k,
                    ))
                }
                _ => None,
            };
            if let Some((x, y)) = spot {
                canvas.set_draw_color(Color::WHITE);
                canvas.fill_rect(Rect::new(x as i32 - 20, y as i32 - 20, 40, 40))?;
            }
            canvas.present();
        }
    }
    Ok(())
}

/// The next colour, remembered in the app's settings.
fn next_colour(colour: &mut String, settings: &mut Settings) {
    let at = COLOURS.iter().position(|(n, _)| *n == colour).unwrap_or(0);
    *colour = COLOURS[(at + 1) % COLOURS.len()].0.to_owned();
    settings.set(Scope::System, "colour", colour.as_str().into());
    if let Err(e) = settings.save() {
        eprintln!("hello: {e}");
    }
}

/// A point on the panel (1024 x 768 units) as a fraction of it, as the touch panel gives one.
fn as_fraction((x, y): (f32, f32)) -> (f32, f32) {
    (x / screens::WIDTH as f32, y / screens::HEIGHT as f32)
}

/// SDL's face buttons as nibkit names them.
fn face(button: Button) -> Option<Face> {
    Some(match button {
        Button::A => Face::A,
        Button::B => Face::B,
        Button::X => Face::X,
        Button::Y => Face::Y,
        _ => return None,
    })
}
