# Touch 'n Drink

Touch 'n Drink is a small device that reads NFC id cards of club members of the [Aeroclub (LSC) Hamm][LSC Hamm] and allows to pay for items like cold drinks offered at the clubhouse. Purchases are forwarded to accounting so members pay via their regular monthly invoices. We're located at airfield [EDLH] in Hamm, Germany.

<img alt="Assembled device" src="images/device-assembled.jpg" style="width: 20em;" />

## Features

- Fetches authorized users and list of articles for sale from [Vereinsflieger] API
- Allows users to identify with NFC tag or id card and purchase articles
- Optional event tracking using [Mixpanel] for usage analytics
- Connects to 2.4 GHz WPA2/WPA3 Wifi (IPv4, DHCP)
- Simple numeric keypad and large, bright OLED display
- Power supply using standard USB-C cable (PD not required)
- Ergonomically priced and widely available hardware components
- Magnetic wall mount

## Hardware

ESP32-C3, Keypad, OLED Display and NFC reader in a custom acrylic case. See [hardware] folder for details.

## PCB

Small custom PCB for the microcontroller and connectors to other components. Either manufactured or manually soldered to a perfboard. See [pcb] folder for details.

## Firmware

Written in [Rust]. See [firmware] folder for details.

## Contributions

If you like this project, want to use it at your club, or if you want to discuss ideas and suggestions, feel free to start a [discussion][discussions] or open an [issue][issues]. Feel free to fork this repository and base your work upon it. Please open a pull request if your changes or features are useful to a broad audience.

## License

Hardware licensed under the [CERN Open Hardware License (Strongly Reciprocal)][OHL]. Software licensed under the [European Union Public License (EUPL)][EUPL]. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, shall be licensed as above, without any additional terms or conditions.

[hardware]: ./hardware
[firmware]: ./firmware
[pcb]: ./pcb

[discussions]: https://github.com/zargony/touch-n-drink/discussions
[issues]: https://github.com/zargony/touch-n-drink/issues

[EDLH]: https://skyvector.com/airport/EDLH/Hamm-Lippewiesen-Airport
[EUPL]: https://interoperable-europe.ec.europa.eu/collection/eupl/eupl-text-eupl-12
[LSC Hamm]: https://flugplatz-hamm.de
[Mixpanel]: https://mixpanel.com
[OHL]: https://cern-ohl.web.cern.ch/home
[Rust]: https://www.rust-lang.org
[Vereinsflieger]: https://www.vereinsflieger.de
