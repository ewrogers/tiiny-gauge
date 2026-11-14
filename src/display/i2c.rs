use display_interface::AsyncWriteOnlyDataCommand;
use embassy_rp::{
    Peripherals, bind_interrupts,
    i2c::{Config, I2c, InterruptHandler},
    peripherals::I2C0,
};
use ssd1306::I2CDisplayInterface;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

pub(super) fn interface(board: Peripherals) -> impl AsyncWriteOnlyDataCommand {
    let bus = I2c::new_async(
        board.I2C0,
        board.PIN_5,
        board.PIN_4,
        Irqs,
        Config::default(),
    );
    I2CDisplayInterface::new_custom_address(bus, 0x3c)
}
