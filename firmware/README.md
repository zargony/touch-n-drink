# Touch 'n Drink Firmware

Firmware images are automatically build by GitHub actions. They can be downloaded from the [releases] page and flashed to a microcontroller [from your browser][esptool-js]. The setup described below is only needed for local development.

## Requirements

For local development, you need a [Rust] toolchain. If your OS doesn't already provide it, easiest way to install and manage Rust toolchains is to use [rustup], or use the official Docker image `rust:1`.

The Rust toolchain needs to support cross-compiling to the microcontroller's target architecture. If you're using `rustup`, it'll automatically download the required components as specified in `rust-toolchain.toml`.

To flash the firmware to a device, the `espflash` and `cargo-espflash` tools can be used. They integrate well with Rust build tools and can be installed with `cargo install espflash cargo-espflash`.

## Building the Firmware

To build the firmware using the debug profile, use:

```sh
cargo xtask build
```

This will run `cargo build` with options to build a firmware image for the microcontroller. It'll then use `espflash save-image` to produce OTA and factory images (.bin files) that can be flashed to the microcontroller.

## Flash Firmware to Device

To build the firmware using the debug profile and flash it to a device, connect the device via its USB-C serial port and use:

```sh
cargo xtask run
```

This will build the firmware image and use `espflash` to flash the produced image to the microcontroller. It then resets the device and monitors any log output.

## Flash Configuration to Device

Configuration is stored in a separate flash partition and is therefore unaffected by firmware updates. As there is currently no way to change the configuration at runtime, it needs to be flashed to the device manually (once).

Create a custom configuration, e.g. `config.json`. See `config-example.json` for available settings. Before flashing, keep it small and remove all comments, either manually or by using the `jq` tool:

```sh
jq -c < config.json > config.min.json
```

Store the minimized configuration to the device's `config` partition using `espflash`:

```sh
espflash write-bin 0xd000 config.min.json
```

## Contributions

If you implement changes or features that can be useful for everyone, please fork this repository and open a pull request. Make sure to also update documentation and code comments accordingly and add a high level description of your changes to the changelog. Also make sure that all CI jobs are passing and ideally try flashing and using the firmware image artifact to verify its behaviour.

[actions]: https://github.com/zargony/touch-n-drink/actions
[releases]: https://github.com/zargony/touch-n-drink/releases

[esptool-js]: https://espressif.github.io/esptool-js
[Rust]: https://www.rust-lang.org
[rustup]: https://rustup.rs
