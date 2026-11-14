use core::{cell::Cell, fmt::Write, net::Ipv4Addr};

use embassy_net::Stack;
use embassy_time::{Duration, with_timeout};
use embedded_storage::nor_flash::NorFlash;
use serde_json_core::heapless::{String, Vec};
use unicode_width::UnicodeWidthStr;

use crate::signal_strength::SignalStrength;

use super::{
    InvalidField, KEY_MAX, Settings, Storage, parse_key, parse_password, parse_ssid, validate_ip,
};

pub(super) const PACKET: usize = 64;

pub(super) trait Serial {
    const SAVE_LABEL: &'static str;
    const SAVE_MESSAGE: &'static str;
    fn write_status(&self, _menu: &mut String<512>, _has_ip: bool) {}
    async fn read(&mut self, data: &mut [u8; PACKET]) -> Result<usize, ()>;
    async fn write(&mut self, data: &[u8]) -> Result<(), ()>;
}

pub(super) async fn session<S: Serial>(
    serial: &mut S,
    storage: &mut Storage<impl NorFlash>,
    current: Option<&Settings>,
    network_stack: &Cell<Option<Stack<'static>>>,
    rssi: &Cell<Option<i32>>,
    wait_for_setup: bool,
) -> Result<Settings, ()> {
    let mut input = Input::new();
    if wait_for_setup {
        input.wait_for_setup(serial).await?;
    }
    let mut draft = current.cloned().unwrap_or_else(|| Settings {
        ssid: String::new(),
        password: String::new(),
        ip: Ipv4Addr::UNSPECIFIED,
        key: String::new(),
        blink_indicator: false,
    });
    let original = draft.clone();
    loop {
        let gauge_ip = network_stack
            .get()
            .filter(Stack::is_link_up)
            .and_then(|stack| stack.config_v4())
            .map(|config| config.address.address());
        show_menu(serial, &original, &draft, gauge_ip, rssi.get()).await?;
        let command = input.prompt(serial, "Choice: ", Echo::Plain).await?;
        let result = match command.trim() {
            "1" => {
                let ssid = input.prompt(serial, "Wi-Fi SSID: ", Echo::Plain).await?;
                let password = input
                    .prompt(
                        serial,
                        "Wi-Fi password (blank for open network): ",
                        Echo::Masked,
                    )
                    .await?;
                parse_ssid(&ssid).and_then(|ssid| {
                    let password = parse_password(&password)?;
                    draft.ssid = ssid;
                    draft.password = password;
                    Ok(())
                })
            }
            "2" => {
                let ip = input
                    .prompt(serial, "Tiiny LAN IPv4 address: ", Echo::Plain)
                    .await?;
                ip.trim()
                    .parse::<Ipv4Addr>()
                    .map_err(|_| InvalidField::Address)
                    .and_then(validate_ip)
                    .map(|ip| draft.ip = ip)
            }
            "3" => {
                let key = input
                    .prompt(serial, "TiinyOS API key: ", Echo::Masked)
                    .await?;
                parse_key(&key).map(|key| draft.key = key)
            }
            "4" => {
                draft.blink_indicator = !draft.blink_indicator;
                Ok(())
            }
            "8" => match Settings::new(
                &draft.ssid,
                &draft.password,
                draft.ip,
                &draft.key,
                draft.blink_indicator,
            ) {
                Err(field) => Err(field),
                Ok(settings) => {
                    if storage.save(&settings).is_err() {
                        send(serial, "Could not save settings.\r\n").await?;
                        continue;
                    }
                    let _ = with_timeout(Duration::from_millis(200), send(serial, S::SAVE_MESSAGE))
                        .await;
                    return Ok(settings);
                }
            },
            "9" => {
                draft.clone_from(&original);
                send(serial, "Changes discarded.\r\n").await?;
                Ok(())
            }
            "0" => {
                draft.clone_from(&original);
                send(
                    serial,
                    "Setup closed. Close your serial terminal or type SETUP to reopen.\r\n",
                )
                .await?;
                input.wait_for_setup(serial).await?;
                Ok(())
            }
            command if command.is_empty() || command.eq_ignore_ascii_case("SETUP") => Ok(()),
            _ => {
                send(serial, "Choose 1, 2, 3, 4, 8, 9, or 0.\r\n").await?;
                Ok(())
            }
        };
        if let Err(field) = result {
            let message = match field {
                InvalidField::Ssid => "Use option 1 to enter a Wi-Fi SSID of 1 to 32 bytes.\r\n",
                InvalidField::Password => {
                    "Use an 8 to 63 character Wi-Fi passphrase or leave it blank.\r\n"
                }
                InvalidField::Address => "Use option 2 to enter the Tiiny's LAN IPv4 address.\r\n",
                InvalidField::Key => {
                    "Use option 3 to enter an API key of up to 504 ASCII characters with no spaces.\r\n"
                }
            };
            send(serial, message).await?;
        }
    }
}

async fn show_menu<S: Serial>(
    serial: &mut S,
    original: &Settings,
    draft: &Settings,
    gauge_ip: Option<Ipv4Addr>,
    rssi: Option<i32>,
) -> Result<(), ()> {
    let mut menu = String::<512>::new();
    let current_ssid = if original.ssid.is_empty() {
        "Unset"
    } else {
        original.ssid.as_str()
    };
    let draft_ssid = if draft.ssid.is_empty() {
        "Unset"
    } else {
        draft.ssid.as_str()
    };
    let key = if draft.key.is_empty() { "Unset" } else { "Set" };
    let blink = if draft.blink_indicator { "On" } else { "Off" };
    let mark = |changed| if changed { " *" } else { "" };
    let wifi_changed = draft.ssid != original.ssid || draft.password != original.password;
    let _ = write!(
        menu,
        "\r\n=== Tiiny Gauge Setup ===\r\nWi-Fi Network: {current_ssid}\r\nGauge IP: "
    );
    if let Some(ip) = gauge_ip {
        let _ = write!(menu, "{ip}");
    } else {
        let _ = menu.push_str("Not connected");
    }
    serial.write_status(&mut menu, gauge_ip.is_some());
    if let Some(rssi) = rssi {
        let quality = SignalStrength::from_rssi(rssi).label();
        let _ = write!(menu, "\r\nWi-Fi Signal: {quality} ({rssi} dBm)");
    } else {
        let _ = menu.push_str("\r\nWi-Fi Signal: Unavailable");
    }
    if wifi_changed {
        let _ = write!(menu, "\r\nPending Wi-Fi Network: {draft_ssid} *");
    }
    let _ = menu.push_str("\r\nTiiny IP: ");
    if draft.ip.is_unspecified() {
        let _ = menu.push_str("Unset");
    } else {
        let _ = write!(menu, "{}", draft.ip);
    }
    let _ = write!(
        menu,
        "{}\r\nAPI KEY: {key}{}\r\nBlink Indicator: {blink}{}\r\n",
        mark(draft.ip != original.ip),
        mark(draft.key != original.key),
        mark(draft.blink_indicator != original.blink_indicator),
    );
    let _ = menu.push_str(concat!(
        "\r\n[1] Set WiFi Network\r\n",
        "[2] Set Tiiny IP\r\n",
        "[3] Set Tiiny API Key\r\n",
        "[4] Toggle Blink Indicator\r\n",
    ));
    let _ = write!(menu, "\r\n[8] {}\r\n", S::SAVE_LABEL);
    let _ = menu.push_str(concat!("[9] Discard Changes\r\n", "[0] Exit\r\n",));
    send(serial, menu.as_str()).await
}

async fn send(serial: &mut impl Serial, message: &str) -> Result<(), ()> {
    for chunk in message.as_bytes().chunks(PACKET - 1) {
        serial.write(chunk).await?;
    }
    Ok(())
}

enum Echo {
    Plain,
    Masked,
}

struct Input {
    packet: [u8; PACKET],
    position: usize,
    length: usize,
    skip_lf: bool,
}

impl Input {
    fn new() -> Self {
        Self {
            packet: [0; PACKET],
            position: 0,
            length: 0,
            skip_lf: false,
        }
    }

    async fn wait_for_setup(&mut self, serial: &mut impl Serial) -> Result<(), ()> {
        while !self
            .prompt(serial, "", Echo::Plain)
            .await?
            .trim()
            .eq_ignore_ascii_case("SETUP")
        {}
        Ok(())
    }

    async fn prompt(
        &mut self,
        serial: &mut impl Serial,
        prompt: &str,
        echo: Echo,
    ) -> Result<String<KEY_MAX>, ()> {
        loop {
            send(serial, prompt).await?;
            let mut line = Vec::<u8, KEY_MAX>::new();
            let mut overflow = 0usize;
            loop {
                if self.position == self.length {
                    self.length = serial.read(&mut self.packet).await?;
                    self.position = 0;
                    if self.length == 0 {
                        continue;
                    }
                }
                let byte = self.packet[self.position];
                self.position += 1;
                if core::mem::take(&mut self.skip_lf) && byte == b'\n' {
                    continue;
                }
                match byte {
                    b'\r' | b'\n' => {
                        self.skip_lf = byte == b'\r';
                        send(serial, "\r\n").await?;
                        break;
                    }
                    8 | 127 if overflow > 0 => overflow -= 1,
                    8 | 127 => {
                        if !line.is_empty() {
                            let start = last_character_start(&line);
                            if let Some(character) = visible_character(&line[start..]) {
                                let width = match echo {
                                    Echo::Plain => character.width(),
                                    Echo::Masked => 1,
                                };
                                for _ in 0..width {
                                    send(serial, "\x08 \x08").await?;
                                }
                            }
                            line.truncate(start);
                        }
                    }
                    32..=126 | 128..=255 => {
                        if overflow != 0 || line.push(byte).is_err() {
                            overflow = overflow.saturating_add(1);
                        } else if let Some(character) =
                            visible_character(&line[last_character_start(&line)..])
                        {
                            let displayed = match echo {
                                Echo::Plain => character,
                                Echo::Masked => "*",
                            };
                            send(serial, displayed).await?;
                        }
                    }
                    _ => {}
                }
            }
            if overflow == 0
                && let Ok(line) = String::from_utf8(line)
            {
                return Ok(line);
            }
            send(serial, "Use valid text of up to 504 bytes.\r\n").await?;
        }
    }
}

fn last_character_start(line: &[u8]) -> usize {
    line.iter()
        .rposition(|byte| byte & 0xc0 != 0x80)
        .unwrap_or(0)
}

fn visible_character(bytes: &[u8]) -> Option<&str> {
    let text = core::str::from_utf8(bytes).ok()?;
    text.chars().next().filter(|ch| !ch.is_control())?;
    Some(text)
}
