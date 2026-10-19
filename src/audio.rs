//! What suits the system's sound server. On nibOS SDL's audio goes to PipeWire at 48 kHz, which
//! takes small buffers well; on the stock OS the system's SDL reaches PulseAudio through an ALSA
//! shim whose small buffers underrun on every period (a gap each time), so ask for big ones.

use crate::platform;

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
