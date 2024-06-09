//! Blinks the LED on a Pico board
//!
//! This will blink an LED attached to GP25, which is the pin the Pico uses for the on-board LED.
#![no_std]
#![no_main]

use bsp::entry;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::OutputPin;
use panic_probe as _;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use rp_pico as bsp;
// use sparkfun_pro_micro_rp2040 as bsp;

use bsp::hal::{
    clocks::{init_clocks_and_plls, Clock},
    pac,
    sio::Sio,
    watchdog::Watchdog,
};
use cortex_m::asm::delay;
use cortex_m::delay::Delay;
use pio_proc::pio_file;
use rp_pico::hal::clocks::ClocksManager;
use rp_pico::hal::fugit::RateExtU32;
use rp_pico::hal::gpio::bank0::Gpio6;
use rp_pico::hal::pio::{Buffers, PIOExt, ShiftDirection};


use rp_pico::Pins;

// from NCC
/// Reset the DMA Peripheral.

fn setup_clocks(pac_watchdog: pac::WATCHDOG,
                pac_pll_sys_device: pac::PLL_SYS,
                pac_clocks_block: pac::CLOCKS,
                pac_xosc_dev: pac::XOSC,
                pac_pll_usb: pac::PLL_USB,
                pac_resets: &mut pac::RESETS,
) -> ClocksManager
{
    info!("Setting up clocks...");

    // set up custom clock frequency of 128MHz
    // custom clock set up from
    // https://github.com/Neotron-Compute/Neotron-Pico-BIOS/blob/fadb7601d290fd62d8a45424c539dc8c0c93cf93/src/main.rs#L346-L404
    // Referred to as NCC
    // vvv from NCC vvv

    // Reset the DMA engine. If we don't do this, starting from probe-rs
    // (as opposed to a cold-start) is unreliable.
    {
        pac_resets.reset().modify(|_r, w| w.dma().set_bit());
        cortex_m::asm::nop();
        pac_resets.reset().modify(|_r, w| w.dma().clear_bit());
        while pac_resets.reset_done().read().dma().bit_is_clear() {}
    }


    // Needed by the clock setup
    let mut watchdog = Watchdog::new(pac_watchdog);


    // Step 1. Turn on the crystal.
    let xosc = rp_pico::hal::xosc::setup_xosc_blocking(pac_xosc_dev, rp_pico::XOSC_CRYSTAL_FREQ.Hz())
        .map_err(|_x| false)
        .unwrap();

    // Step 2. Configure watchdog tick generation to tick over every microsecond.
    watchdog.enable_tick_generation((rp_pico::XOSC_CRYSTAL_FREQ / 1_000_000) as u8);

    // Step 3. Create a clocks manager.
    let mut clocks = rp_pico::hal::clocks::ClocksManager::new(pac_clocks_block);

    // Step 4. Set up the system PLL.
    //
    // We take the Crystal Oscillator (=12 MHz) with no divider, and ×128 to
    // give a FOUTVCO of [1536] MHz. This must be in the range 750 MHz - 1600 MHz.
    // The factor of 128 is calculated automatically given the desired FOUTVCO.
    //
    // Next we ÷6 on the first post divider to give 256 MHz.
    //
    // Finally we ÷2 on the second post divider to give 128 MHz.
    //
    // We note from the [RP2040
    // Datasheet](https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf#page=231),
    // Section 2.18.2.1:
    //
    // > Jitter is minimised by running the VCO at the highest possible
    // > frequency, so that higher post-divide values can be used.


    let pll_sys = rp_pico::hal::pll::setup_pll_blocking(
        pac_pll_sys_device,
        xosc.operating_frequency(),
        rp_pico::hal::pll::PLLConfig {
            vco_freq: 1536.MHz(),
            refdiv: 1,
            post_div1: 6,
            post_div2: 2,
        },
        &mut clocks,
        pac_resets,
    )
        .map_err(|_x| false)
        .unwrap();

    // Step 5. Set up a 48 MHz PLL for the USB system.
    let pll_usb = rp_pico::hal::pll::setup_pll_blocking(
        pac_pll_usb,
        xosc.operating_frequency(),
        rp_pico::hal::pll::common_configs::PLL_USB_48MHZ,
        &mut clocks,
        pac_resets,
    )
        .map_err(|_x| false)
        .unwrap();
    // Step 6. Set the system to run from the PLLs we just configured.
    clocks
        .init_default(&xosc, &pll_sys, &pll_usb)
        .map_err(|_x| false)
        .unwrap();

    info!("Clocks OK");


    // ^^^ from NCC ^^^
    return clocks;
}

fn setup_pins_delay(
    pac_resets: &mut pac::RESETS,
    io_bank0: pac::IO_BANK0,
    pads_bank0: pac::PADS_BANK0,
    system_clock_freq_hz: u32,
    pac_sio: pac::SIO, ) -> (Pins, Delay)
{
    info!("setting up pins and delay...");
    // let mut pp = pac::Peripherals::take().unwrap();
    let pc = pac::CorePeripherals::take().unwrap();


    let sio = Sio::new(pac_sio);

    let mut delay = Delay::new(pc.SYST, system_clock_freq_hz);

    let pins = Pins::new(
        io_bank0,
        pads_bank0,
        sio.gpio_bank0,
        pac_resets,
    );
    info!("pins and delay OK");
    return (pins, delay);
}


#[entry]
fn main() -> ! {
    let mut pp = pac::Peripherals::take().unwrap();
    let clocks = setup_clocks(pp.WATCHDOG, pp.PLL_SYS, pp.CLOCKS, pp.XOSC, pp.PLL_USB,
                              &mut pp.RESETS,
    );

    let (pins, mut delay) = setup_pins_delay(
        &mut pp.RESETS,
        pp.IO_BANK0,
        pp.PADS_BANK0,
        clocks.system_clock.freq().to_Hz(),
        pp.SIO);

    let mut start_pin = pins.gpio3.into_push_pull_output();
    let mut trigger_pin = pins.gpio2.into_push_pull_output();
    trigger_pin.set_low().unwrap();
    start_pin.set_high().unwrap();
    info!("PIN 3 is high PIN 2 is low");
    
    
    info!("Setting up PIO...");
    let (mut pio0, sm0, _, _, _) = pp.PIO0.split(&mut pp.RESETS);

    // Create a pio program
    let program = pio_proc::pio_asm!(
    "wait 0 pin 3",
    ".wrap_target",
        "set pins 0 [1]",
        "loop1:",
            "out x 1",
            "jmp x-- loop1",
        "set pins, 1 [1]",
        "loop2:",
            "out y 1",
            "jmp y-- loop2",
    ".wrap",
    options(max_program_size = 32) // Optional, defaults to 32
    );

    let installed = pio0.install(&program.program).unwrap();
    info!("PIO program install ok");
    // Set gpio25 to pio
    let _led: rp_pico::hal::gpio::Pin<_, rp_pico::hal::gpio::FunctionPio0, rp_pico::hal::gpio::PullNone> =
        pins.gpio6.reconfigure();
    let led_pin_id = 6;

    // Build the pio program and set pin both for set and side set!
    // We are running with the default divider which is 1 (max speed)
    let (mut sm, _, mut tx) = rp_pico::hal::pio::PIOBuilder::from_installed_program(installed)
        .set_pins(led_pin_id, 1)
        .buffers(Buffers::OnlyTx)
        .autopull(true)
        .pull_threshold(32)
        .out_shift_direction(ShiftDirection::Left)
        .build(sm0);

    // Set pio pindir for gpio25
    sm.set_pindirs([(led_pin_id, rp_pico::hal::pio::PinDir::Output)]);
    info!("PIO setup ok");

   

    // Start state machine
    let sm = sm.start();
    
    info!("PIO start ok");
    info!("loading FIFO");
    let mut started = false;
    info!("delay 2 sec");
    delay.delay_ms(5000);
    info!("start");

    tx.write(0b11111110111111101111111011111110);
    while (tx.is_full()) {}
    tx.write(0b11111110111111101111111011111110);
    while (tx.is_full()) {}
    tx.write(0b11111110111111101111111011111110);
    while (tx.is_full()) {}
    tx.write(0b11111110111111101111111011111110);
    if !started {
        started = true;
        trigger_pin.set_high().unwrap();
        delay.delay_ms(1);
        start_pin.set_low().unwrap();
        info!("PIN 3 is low Pin 2 is High");
    }
    loop{
        while (tx.is_full()) {}
        tx.write(0b11111110111111101111111011111110);
    }


    // This is the correct pin on the Raspberry Pico board. On other boards, even if they have an
    // on-board LED, it might need to be changed.
    //
    // Notably, on the Pico W, the LED is not connected to any of the RP2040 GPIOs but to the cyw43 module instead.
    // One way to do that is by using [embassy](https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/wifi_blinky.rs)
    //
    // If you have a Pico W and want to toggle a LED with a simple GPIO output pin, you can connect an external
    // LED to one of the GPIO pins, and reference that pin here. Don't forget adding an appropriate resistor
    // in series with the LED.


    // let mut led_pin = pins.led.into_push_pull_output();

    loop {
        // info!("on!");
        // led_pin.set_high().unwrap();
        // delay.delay_ms(500);
        // info!("off!");
        // led_pin.set_low().unwrap();
        // delay.delay_ms(500);
    }
}

// End of file
