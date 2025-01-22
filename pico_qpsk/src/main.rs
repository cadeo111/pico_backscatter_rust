#![no_std]
#![no_main]

use bsp::entry;
use bsp::hal::clocks::ClocksManager;
use bsp::hal::fugit::RateExtU32;
use bsp::hal::pio::{Buffers, PIOExt, ShiftDirection};
use bsp::hal::{pac, sio::Sio, watchdog::Watchdog};
use bsp::Pins;
use cortex_m::delay::Delay;
use defmt::panic;
use defmt::*;
#[allow(unused_imports)]
use defmt_rtt as _;
use embedded_hal::digital::OutputPin;
use heapless::Vec;
use ieee802154::mac::{PanId, ShortAddress};
use itertools::Itertools;
#[allow(unused_imports)]
use panic_probe as _;
use rp_pico as bsp;
use rp_pico::hal::gpio::bank0::Gpio3;
use rp_pico::hal::gpio::{Function, FunctionPio0, Pin, PinId, PullNone, PullType, ValidFunction};
use rp_pico::hal::pio::{Tx, SM0};
use rp_pico::hal::Clock;
use rp_pico::pac::RESETS;

use crate::data_array::PACKET_IN_RAW_PIO_BYTECODE;
use crate::packet::PhysicalFrame;
use crate::pio_bytecode_gen::{convert, repeat4};
use crate::usb_serial::{init_usb_bus, USBSerial};
// this allows panic handling

mod data_array;

mod error;
mod packet;
mod pio_bytecode_gen;
mod usb_serial;

/// Sets the system clock to 128MHz
///
/// # Arguments
///
/// * `pac_watchdog`: pac::Peripherals arg for init
/// * `pac_pll_sys_device`:  pac::Peripherals arg for init
/// * `pac_clocks_block`: pac::Peripherals arg for init
/// * `pac_xosc_dev`: pac::Peripherals arg for init
/// * `pac_pll_usb`: pac::Peripherals arg for init
/// * `pac_resets`: pac::Peripherals arg for init
///
/// returns: ClocksManager
///
/// # Examples
///
/// ```
///     let clocks = setup_clocks(
///         pp.WATCHDOG,
///         pp.PLL_SYS,
///         pp.CLOCKS,
///         pp.XOSC,
///         pp.PLL_USB,
///         &mut pp.RESETS,
///     );
/// ```
fn setup_clocks(
    pac_watchdog: pac::WATCHDOG,
    pac_pll_sys_device: pac::PLL_SYS,
    pac_clocks_block: pac::CLOCKS,
    pac_xosc_dev: pac::XOSC,
    pac_pll_usb: pac::PLL_USB,
    pac_resets: &mut RESETS,
) -> ClocksManager {
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
    let xosc = bsp::hal::xosc::setup_xosc_blocking(pac_xosc_dev, bsp::XOSC_CRYSTAL_FREQ.Hz())
        .map_err(|_x| false)
        .unwrap();

    // Step 2. Configure watchdog tick generation to tick over every microsecond.
    watchdog.enable_tick_generation((bsp::XOSC_CRYSTAL_FREQ / 1_000_000) as u8);

    // Step 3. Create a clocks manager.
    let mut clocks = ClocksManager::new(pac_clocks_block);

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

    let pll_sys = bsp::hal::pll::setup_pll_blocking(
        pac_pll_sys_device,
        xosc.operating_frequency(),
        bsp::hal::pll::PLLConfig {
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
    let pll_usb = bsp::hal::pll::setup_pll_blocking(
        pac_pll_usb,
        xosc.operating_frequency(),
        bsp::hal::pll::common_configs::PLL_USB_48MHZ,
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
    clocks
}

///
///
/// # Arguments
///
/// * `pac_resets`: pac::Peripherals arg for init
/// * `io_bank0`: pac::Peripherals arg for init
/// * `pads_bank0`: pac::Peripherals arg for init
/// * `system_clock_freq_hz`: the clock frequency you set up
/// * `pac_sio`:
///
/// returns: (Pins, Delay)
///
/// # Examples
///
/// ```
///  let (pins, mut delay) = setup_pins_delay(
///         &mut pp.RESETS,
///         pp.IO_BANK0,
///         pp.PADS_BANK0,
///         clocks.system_clock.freq().to_Hz(),
///         pp.SIO,
///     );
///
///
/// ```
fn setup_pins_delay(
    pac_resets: &mut RESETS,
    io_bank0: pac::IO_BANK0,
    pads_bank0: pac::PADS_BANK0,
    system_clock_freq_hz: u32,
    pac_sio: pac::SIO,
) -> (Pins, Delay) {
    info!("setting up pins and delay...");
    // let mut pp = pac::Peripherals::take().unwrap();
    let pc = pac::CorePeripherals::take().unwrap();

    let sio = Sio::new(pac_sio);

    let delay = Delay::new(pc.SYST, system_clock_freq_hz);

    let pins = Pins::new(io_bank0, pads_bank0, sio.gpio_bank0, pac_resets);
    info!("pins and delay OK");
    (pins, delay)
}

/// Initialize the PIO block with the OQPSK state machine
/// this PIO program can generate any signal that has at least 4 cycle low and high
/// # Arguments
///
/// * `gpio3`: the trigger pin set in the program, the PIO waits for it to go low before starting to read data
/// * `antenna_pin`: the pin that the PIO should control
/// * `pio`:  which pio to use PIO0 or PIO1
/// * `resets`: idk, required to init
///
/// returns: (Tx<(PIOS, SM0)>, impl FnOnce()+Sized)
/// Tx is the writer to the pio buffer, send the data here
/// the function will start the pio reading data, ie, it pulls pin 3 low
///
/// # Examples
///
/// ```
/// let (mut tx, start_pio_execution) = initialize_pio(pins.gpio3, pins.gpio6, pp.PIO0, &mut pp.RESETS);
/// start_pio_execution();
///
///  while tx.is_full() {}
///   tx.write(0b111101100011);
///
/// ```
fn initialize_pio<F, PD, P, F2, PD2, PIOS>(
    gpio3: Pin<Gpio3, F, PD>,
    antenna_pin: Pin<P, F2, PD2>,
    pio: PIOS,
    resets: &mut RESETS,
) -> (Tx<(PIOS, SM0)>, impl FnOnce())
where
    P: ValidFunction<FunctionPio0>,
    F: Function,
    PD: PullType,
    P: PinId,
    F2: Function,
    PD2: PullType,
    PIOS: PIOExt,
{
    info!("Setting up PIO...");

    // this must start high as the pio starts when it goes low should you want to push some data before starting
    let mut start_pin = gpio3.into_push_pull_output();
    start_pin.set_high().unwrap();

    let (mut pio, sm0, _, _, _) = pio.split(resets);

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

    let installed = pio.install(&program.program).unwrap();
    info!("PIO program install ok");
    // Set gpio25 to pio

    let antenna_pin: Pin<_, FunctionPio0, PullNone> = antenna_pin.reconfigure::<FunctionPio0, PullNone>();

    let antenna_pin_id = antenna_pin.id().num;

    // Build the pio program and set pin both for set and side set!
    // We are running with the default divider which is 1 (max speed)
    let (mut sm, _, tx) = bsp::hal::pio::PIOBuilder::from_installed_program(installed)
        .set_pins(antenna_pin_id, 1)
        .buffers(Buffers::OnlyTx)
        .autopull(true)
        .pull_threshold(32)
        .out_shift_direction(ShiftDirection::Left)
        .build(sm0);

    sm.set_pindirs([(antenna_pin_id, bsp::hal::pio::PinDir::Output)]);
    info!("PIO setup ok");

    sm.start();

    info!("PIO start ok");

    (tx, move || {
        start_pin.set_low().unwrap();
    })
}

const MAX_PAYLOAD_SIZE: usize = 4;
const MAX_FRAME_SIZE: usize = to_max_frame_size!(MAX_PAYLOAD_SIZE);
fn get_generated_frame_bytes() -> Vec<u8, MAX_FRAME_SIZE> {
    let payload: [u8; MAX_PAYLOAD_SIZE] = [0x01, 0x02, 0xA, 0xB];
    let frame: PhysicalFrame<MAX_FRAME_SIZE> = PhysicalFrame::new(
        1,
        PanId(0x4444),        // dest
        ShortAddress(0xABCD), // dest
        PanId(0x2222),        // src
        ShortAddress(0x1234), // src
        &payload,
    )
    .unwrap();
    let frame_bytes = frame.to_bytes().unwrap_or_else(|err| {
        panic!(
            "Failed to convert frame to bytes, this should never happen ERR:{:?}",
            err
        );
    });
    info!("Created frame -> :{=[u8]:#x}", &frame_bytes);
    frame_bytes
}

fn get_hex_string_as_bytes<const MAX_VEC_SIZE: usize>(message_str: &str) -> Vec<u8, MAX_VEC_SIZE> {
    let str_iter = message_str
        .chars()
        .map(|c| c.to_digit(16).unwrap().try_into().unwrap())
        .tuples()
        .flat_map(|(a, b): (u8, u8)| [a << 4 | b]);
    Vec::from_iter(str_iter)
}

#[entry]
fn main() -> ! {
    // get the hardware peripherals
    let mut pp = pac::Peripherals::take().unwrap();

    // set up the correct clock speed (128MHz)
    let clocks = setup_clocks(
        pp.WATCHDOG,
        pp.PLL_SYS,
        pp.CLOCKS,
        pp.XOSC,
        pp.PLL_USB,
        &mut pp.RESETS,
    );

    // set up GPIO and Delay function
    let (pins, mut delay) = setup_pins_delay(
        &mut pp.RESETS,
        pp.IO_BANK0,
        pp.PADS_BANK0,
        clocks.system_clock.freq().to_Hz(),
        pp.SIO,
    );

    let bus = init_usb_bus(
        pp.USBCTRL_REGS,
        pp.USBCTRL_DPRAM,
        clocks.usb_clock,
        &mut pp.RESETS,
    );
    let mut serial = USBSerial::new(&bus);
    let buff = serial.poll_until_enter::<64>();
    match buff {
        Ok(vec) => {
            let s = heapless::String::<64>::from_utf8(vec).unwrap();
            s.as_str();
            serial.write(&s)
        }
        Err(err) => {
            error!("{}", err)
        }
    }
    serial.write("1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59,60,61,62,63,64,65,66,67,68,69,70,71,72,73,74,75,76,77,78,79,80,81,82,83,84,85,86,87,88,89,90,91,92,93,94,95,96,97,98,99,100,101,102,103,104,105,106,107,108,109,110,111,112,113,114,115,116,117,118,119,120,121,122,123,124,125,126,127,128,129,130,131,132,133,134,135,136,137,138,139,140,141,142,143,144,145,146,147,148,149,150,151,152,153,154,155,156,157,158,159,160,161,162,163,164,165,166,167,168,169,170,171,172,173,174,175,176,177,178,179,180,181,182,183,184,185,186,187,188,189,190,191,192,193,194,195,196,197,198,199,200,201,202,203,204,205,206,207,208,209,210,211,212,213,214,215,216,217,218,219,220,221,222,223,224,225,226,227,228,229,230,231,232,233,234,235,236,237,238,239,240,241,242,243,244,245,246,247,248,249,250,251,252,253,254,255,256,257,258,259,260,261,262,263,264,265,266,267,268,269,270,271,272,273,274,275,276,277,278,279,280,281,282,283,284,285,286,287,288,289,290,291,292,293,294,295,296,297,298,299,300,301,302,303,304,305,306,307,308,309,310,311,312,313,314,315,316,317,318,319,320,321,322,323,324,325,326,327,328,329,330,331,332,333,334,335,336,337,338,339,340,341,342,343,344,345,346,347,348,349,350,351,352,353,354,355,356,357,358,359,360,361,362,363,364,365,366,367,368,369,370,371,372,373,374,375,376,377,378,379,380,381,382,383,384,385,386,387,388,389,390,391,392,393,394,395,396,397,398,399,400,401,402,403,404,405,406,407,408,409,410,411,412,413,414,415,416,417,418,419,420,421,422,423,424,425,426,427,428,429,430,431,432,433,434,435,436,437,438,439,440,441,442,443,444,445,446,447,448,449,450,451,452,453,454,455,456,457,458,459,460,461,462,463,464,465,466,467,468,469,470,471,472,473,474,475,476,477,478,479,480,481,482,483,484,485,486,487,488,489,490,491,492,493,494,495,496,497,498,499,500,501,502,503,504,505,506,507,508,509,510,511,512,513,514,515,516,517,518,519,520,521,522,523,524,525,526,527,528,529,530,531,532,533,534,535,536,537,538,539,540,541,542,543,544,545,546,547,548,549,550,551,552,553,554,555,556,557,558,559,560,561,562,563,564,565,566,567,568,569,570,571,572,573,574,575,576,577,578,579,580,581,582,583,584,585,586,587,588,589,590,591,592,593,594,595,596,597,598,599,600,601,602,603,604,605,606,607,608,609,610,611,612,613,614,615,616,617,618,619,620,621,622,623,624,625,626,627,628,629,630,631,632,633,634,635,636,637,638,639,640,641,642,643,644,645,646,647,648,649,650,651,652,653,654,655,656,657,658,659,660,661,662,663,664,665,666,667,668,669,670,671,672,673,674,675,676,677,678,679,680,681,682,683,684,685,686,687,688,689,690,691,692,693,694,695,696,697,698,699,700,701,702,703,704,705,706,707,708,709,710,711,712,713,714,715,716,717,718,719,720,721,722,723,724,725,726,727,728,729,730,731,732,733,734,735,736,737,738,739,740,741,742,743,744,745,746,747,748,749,750,751,752,753,754,755,756,757,758,759,760,761,762,763,764,765,766,767,768,769,770,771,772,773,774,775,776,777,778,779,780,781,782,783,784,785,786,787,788,789,790,791,792,793,794,795,796,797,798,799,800,801,802,803,804,805,806,807,808,809,810,811,812,813,814,815,816,817,818,819,820,821,822,823,824,825,826,827,828,829,830,831,832,833,834,835,836,837,838,839,840,841,842,843,844,845,846,847,848,849,850,851,852,853,854,855,856,857,858,859,860,861,862,863,864,865,866,867,868,869,870,871,872,873,874,875,876,877,878,879,880,881,882,883,884,885,886,887,888,889,890,891,892,893,894,895,896,897,898,899,900,901,902,903,904,905,906,907,908,909,910,911,912,913,914,915,916,917,918,919,920,921,922,923,924,925,926,927,928,929,930,931,932,933,934,935,936,937,938,939,940,941,942,943,944,945,946,947,948,949,950,951,952,953,954,955,956,957,958,959,960,961,962,963,964,965,966,967,968,969,970,971,972,973,974,975,976,977,978,979,980,981,982,983,984,985,986,987,988,989,990,991,992,993,994,995,996,997,998,999,1000");
    
    panic!("DONE");
    // loop{
    //     serial.poll()
    // }
    //

    // Set up PIO to control transmission
    let (mut tx, start_pio_execution) = initialize_pio(pins.gpio3, pins.gpio6, pp.PIO0, &mut pp.RESETS);

    // generate a frame on the pico
    let generated_frame_bytes = get_generated_frame_bytes();
    // convert the bytes to PIO O-QPSK
    let iter_frame = convert(&generated_frame_bytes, repeat4);

    // create data from hex string
    let message_str = "00000000A71741880B222234124444CDAB0102030405020202090A4B49";
    // the vector needs a compile time max capacity, the number of bytes min needed
    // is the same as the number of characters in the string
    const MESSAGE_STR_LEN: usize = 58;
    let str_vec = get_hex_string_as_bytes::<MESSAGE_STR_LEN>(message_str);
    let iter_string = convert(&str_vec, repeat4);

    info!("delay 5 sec");
    delay.delay_ms(5000);
    info!("start");
    start_pio_execution();
    info!("PIN 3 is low");
    // swap between the three message options every 10 loops
    let mut idx = 0;

    // if the current generated content is from a the generated frame or the string
    let mut is_frame = false;

    // the buffer to hold the calculated values for the pio,
    // while it is possible to just iterate through a clone of the iterator directly
    // the Pico is not fast enough to do this and keep up the speed required to send the message
    // maybe with an adjustment the timing this could work
    // for now on each switch the iterators are loaded into the buffer and then sent 10 times
    let mut buffer: Vec<u32, 4000> = Vec::new();
    loop {
        match idx % 30 {
            0..=10 => {
                if !is_frame {
                    buffer.clear();
                    info!("cleared buffer for frame...");
                    buffer.extend(iter_frame.clone());
                    info!(" buffer for frame is {} bytes...", buffer.len());
                    is_frame = true;
                }
                info!("sending gen packet from frame...");
                for i in &buffer {
                    while tx.is_full() {}
                    tx.write(*i);
                }
            }
            11..=20 => {
                if is_frame {
                    buffer.clear();
                    info!("cleared buffer for string...");
                    buffer.extend(iter_string.clone());
                    info!(" buffer for string is {} bytes...", buffer.len());
                    is_frame = false;
                }
                info!("sending gen packet from string...");
                for i in &buffer {
                    while tx.is_full() {}
                    tx.write(*i);
                }
            }
            _ => {
                info!("sending static packet...");
                for i in PACKET_IN_RAW_PIO_BYTECODE {
                    while tx.is_full() {}
                    tx.write(*i);
                }
            }
        }
        // need to make sure there is at least min delay need for inter-frame spacing
        info!("delay 1 second");
        delay.delay_ms(1000);
        idx += 1;
    }

    // user can send random packets, settings: number of packets, length of packet < max, time between packets
    // pico will send over uart the contents of the random packets
    // user can send standard predefined packet, settings:  number of packets, length of packet, time between packets
}

// End of file
