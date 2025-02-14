use crate::pio_bytecode_gen::ConvertIterType;
use crate::pio_helpers::{get_seq_frame_bytes, PioControl, StandardTransmitOption};
use crate::serial_executor::CommandError::ArgsError;
use crate::to_max_frame_size;
use crate::usb_serial::USBSerial;
use core::fmt::Write;
use cortex_m::delay::Delay;
use defmt::{info, warn};
use heapless::{String, Vec};
use owo_colors::{colors::*, OwoColorize, XtermColors};
use rp_pico::hal::gpio::PullDown;
use rp_pico::hal::pio::{Tx, SM0};
use rp_pico::hal::reset;
use rp_pico::pac::PIO0;

#[allow(clippy::enum_variant_names)]
enum FrequencyOffsetCommandOption {
    F2MHz,
    F4MHz,
    F8MHz,
}

impl FrequencyOffsetCommandOption {
    fn parse_from_str<'a>(value: &str, possible_error: CommandError<'a>) -> Result<Self, CommandError<'a>> {
        Ok(match value {
            "2" | "2mhz" | "2MHz" => Self::F2MHz,
            "4" | "4mhz" | "4MHz" => Self::F4MHz,
            "8" | "8mhz" | "8MHz" => Self::F8MHz,
            _ => Err(possible_error)?,
        })
    }
}

enum Command {
    Restart,
    Help,
    SendSequentialPacket {
        interval_ms: u32,
        number_packets: u32,
        payload_length: Option<u32>,
    },
    SetFrequencyOffset {
        frequency: FrequencyOffsetCommandOption,
    },
}

enum CommandError<'a> {
    UnknownCommand(&'a str),
    UnknownError,
    ArgsError { arg_name: &'static str },
}

impl Command {
    fn from_str<const SIZE: usize>(input: &String<SIZE>) -> Result<Command, CommandError> {
        let mut iter = input.split_whitespace();
        match iter.next().ok_or(CommandError::UnknownError)? {
            "restart" => Ok(Self::Restart),
            "help" => Ok(Self::Help),
            "ssp" => {
                let interval_ms_str = iter.next().ok_or(CommandError::UnknownError)?;

                let interval_ms = if interval_ms_str.ends_with("ms") {
                    let slice = &interval_ms_str[0..interval_ms_str.len() - 2];
                    slice.parse().map_err(|_| ArgsError {
                        arg_name: "interval_ms ms",
                    })?
                } else if interval_ms_str.ends_with("s") {
                    let slice = &interval_ms_str[0..interval_ms_str.len() - 1];
                    let secs: u32 = slice.parse().map_err(|_| ArgsError {
                        arg_name: "interval_ms s",
                    })?;
                    secs * 1000
                } else {
                    interval_ms_str.parse().map_err(|_| ArgsError {
                        arg_name: "interval_ms",
                    })?
                };

                let number_packets =
                    iter.next()
                        .ok_or(CommandError::UnknownError)?
                        .parse()
                        .map_err(|_| ArgsError {
                            arg_name: "number_packets",
                        })?;

                let payload_length = {
                    let option = iter.next().map(|v| {
                        v.parse::<u32>().map_err(|_| ArgsError {
                            arg_name: "number_packets",
                        })
                    });
                    if let Some(result) = option {
                        Some(result?)
                    } else {
                        None
                    }
                };

                if payload_length > Some(MAX_PAYLOAD_SIZE as u32) {
                    return Err(ArgsError {
                        arg_name: "payload_length",
                    });
                }

                Ok(Self::SendSequentialPacket {
                    interval_ms,
                    number_packets,
                    payload_length,
                })
            }
            "freq" => {
                let frequency_option_str = iter.next().ok_or(CommandError::UnknownError)?;
                let frequency = FrequencyOffsetCommandOption::parse_from_str(
                    frequency_option_str,
                    ArgsError {
                        arg_name: "frequency",
                    },
                )?;
                Ok(Self::SetFrequencyOffset { frequency })
            }
            _ => Err(CommandError::UnknownCommand(input.as_str())),
        }
    }
}

fn help(serial: &mut USBSerial) {
    const SERIAL_PANIC_ERROR_MESSAGE: &str = "write error:help";

    writeln!(
        serial,
        "{}{}{}",
        "Available commands:\
\n\
    \n\r- restart > restart device\
\n\
    \n\r- help > this menu\
\n\
    \n\r- ssp <interval> <number_packets> <payload_length=4>\
    \n\r\t send a packet with a sequential payload repeatedly\
    \n\r\t- interval: interval between packets in millisecond or seconds (1s/1000ms/1000)\
    \n\r\t- number_packets: number of packets to send\
    \n\r\t- payload_length: how long the payload should be (optional, default:4, max:"
            .fg::<Green>(),
        MAX_PAYLOAD_SIZE.fg::<Green>(),
        ")\
    \n\r\t Example: ssp 1s 1000 5\
    \n\r\t This will send 1000 packets with an interval of 1 second with a payload of 5 bytes\
\n\
    \n\r- srp <interval> <number_packets> <payload_length=4>\
    \n\r\t (unimplemented) send a packet with with a different payload each time,\
    the bytes will be sort of random each packet but will be the same each time this is run\
    \n\r\t- interval: interval between packets in millisecond or seconds (1s/1000ms/1000)\
    \n\r\t- number_packets: number of packets to send\
    \n\r\t- payload_length: how long the payload should be (optional, default:4)\
    \n\r\t Example: srp 10 1000\
    \n\r\t This will send 1000 packets with an interval of 10 milliseconds\
\n\
    \n\r- freq <frequency>\
    \n\r\t change the frequency offset, starts at 8MHz\
    \n\r\t- frequency: the offset frequecy of 2/4/8 MHz (2/2mhz/2MHz)\
    \n\r\t Example: freq 2\
    \n\r\t This will set the offset frequency to 2Mhz\
    "
        .fg::<Green>()
    )
    .expect(SERIAL_PANIC_ERROR_MESSAGE)
}
fn restart() {
    reset();
}

fn check_packet_size(packet_pio_iter: ConvertIterType, serial: &mut USBSerial) {
    const SERIAL_PANIC_ERROR_MESSAGE: &str = "write error:check_packet_size";
    let (len, _) = packet_pio_iter
        .enumerate()
        .last()
        .expect("packet iterator should have a defined size");
    info!("Iterator is {} u32 long", len);
    if len > MAX_PACKET_PIO_BUFFER {
        warn!(
            "Packet pio iterator length is {} longer than {} item buffer!\n\
       this means the whole packet will not be transmitted",
            len - MAX_PACKET_PIO_BUFFER,
            MAX_PACKET_PIO_BUFFER
        );
        writeln!(
            serial,
            "{}{}{}{}{}{}",
            "Packet pio iterator length is "
                .color(XtermColors::White)
                .on_color(XtermColors::BlazeOrange)
                .italic(),
            len - MAX_PACKET_PIO_BUFFER,
            " longer than "
                .color(XtermColors::White)
                .on_color(XtermColors::BlazeOrange)
                .italic(),
            MAX_PACKET_PIO_BUFFER,
            " item buffer!\n"
                .color(XtermColors::White)
                .on_color(XtermColors::BlazeOrange)
                .italic(),
            "this means the whole packet will not be transmitted"
                .color(XtermColors::White)
                .on_color(XtermColors::BlazeOrange)
                .italic(),
        )
        .expect(SERIAL_PANIC_ERROR_MESSAGE);
    }
}

const DEFAULT_PAYLOAD_SIZE: u32 = 4;
const MAX_PAYLOAD_SIZE: usize = 1000;

struct UserPacketOptions {
    transmit_option: StandardTransmitOption,
    payload_length: Option<u32>,
    interval_ms: u32,
    number_packets: u32,
}

fn send_generic_packet(
    serial: &mut USBSerial,
    delay: &mut Delay,
    tx: &mut Tx<(PIO0, SM0)>,
    pio_ctrl: &mut PioControl<PIO0, PullDown>,
    user_options: UserPacketOptions,
) {
    const SERIAL_PANIC_ERROR_MESSAGE: &str = "write error:send_generic_packet";

    let UserPacketOptions {
        transmit_option,
        payload_length,
        interval_ms,
        number_packets,
    } = user_options;

    let payload_size = payload_length.unwrap_or(DEFAULT_PAYLOAD_SIZE);

    writeln!(serial, "sending generic packet...").expect(SERIAL_PANIC_ERROR_MESSAGE);
    writeln!(
        serial,
        "interval_ms: {}, number_packets: {}, payload_size:{}",
        interval_ms, number_packets, payload_size
    )
    .expect(SERIAL_PANIC_ERROR_MESSAGE);

    let frame_bytes = get_seq_frame_bytes::<MAX_PAYLOAD_SIZE, { to_max_frame_size!(MAX_PAYLOAD_SIZE) }>(
        payload_size as usize,
    );
    let packet_pio_iter = transmit_option.convert(&frame_bytes);

    check_packet_size(packet_pio_iter.clone(), serial);

    send_packets(
        serial,
        delay,
        interval_ms,
        number_packets,
        tx,
        pio_ctrl,
        packet_pio_iter,
        &mut |serial, packets_sent| {
            info!("sending packet {}/{} ", packets_sent, number_packets);
            writeln!(
                serial,
                "{}{}/{}",
                "sending packet... ".fg::<Yellow>(),
                packets_sent,
                number_packets
            )
            .expect(SERIAL_PANIC_ERROR_MESSAGE);
        },
        &mut |serial, packets_sent| {
            writeln!(
                serial,
                "{} {}/{} packets sent",
                "Exited Early!"
                    .color(XtermColors::White)
                    .on_color(XtermColors::BlazeOrange)
                    .italic(),
                packets_sent,
                number_packets
            )
            .expect(SERIAL_PANIC_ERROR_MESSAGE);
        },
        &mut |serial| {
            info!("stopped sending packets");
            writeln!(serial, "{}", "Done!".fg::<Green>()).expect(SERIAL_PANIC_ERROR_MESSAGE);
        },
    )
}

const MAX_PACKET_PIO_BUFFER: usize = 4000;

#[allow(clippy::too_many_arguments)]
fn send_packets(
    serial: &mut USBSerial,
    delay: &mut Delay,
    interval_ms: u32,
    number_packets: u32,
    tx: &mut Tx<(PIO0, SM0)>,
    pio_ctrl: &mut PioControl<PIO0, PullDown>,
    packet_pio_iter: ConvertIterType,
    on_send_packet: &mut (impl FnMut(&mut USBSerial, u32) + Sized),
    on_exit_early: &mut (impl FnMut(&mut USBSerial, u32) + Sized),
    on_exit_normal: &mut (impl FnMut(&mut USBSerial) + Sized),
) {
    let mut buffer: Vec<u32, MAX_PACKET_PIO_BUFFER> = Vec::new();
    let iter = packet_pio_iter.take(MAX_PACKET_PIO_BUFFER);
    buffer.extend(iter);

    for i in 0..number_packets {
        let mut started = false;
        on_send_packet(serial, i + 1);
        for i in &buffer {
            while tx.is_full() {
                if !started {
                    started = true;
                    pio_ctrl.start();
                }
            }
            tx.write(*i);
        }
        pio_ctrl.stop();
        delay.delay_ms(interval_ms);
        if serial.poll_is_etx() {
            on_exit_early(serial, i + 1);

            return;
        }
    }
    pio_ctrl.stop();
    on_exit_normal(serial);
}

pub fn executor(
    serial: &mut USBSerial,
    delay: &mut Delay,
    tx: &mut Tx<(PIO0, SM0)>,
    pio_ctrl: &mut PioControl<PIO0, PullDown>,
    base_transmit_option: StandardTransmitOption,
) -> ! {
    let mut transmit_option = base_transmit_option;

    let mut command_buffer = Vec::<u8, 64>::new();
    loop {
        command_buffer.clear();
        let response = serial.poll_until_enter(&mut command_buffer, true);
        if response.is_err() {
            writeln!(serial, "command too long, try again").expect("write error:cmd_too_long");
            continue;
        }
        // vec has command
        let command_string = command_buffer
            .as_slice()
            .iter()
            .map(|x| *x as char)
            .collect::<String<64>>();
        match Command::from_str(&command_string) {
            Ok(response) => match response {
                Command::Restart => {
                    restart();
                }
                Command::Help => {
                    help(serial);
                }
                Command::SendSequentialPacket {
                    interval_ms,
                    number_packets,
                    payload_length,
                } => {
                    send_generic_packet(
                        serial,
                        delay,
                        tx,
                        pio_ctrl,
                        UserPacketOptions {
                            transmit_option,
                            payload_length,
                            interval_ms,
                            number_packets,
                        },
                    );
                }

                Command::SetFrequencyOffset { frequency } => {
                    const SERIAL_PANIC_ERROR_MESSAGE: &str =
                        "write error:executor:Command::SetFrequencyOffset";
                    match frequency {
                        FrequencyOffsetCommandOption::F2MHz => {
                            transmit_option = StandardTransmitOption::Clk128MHzOffset2MHz;
                            writeln!(serial, "Changed Frequency offset to 2MHz")
                                .expect(SERIAL_PANIC_ERROR_MESSAGE);
                        }
                        FrequencyOffsetCommandOption::F4MHz => {
                            transmit_option = StandardTransmitOption::Clk128MHzOffset4MHz;
                            writeln!(serial, "Changed Frequency offset to 4MHz")
                                .expect(SERIAL_PANIC_ERROR_MESSAGE);
                        }
                        FrequencyOffsetCommandOption::F8MHz => {
                            transmit_option = StandardTransmitOption::Clk128MHzOffset8MHz;
                            writeln!(serial, "Changed Frequency offset to 8MHz")
                                .expect(SERIAL_PANIC_ERROR_MESSAGE);
                        }
                    }
                }
            },
            Err(err) => match err {
                CommandError::UnknownError => {
                    writeln!(serial, "{}", "unknown command error, try help".fg::<Red>())
                        .expect("write error:UnknownError");
                }
                ArgsError { arg_name } => {
                    writeln!(
                        serial,
                        "{} {arg_name} {}",
                        "error with arg:".fg::<Red>(),
                        ", try help".fg::<Red>()
                    )
                    .expect("write error:ArgsError");
                }
                CommandError::UnknownCommand(command) => {
                    writeln!(
                        serial,
                        "{} {command} {}",
                        "unknown command,".fg::<Red>(),
                        "try help".fg::<Red>()
                    )
                    .expect("write error:UnknownCommand");
                }
            },
        }

        // write prompt, its here because it can get weird if at the top
        write!(serial, "> ").expect("write error:cmd_prompt");
    }
}
