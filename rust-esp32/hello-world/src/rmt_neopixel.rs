use std::time::Duration;

use anyhow::Result;
use esp_idf_svc::hal::{
    prelude::Peripherals,
    rmt::{config::TransmitConfig, FixedLengthSignal, PinState, Pulse, TxRmtDriver},
};

use crate::rgb::Rgb;

pub fn get_tmx_rmt_driver(peripherals: Peripherals) -> anyhow::Result<TxRmtDriver<'static>> {
    // Onboard RGB LED pin
    // ESP32-C3-DevKitC-02 gpio8, ESP32-C3-DevKit-RUST-1 gpio2
    let led = peripherals.pins.gpio8;
    let channel = peripherals.rmt.channel0;
    let config = TransmitConfig::new().clock_divider(1);
    let tx = TxRmtDriver::new(channel, led, &config)?;

    Ok(tx)
}

pub fn neopixel(rgb: Rgb, tx: &mut TxRmtDriver) -> Result<()> {
    let color: u32 = rgb.into();
    let ticks_hz = tx.counter_clock()?;
    let (t0h, t0l, t1h, t1l) = (
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_nanos(350))?,
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_nanos(800))?,
        Pulse::new_with_duration(ticks_hz, PinState::High, &Duration::from_nanos(700))?,
        Pulse::new_with_duration(ticks_hz, PinState::Low, &Duration::from_nanos(600))?,
    );
    let mut signal = FixedLengthSignal::<24>::new();
    for i in (0..24).rev() {
        let p = 2_u32.pow(i);
        let bit: bool = p & color != 0;
        let (high_pulse, low_pulse) = if bit { (t1h, t1l) } else { (t0h, t0l) };
        signal.set(23 - i as usize, &(high_pulse, low_pulse))?;
    }
    tx.start_blocking(&signal)?;
    Ok(())
}
