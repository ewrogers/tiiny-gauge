use display_interface::AsyncWriteOnlyDataCommand;
use embassy_rp::{
    Peri,
    gpio::{Level, Output},
    peripherals::{DMA_CH2, PIN_16, PIN_17, PIN_18, PIN_19, PIN_20, SPI0},
    spi::{Config, Spi},
};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use ssd1306::prelude::SPIInterface;

pub(crate) fn interface(
    bus: Peri<'static, SPI0>,
    clk: Peri<'static, PIN_18>,
    mosi: Peri<'static, PIN_19>,
    dma: Peri<'static, DMA_CH2>,
    cs: Peri<'static, PIN_17>,
    dc: Peri<'static, PIN_20>,
    reset: Peri<'static, PIN_16>,
) -> (impl AsyncWriteOnlyDataCommand + 'static, Output<'static>) {
    let reset = Output::new(reset, Level::Low);
    let cs = Output::new(cs, Level::High);
    let dc = Output::new(dc, Level::Low);
    let bus = Spi::new_txonly(bus, clk, mosi, dma, crate::wifi::Irqs, Config::default());
    let device = match ExclusiveDevice::new(bus, cs, Delay) {
        Ok(device) => device,
        Err(never) => match never {},
    };

    (SPIInterface::new(device, dc), reset)
}
