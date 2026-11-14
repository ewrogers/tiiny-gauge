# Tiiny Gauge

A 128×64 OLED dashboard for Tiiny Pocket, built with Rust and Embassy on Pico W. The planned dashboard shows the loaded model, NPU, CPU and RAM usage, and elapsed busy time over Wi-Fi.

The first firmware displays **Tiiny Gauge** and **OLED ready** so you can verify the screen and wiring.

## Wire the display

Use a 3.3 V-compatible SSD1306 I²C OLED at address `0x3C`. Connect by the labels on your display.

![Pico WH breadboard diagram with physical pin numbers, pin names, and four I²C OLED jumpers](docs/breadboard.png)

| OLED | Pico W |
| --- | --- |
| SDA | GP4 (pin 6) |
| SCL | GP5 (pin 7) |
| GND | GND (pin 8) |
| VCC | 3V3(OUT) (pin 36) |

## Build and flash

With Rust installed through rustup, run these commands from the repository root.

```sh
cargo build --release --locked
cargo install elf2uf2-rs --locked
elf2uf2-rs target/thumbv6m-none-eabi/release/tiiny-gauge target/tiiny-gauge.uf2
```

Hold **BOOTSEL** while plugging the Pico W into USB, then copy `target/tiiny-gauge.uf2` to the **RPI-RP2** drive. The board reboots and shows the boot screen.
