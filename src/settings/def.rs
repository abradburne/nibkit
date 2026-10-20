//! What a setting is: its key, its label, the values it takes, the levels it can be set at and
//! its default. An app defines its own; DSlot also makes them from the options an emulator
//! declares in its description file.

use super::Scope;
use toml::Value;

/// The values a setting takes.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// One of a list, each value with its label.
    Choice(Vec<(Value, String)>),
    /// On or off.
    Toggle,
    /// A whole number from `min` to `max` in steps of `step`, shown with `unit` after it.
    Number {
        /// The lowest value.
        min: i64,
        /// The highest value.
        max: i64,
        /// How far apart the values are.
        step: i64,
        /// What is shown after the value ("%", "ms").
        unit: String,
    },
}

impl Kind {
    /// A choice of strings: `(value as kept, label as shown)`.
    pub fn choice(values: &[(&str, &str)]) -> Kind {
        Kind::Choice(
            values
                .iter()
                .map(|(v, l)| (Value::from(*v), (*l).to_owned()))
                .collect(),
        )
    }
}

/// The levels a setting can be set at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Levels {
    /// Whether it can be set for everything.
    pub system: bool,
    /// Whether it can be set for a console.
    pub console: bool,
    /// Whether it can be set for a game.
    pub game: bool,
}

impl Levels {
    /// The system's only: how the device or the app as a whole behaves.
    pub const SYSTEM: Levels = Levels {
        system: true,
        console: false,
        game: false,
    };
    /// For everything, overridden per console.
    pub const SYSTEM_AND_CONSOLE: Levels = Levels {
        system: true,
        console: true,
        game: false,
    };
    /// Per console, overridden per game: how games play.
    pub const CONSOLE_AND_GAME: Levels = Levels {
        system: false,
        console: true,
        game: true,
    };
    /// At every level.
    pub const ALL: Levels = Levels {
        system: true,
        console: true,
        game: true,
    };

    /// Whether it can be set at `scope`.
    pub fn has(self, scope: &Scope) -> bool {
        match scope {
            Scope::System => self.system,
            Scope::Console(_) => self.console,
            Scope::Game(_) => self.game,
        }
    }
}

/// A setting.
#[derive(Clone, Debug, PartialEq)]
pub struct Def {
    /// Its key in the settings file. A dot puts it in a table of its own: `dsperate.hires3d`
    /// is `hires3d` in `[console.NDS.dsperate]`.
    pub key: String,
    /// What the menu calls it.
    pub label: String,
    /// The values it takes.
    pub kind: Kind,
    /// The levels it can be set at.
    pub levels: Levels,
    /// What it is when no level sets it. `None`: up to whatever the value is passed to.
    pub default: Option<Value>,
}

impl Def {
    /// How `value` is shown.
    pub fn show(&self, value: &Value) -> String {
        match &self.kind {
            Kind::Choice(choices) => choices
                .iter()
                .find(|(v, _)| v == value)
                .map_or_else(|| display(value), |(_, label)| label.clone()),
            Kind::Toggle => if value.as_bool() == Some(true) {
                "On"
            } else {
                "Off"
            }
            .to_owned(),
            Kind::Number { unit, .. } => format!("{}{unit}", display(value)),
        }
    }

    /// The value one step from `from` in direction `dir` (wrapping round a list, held at the
    /// ends of a range). From nothing, it steps from the default; with no default, it lands on
    /// the first value.
    pub fn step(&self, from: Option<&Value>, dir: i32) -> Value {
        let from = from.or(self.default.as_ref());
        match &self.kind {
            Kind::Choice(choices) => {
                let Some(from) = from else {
                    return choices[0].0.clone();
                };
                let at = choices.iter().position(|(v, _)| v == from);
                let n = choices.len() as i32;
                let next = at.map_or(0, |at| (at as i32 + dir).rem_euclid(n));
                choices[next as usize].0.clone()
            }
            Kind::Toggle => Value::Boolean(!from.and_then(Value::as_bool).unwrap_or(false)),
            Kind::Number { min, max, step, .. } => {
                let Some(n) = from.and_then(Value::as_integer) else {
                    return Value::Integer(*min);
                };
                Value::Integer((n + step * dir as i64).clamp(*min, *max))
            }
        }
    }

    /// Whether `value` is one this setting takes (a hand-edited file may say anything).
    pub fn accepts(&self, value: &Value) -> bool {
        match &self.kind {
            Kind::Choice(choices) => choices.iter().any(|(v, _)| v == value),
            Kind::Toggle => value.is_bool(),
            Kind::Number { min, max, .. } => value
                .as_integer()
                .is_some_and(|n| (*min..=*max).contains(&n)),
        }
    }
}

/// A value as written, without TOML's quotes.
fn display(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping() {
        let view = Def {
            key: "view".into(),
            label: "View".into(),
            kind: Kind::choice(&[("shelf", "Carts"), ("grid", "Cover grid")]),
            levels: Levels::SYSTEM_AND_CONSOLE,
            default: Some(Value::from("shelf")),
        };
        assert_eq!(view.step(None, 1), Value::from("grid"));
        assert_eq!(
            view.step(Some(&Value::from("grid")), 1),
            Value::from("shelf")
        );
        assert_eq!(view.show(&Value::from("grid")), "Cover grid");
        assert!(view.accepts(&Value::from("shelf")) && !view.accepts(&Value::from("wall")));

        let sounds = Def {
            kind: Kind::Toggle,
            default: Some(Value::Boolean(true)),
            ..view.clone()
        };
        assert_eq!(sounds.step(None, 1), Value::Boolean(false));
        assert_eq!(sounds.show(&Value::Boolean(true)), "On");

        let skip = Def {
            kind: Kind::Number {
                min: 0,
                max: 3,
                step: 1,
                unit: String::new(),
            },
            default: None,
            ..view
        };
        assert_eq!(skip.step(None, 1), Value::Integer(0));
        assert_eq!(skip.step(Some(&Value::Integer(3)), 1), Value::Integer(3));
        assert_eq!(skip.step(Some(&Value::Integer(2)), -1), Value::Integer(1));
        assert!(!skip.accepts(&Value::Integer(9)));
        assert!(Levels::CONSOLE_AND_GAME.has(&Scope::Game("GB/Tetris".into())));
        assert!(!Levels::SYSTEM.has(&Scope::Console("GB".into())));
    }
}
