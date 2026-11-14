use core::{cell::Cell, panic::PanicInfo};
use display_interface::{AsyncWriteOnlyDataCommand, DataFormat, DisplayError};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_net::{Runner, Stack, StackResources};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, signal::Signal};
use embassy_time::{Ticker, Timer, with_timeout};
use esp_bootloader_esp_idf::partitions::{PARTITION_TABLE_MAX_LEN, read_partition_table};
use esp_hal::{
    Async, Blocking,
    gpio::Level,
    i2c::master::{Config, I2c, SoftwareTimeout},
    interrupt::software::SoftwareInterruptControl,
    rmt::{Channel, PulseCode, Rmt, Tx, TxChannelConfig, TxChannelCreator},
    rng::Rng,
    time::{Duration, Rate},
    timer::timg::TimerGroup,
    uart::{Config as UartConfig, Uart},
};

use esp_radio::wifi::{self, AuthenticationMethod, Interface, WifiController, sta::StationConfig};
use esp_storage::FlashStorage;
use static_cell::StaticCell;

use crate::{display, setup, telemetry};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let board = esp_hal::init(esp_hal::Config::default());
    let serial = Uart::new(board.UART0, UartConfig::default())
        .expect("default UART clock")
        .with_tx(board.GPIO21)
        .with_rx(board.GPIO20)
        .into_async();
    esp_alloc::heap_allocator!(size: 72 * 1024);
    let timers = TimerGroup::new(board.TIMG0);
    let interrupts = SoftwareInterruptControl::new(board.SW_INTERRUPT);
    esp_rtos::start(timers.timer0, interrupts.software_interrupt0);

    let i2c = I2c::new(
        board.I2C0,
        Config::default()
            .with_frequency(Rate::from_khz(50))
            .with_software_timeout(SoftwareTimeout::Transaction(Duration::from_millis(20))),
    )
    .expect("50 kHz I2C clock")
    .with_sda(board.GPIO4)
    .with_scl(board.GPIO5);
    let mut flash = FlashStorage::new(board.FLASH);
    let mut table_bytes = [0; PARTITION_TABLE_MAX_LEN];
    let first_slot = read_partition_table(&mut flash, &mut table_bytes)
        .ok()
        .and_then(|table| {
            table.iter().find_map(|partition| {
                (partition.label().starts_with(b"tiiny_settings\0")
                    && partition.raw_type() == 1
                    && partition.raw_subtype() == 0x06
                    && partition.offset() == 0x3fe000
                    && partition.len() == 0x2000)
                    .then_some(partition.offset())
            })
        });
    let mut storage = setup::Storage::new(flash, first_slot);
    let settings = storage.load().or_else(setup::baked);
    let state = Cell::new(telemetry::Status::Setup);
    let network_stack = Cell::new(None);
    let network = NetworkState {
        updates: Signal::new(),
        wifi_status: Cell::new(setup::WifiStatus::Idle),
        responses: Signal::new(),
        rssi: Cell::new(None),
    };
    let indicator = async {
        if let Ok(rmt) = Rmt::new(board.RMT, Rate::from_mhz(80)) {
            let config = TxChannelConfig::default()
                .with_clk_divider(1)
                .with_idle_output(true)
                .with_idle_output_level(Level::Low);
            if let Ok(channel) = rmt.into_async().channel0.configure_tx(&config) {
                blink_indicator(channel.with_pin(board.GPIO8), &network.responses).await;
            }
        }
        core::future::pending::<()>().await;
    };
    embassy_futures::join::join3(
        embassy_futures::join::join(render(i2c, &state, &network.rssi), indicator),
        setup::run(
            serial,
            storage,
            settings.clone(),
            &network_stack,
            &network.rssi,
            &network.updates,
            &network.wifi_status,
        ),
        run_network(
            spawner,
            board.WIFI,
            settings,
            &state,
            &network_stack,
            &network,
        ),
    )
    .await;
    core::future::pending().await
}

struct NetworkState {
    updates: Signal<NoopRawMutex, setup::Settings>,
    wifi_status: Cell<setup::WifiStatus>,
    responses: Signal<NoopRawMutex, ()>,
    rssi: Cell<Option<i32>>,
}

async fn run_network(
    spawner: Spawner,
    radio: esp_hal::peripherals::WIFI<'static>,
    settings: Option<setup::Settings>,
    state: &Cell<telemetry::Status>,
    network_stack: &Cell<Option<Stack<'static>>>,
    network: &NetworkState,
) {
    let mut settings = match settings {
        Some(settings) => settings,
        None => network.updates.wait().await,
    };
    state.set(telemetry::Status::WifiConnecting);
    let initial_config = station_config(&settings);
    let (mut controller, interfaces) = match wifi::new(
        radio,
        wifi::ControllerConfig::default().with_initial_config(initial_config),
    ) {
        Ok(pair) => pair,
        Err(error) => {
            network
                .wifi_status
                .set(setup::WifiStatus::InitFailed(error));
            state.set(telemetry::Status::WifiLost(settings.ip));
            return;
        }
    };
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let rng = Rng::new();
    let seed = (u64::from(rng.random()) << 32) | u64::from(rng.random());
    let (stack, runner) = embassy_net::new(
        interfaces.station,
        embassy_net::Config::dhcpv4(Default::default()),
        RESOURCES.init(StackResources::new()),
        seed,
    );
    network_stack.set(Some(stack));
    spawner.spawn(network_task(runner).expect("one network task"));
    loop {
        state.set(telemetry::Status::WifiConnecting);
        if let Err(error) = controller.set_config(&station_config(&settings)) {
            network
                .wifi_status
                .set(setup::WifiStatus::ConfigFailed(error));
            state.set(telemetry::Status::WifiLost(settings.ip));
            settings = network.updates.wait().await;
            continue;
        }
        let active = embassy_futures::join::join(
            maintain(&mut controller, network),
            telemetry::poll(
                stack,
                settings.ip,
                &settings.key,
                state,
                settings.blink_indicator.then_some(&network.responses),
            ),
        );
        settings = match select(network.updates.wait(), active).await {
            Either::First(settings) => settings,
            Either::Second((never, _)) => match never {},
        };
        network.responses.reset();
        network.rssi.set(None);
        let _ = with_timeout(
            embassy_time::Duration::from_secs(3),
            controller.disconnect_async(),
        )
        .await;
    }
}

#[embassy_executor::task]
async fn network_task(mut runner: Runner<'static, Interface<'static>>) -> ! {
    runner.run().await
}

fn station_config(settings: &setup::Settings) -> wifi::Config {
    let auth = if settings.password.is_empty() {
        AuthenticationMethod::None
    } else {
        AuthenticationMethod::Wpa2Personal
    };
    wifi::Config::Station(
        StationConfig::default()
            .with_ssid(settings.ssid.as_str())
            .with_auth_method(auth)
            .with_password(settings.password.as_str().into()),
    )
}

async fn maintain(controller: &mut WifiController<'_>, network: &NetworkState) -> ! {
    loop {
        network.wifi_status.set(setup::WifiStatus::Joining);
        match controller.connect_async().await {
            Ok(_) => {
                network.wifi_status.set(setup::WifiStatus::Associated);
                network.rssi.set(controller.rssi().ok());
                let mut ticks = Ticker::every(embassy_time::Duration::from_secs(2));
                while let Either::Second(()) =
                    select(controller.wait_for_disconnect_async(), ticks.next()).await
                {
                    network.rssi.set(controller.rssi().ok());
                }
                network.rssi.set(None);
                network.wifi_status.set(setup::WifiStatus::Joining);
            }
            Err(error) => network
                .wifi_status
                .set(setup::WifiStatus::JoinFailed(error)),
        }
        Timer::after_secs(3).await;
    }
}

async fn blink_indicator(
    mut channel: Channel<'_, Async, Tx>,
    responses: &Signal<NoopRawMutex, ()>,
) -> ! {
    let off = led_frame([0, 0, 0]);
    let lime = led_frame([13, 5, 0]);
    let _ = channel.transmit(&off).await;
    loop {
        responses.wait().await;
        let _ = channel.transmit(&lime).await;
        Timer::after_millis(120).await;
        let _ = channel.transmit(&off).await;
    }
}

fn led_frame(grb: [u8; 3]) -> [PulseCode; 26] {
    // The addressable LED uses GRB order, MSB first, and an 800 kHz bit stream.
    const ZERO: PulseCode = PulseCode::new(Level::High, 32, Level::Low, 68);
    const ONE: PulseCode = PulseCode::new(Level::High, 64, Level::Low, 36);
    let mut frame = [ZERO; 26];
    for (value, pulses) in grb.into_iter().zip(frame[..24].chunks_exact_mut(8)) {
        for (bit, pulse) in pulses.iter_mut().enumerate() {
            if value & (0x80 >> bit) != 0 {
                *pulse = ONE;
            }
        }
    }
    frame[24] = PulseCode::new(Level::Low, 12_000, Level::Low, 12_000);
    frame[25] = PulseCode::end_marker();
    frame
}

async fn render(
    mut i2c: I2c<'static, Blocking>,
    state: &Cell<telemetry::Status>,
    rssi: &Cell<Option<i32>>,
) -> ! {
    loop {
        let address = [0x3c_u8, 0x3d]
            .into_iter()
            .find(|&address| i2c.write(address, &[0x00, 0xae]).is_ok());
        if let Some(address) = address {
            let mut bus = OledBus {
                i2c: &mut i2c,
                address,
            };
            let _ = display::draw_dashboard(&mut bus, state, rssi).await;
        }
        Timer::after_secs(1).await;
    }
}

struct OledBus<'a> {
    i2c: &'a mut I2c<'static, Blocking>,
    address: u8,
}

impl OledBus<'_> {
    fn transfer(&mut self, control: u8, data: &[u8]) -> Result<(), DisplayError> {
        let mut packet = [0; 9];
        packet[0] = control;
        for chunk in data.chunks(packet.len() - 1) {
            packet[1..=chunk.len()].copy_from_slice(chunk);
            let result = self.i2c.write(self.address, &packet[..=chunk.len()]);
            if result.is_err() {
                return Err(DisplayError::BusWriteError);
            }
        }
        Ok(())
    }
}

impl AsyncWriteOnlyDataCommand for OledBus<'_> {
    async fn send_commands(&mut self, data: DataFormat<'_>) -> Result<(), DisplayError> {
        let DataFormat::U8(data) = data else {
            return Err(DisplayError::DataFormatNotImplemented);
        };
        self.transfer(0x00, data)
    }

    async fn send_data(&mut self, data: DataFormat<'_>) -> Result<(), DisplayError> {
        let DataFormat::U8(data) = data else {
            return Err(DisplayError::DataFormatNotImplemented);
        };
        self.transfer(0x40, data)
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
