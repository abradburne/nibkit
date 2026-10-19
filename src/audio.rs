//! What suits the system's sound server. On nibOS SDL's audio goes to PipeWire at 48 kHz, which
//! takes small buffers well; on the stock OS the system's SDL reaches PulseAudio through an ALSA
//! shim whose small buffers underrun on every period (a gap each time), so ask for big ones.

use crate::platform;
use std::{io, process::Command};

/// The sample rate to open audio at (no resampling on the way) and a buffer size, in frames,
/// that plays without gaps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Advice {
    pub rate: u32,
    pub buffer_frames: u32,
}

pub const ADVICE: Advice = Advice {
    rate: platform::AUDIO_RATE,
    buffer_frames: platform::AUDIO_BUFFER,
};

/// Which tuning the speaker amps use. nibOS ships its own (the DSP bypassed, conservative gain);
/// the official one, with the DSP's speaker protection and EQ, is imported from the user's own
/// stock firmware ([`import_tuning`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tuning {
    Nibos,
    Official,
    /// A file nibOS didn't put there.
    Other,
    /// None: the speakers stay silent.
    None,
}

impl Tuning {
    fn parse(text: &str) -> Option<Tuning> {
        match text
            .trim()
            .strip_prefix("speaker tuning: ")?
            .split_whitespace()
            .next()?
        {
            "nibos" => Some(Tuning::Nibos),
            "official" => Some(Tuning::Official),
            "other" => Some(Tuning::Other),
            "none" => Some(Tuning::None),
            _ => None,
        }
    }
}

/// The speakers' tuning now; `None` where the system has no such choice.
pub fn tuning() -> Option<Tuning> {
    let out = Command::new(platform::AMP_TUNING?)
        .arg("tuning")
        .output()
        .ok()?;
    Tuning::parse(&String::from_utf8_lossy(&out.stdout))
}

fn nib_audio(args: &[&str]) -> io::Result<String> {
    let cmd = platform::AMP_TUNING.ok_or(io::ErrorKind::Unsupported)?;
    let out = Command::new("sudo")
        .arg("-n")
        .arg(cmd)
        .args(args)
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if out.status.success() {
        Ok(text)
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ))
    }
}

/// Imports the official tuning from the user's stock firmware: a stock card in TF2, or a stock
/// image (.img, .img.gz, .img.xz) on the games partition; then uses it. It applies at the next
/// restart. The message on success or failure is a sentence to show the user.
pub fn import_tuning() -> io::Result<String> {
    nib_audio(&["import-tuning"])
}

/// Uses nibOS's tuning, or the imported official one again (from the next restart).
pub fn set_tuning(official: bool) -> io::Result<String> {
    nib_audio(&["tuning", if official { "official" } else { "nibos" }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuning_lines() {
        assert_eq!(
            Tuning::parse("speaker tuning: official\n"),
            Some(Tuning::Official)
        );
        assert_eq!(
            Tuning::parse("speaker tuning: nibos (restart to apply)"),
            Some(Tuning::Nibos)
        );
        assert_eq!(Tuning::parse("something else"), None);
    }
}
