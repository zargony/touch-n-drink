# Touch 'n Drink Firmware

Firmware images are automatically build by GitHub actions. They can be downloaded as artifacts of recent [CI][actions] runs <!-- eventually: on the [releases] page --> and flashed [from your browser][esptool-js]. The setup described below is only needed for local development.

## Requirements

For local development, you need a [Rust] toolchain. If your OS doesn't already provide it, easiest way to install and manage Rust toolchains is to use [rustup]. Alternatively, you can run Rust using the official Docker image `rust:1`.

To flash the firmware to a device, the `espflash` tool can be used. It integrates well with the Rust build tools and can be installed with `cargo install espflash cargo-espflash`.

## Building the Firmware

Use Rust's build tool `cargo` to build the firmware:

```sh
cargo build --release
```

## Flash to Device

To flash the firmware to a device, connect the device via its USB-C serial port and use `espflash`:

```sh
cargo espflash flash --release
```

## Contributions

If you implement changes or features that can be useful for everyone, please fork this repository and open a pull request. Make sure to also update documentation and code comments accordingly and add a high level description of your changes to the changelog. Also make sure that all CI jobs are passing and ideally try flashing and using the firmware image artifact to verify its behaviour.

[actions]: https://github.com/zargony/touch-n-drink/actions
[releases]: https://github.com/zargony/touch-n-drink/releases

[esptool-js]: https://espressif.github.io/esptool-js
[Rust]: https://www.rust-lang.org
[rustup]: https://rustup.rs
