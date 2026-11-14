use core::future::pending;
use embassy_rp::Peripherals;
use embassy_time::Timer;
use ssd1306::{
    Ssd1306Async, mode::DisplayConfigAsync, rotation::DisplayRotation, size::DisplaySize128x64,
};

#[cfg(not(feature = "oled-spi"))]
mod i2c;
#[cfg(feature = "oled-spi")]
mod spi;

pub(super) async fn run(board: Peripherals) {
    #[cfg(not(feature = "oled-spi"))]
    let interface = i2c::interface(board);
    #[cfg(feature = "oled-spi")]
    let (interface, mut reset) = spi::interface(board);

    let mut display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_terminal_mode();

    while async {
        #[cfg(feature = "oled-spi")]
        {
            reset.set_low();
            Timer::after_millis(10).await;
            reset.set_high();
            Timer::after_millis(1).await;
        }
        display.init().await?;
        display.clear().await?;
        display.write_str("Tiiny Gauge\nOLED ready").await
    }
    .await
    .is_err()
    {
        Timer::after_secs(1).await;
    }

    pending::<()>().await;
}
