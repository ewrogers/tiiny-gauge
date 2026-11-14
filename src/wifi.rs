use cyw43::{Aligned, JoinOptions, aligned_bytes};
use cyw43_firmware::{CYW43_43439A0, CYW43_43439A0_CLM};
use cyw43_pio::{DEFAULT_CLOCK_DIVIDER, PioSpi};
use embassy_executor::Spawner;
use embassy_net::{Config, Runner, Stack, StackResources};
use embassy_rp::{
    Peri, bind_interrupts,
    clocks::RoscRng,
    dma,
    gpio::{Level, Output},
    peripherals::{DMA_CH0, PIN_23, PIN_24, PIN_25, PIN_29, PIO0},
    pio::{InterruptHandler, Pio},
};
use embassy_time::{Duration, Timer, with_timeout};
use static_cell::StaticCell;

#[cfg(feature = "oled-spi")]
use embassy_rp::peripherals::DMA_CH2;

bind_interrupts!(pub(crate) struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, #[cfg(feature = "oled-spi")] dma::InterruptHandler<DMA_CH2>;
});

type Radio = cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>;

#[embassy_executor::task]
async fn radio_task(runner: cyw43::Runner<'static, Radio>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn network_task(mut runner: Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

pub(crate) async fn start(
    spawner: Spawner,
    pio: Peri<'static, PIO0>,
    dma: Peri<'static, DMA_CH0>,
    power: Peri<'static, PIN_23>,
    data: Peri<'static, PIN_24>,
    select: Peri<'static, PIN_25>,
    clock: Peri<'static, PIN_29>,
) -> (Stack<'static>, cyw43::Control<'static>) {
    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    static FW: Aligned<cyw43::A4, [u8; CYW43_43439A0.len()]> = Aligned(*CYW43_43439A0);
    let power = Output::new(power, Level::Low);
    let cs = Output::new(select, Level::High);
    let mut pio = Pio::new(pio, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        data,
        clock,
        dma::Channel::new(dma, Irqs),
    );
    let (device, mut control, runner) = cyw43::new(
        STATE.init(cyw43::State::new()),
        power,
        spi,
        &FW,
        aligned_bytes!("../firmware/nvram_rp2040.bin"),
    )
    .await;
    spawner.spawn(radio_task(runner).expect("one radio task"));
    control.init(CYW43_43439A0_CLM).await;

    let (stack, runner) = embassy_net::new(
        device,
        Config::dhcpv4(Default::default()),
        RESOURCES.init(StackResources::new()),
        RoscRng.next_u64(),
    );
    spawner.spawn(network_task(runner).expect("one network task"));
    (stack, control)
}

pub(crate) async fn maintain(
    stack: Stack<'_>,
    mut control: cyw43::Control<'_>,
    ssid: &str,
    password: &str,
) -> ! {
    loop {
        if control
            .join(ssid, JoinOptions::new(password.as_bytes()))
            .await
            .is_ok()
            && with_timeout(Duration::from_secs(15), stack.wait_config_up())
                .await
                .is_ok()
        {
            stack.wait_config_down().await;
        }
        control.leave().await;
        Timer::after_secs(3).await;
    }
}
