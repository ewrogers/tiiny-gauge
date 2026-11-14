#![no_std]
#![no_main]

mod display;
#[cfg(target_arch = "arm")]
mod pico;
mod setup;
mod signal_strength;
mod telemetry;
#[cfg(target_arch = "arm")]
mod wifi;

#[cfg(all(
    target_arch = "riscv32",
    feature = "esp32c3",
    not(feature = "oled-spi")
))]
mod esp32c3;
#[cfg(all(
    target_arch = "riscv32",
    any(not(feature = "esp32c3"), feature = "oled-spi")
))]
compile_error!("ESP32-C3 display bring-up requires esp32c3 and uses I2C.");
