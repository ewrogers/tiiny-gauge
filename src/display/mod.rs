use core::cell::Cell;
use display_interface::{AsyncWriteOnlyDataCommand, DataFormat, DisplayError};
#[cfg(any(target_arch = "arm", feature = "oled-spi"))]
use embassy_time::Timer;
use embassy_time::{Duration, Ticker};

use crate::telemetry::Status;

mod font;
#[cfg(all(target_arch = "arm", not(feature = "oled-spi")))]
pub(crate) mod i2c;
#[cfg(all(target_arch = "arm", feature = "oled-spi"))]
pub(crate) mod spi;
mod ui;

#[cfg(any(target_arch = "arm", feature = "oled-spi"))]
pub(crate) async fn run(
    mut bus: impl AsyncWriteOnlyDataCommand,
    mut reset: impl AsyncFnMut(),
    state: &Cell<Status>,
    rssi: &Cell<Option<i32>>,
) -> ! {
    loop {
        reset().await;
        let _ = draw_dashboard(&mut bus, state, rssi).await;
        Timer::after_secs(1).await;
    }
}

pub(crate) async fn draw_dashboard(
    bus: &mut impl AsyncWriteOnlyDataCommand,
    state: &Cell<Status>,
    rssi: &Cell<Option<i32>>,
) -> Result<(), DisplayError> {
    // Both controllers accept page writes. Their charge pump and column offset differ.
    #[cfg(feature = "oled-sh1106")]
    let power = [0xad, 0x8b];
    #[cfg(not(feature = "oled-sh1106"))]
    let power = [0x8d, 0x14, 0x20, 0x02];
    const OFFSET: u8 = if cfg!(feature = "oled-sh1106") { 2 } else { 0 };

    bus.send_commands(DataFormat::U8(&[0xae, 0xd5, 0x80, 0xa8, 0x3f]))
        .await?;
    bus.send_commands(DataFormat::U8(&[0xd3, 0x00, 0x40]))
        .await?;
    bus.send_commands(DataFormat::U8(&power)).await?;
    bus.send_commands(DataFormat::U8(&[0xa1, 0xc8, 0xda, 0x12, 0x81, 0x7f]))
        .await?;
    bus.send_commands(DataFormat::U8(&[0xa4, 0xa6])).await?;

    let mut last: Option<ui::Frame> = None;
    let mut ticker = Ticker::every(Duration::from_millis(250));
    loop {
        let frame = ui::Frame::render(state.get(), rssi.get());
        for (index, page) in frame.pages.iter().enumerate() {
            if last.as_ref().is_some_and(|old| old.pages[index] == *page) {
                continue;
            }
            bus.send_commands(DataFormat::U8(&[0xb0 | index as u8, OFFSET, 0x10]))
                .await?;
            bus.send_data(DataFormat::U8(page)).await?;
        }
        if last.is_none() {
            bus.send_commands(DataFormat::U8(&[0xaf])).await?;
        }
        last = Some(frame);
        ticker.next().await;
    }
}
