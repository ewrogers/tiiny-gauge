#![no_std]
#![no_main]

use core::{cell::Cell, future::pending, net::Ipv4Addr};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use panic_halt as _;

mod display;
mod telemetry;
mod wifi;

mod config {
    include!(concat!(env!("OUT_DIR"), "/local_config.rs"));
}

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
    let network = async {
        if let Ok(ip) = config::TIINY_IP.parse::<Ipv4Addr>()
            && !config::WIFI_SSID.is_empty()
            && !config::TIINY_API_KEY.is_empty()
        {
            state.set(telemetry::Status::WifiLost);
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
            join(
                wifi::maintain(stack, control, config::WIFI_SSID, config::WIFI_PASSWORD),
                telemetry::poll(stack, ip, config::TIINY_API_KEY, &state),
            )
            .await;
        } else {
            pending::<()>().await;
        }
    };
    join(display::run(bus, reset, &state), network).await;
}
