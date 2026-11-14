#![no_std]
#![no_main]

use core::future::pending;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    i2c::{Config, I2c, InterruptHandler},
    peripherals::I2C0,
};
use embassy_time::Timer;
use panic_halt as _;
use ssd1306::{
    I2CDisplayInterface, Ssd1306Async, mode::DisplayConfigAsync, rotation::DisplayRotation,
    size::DisplaySize128x64,
};

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let board = embassy_rp::init(Default::default());
    let bus = I2c::new_async(
        board.I2C0,
        board.PIN_5,
        board.PIN_4,
        Irqs,
        Config::default(),
    );
    let interface = I2CDisplayInterface::new_custom_address(bus, 0x3c);
    let mut display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_terminal_mode();

    while async {
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
