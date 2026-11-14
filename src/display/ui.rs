use core::fmt::Write;
use embassy_time::Instant;
use serde_json_core::heapless::String;

use super::font::glyph;
use crate::telemetry::{Memory, Snapshot, Status};

pub(super) struct Frame {
    pub(super) pages: [[u8; 128]; 8],
}

impl Frame {
    pub(super) fn render(status: Status) -> Self {
        let mut frame = Self {
            pages: [[0; 128]; 8],
        };
        match status {
            Status::Live(sample) => frame.live(sample),
            Status::Setup => frame.placeholder(false, "SET UP WI-FI", "SEE README"),
            Status::WifiLost => frame.placeholder(false, "WI-FI LOST", "RECONNECTING"),
            Status::TiinyUnavailable => frame.placeholder(true, "TIINY UNAVAILABLE", "RETRYING"),
        }
        frame
    }

    fn live(&mut self, sample: Snapshot) {
        let name = sample.models.name();
        let mut second = String::<24>::new();
        if name.is_empty() {
            self.text(4, 4, "NO MODEL LOADED");
        } else {
            let cut = name.len().min(15);
            let split = name[..cut].rfind('-').filter(|&i| i >= 9);
            let cut = if name.len() > 15 {
                split.unwrap_or(cut)
            } else {
                cut
            };
            self.text(4, 4, &name[..cut]);
            let rest = name[cut..].trim_start_matches('-');
            let _ = second.push_str(&rest[..rest.len().min(13)]);
            if rest.len() > 13 {
                second.truncate(10);
                let _ = second.push_str("...");
            }
            if sample.models.count > 1 && second.len() <= 10 {
                let _ = write!(second, " +{}", sample.models.count - 1);
            }
        }

        self.icon(115, 3, WIFI, 9);
        if let Some(start) = sample.busy_since {
            self.icon(103, 4, HOURGLASS, 7);
            let seconds = Instant::now()
                .saturating_duration_since(start)
                .as_secs()
                .min(359_999);
            let mut elapsed = String::<8>::new();
            let hours = seconds / 3600;
            let minutes = seconds / 60 % 60;
            if hours > 0 {
                let _ = write!(elapsed, "{hours}:{minutes:02}:{:02}", seconds % 60);
            } else {
                let _ = write!(elapsed, "{minutes:02}:{:02}", seconds % 60);
            }
            let available = (120 - elapsed.len() * 6) / 6;
            if second.len() > available {
                second.truncate(available - 3);
                let _ = second.push_str("...");
            }
            self.right(124, 13, elapsed.as_str());
        } else {
            self.icon(98, 3, IDLE, 14);
        }
        self.text(4, 13, second.as_str());
        self.rect(4, 23, 120, 1, true);
        self.percentage(27, "NPU", sample.npu);
        self.percentage(36, "CPU", sample.cpu);
        self.memory(45, "SOC RAM", sample.soc_ram, "--");
        self.memory(54, "NPU RAM", sample.npu_ram, "--/48G");
    }

    fn placeholder(&mut self, connected: bool, title: &str, subtitle: &str) {
        self.text(4, 4, "TIINY POCKET");
        self.icon(103, 4, UNAVAILABLE, 7);
        self.icon(115, 3, WIFI, 9);
        if !connected {
            for offset in 0..9 {
                self.rect(114 + offset, 11 - offset, 3, 1, false);
            }
            for offset in 0..9 {
                self.pixel(115 + offset, 11 - offset, true);
            }
        }
        self.rect(4, 15, 120, 1, true);
        self.center(30, title);
        self.center(43, subtitle);
    }

    fn percentage(&mut self, y: usize, label: &str, load: Option<f32>) {
        let value = load
            .filter(|value| value.is_finite())
            .map(|value| value.clamp(0.0, 100.0));
        let mut text = String::<8>::new();
        if let Some(value) = value {
            let _ = write!(text, "{}%", (value + 0.5) as u8);
        }
        self.meter(y, label, text.as_str(), value.map(|value| value / 100.0));
    }

    fn memory(&mut self, y: usize, label: &str, memory: Memory, unavailable: &str) {
        const GIB: u64 = 1 << 30;
        let mut text = String::<16>::new();
        let fraction = memory
            .used
            .zip(memory.total)
            .filter(|&(_, total)| total > 0)
            .map(|(used, total)| {
                if let (Ok(used), Ok(total)) =
                    (u16::try_from(used / GIB), u16::try_from(total / GIB))
                    && used < 100
                    && total < 100
                {
                    let _ = write!(text, "{used}/{total}G");
                }
                (used as f32 / total as f32).clamp(0.0, 1.0)
            });
        let value = if text.is_empty() {
            unavailable
        } else {
            text.as_str()
        };
        self.meter(y, label, value, fraction);
    }

    fn meter(&mut self, y: usize, label: &str, value: &str, fraction: Option<f32>) {
        self.text(4, y, label);
        if let Some(fraction) = fraction {
            self.rect(49, y + 1, 35, 1, true);
            self.rect(49, y + 5, 35, 1, true);
            self.rect(49, y + 1, 1, 5, true);
            self.rect(83, y + 1, 1, 5, true);
            self.rect(51, y + 3, (31.0 * fraction + 0.5) as usize, 1, true);
        }
        self.right(124, y, if value.is_empty() { "--" } else { value });
    }

    fn text(&mut self, x: usize, y: usize, text: &str) {
        for (index, byte) in text.bytes().enumerate() {
            for (row, pattern) in glyph(byte).into_iter().enumerate() {
                for col in 0..5 {
                    if pattern & (16 >> col) != 0 {
                        self.pixel(x + index * 6 + col, y + row, true);
                    }
                }
            }
        }
    }

    fn right(&mut self, x: usize, y: usize, text: &str) {
        self.text(x.saturating_sub(text.len() * 6), y, text);
    }

    fn center(&mut self, y: usize, text: &str) {
        self.text(128usize.saturating_sub(text.len() * 6) / 2, y, text);
    }

    fn icon(&mut self, x: usize, y: usize, shape: &[u16], width: usize) {
        for (row, bits) in shape.iter().enumerate() {
            for column in 0..width {
                if bits & (1 << (width - 1 - column)) != 0 {
                    self.pixel(x + column, y + row, true);
                }
            }
        }
    }

    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, on: bool) {
        for row in y..y + h {
            for col in x..x + w {
                self.pixel(col, row, on);
            }
        }
    }

    fn pixel(&mut self, x: usize, y: usize, on: bool) {
        if x < 128 && y < 64 {
            let mask = 1 << (y % 8);
            let byte = &mut self.pages[y / 8][x];
            if on {
                *byte |= mask;
            } else {
                *byte &= !mask;
            }
        }
    }
}

const HOURGLASS: &[u16] = &[127, 34, 20, 8, 20, 62, 127];
const IDLE: &[u16] = &[
    0b00000000011111,
    0b00000000000001,
    0b00000000000010,
    0b00000000000100,
    0b00011110001000,
    0b00000100010000,
    0b00001000011111,
    0b00011110000000,
    0b10000000000000,
];
const UNAVAILABLE: &[u16] = &[65, 34, 20, 8, 20, 34, 65];
const WIFI: &[u16] = &[480, 24, 4, 450, 34, 17, 17, 393, 393];
