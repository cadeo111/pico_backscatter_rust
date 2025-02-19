connect firefly, pico and ti board to windows laptop
firefly and pico need to be `bind` -ed and `attach`-ed to WSL
connect to firefly with 
`sudo su`
then in super user
`uvx --from pyserial pyserial-miniterm -e <PORT>  115200`

once in the serial com type `r <ENTER KEY>` 

to set frequency 2452 MHz type `f 2452<ENTER KEY>` 

to start carier generation  type `a <ENTER KEY>` 

for the pico 

do `sudo tio -l`
copy the by id for `usb-Fake_company_Serial_port_TEST`
do `sudo tio <copied id>`
to start packet refelction/transmition
type `ssp 10 100000<ENTER KEY>` this will send 1 packet every 10ms 100,000 times
press ctrl-c to cancel before it has sent all the packets


for usrp

run uhd_usrp_probe.exe to set up the usrp after pluging in
the bind the device with usbipd and then attach it to wsl






