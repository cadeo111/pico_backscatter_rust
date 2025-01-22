use defmt::{info, Format};
// A shorter alias for the Peripheral Access Crate, which provides low-level
// register access
use rp_pico::hal::{clocks, pac};

// A shorter alias for the Hardware Abstraction Layer, which provides
// higher-level drivers.
use rp_pico::hal;

// USB Device support
use usb_device::{class_prelude::*, prelude::*};

// USB Communications Class Device support
use usbd_serial::SerialPort;

pub struct USBSerial<'usb> {
    serial: SerialPort<'usb, hal::usb::UsbBus>,
    device: UsbDevice<'usb, hal::usb::UsbBus>,
}

#[derive(Format)]
pub struct PollBufferError;

impl<'usb> USBSerial<'usb> {
    pub fn new(bus: &'usb UsbBusAllocator<hal::usb::UsbBus>) -> Self {
        // Set up the USB Communications Class Device driver
        let serial: SerialPort<rp_pico::hal::usb::UsbBus> = SerialPort::new(bus);

        // Create a USB device with a fake VID and PID
        let device: UsbDevice<rp_pico::hal::usb::UsbBus> =
            UsbDeviceBuilder::new(bus, UsbVidPid(0x16c0, 0x27dd))
                .strings(&[StringDescriptors::default()
                    .manufacturer("Fake company")
                    .product("Serial port")
                    .serial_number("TEST")])
                .unwrap()
                .device_class(2) // from: https://www.usb.org/defined-class-codes
                .build();
        Self { serial, device }
    }

    pub fn poll(&mut self) -> Option<([u8; 64], usize)> {
        if self.device.poll(&mut [&mut self.serial]) {
            let mut buf = [0u8; 64];
            match self.serial.read(&mut buf) {
                Err(_e) => {
                    // Do nothing
                }
                Ok(0) => {
                    // Do nothing
                }
                Ok(count) => {
                    // Convert to upper case
                    // buf.iter_mut().take(count).for_each(|b| {
                    //     info!("received {:?}", *b as char);
                    //     b.make_ascii_uppercase();
                    // });
                    return Some((buf, count));

                    // Send back to the host
                    // let mut wr_ptr = &buf[..count];
                    // while !wr_ptr.is_empty() {
                    //     info!("writing {:?}", *wr_ptr);
                    //     match self.serial.write(wr_ptr) {
                    //         Ok(len) => wr_ptr = &wr_ptr[len..],
                    //         // On error, just drop unwritten data.
                    //         // One possible error is Err(WouldBlock), meaning the USB
                    //         // write buffer is full.
                    //         Err(_) => break,
                    //     };
                    // }
                }
            }
        }
        None
    }

    // this might not work on windows
    pub fn poll_until_enter<const MAX_BUFFER_SIZE: usize>(
        &mut self,
    ) -> Result<heapless::Vec<u8, MAX_BUFFER_SIZE>, PollBufferError> {
        let mut vec = heapless::Vec::<u8, MAX_BUFFER_SIZE>::new();
        loop {
            if let Some((buffer, num_of_chars)) = self.poll() {
                for (i, item) in buffer.into_iter().take(num_of_chars).enumerate() {
                    // enter key
                    if item == 13 || item == 10 {
                        info!("finished -> {}", vec);
                        return Ok(vec);
                    }
                    info!(
                        "received {:?} = {:?} ({}/{})",
                        item as char,
                        item,
                        i + 1,
                        num_of_chars
                    );
                    vec.push(item).map_err(|_err| PollBufferError {})?;
                }
            }
        }
    }

    //         // This only works reliably because the number of bytes written to
    //         // the serial port is smaller than the buffers available to the USB
    //         // peripheral. In general, the return value should be handled, so that
    //         // bytes not transferred yet don't get lost.
    pub fn write(&mut self, text: &str) {
        let bytes = text.as_bytes();

        if bytes.len() < 64 {
            self.serial
                .write(bytes)
                .expect("failed to write less than 64 bytes");
            return;
        }
        let mut idx = 0;
        while (idx * 64) < bytes.len() {
            let mut slice;
            if bytes[(idx * 64)..].len() >= 64 {
                slice = &bytes[(idx * 64)..((idx * 64) + 64)];
            } else {
                slice = &bytes[(idx * 64)..];
            }
            loop {
                match self.serial.write(slice) {
                    Ok(num_written) => {
                        if num_written == slice.len() {
                            break;
                        }
                        slice = &slice[num_written..];
                    }
                    Err(e) => match e {
                        UsbError::WouldBlock => {
                            info!("USB write buffer full");
                            continue;
                        }
                        _ => {
                            panic!("error sending serial data {:?}", e)
                        }
                    },
                }
            }
            idx += 1;
        }

        if let Err(err) = self.serial.flush() {
            let mut possible_err = Some(err);
            while let Some(err) = possible_err {
                match err {
                    UsbError::WouldBlock => {
                        let res = self.serial.flush();
                        if res.is_ok() {
                            possible_err = None;
                        } else if let Err(err) = res {
                            possible_err = Some(err);
                        }
                    }
                    _ => {
                        panic!("failed flushing serial data to usb {:?}", err)
                    }
                }
            }
        }
    }
}

pub fn init_usb_bus(
    pac_usbctrl_regs: pac::USBCTRL_REGS,
    pac_usbctrl_dpram: pac::USBCTRL_DPRAM,
    clocks_usb_clock: clocks::UsbClock,
    pac_resets: &mut pac::RESETS,
) -> UsbBusAllocator<hal::usb::UsbBus> {
    // Set up the USB driver
    UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac_usbctrl_regs,
        pac_usbctrl_dpram,
        clocks_usb_clock,
        true,
        pac_resets,
    ))
}

//
//
// pub fn set_up_usb_serial<'usb>(
//
// ) -> USBSerial<'usb> {
//
//
//     // Set up the USB Communications Class Device driver
//     let serial: SerialPort<rp_pico::hal::usb::UsbBus> = SerialPort::new(&bus);
//
//     // Create a USB device with a fake VID and PID
//     let device: UsbDevice<rp_pico::hal::usb::UsbBus> = UsbDeviceBuilder::new(&bus, UsbVidPid(0x16c0, 0x27dd))
//         .strings(&[StringDescriptors::default()
//             .manufacturer("Fake company")
//             .product("Serial port")
//             .serial_number("TEST")])
//         .unwrap()
//         .device_class(2) // from: https://www.usb.org/defined-class-codes
//         .build();
//
//     return USBSerial { bus, serial, device };
//
//     // let mut said_hello = false;
//     // loop {
//     //     // A welcome message at the beginning
//     //     if !said_hello && timer.get_counter().ticks() >= 2_000_000 {
//     //         said_hello = true;
//     //         let _ = serial.write(b"Hello, World!\r\n");
//     //
//     //         let time = timer.get_counter().ticks();
//     //         let mut text: String<64> = String::new();
//     //         writeln!(&mut text, "Current timer ticks: {}", time).unwrap();
//     //
//     //         // This only works reliably because the number of bytes written to
//     //         // the serial port is smaller than the buffers available to the USB
//     //         // peripheral. In general, the return value should be handled, so that
//     //         // bytes not transferred yet don't get lost.
//     //         let _ = serial.write(text.as_bytes());
//     //     }
//     //
//     //     // Check for new data
//     //     if usb_dev.poll(&mut [&mut serial]) {
//     //         let mut buf = [0u8; 64];
//     //         match serial.read(&mut buf) {
//     //             Err(_e) => {
//     //                 // Do nothing
//     //             }
//     //             Ok(0) => {
//     //                 // Do nothing
//     //             }
//     //             Ok(count) => {
//     //                 // Convert to upper case
//     //                 buf.iter_mut().take(count).for_each(|b| {
//     //                     b.make_ascii_uppercase();
//     //                 });
//     //                 // Send back to the host
//     //                 let mut wr_ptr = &buf[..count];
//     //                 while !wr_ptr.is_empty() {
//     //                     match serial.write(wr_ptr) {
//     //                         Ok(len) => wr_ptr = &wr_ptr[len..],
//     //                         // On error, just drop unwritten data.
//     //                         // One possible error is Err(WouldBlock), meaning the USB
//     //                         // write buffer is full.
//     //                         Err(_) => break,
//     //                     };
//     //                 }
//     //             }
//     //         }
//     //     }
//     // }
// }
