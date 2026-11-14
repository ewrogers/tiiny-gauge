use core::{cell::Cell, fmt::Write};

use embassy_net::Stack;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use embedded_storage::nor_flash::NorFlash;
use esp_hal::{Async, uart::Uart};
use esp_radio::wifi::WifiError;
use serde_json_core::heapless::String;

use super::{
    Settings, Storage,
    serial::{PACKET, Serial, session},
};

#[derive(Clone, Copy)]
pub(crate) enum WifiStatus {
    Idle,
    Joining,
    Associated,
    InitFailed(WifiError),
    ConfigFailed(WifiError),
    JoinFailed(WifiError),
}

struct UartMenu<'a> {
    uart: Uart<'static, Async>,
    status: &'a Cell<WifiStatus>,
}

impl Serial for UartMenu<'_> {
    const SAVE_LABEL: &'static str = "Save & Apply";
    const SAVE_MESSAGE: &'static str = "Settings saved. Applying...\r\n";

    fn write_status(&self, menu: &mut String<512>, has_ip: bool) {
        let _ = match self.status.get() {
            WifiStatus::Idle => return,
            WifiStatus::Joining => menu.push_str("\r\nWi-Fi: joining"),
            WifiStatus::Associated if has_ip => menu.push_str("\r\nWi-Fi: connected"),
            WifiStatus::Associated => menu.push_str("\r\nWi-Fi: waiting for DHCP"),
            WifiStatus::InitFailed(error) => return write_error(menu, "init failed", error),
            WifiStatus::ConfigFailed(error) => {
                return write_error(menu, "configuration failed", error);
            }
            WifiStatus::JoinFailed(error) => return write_error(menu, "join failed", error),
        };
    }

    async fn read(&mut self, data: &mut [u8; PACKET]) -> Result<usize, ()> {
        self.uart.read_async(data).await.map_err(|_| ())
    }

    async fn write(&mut self, mut data: &[u8]) -> Result<(), ()> {
        while !data.is_empty() {
            let sent = self.uart.write_async(data).await.map_err(|_| ())?;
            data = &data[sent..];
        }
        Ok(())
    }
}

fn write_error(menu: &mut String<512>, stage: &str, error: WifiError) {
    match error {
        WifiError::Disconnected(info) => {
            let _ = write!(menu, "\r\nWi-Fi: {stage} ({:?})", info.reason);
        }
        other => {
            let _ = write!(menu, "\r\nWi-Fi: {stage} ({other:?})");
        }
    }
}

pub(crate) async fn run(
    uart: Uart<'static, Async>,
    mut storage: Storage<impl NorFlash>,
    mut current: Option<Settings>,
    network_stack: &Cell<Option<Stack<'static>>>,
    rssi: &Cell<Option<i32>>,
    updates: &Signal<NoopRawMutex, Settings>,
    status: &Cell<WifiStatus>,
) -> ! {
    let mut serial = UartMenu { uart, status };
    let _ = serial
        .write(b"Tiiny Gauge: type SETUP and press Enter.\r\n")
        .await;
    loop {
        if let Ok(settings) = session(
            &mut serial,
            &mut storage,
            current.as_ref(),
            network_stack,
            rssi,
            true,
        )
        .await
        {
            current = Some(settings.clone());
            updates.signal(settings);
            let _ = serial
                .write(b"Type SETUP and press Enter to reopen.\r\n")
                .await;
        }
    }
}
