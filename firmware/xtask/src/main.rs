use anyhow::{Error, Result, anyhow};
use clap::builder::PossibleValue;
use clap::{Parser, ValueEnum};
use clap_cargo::style::{CLAP_STYLING, ERROR, NOTE};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fs, process};
use xshell::{Shell, cmd};

// TODO: Instead of defining these ourselves, use esp_metadata::Chip? (implements ValueEnum)
#[rustfmt::skip]
const CHIPS: [Chip; 1] = [
    Chip { name: "esp32c3", package: "esp32", target: "riscv32imc-unknown-none-elf" },
];

const DEFAULT_CHIP: &str = "esp32c3";

fn main() {
    if let Err(err) = Cli::parse().command.run() {
        eprintln!("\n{ERROR}Error: {err:#} {ERROR:#}\n");
        process::exit(-1);
    }
}

#[derive(Debug, clap::Parser)]
#[command(
    bin_name = "cargo xtask",
    arg_required_else_help = true,
    styles(CLAP_STYLING)
)]
struct Cli {
    /// Path to cargo binary
    #[arg(
        long,
        env,
        hide_env_values = true,
        global = true,
        help_heading = "Global Options",
        default_value = "cargo"
    )]
    cargo: PathBuf,

    /// Path to espflash binary
    #[arg(
        long,
        env,
        hide_env_values = true,
        global = true,
        help_heading = "Global Options",
        default_value = "espflash"
    )]
    espflash: PathBuf,

    #[command(subcommand)]
    command: Command,
}

/// Chip configuration (determines package, target and features to use)
#[derive(Debug, Clone)]
struct Chip<'a> {
    /// Name of the chip, for use with `--chip name` options
    name: &'a str,
    /// Package to build for this chip
    package: &'a str,
    /// Target triple to use to build for this chip
    target: &'a str,
    // /// Features to use to build for this chip
    // features: &'a [&'a str],
}

impl FromStr for Chip<'_> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        CHIPS
            .iter()
            .find(|chip| chip.name == s)
            .cloned()
            .ok_or_else(|| anyhow!("Unknown chip"))
    }
}

impl ValueEnum for Chip<'static> {
    fn value_variants<'a>() -> &'a [Self] {
        &CHIPS
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.name))
    }
}

fn shell(cargo: &Path) -> Result<Shell> {
    let sh = Shell::new()?;

    let workspace_cargo_toml: PathBuf = cmd!(
        sh,
        "{cargo} locate-project --workspace --message-format plain"
    )
    .read()?
    .into();
    let workspace_root = workspace_cargo_toml
        .parent()
        .ok_or_else(|| anyhow!("Unable to determine workspace root"))?;

    sh.change_dir(workspace_root);
    Ok(sh)
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Print build information
    #[command(subcommand)]
    Print(PrintCommand),

    /// Run code checks
    ///
    /// Checks code to catch common mistakes.
    Clippy(ClippyArgs),

    /// Build firmware
    ///
    /// Builds firmware for the selected chip. Produces OTA and factory .bin files and sha256 sums.
    Build(BuildArgs),

    /// Build, flash and run firmware on an attached device
    ///
    /// Builds firmware for the selected chip, flashes it to the locally attached device and
    /// monitors its log output.
    Run(RunArgs),
}

impl Command {
    fn run(self) -> Result<()> {
        match self {
            Command::Print(print) => {
                print.run();
                Ok(())
            }
            Command::Clippy(clippy) => clippy.run(),
            Command::Build(build) => build.run(),
            Command::Run(run) => run.run(),
        }
    }
}

/// Print command
#[derive(Debug, clap::Subcommand)]
enum PrintCommand {
    /// Print list of chips available for building
    Chips,

    /// Print package to use for the given chip
    Package {
        #[arg(long, value_enum)]
        chip: Chip<'static>,
    },

    /// Print target triple to use for the given chip
    Target {
        #[arg(long, value_enum)]
        chip: Chip<'static>,
    },
}

impl PrintCommand {
    fn run(self) {
        match self {
            Self::Chips => {
                for chip in CHIPS {
                    println!("{}", chip.name);
                }
            }
            Self::Package { chip } => println!("{}", chip.package),
            Self::Target { chip } => println!("{}", chip.target),
        }
    }
}

/// Clippy command arguments
#[derive(Debug, clap::Args)]
struct ClippyArgs {
    /// The chip to check for
    #[arg(long, value_enum, default_value = DEFAULT_CHIP)]
    chip: Chip<'static>,

    #[arg(from_global)]
    cargo: PathBuf,

    /// Additional arguments forwarded to `cargo clippy`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

impl ClippyArgs {
    fn run(self) -> Result<()> {
        let sh = shell(&self.cargo)?;
        let chip = self.chip.name;
        let package = self.chip.package;
        let target = self.chip.target;
        let cargo = self.cargo;
        let args = &self.args;

        // Cargo clippy
        println!("{NOTE}    Checking{NOTE:#} code for {chip}",);
        cmd!(
            sh,
            "{cargo} clippy --package touch-n-drink-{package} --target {target} -- {args...}"
        )
        .run()?;

        Ok(())
    }
}

/// Build command arguments
#[derive(Debug, clap::Args)]
struct BuildArgs {
    /// The chip to build for
    #[arg(long, value_enum, default_value = DEFAULT_CHIP)]
    chip: Chip<'static>,

    #[arg(from_global)]
    cargo: PathBuf,
    #[arg(from_global)]
    espflash: PathBuf,

    /// Additional arguments forwarded to `cargo build`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

fn generate_sha256(path: &Path) -> Result<()> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    let digest = loop {
        match file.read(&mut buffer)? {
            0 => break hasher.finalize(),
            len => hasher.update(&buffer[..len]),
        }
    };
    let mut file = fs::File::create(path.with_extension("sha256"))?;
    for b in digest {
        write!(file, "{b:02x}")?;
    }
    writeln!(file, "  {}", path.file_name().unwrap().display())?;
    Ok(())
}

impl BuildArgs {
    fn profile(&self) -> &'static str {
        // TODO: Also support `--profile=foo`
        match self.args.iter().position(|arg| arg == "--release") {
            Some(_) => "release",
            None => "debug",
        }
    }

    fn run(self) -> Result<()> {
        let sh = shell(&self.cargo)?;
        let profile = self.profile();
        let chip = self.chip.name;
        let package = self.chip.package;
        let target = self.chip.target;
        let cargo = self.cargo;
        let espflash = self.espflash;
        let args = &self.args;

        // Find out target dir
        let target_dir: PathBuf = match sh.var_os("CARGO_TARGET_DIR") {
            Some(dir) => dir.into(),
            None => "target".into(),
        };

        // Cargo build
        println!("{NOTE}    Building{NOTE:#} firmware image for {chip}",);
        cmd!(
            sh,
            "{cargo} build --package touch-n-drink-{package} --target {target} {args...}"
        )
        .run()?;

        // Create target directory for images
        let target_image_dir = target_dir.join(profile).join("images");
        sh.create_dir(&target_image_dir)?;

        // Copy ELF file
        let elf_file = target_image_dir.join(format!("touch-n-drink-{chip}.elf"));
        sh.copy_file(
            target_dir
                .join(target)
                .join(profile)
                .join(format!("touch-n-drink-{package}")),
            &elf_file,
        )?;

        // Generate OTA image
        let ota_image = target_image_dir.join(format!("touch-n-drink-{chip}.bin"));
        cmd!(
            sh,
            "{espflash} save-image --chip {chip} {elf_file} {ota_image}"
        )
        .run()?;
        generate_sha256(&ota_image)?;
        println!(
            "{NOTE}       Image{NOTE:#} for {chip}, firware only (OTA): {}",
            ota_image.display()
        );

        // Generate factory image
        let factory_image = target_image_dir.join(format!("touch-n-drink-{chip}.factory.bin"));
        cmd!(
            sh,
            "{espflash} save-image --chip {chip} --merge --skip-padding --partition-table {package}/partitions.csv {elf_file} {factory_image}"
        )
        .run()?;
        generate_sha256(&factory_image)?;
        println!(
            "{NOTE}       Image{NOTE:#} for {chip}, with bootloader (factory): {}",
            factory_image.display()
        );

        Ok(())
    }
}

/// Run command arguments
#[derive(Debug, clap::Args)]
struct RunArgs {
    /// The chip to build for
    #[arg(long, value_enum, default_value = DEFAULT_CHIP)]
    chip: Chip<'static>,

    #[arg(from_global)]
    cargo: PathBuf,
    #[arg(from_global)]
    espflash: PathBuf,

    /// Additional arguments forwarded to `cargo run`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

impl RunArgs {
    fn run(self) -> Result<()> {
        let sh = shell(&self.cargo)?;
        let chip = self.chip.name;
        let package = self.chip.package;
        let target = self.chip.target;
        let cargo = self.cargo;
        let args = &self.args;

        // Cargo run
        println!("{NOTE}     Running{NOTE:#} firmware image for {chip}",);
        cmd!(
            sh,
            "{cargo} run --package touch-n-drink-{package} --target {target} {args...}"
        )
        .run()?;

        Ok(())
    }
}
