#![no_std]
#![no_main]

use crate::pio_helpers::{get_testing_generated_frame_bytes, initialize_pio, StandardTransmitOption};
use crate::serial_executor::executor;
use crate::usb_serial::USBSerial;
use bsp::entry;
#[allow(unused_imports)]
use defmt_rtt as _;
#[allow(unused_imports)]
use panic_probe as _;
use rp_pico as bsp;
// this allows panic handling

mod board_setup;
mod data_array;
mod error;
mod packet;
mod pio_bytecode_gen;
mod pio_helpers;
mod serial_executor;
mod usb_serial;


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

    let generated_frame_bytes = get_testing_generated_frame_bytes();
    // let waves = generate_waves::<16>();
    let mut iter_frame = transmission_type.convert(&generated_frame_bytes);

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
