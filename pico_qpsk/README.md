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
3. Either set up pico probe or other
   debugger (only way to get text output) or enable to uf2 loader in `.cargo/config`
3. run `cargo run` to build and flash the code
6. set up the launch pad in 802.15.4 mode in TI Smart RF Studio 7
2. go to packet rx
3. set frequncy to 2460 MHz (Channel 22)
4. plug in firefly, connect with this kind of command
    ```
    bash python3 -m  serial.tools.miniterm -e /dev/tty.usbserial-14410 115200
    ```
5. type `r` and hit enter to restart the firefly / clear configuration
6. type `f 2452`and hit enter to set the carier to 2452 MHz
7. type `a` and hit enter to start the carier
8. on Smart RF studio click start and you should see some packets
9. make sure that the firefly is within 6-10cm of the backscatter board

