#![no_std]
#![no_main]
use crate::packet::PhysicalFrame;
use crate::pio_bytecode_gen::{convert, repeat1, repeat4};
use crate::pio_helpers::{initialize_pio, StandardTransmitOption};
use crate::serial_executor::executor;
use crate::usb_serial::{init_usb_bus, USBSerial};
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
use rp_pico::hal::{reset, Clock};
use rp_pico::pac::RESETS;
// this allows panic handling

mod board_setup;
mod data_array;
mod error;
mod packet;
mod pio_bytecode_gen;
mod pio_helpers;
mod serial_executor;
mod usb_serial;

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
    let transmission_type = StandardTransmitOption::Clk128MHzOffset8MHz;

    let (pins, mut delay, mut resets, bus, pio) = board_setup::setup(transmission_type.processor_clock());

    let mut serial = USBSerial::new(&bus);

    // Set up PIO to control transmission
    let (mut tx, mut start_pio_execution, mut change_clk_divider) =
        initialize_pio(pins.gpio3, pins.gpio6, pio, &mut resets);

    // set the correct clock divider
    change_clk_divider(transmission_type.state_machine_clock());

    let generated_frame_bytes = get_generated_frame_bytes();
    // let waves = generate_waves::<16>();
    let mut iter_frame = convert(&generated_frame_bytes, repeat1, &wave_array!(16));

    executor(
        &mut serial,
        &mut delay,
        &mut tx,
        &mut start_pio_execution,
        &mut iter_frame,
    );
    //
    //
    // // generate a frame on the pico
    // let generated_frame_bytes = get_generated_frame_bytes();
    // // convert the bytes to PIO O-QPSK
    // let iter_frame = convert(&generated_frame_bytes, repeat4);
    //
    // // create data from hex string
    // let message_str = "00000000A71741880B222234124444CDAB0102030405020202090A4B49";
    // // the vector needs a compile time max capacity, the number of bytes min needed
    // // is the same as the number of characters in the string
    // const MESSAGE_STR_LEN: usize = 58;
    // let str_vec = get_hex_string_as_bytes::<MESSAGE_STR_LEN>(message_str);
    // let iter_string = convert(&str_vec, repeat4);
    //
    // info!("delay 5 sec");
    // delay.delay_ms(5000);
    // info!("start");
    // start_pio_execution();
    // info!("PIN 3 is low");
    // // swap between the three message options every 10 loops
    // let mut idx = 0;
    //
    // // if the current generated content is from a the generated frame or the string
    // let mut is_frame = false;
    //
    // // the buffer to hold the calculated values for the pio,
    // // while it is possible to just iterate through a clone of the iterator directly
    // // the Pico is not fast enough to do this and keep up the speed required to send the message
    // // maybe with an adjustment the timing this could work
    // // for now on each switch the iterators are loaded into the buffer and then sent 10 times
    // let mut buffer: Vec<u32, 4000> = Vec::new();
    // loop {
    //     match idx % 30 {
    //         0..=10 => {
    //             if !is_frame {
    //                 buffer.clear();
    //                 info!("cleared buffer for frame...");
    //                 buffer.extend(iter_frame.clone());
    //                 info!(" buffer for frame is {} bytes...", buffer.len());
    //                 is_frame = true;
    //             }
    //             info!("sending gen packet from frame...");
    //             for i in &buffer {
    //                 while tx.is_full() {}
    //                 tx.write(*i);
    //             }
    //         }
    //         11..=20 => {
    //             if is_frame {
    //                 buffer.clear();
    //                 info!("cleared buffer for string...");
    //                 buffer.extend(iter_string.clone());
    //                 info!(" buffer for string is {} bytes...", buffer.len());
    //                 is_frame = false;
    //             }
    //             info!("sending gen packet from string...");
    //             for i in &buffer {
    //                 while tx.is_full() {}
    //                 tx.write(*i);
    //             }
    //         }
    //         _ => {
    //             info!("sending static packet...");
    //             for i in PACKET_IN_RAW_PIO_BYTECODE {
    //                 while tx.is_full() {}
    //                 tx.write(*i);
    //             }
    //         }
    //     }
    //     // need to make sure there is at least min delay need for inter-frame spacing
    //     info!("delay 1 second");
    //     delay.delay_ms(1000);
    //     idx += 1;
    // }

    // user can send random packets, settings: number of packets, length of packet < max, time between packets
    // pico will send over uart the contents of the random packets
    // user can send standard predefined packet, settings:  number of packets, length of packet, time between packets
}

// End of file
