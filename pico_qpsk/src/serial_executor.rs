use crate::pio_helpers::{get_seq_frame_bytes, PioControl, StandardTransmitOption};
use crate::to_max_frame_size;
use crate::usb_serial::USBSerial;
use core::fmt::Write;
use cortex_m::delay::Delay;
use defmt::info;
use heapless::{String, Vec};
use owo_colors::{colors::*, OwoColorize, XtermColors};
use rp_pico::hal::gpio::PullDown;
use rp_pico::hal::pio::{Tx, SM0};
use rp_pico::hal::reset;
use rp_pico::pac::PIO0;

enum Command {
    Restart,
    Help,
    SendGenericPacket { interval_ms: u32, number_packets: u32 },
}

enum CommandError<'a> {
    UnknownCommand(&'a str),
    UnknownError,
    ArgsError(&'static str),
}

impl Command {
    fn from_str<const SIZE: usize>(input: &String<SIZE>) -> Result<Command, CommandError> {
        let mut iter = input.split_whitespace();
        match iter.next().ok_or(CommandError::UnknownError)? {
            "restart" => Ok(Self::Restart),
            "help" => Ok(Self::Help),
            "sgp" => {
                let interval_ms_str = iter.next().ok_or(CommandError::UnknownError)?;

                let interval_ms = if interval_ms_str.ends_with("ms") {
                    let slice = &interval_ms_str[0..interval_ms_str.len() - 2];
                    slice
                        .parse()
                        .map_err(|_| CommandError::ArgsError("interval_ms ms"))?
                } else if interval_ms_str.ends_with("s") {
                    let slice = &interval_ms_str[0..interval_ms_str.len() - 1];
                    let secs: u32 = slice
                        .parse()
                        .map_err(|_| CommandError::ArgsError("interval_ms s"))?;
                    secs * 1000
                } else {
                    interval_ms_str
                        .parse()
                        .map_err(|_| CommandError::ArgsError("interval_ms"))?
                };

                let number_packets = iter
                    .next()
                    .ok_or(CommandError::UnknownError)?
                    .parse()
                    .map_err(|_| CommandError::ArgsError("number_packets"))?;

                Ok(Self::SendGenericPacket {
                    interval_ms,
                    number_packets,
                })
            }
            _ => Err(CommandError::UnknownCommand(input.as_str())),
        }
    }
}

fn help(serial: &mut USBSerial) {
    writeln!(serial, "{}", "Available commands:".fg::<Green>()).expect("write error:help");
    writeln!(serial, "\t{}", "- restart > restart device".fg::<Green>()).expect("write error:help");
    writeln!(serial, "\t{}", "- help > this menu".fg::<Green>()).expect("write error:help");
    writeln!(serial, "\t{}", "- sgp <interval> <number_packets>".fg::<Green>()).expect("write error:help");
    writeln!(
        serial,
        "\t\t{}",
        "- interval: interval between packets in millisecond or seconds (1s 1000ms or 1000)".fg::<Green>()
    )
    .expect("write error:help");
    writeln!(
        serial,
        "\t\t{}",
        "- number_packets: number of packets to send".fg::<Green>()
    )
    .expect("write error:help");
    writeln!(serial, "\t\t{}", "Example: sgp 100 1000".fg::<Green>()).expect("write error:help");
    writeln!(
        serial,
        "\t\t{}",
        "This will send 1000 packets with an interval of 100ms".fg::<Green>()
    )
    .expect("write error:help");
    writeln!(
        serial,
        "\t\t{}",
        "This command is not implemented yet".fg::<Green>()
    )
    .expect("write error:help");
}
fn restart() {
    reset();
}
fn send_generic_packet(
    serial: &mut USBSerial,
    delay: &mut Delay,
    interval_ms: u32,
    number_packets: u32,
    tx: &mut Tx<(PIO0, SM0)>,
    pio_ctrl: &mut PioControl<PIO0, PullDown>,
    transmit_option: &StandardTransmitOption,
) {
    let frame_bytes = get_seq_frame_bytes::<5, { to_max_frame_size!(5) }>();
    let iter_frame = transmit_option.convert(&frame_bytes);

    writeln!(serial, "sending generic packet... (Not implemented yet)")
        .expect("write error:send_generic_packet");
    writeln!(
        serial,
        "interval_ms: {}, number_packets: {}",
        interval_ms, number_packets
    )
    .expect("write error:send_generic_packet");

    let mut buffer: Vec<u32, 4000> = Vec::new();
    buffer.extend(iter_frame.clone());
    pio_ctrl.start();
    for i in 0..number_packets {
        info!("sending gen packet from frame... {}/{} ", i + 1, number_packets);
        writeln!(
            serial,
            "{}{}/{}",
            "sending gen packet from frame...".fg::<Yellow>(),
            (i + 1),
            number_packets
        )
        .expect("write error:send_generic_packet");
        for i in &buffer {
            while tx.is_full() {}
            tx.write(*i);
        }
        delay.delay_ms(interval_ms);
        if serial.poll_is_etx() {
            pio_ctrl.stop();
            writeln!(
                serial,
                "{}",
                "Exited Early!"
                    .color(XtermColors::White)
                    .on_color(XtermColors::BlazeOrange)
                    .italic()
            )
            .expect("write error:send_generic_packet");

            return;
        }
    }
    pio_ctrl.stop();
    info!("stopped sending packets");
    writeln!(serial, "{}", "Done!".fg::<Green>()).expect("write error:send_generic_packet");
}

pub fn executor(
    serial: &mut USBSerial,
    delay: &mut Delay,
    tx: &mut Tx<(PIO0, SM0)>,
    pio_ctrl: &mut PioControl<PIO0, PullDown>,
    base_transmit_option: &StandardTransmitOption,
) -> ! {
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
                Command::SendGenericPacket {
                    interval_ms,
                    number_packets,
                } => {
                    send_generic_packet(
                        serial,
                        delay,
                        interval_ms,
                        number_packets,
                        tx,
                        pio_ctrl,
                        base_transmit_option,
                    );
                }
            },
            Err(err) => match err {
                CommandError::UnknownError => {
                    writeln!(serial, "{}", "unknown command error, try help".fg::<Red>())
                        .expect("write error:UnknownError");
                }
                CommandError::ArgsError(arg_name) => {
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
