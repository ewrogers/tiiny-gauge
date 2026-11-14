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

#[cfg(all(target_arch = "riscv32", feature = "esp32c3"))]
mod esp32c3;
#[cfg(all(target_arch = "riscv32", not(feature = "esp32c3")))]
compile_error!("ESP32-C3 firmware requires the esp32c3 feature.");
