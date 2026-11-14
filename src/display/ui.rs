use core::{fmt::Write, net::Ipv4Addr};
use embassy_time::Instant;
use serde_json_core::heapless::String;

use super::font::glyph;
use crate::signal_strength::SignalStrength;
use crate::telemetry::{Memory, Snapshot, Status};

#[derive(Clone, Copy)]
enum WifiIcon {
    Disconnected,
    Connected(Option<i32>),
}

pub(super) struct Frame {
    pub(super) pages: [[u8; 128]; 8],
}

impl Frame {
    pub(super) fn render(status: Status, rssi: Option<i32>) -> Self {
        let mut frame = Self {
            pages: [[0; 128]; 8],
        };
        match status {
            Status::Live(sample) => frame.live(sample, rssi),
            Status::Setup => frame.placeholder(
                WifiIcon::Disconnected,
                "SET UP WI-FI",
                "USB SERIAL SETUP",
                false,
                None,
            ),
            Status::WifiConnecting => frame.connecting(),
            Status::WifiLost(ip) => frame.placeholder(
                WifiIcon::Disconnected,
                "WI-FI LOST",
                "RECONNECTING",
                true,
                Some(ip),
            ),
            Status::TiinyUnavailable(ip) => frame.placeholder(
                WifiIcon::Connected(rssi),
                "TIINY UNAVAILABLE",
                "RETRYING",
                true,
                Some(ip),
            ),
            Status::InvalidApiKey(ip) => frame.placeholder(
                WifiIcon::Connected(rssi),
                "INVALID API KEY",
                "CHECK USB SETUP",
                false,
                Some(ip),
            ),
            Status::AccessDenied(ip) => frame.placeholder(
                WifiIcon::Connected(rssi),
                "ACCESS DENIED",
                "CHECK TIINY ACCESS",
                false,
                Some(ip),
            ),
        }
        frame
    }

    fn connecting(&mut self) {
        let waves = (Instant::now().as_millis() / 250 % 5).min(3) as usize;
        for (inner, outer) in [(7, 10), (14, 17), (21, 24)].into_iter().take(waves) {
            let band = inner * inner..outer * outer;
            for up in 1..outer {
                for across in 0..=up {
                    if band.contains(&(up * up + across * across)) {
                        self.pixel(64 - across, 36 - up, true);
                        self.pixel(64 + across, 36 - up, true);
                    }
                }
            }
        }
        self.rect(62, 35, 5, 3, true);
        self.rect(63, 34, 3, 5, true);
        self.center(47, "CONNECTING TO WIFI...");
    }

    fn live(&mut self, sample: Snapshot, rssi: Option<i32>) {
        let now = Instant::now();
        let name = sample.models.name();
        let loading_percent = sample
            .loading_percent
            .filter(|_| sample.loading && sample.busy_since.is_none());
        let header_limit = if loading_percent.is_some() { 12 } else { 15 };
        let mut second = String::<24>::new();
        if name.is_empty() {
            if sample.loading {
                self.text(4, 4, "LOADING");
                let _ = second.push_str("MODEL");
            } else {
                self.text(4, 4, "NO MODEL LOADED");
            }
        } else {
            let cut = name.len().min(header_limit);
            let split = name[..cut].rfind('-').filter(|&i| i >= 9);
            let cut = if name.len() > header_limit {
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

        self.wifi_icon(WifiIcon::Connected(rssi));
        if let Some(start) = sample.busy_since {
            let elapsed_time = now.saturating_duration_since(start);
            let raised = (elapsed_time.as_millis() / 250 % 3) as usize;
            for dot in 0..3 {
                let y = if dot == raised { 4 } else { 7 };
                self.rect(100 + dot * 4, y, 3, 3, true);
            }
            let seconds = elapsed_time.as_secs().min(359_999);
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
        } else if sample.loading {
            let frame = (now.as_millis() / 500 % 3) as usize;
            self.icon(104, 3 + frame * 2, LOADING, 5);
            self.rect(103, 12, 7, 1, true);
            if let Some(progress) = loading_percent {
                let mut percent = String::<4>::new();
                let _ = write!(percent, "{progress}%");
                self.right(101, 5, percent.as_str());
            }
        } else {
            let frame = (now.as_millis() / 250 % 8) as usize;
            let visible = match frame {
                1 => IDLE_DOT,
                2 => IDLE_SMALL_Z,
                3..=6 => IDLE,
                _ => &[],
            };
            self.icon(98, 3, visible, 14);
        }
        self.text(4, 13, second.as_str());
        self.rect(4, 23, 120, 1, true);
        self.percentage(27, "NPU", sample.npu);
        self.percentage(36, "SOC", sample.cpu);
        self.memory(45, "SOC RAM", sample.soc_ram, "--");
        self.memory(54, "NPU RAM", sample.npu_ram, "--/48G");
    }

    fn wifi_icon(&mut self, wifi: WifiIcon) {
        let bars = match wifi {
            WifiIcon::Connected(Some(rssi)) => SignalStrength::from_rssi(rssi).bars(),
            WifiIcon::Connected(None) | WifiIcon::Disconnected => 0,
        };
        for (index, height) in [3, 6, 9].into_iter().enumerate() {
            let x = 115 + index * 4;
            let y = 12 - height;
            self.rect(x, y, 3, height, true);
            if index >= bars {
                self.rect(x + 1, y + 1, 1, height - 2, false);
            }
        }
        if matches!(wifi, WifiIcon::Disconnected) {
            for offset in 0..11 {
                self.rect(114 + offset, 11 - offset * 8 / 10, 3, 1, false);
            }
            for offset in 0..11 {
                self.pixel(115 + offset, 11 - offset * 8 / 10, true);
            }
        }
    }

    fn placeholder(
        &mut self,
        wifi: WifiIcon,
        title: &str,
        subtitle: &str,
        animate: bool,
        ip: Option<Ipv4Addr>,
    ) {
        self.text(4, 4, "TIINY POCKET");
        self.icon(103, 4, UNAVAILABLE, 7);
        self.wifi_icon(wifi);
        self.rect(4, 15, 120, 1, true);
        self.center(30, title);
        if animate {
            // Reserve room for all three dots so the retry text stays in place.
            let x = (128 - (subtitle.len() + 3) * 6) / 2;
            self.text(x, 43, subtitle);
            let dots = (Instant::now().as_millis() / 250 % 4) as usize;
            self.text(x + subtitle.len() * 6, 43, &"..."[..dots]);
        } else {
            self.center(43, subtitle);
        }
        if let Some(ip) = ip {
            let mut address = String::<15>::new();
            let _ = write!(address, "{ip}");
            self.center(54, address.as_str());
        }
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

const LOADING: &[u16] = &[0b00100, 0b00100, 0b10101, 0b01110, 0b00100];
const IDLE_DOT: &[u16] = &[0, 0, 0, 0, 0, 0, 0, 0, 0b10000000000000];
const IDLE_SMALL_Z: &[u16] = &[
    0,
    0,
    0,
    0,
    0b00011110000000,
    0b00000100000000,
    0b00001000000000,
    0b00011110000000,
    0b10000000000000,
];
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
