use serde::Deserialize;

type Rgb = [u8; 3];

#[derive(Debug, Clone, Deserialize)]
pub enum LedMode {
    /// Static color(s); always exactly 4 entries, missing entries repeat the last value.
    Static { colors: Vec<Rgb> },
}

#[derive(Debug, Clone, Deserialize)]
pub struct LedConfig {
    /// `None` - no LED command will be sent, device keeps its current state
    pub mode: Option<LedMode>,
    /// LED brightness 0-100
    #[serde(default = "default_brightness")]
    pub brightness: u8,
}

impl Default for LedConfig {
    fn default() -> Self {
        Self {
            mode: None,
            brightness: default_brightness(),
        }
    }
}

fn default_brightness() -> u8 {
    100
}

/// Extends `colors` to `count` entries by repeating the last value.
fn fill_colors(colors: &mut Vec<Rgb>, count: usize) {
    if let Some(&last) = colors.last() {
        colors.resize(count, last);
    }
}

impl LedConfig {
    /// Normalizes the config after deserialization: fills missing LED colors and
    /// clears the mode if colors is empty.
    pub fn resolved_colors(mut self) -> Self {
        if let Some(LedMode::Static { ref mut colors }) = self.mode {
            if colors.is_empty() {
                self.mode = None;
            } else {
                fill_colors(colors, 4);
            }
        }
        self
    }
}
