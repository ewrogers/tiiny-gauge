use core::{cell::Cell, future::pending};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_sync::signal::Signal;
use panic_halt as _;

use crate::{display, setup, telemetry, wifi};

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let board = embassy_rp::init(Default::default());
    #[cfg(not(feature = "oled-spi"))]
    let (bus, reset_display) = (
        display::i2c::interface(board.I2C0, board.PIN_5, board.PIN_4),
        async || {},
    );
    #[cfg(feature = "oled-spi")]
    let (bus, reset_display) = {
        let (bus, mut reset) = display::spi::interface(
            board.SPI0,
            board.PIN_18,
            board.PIN_19,
            board.DMA_CH2,
            board.PIN_17,
            board.PIN_20,
            board.PIN_16,
        );
        (bus, async move || {
            reset.set_low();
            embassy_time::Timer::after_millis(10).await;
            reset.set_high();
            embassy_time::Timer::after_millis(1).await;
        })
    };
    let state = Cell::new(telemetry::Status::Setup);
    const FLASH_SIZE: usize = 2 * 1024 * 1024;
    const SETTINGS_OFFSET: u32 = (FLASH_SIZE - 2 * embassy_rp::flash::ERASE_SIZE) as u32;
    let flash = embassy_rp::flash::Flash::<_, _, FLASH_SIZE>::new_blocking(board.FLASH);
    let mut storage = setup::Storage::new(flash, Some(SETTINGS_OFFSET));
    let settings = storage.load().or_else(setup::baked);
    let network_stack = Cell::new(None);
    let rssi = Cell::new(None);
    let serial = setup::run(
        board.USB,
        board.WATCHDOG,
        storage,
        settings.as_ref(),
        &network_stack,
        &rssi,
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
            let responses = Signal::new();
            join(
                wifi::maintain(
                    stack,
                    control,
                    settings.ssid.as_str(),
                    settings.password.as_str(),
                    &responses,
                    &rssi,
                ),
                telemetry::poll(
                    stack,
                    settings.ip,
                    settings.key.as_str(),
                    &state,
                    settings.blink_indicator.then_some(&responses),
                ),
            )
            .await;
        } else {
            pending::<()>().await;
        }
    };
    embassy_futures::join::join3(
        display::run(bus, reset_display, &state, &rssi),
        network,
        serial,
    )
    .await;
}
