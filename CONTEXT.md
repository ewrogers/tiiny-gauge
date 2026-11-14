# Project context

Tiiny Gauge is a desk display for Tiiny Pocket. The first target is a Raspberry Pi Pico W (RP2040) and a 128×64 monochrome I²C OLED. The boot screen assumes an SSD1306-compatible module at address `0x3C` and 3.3 V operation.

## Firmware

Use a single Cargo binary crate at the repo root. Extract a second crate when there is a second consumer or a concrete need to build part of the code independently. First flash the boot screen to validate the board, power, and I²C. Then verify the read-only telemetry responses on the user's Tiiny firmware over the LAN before writing the Pico client.

The Pico is intended to poll the Tiiny directly over the local network every two seconds. Allow one poll cycle in flight and skip ticks while it is pending. Candidate endpoints are `/api/v1/sys/status` and `/api/v1/models/npu/status`. Community testing used the `p8800.api.tiiny` HTTP host for model status. Confirm firmware response fields, authentication, and reachability before relying on them. See the [Tiiny SDK overview](https://dev.tiiny.ai/) and [AINode Pocket hardware notes](https://github.com/getainode/ainode-pocket#how-a-device-is-actually-reached).

## Display

- Wrap long model names across two header lines. Reserve the right side for status icons and an elapsed timer.
- Use an hourglass for busy, a small and large Z for idle, and an X for unavailable. Use RSS-style arcs for the Pico's Wi-Fi link with a diagonal slash when disconnected.
- Show NPU utilization, CPU utilization, SoC RAM, and dNPU RAM as four distinct meters. When telemetry is unavailable, replace the meters with a centered message. Distinguish Tiiny timeouts from loss of Wi-Fi.
- Count observed busy time from the first busy poll and update the timer once a second between polls. A full agent-task timer requires a start timestamp from the agent runner and may include gaps between inference calls.
