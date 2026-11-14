# Project context

Tiiny Gauge is a desk display for Tiiny Pocket. The first target is a Raspberry Pi Pico W (RP2040) with a 3.3 V-compatible 128×64 monochrome SSD1306 OLED. The default build uses I²C at address `0x3C`. The `oled-spi` feature selects write-only SPI0 with reset, data/command, and chip-select GPIOs. Both transports share the display code. See the [README wiring tables](README.md#wire-the-display).

## Firmware

Default UF2 builds omit credentials. USB CDC setup saves validated settings in the final two RP2040 flash sectors, reserved by `pico-memory.x`. Writes alternate sectors and verify the new record before rebooting. Saved settings take precedence over the optional `baked-config` feature.

The USB menu shows the Gauge’s live IPv4 address and keeps edits in memory until Save & Restart on Pico W or Save & Apply on ESP32-C3. ESP32-C3 reconnects without resetting the chip. Discard Changes restores the settings in use when the menu opened. Exit discards edits and closes the menu while the Gauge continues running. Typing `SETUP` reopens the menu on the same serial connection. Show API key presence and mask credential input.

The Blink Indicator defaults to Off for new setups and saved records without an explicit On flag. When enabled, the board pulses its onboard LED after a successful telemetry poll. Changes to the indicator take effect after Save & Restart.

Use a single Cargo binary crate at the repo root. Extract a second crate when there is a second consumer or a concrete need to build part of the code independently. First flash the boot screen to validate the board, power, and I²C. Then verify the read-only telemetry responses on the user's Tiiny firmware over the LAN before writing the Pico client.

The ESP32-C3 build shares the dashboard, setup menu, settings record, and two-second telemetry poller. It uses SH1106 I²C on GPIO4 (SDA) and GPIO5 (SCL). On the DevKitC-02, the USB-to-UART bridge carries the setup menu over UART0. Type `SETUP` to open it because the UART transport has no USB terminal-open event. The ESP32-C3 settings store verifies the dedicated two-sector partition before reading or writing it. The ESP32-C3 SPI build uses GPIO6 (SCK), GPIO7 (MOSI), GPIO4 (reset), GPIO5 (D/C), and GPIO10 (CS), leaving the boot straps and UART0 available.

The Pico is intended to poll the Tiiny directly over the local network every two seconds. Allow one poll cycle in flight and skip ticks while it is pending. Candidate endpoints are `/api/v1/sys/status` and `/api/v1/models/npu/status`. Community testing used the `p8800.api.tiiny` HTTP host for model status. Confirm firmware response fields, authentication, and reachability before relying on them. See the [Tiiny SDK overview](https://dev.tiiny.ai/) and [AINode Pocket hardware notes](https://github.com/getainode/ainode-pocket#how-a-device-is-actually-reached).

## Display

- Wrap long model names across two header lines. Reserve the right side for status icons and an elapsed timer.
- Animate a descending arrow while a model is pending or loading. Show the first reported loading percentage from `/api/v1/models/running`, preferring pending instances. Animate three hopping dots during inference and a rising dot and two Z shapes when idle. Use an X for unavailable. Use ascending bars for the Gauge's Wi-Fi signal with a diagonal slash when disconnected.
- Sample the Gauge’s Wi-Fi RSSI every two seconds. Fill one bar for Poor (below −75 dBm), two for Good (−75 to −61 dBm), and three for Excellent (−60 dBm and up). Keep the remaining bars hollow and show the same label and exact reading in serial setup.
- Show a large Wi-Fi icon with three outward waves and `CONNECTING TO WIFI...` during the initial connection. Show `WI-FI LOST` with animated `RECONNECTING` dots if the first connection attempt takes longer than 15 seconds or an established link drops.
- Show NPU utilization, SoC CPU utilization labeled `SOC`, SoC RAM, and dNPU RAM as four distinct meters. Use `/api/v1/npu/status` for dNPU RAM when it reports hardware counters. When those counters are missing, show `--/48G` without a fill bar. The general system endpoint can return a fluctuating NPU memory total. When the Tiiny is unreachable over Wi-Fi, replace the meters with `TIINY UNAVAILABLE` and animate the dots after `RETRYING`.
- Start the busy timer when a running model instance reports a nonzero `active_request_count`. Clear the timer when no requests remain. Update the elapsed seconds between telemetry polls. An agent runner would need to supply a start timestamp for the full task duration.
