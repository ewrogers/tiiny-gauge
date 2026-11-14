use super::{
    Settings, Storage,
    serial::{PACKET, Serial, session},
};
use core::{cell::Cell, future::pending};
use embassy_futures::{join::join, select::select};
use embassy_net::Stack;
use embassy_rp::{
    Peri, bind_interrupts,
    peripherals::{USB, WATCHDOG},
    usb::{Driver, InterruptHandler},
    watchdog::Watchdog,
};
use embassy_time::Timer;
use embassy_usb::{
    Builder, Config,
    class::cdc_acm::{CdcAcmClass, Receiver, Sender, State},
};
use embedded_storage::nor_flash::NorFlash;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

struct UsbSerial<'a> {
    tx: Sender<'a, Driver<'a, USB>>,
    rx: Receiver<'a, Driver<'a, USB>>,
}

pub(crate) async fn run(
    usb: Peri<'static, USB>,
    watchdog: Peri<'static, WATCHDOG>,
    mut storage: Storage<impl NorFlash>,
    current: Option<&Settings>,
    network_stack: &Cell<Option<Stack<'static>>>,
    rssi: &Cell<Option<i32>>,
) -> ! {
    let mut config = Config::new(0x2e8a, 0x000a);
    config.product = Some("Tiiny Gauge");
    let mut descriptors = [0; 128];
    let mut bos = [0; 64];
    let mut control = [0; PACKET];
    let mut state = State::new();
    let mut builder = Builder::new(
        Driver::new(usb, Irqs),
        config,
        &mut descriptors,
        &mut bos,
        &mut [],
        &mut control,
    );
    let (tx, rx, line_control) =
        CdcAcmClass::new(&mut builder, &mut state, PACKET as u16).split_with_control();
    let mut serial = UsbSerial { tx, rx };
    let mut device = builder.build();
    let mut watchdog = Watchdog::new(watchdog);
    join(device.run(), async {
        loop {
            serial.rx.wait_connection().await;
            while !line_control.dtr() {
                line_control.control_changed().await;
            }
            let _ = select(
                session(
                    &mut serial,
                    &mut storage,
                    current,
                    network_stack,
                    rssi,
                    false,
                ),
                async {
                    while line_control.dtr() {
                        line_control.control_changed().await;
                    }
                },
            )
            .await;
            if storage.saved_this_boot() {
                Timer::after_millis(200).await;
                watchdog.trigger_reset();
                pending::<()>().await;
            }
        }
    })
    .await;
    pending().await
}

impl Serial for UsbSerial<'_> {
    const SAVE_LABEL: &'static str = "Save & Restart";
    const SAVE_MESSAGE: &'static str = "Settings saved. Restarting...\r\n";

    async fn read(&mut self, data: &mut [u8; PACKET]) -> Result<usize, ()> {
        self.rx.read_packet(data).await.map_err(|_| ())
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), ()> {
        self.tx.write_packet(data).await.map_err(|_| ())
    }
}
