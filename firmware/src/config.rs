use crate::json::{self, FromJson, FromJsonObject};
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

impl FromJson for SensitiveString {
    async fn from_json<R: BufRead>(
        reader: &mut json::Reader<R>,
    ) -> Result<Self, json::Error<R::Error>> {
        Ok(Self(reader.read().await?))
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
    /// Vereinsflieger API username
    pub vf_username: String,
    /// MD5 (hex) of Vereinsflieger API password
    pub vf_password_md5: SensitiveString,
    /// Vereinsflieger API appkey
    pub vf_appkey: SensitiveString,
    /// Vereinsflieger API cid (optional)
    pub vf_cid: Option<u32>,
    /// Vereinsflieger article id for purchase
    pub vf_article_id: u32,
}

impl FromJsonObject for Config {
    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        reader: &mut json::Reader<R>,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "wifi-ssid" => self.wifi_ssid = reader.read().await?,
            "wifi-password" => self.wifi_password = reader.read().await?,
            "vf-username" => self.vf_username = reader.read().await?,
            "vf-password-md5" => self.vf_password_md5 = reader.read().await?,
            "vf-appkey" => self.vf_appkey = reader.read().await?,
            "vf-cid" => self.vf_cid = Some(reader.read().await?),
            "vf-article-id" => self.vf_article_id = reader.read().await?,
            _ => _ = reader.read_any().await?,
        }
        Ok(())
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
