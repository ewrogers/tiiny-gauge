#![no_std]
#![no_main]

use core::{cell::Cell, future::pending};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use panic_halt as _;

mod display;
mod setup;
mod telemetry;
mod wifi;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let board = embassy_rp::init(Default::default());
    #[cfg(not(feature = "oled-spi"))]
    let (bus, reset) = (
        display::i2c::interface(board.I2C0, board.PIN_5, board.PIN_4),
        None,
    );
    #[cfg(feature = "oled-spi")]
    let (bus, reset) = {
        let (bus, reset) = display::spi::interface(
            board.SPI0,
            board.PIN_18,
            board.PIN_19,
            board.DMA_CH2,
            board.PIN_17,
            board.PIN_20,
            board.PIN_16,
        );
        (bus, Some(reset))
    };
    let state = Cell::new(telemetry::Status::Setup);
    let mut storage = setup::Storage::new(board.FLASH);
    let settings = storage.load().or_else(setup::baked);
    let network_stack = Cell::new(None);
    let serial = setup::run(
        board.USB,
        board.WATCHDOG,
        storage,
        settings.as_ref(),
        &network_stack,
    );
    let network = async {
        if let Some(settings) = &settings {
            state.set(telemetry::Status::WifiConnecting);
            let (stack, control) = wifi::start(
                spawner,
                board.PIO0,
                board.DMA_CH0,
                board.PIN_23,
                board.PIN_24,
                board.PIN_25,
                board.PIN_29,
            )
            .await;
            network_stack.set(Some(stack));
            join(
                wifi::maintain(
                    stack,
                    control,
                    settings.ssid.as_str(),
                    settings.password.as_str(),
                ),
                telemetry::poll(stack, settings.ip, settings.key.as_str(), &state),
            )
            .await;
        } else {
            pending::<()>().await;
        }
    };
    embassy_futures::join::join3(display::run(bus, reset, &state), network, serial).await;
}
