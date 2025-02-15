## O-QPSK on backscatter platform in rust

### To Use:

1. Make sure you have rust installed and all the dependencies for the project
    1. [to install rust](https://www.rust-lang.org/tools/install)
    2. to have everything up to date run
       ```bash
       rustup self update
       rustup update stable
       rustup target add thumbv6m-none-eabi
       ```
    3. `cargo install flip-link`
    4. install uv (python runner for serial, or you can use another serial monitor but sometimes the format is wonky for firefly)
    5. `cargo install --git https://github.com/astral-sh/uv uv`
2. Either set up pico probe or other
   debugger (only way to get text output) or enable to uf2 loader in `.cargo/config`
3. run `cargo run` to build and flash the code
4. set up the launch pad in 802.15.4 mode in TI Smart RF Studio 7
5. go to packet rx
6. set frequncy to 2460 MHz (Channel 22)
7. plug in firefly, connect with this kind of command (usb device is usually COM-n on windows  and like /dev/tty.usbserial-14440 on mac/linux)
    ```
    uvx --from pyserial pyserial-miniterm -e <usb-device> 115200
    ```
   
8. type `r` and hit enter to restart the firefly / clear configuration
9. type `f 2452`and hit enter to set the carier to 2452 MHz
10. type `a` and hit enter to start the carier
11. on Smart RF studio click start and you should see some packets
12. make sure that the firefly is within 6-10cm of the backscatter board

