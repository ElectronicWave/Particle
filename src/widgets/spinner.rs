use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

const FRAME_MS: u128 = 100;
// Four-dot braille frames with 180-degree symmetry, so the visual center stays fixed.
const FRAMES: [&str; 4] = ["⠳", "⠭", "⠞", "⠭"];

pub const INTERVAL: Duration = Duration::from_millis(FRAME_MS as u64);

pub fn frame() -> &'static str {
    let elapsed = elapsed().as_millis();
    FRAMES[(elapsed / FRAME_MS) as usize % FRAMES.len()]
}

fn elapsed() -> Duration {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed()
}
