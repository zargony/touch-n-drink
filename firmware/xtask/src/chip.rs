use anyhow::anyhow;
use clap::ValueEnum;
use clap::builder::PossibleValue;
use std::str::FromStr;

// TODO: Instead of defining these ourselves, use esp_metadata::Chip? (implements ValueEnum)
#[rustfmt::skip]
pub const ALL_CHIPS: [Chip; 9] = [
    Chip { name: "esp32", package: "touch-n-drink-esp32", target: "xtensa-esp32-none-elf", feature: "esp32" },
    Chip { name: "esp32c2", package: "touch-n-drink-esp32", target: "riscv32imc-unknown-none-elf", feature: "esp32c2" },
    Chip { name: "esp32c3", package: "touch-n-drink-esp32", target: "riscv32imc-unknown-none-elf", feature: "esp32c3" },
    Chip { name: "esp32c5", package: "touch-n-drink-esp32", target: "riscv32imac-unknown-none-elf", feature: "esp32c5" },
    Chip { name: "esp32c6", package: "touch-n-drink-esp32", target: "riscv32imac-unknown-none-elf", feature: "esp32c6" },
    Chip { name: "esp32c61", package: "touch-n-drink-esp32", target: "riscv32imac-unknown-none-elf", feature: "esp32c61" },
    Chip { name: "esp32h2", package: "touch-n-drink-esp32", target: "riscv32imac-unknown-none-elf", feature: "esp32h2" },
    Chip { name: "esp32s2", package: "touch-n-drink-esp32", target: "xtensa-esp32s2-none-elf", feature: "esp32s2" },
    Chip { name: "esp32s3", package: "touch-n-drink-esp32", target: "xtensa-esp32s3-none-elf", feature: "esp32s3" },
];

pub const DEFAULT_CHIP: &str = "esp32c3";

/// Chip configuration (determines package, target and features to use)
#[derive(Debug, Clone)]
pub struct Chip<'a> {
    /// Name of the chip (firmware variant), for use with `--chip name` options
    pub name: &'a str,
    /// Package to build for this chip
    pub package: &'a str,
    /// Target triple to use to build for this chip
    pub target: &'a str,
    /// Feature to use to build for this chip
    pub feature: &'a str,
}

impl FromStr for Chip<'_> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        ALL_CHIPS
            .iter()
            .find(|chip| chip.name == s)
            .cloned()
            .ok_or_else(|| anyhow!("Unknown chip"))
    }
}

impl ValueEnum for Chip<'static> {
    fn value_variants<'a>() -> &'a [Self] {
        &ALL_CHIPS
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.name))
    }
}

impl Chip<'_> {
    pub fn cargo_args(&self) -> [&str; 7] {
        [
            "--package",
            self.package,
            "--target",
            self.target,
            "--no-default-features",
            "--features",
            self.feature,
        ]
    }
}
