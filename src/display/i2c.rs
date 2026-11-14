use display_interface::AsyncWriteOnlyDataCommand;
use embassy_rp::{
    Peri, bind_interrupts,
    i2c::{Config, I2c, InterruptHandler},
    peripherals::{I2C0, PIN_4, PIN_5},
};
use ssd1306::I2CDisplayInterface;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

pub(crate) fn interface(
    bus: Peri<'static, I2C0>,
    scl: Peri<'static, PIN_5>,
    sda: Peri<'static, PIN_4>,
) -> impl AsyncWriteOnlyDataCommand + 'static {
    let bus = I2c::new_async(bus, scl, sda, Irqs, Config::default());
    I2CDisplayInterface::new_custom_address(bus, 0x3c)
}
