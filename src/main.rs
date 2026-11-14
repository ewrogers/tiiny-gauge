#![no_std]
#![no_main]

use embassy_executor::Spawner;
use panic_halt as _;

mod display;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let board = embassy_rp::init(Default::default());
    display::run(board).await;
}
