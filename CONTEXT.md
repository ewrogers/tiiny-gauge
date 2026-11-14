# Project context

Tiiny Gauge is a desk display for Tiiny Pocket. The first target is a Raspberry Pi Pico W (RP2040) with a 3.3 V-compatible 128×64 monochrome SSD1306 OLED. The default build uses I²C at address `0x3C`. The `oled-spi` feature selects write-only SPI0 with reset, data/command, and chip-select GPIOs. Both transports share the display code. See the [README wiring tables](README.md#wire-the-display).

## Firmware

Use a single Cargo binary crate at the repo root. Extract a second crate when there is a second consumer or a concrete need to build part of the code independently. First flash the boot screen to validate the board, power, and I²C. Then verify the read-only telemetry responses on the user's Tiiny firmware over the LAN before writing the Pico client.

The Pico is intended to poll the Tiiny directly over the local network every two seconds. Allow one poll cycle in flight and skip ticks while it is pending. Candidate endpoints are `/api/v1/sys/status` and `/api/v1/models/npu/status`. Community testing used the `p8800.api.tiiny` HTTP host for model status. Confirm firmware response fields, authentication, and reachability before relying on them. See the [Tiiny SDK overview](https://dev.tiiny.ai/) and [AINode Pocket hardware notes](https://github.com/getainode/ainode-pocket#how-a-device-is-actually-reached).

## Display

- Wrap long model names across two header lines. Reserve the right side for status icons and an elapsed timer.
- Animate a descending arrow while a model is pending or loading. Show the first reported loading percentage from `/api/v1/models/running`, preferring pending instances. Animate three hopping dots during inference and a rising dot and two Z shapes when idle. Use an X for unavailable. Use RSS-style arcs for the Pico's Wi-Fi link with a diagonal slash when disconnected.
- Show a large Wi-Fi icon with three outward waves and `CONNECTING TO WIFI...` during the initial connection. Use the Wi-Fi lost screen if the first connection attempt takes longer than 15 seconds or an established link drops.
- Show NPU utilization, SoC CPU utilization labeled `SOC`, SoC RAM, and dNPU RAM as four distinct meters. Use `/api/v1/npu/status` for dNPU RAM when it reports hardware counters. When those counters are missing, show `--/48G` without a fill bar. The general system endpoint can return a fluctuating NPU memory total. When telemetry is unavailable, replace the meters with a centered message. Distinguish Tiiny timeouts from loss of Wi-Fi.
- Start the busy timer when a running model instance reports a nonzero `active_request_count`. Clear the timer when no requests remain. Update the elapsed seconds between telemetry polls. An agent runner would need to supply a start timestamp for the full task duration.
