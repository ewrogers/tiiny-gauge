use display_interface::AsyncWriteOnlyDataCommand;
use embassy_rp::{
    Peripherals, bind_interrupts, dma,
    gpio::{Level, Output},
    peripherals::DMA_CH2,
    spi::{Config, Spi},
};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use ssd1306::prelude::SPIInterface;

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH2>;
});

pub(super) fn interface(board: Peripherals) -> (impl AsyncWriteOnlyDataCommand, Output<'static>) {
    let reset = Output::new(board.PIN_16, Level::Low);
    let cs = Output::new(board.PIN_17, Level::High);
    let dc = Output::new(board.PIN_20, Level::Low);
    let bus = Spi::new_txonly(
        board.SPI0,
        board.PIN_18,
        board.PIN_19,
        board.DMA_CH2,
        Irqs,
        Config::default(),
    );
    let device = match ExclusiveDevice::new(bus, cs, Delay) {
        Ok(device) => device,
        Err(never) => match never {},
    };

    (SPIInterface::new(device, dc), reset)
}
