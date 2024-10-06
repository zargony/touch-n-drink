use crate::json::{self, FromJson};
use alloc::string::String;
use core::fmt;
use core::ops::Deref;
use embedded_io_async::BufRead;
use embedded_storage::ReadStorage;
use esp_partition_table::{DataPartitionType, PartitionTable, PartitionType};
use esp_storage::FlashStorage;
use log::{debug, info, warn};

/// String with sensitive content (debug and display output redacted)
#[derive(Default)]
pub struct SensitiveString(String);

impl TryFrom<json::Value> for SensitiveString {
    type Error = json::TryFromValueError;

    fn try_from(value: json::Value) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}

impl fmt::Debug for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            self.0.fmt(f)
        } else {
            "<redacted>".fmt(f)
        }
    }
}

impl fmt::Display for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            self.0.fmt(f)
        } else {
            "<redacted>".fmt(f)
        }
    }
}

impl Deref for SensitiveString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// System configuration
///
/// System configuration is stored in the `nvs` flash partition, so it stays unaffected by firmware
/// updates via USB or OTA. Currently, configuration is read-only at runtime, i.e. it needs to be
/// flashed manually once per device. To make this easier, it is expected to be stored in JSON
/// format at the first sector (4 kb) of the `nvs` flash data partition (this is incompatible with
/// the format that IDF nvs functions expect in this flash partition). See README.md for details
/// on how to flash a configuration.
///
/// If there is no valid JSON or no valid `nvs` data partition, a default configuration is provided
/// (which isn't very useful, but at least doesn't prevent the device from starting).
#[derive(Debug, Default)]
pub struct Config {
    /// Wifi SSID to connect to
    pub wifi_ssid: String,
    /// Wifi password
    pub wifi_password: SensitiveString,
}

impl FromJson for Config {
    async fn from_json<R: BufRead>(
        reader: &mut json::Reader<R>,
    ) -> Result<Self, json::Error<R::Error>> {
        let mut this = Self::default();
        reader
            .read_object(|k, v: json::Value| {
                match &*k {
                    "wifi-ssid" => this.wifi_ssid = v.try_into()?,
                    "wifi-password" => this.wifi_password = v.try_into()?,
                    _ => (),
                }
                Ok(())
            })
            .await?;
        Ok(this)
    }
}

impl Config {
    /// Read configuration from nvs flash partition
    pub async fn read() -> Self {
        let mut storage = FlashStorage::new();

        // Read partition table (at 0x8000 by default)
        let table = PartitionTable::default();
        debug!("Config: Reading partition table at 0x{:x}", table.addr);

        // Look up nvs data partition (at 0x9000 by default)
        let nvs_offset = if let Some(offset) = table
            .iter_storage(&mut storage, false)
            .flatten()
            .find(|partition| partition.type_ == PartitionType::Data(DataPartitionType::Nvs))
            .map(|partition| partition.offset)
        {
            debug!("Config: Found nvs data partition at offset 0x{:x}", offset);
            offset
        } else {
            warn!("Config: Unable to find nvs data partition");
            return Self::default();
        };

        // Read first sector (4 kb) of nvs partition
        let mut bytes = [0; FlashStorage::SECTOR_SIZE as usize];
        if let Err(_err) = storage.read(nvs_offset, &mut bytes) {
            warn!("Config: Unable to read nvs partition");
            return Self::default();
        }

        // Parse JSON config
        let config = match json::Reader::new(&bytes[..]).read().await {
            Ok(config) => config,
            Err(err) => {
                warn!(
                    "Config: Unable to parse configuration in nvs partition: {}",
                    err
                );
                return Self::default();
            }
        };

        debug!("Config: System configuration: {:?}", config);
        info!("Config: Configuration loaded from nvs partition");
        config
    }
}
