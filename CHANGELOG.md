# Changelog

## [1.1.0](https://github.com/ewrogers/tiiny-gauge/releases/tag/v1.1.0)

- Added ESP32-C3-DevKitC-02 support for SH1106 I²C displays, with USB-to-UART setup, settings that survive power cycles, and the same live dashboard as Pico W. **Save & Apply** reconnects Wi-Fi without rebooting.
- Added Wi-Fi signal bars on both boards and a Poor, Good, or Excellent rating with the exact dBm reading in setup. Signal strength updates every two seconds.
- Moved the Blink Indicator to `[4]` on both boards. The ESP32-C3’s onboard RGB LED pulses lime green at 5% peak brightness when the indicator is enabled and a Tiiny poll succeeds.

## [1.0.1](https://github.com/ewrogers/tiiny-gauge/releases/tag/v1.0.1)

- Added an optional Pico W LED blink after each successful Tiiny poll. It defaults to Off. Use `[5] Toggle Blink Indicator` and `[8] Save & Restart` to enable it.
- Grouped the USB setup actions below the settings. Use `[8] Save & Restart` to apply edits, `[9] Discard Changes` to reset the form, or `[0] Exit` to close setup and keep the saved settings.

## [1.0.0](https://github.com/ewrogers/tiiny-gauge/releases/tag/v1.0.0)

- Shipped credential-free Pico W UF2 downloads for SSD1306 and SH1106 OLEDs over I²C and SPI.
- Added USB serial setup for Wi-Fi and Tiiny settings, with masked credential input and settings that survive a power cycle.
- Displayed live model and resource usage, inference time, loading progress, and connection status.
