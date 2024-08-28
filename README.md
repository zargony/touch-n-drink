# Touch 'n Drink

Touch 'n Drink is a small device that reads NFC id cards of club members of the [Aeroclub (LSC) Hamm][LSC Hamm] and allows to pay for items like cold drinks offered at the clubhouse. Purchases are forwarded to accounting so members pay via their regular monthly invoices. We're located at airfield [EDLH] in Hamm, Germany.

<img alt="Assembled device" src="images/hardware-assembled.jpeg" style="width: 20em;" />

## Hardware

ESP32-C3, Keypad, OLED Display and NFC reader in a custom acrylic case. See [hardware] folder for details.

## Firmware

Written in [Rust]. See [firmware] folder for details.

## Contributions

If you like this project, want to use it at your club, or if you want to discuss ideas and suggestions, feel free to start a [discussion][discussions] or open an [issue][issues]. Feel free to fork this repository and base your work upon it. If you implement changes or features that can be useful for everyone, please open a pull request.

## License

Hardware licensed under [CERN Open Hardware License Version 2 (Permissive)]. Software licensed under either of [Apache License 2.0] or [MIT License], at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.

[hardware]: ./hardware
[firmware]: ./firmware

[discussions]: https://github.com/zargony/touch-n-drink/discussions
[issues]: https://github.com/zargony/touch-n-drink/issues

[LSC Hamm]: https://flugplatz-hamm.de
[EDLH]: https://skyvector.com/airport/EDLH/Hamm-Lippewiesen-Airport
[Rust]: https://rust-lang.org

[Apache License 2.0]: https://opensource.org/license/apache-2-0
[CERN Open Hardware License Version 2 (Permissive)]: https://opensource.org/license/cern-ohl-p
[MIT License]: https://opensource.org/license/mit
