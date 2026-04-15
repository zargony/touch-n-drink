use anyhow::anyhow;
use clap::ValueEnum;
use clap::builder::PossibleValue;
use std::str::FromStr;

// TODO: Instead of defining these ourselves, use esp_metadata::Chip? (implements ValueEnum)
#[rustfmt::skip]
pub const ALL_CHIPS: [Chip; 1] = [
    Chip { name: "esp32c3", package: "esp32", target: "riscv32imc-unknown-none-elf" },
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
    // /// Features to use to build for this chip
    // pub features: &'a [&'a str],
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
