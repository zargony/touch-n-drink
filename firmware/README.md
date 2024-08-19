# Touch 'n Drink Firmware

Firmware images are automatically build by GitHub actions. They can be downloaded as artifacts of recent [CI][actions] runs <!-- eventually: on the [releases] page --> and flashed [from your browser][esptool-js]. The setup described below is only needed for local development.

## Requirements

### Rust Toolchain

If your OS doesn't provide a Rust compiler, easiest way to install and manage Rust toolchains is to install **rustup** (<https://rustup.rs>). It installs the latest stable Rust toolchain which can later be updated by running `rustup update`.

### espflash

To flash the firmware to a device, the `espflash` tool can be used. It integrates well with the Rust build tools. To install it, run:

```sh
cargo install espflash cargo-espflash
```

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

[actions]: https://github.com/zargony/touch-n-drink/actions
[esptool-js]: https://espressif.github.io/esptool-js/
[releases]: https://github.com/zargony/touch-n-drink/releases
