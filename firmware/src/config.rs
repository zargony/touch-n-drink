use crate::article::ArticleId;
use crate::json::{self, FromJson, FromJsonObject};
use alloc::string::String;
use core::fmt;
use core::ops::Deref;
use embedded_io_async::BufRead;
use embedded_storage::ReadStorage;
use esp_partition_table::{PartitionTable, PartitionType};
use esp_storage::FlashStorage;
use log::{debug, info, warn};

/// String with sensitive content (debug and display output redacted)
#[derive(Default)]
pub struct SensitiveString(String);

impl FromJson for SensitiveString {
    async fn from_json<R: BufRead>(
        json: &mut json::Reader<R>,
    ) -> Result<Self, json::Error<R::Error>> {
        Ok(Self(json.read().await?))
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
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// System configuration
///
/// System configuration is stored in the `config` flash data partition, so it stays unaffected by
/// firmware updates via USB or OTA. Currently, configuration is read-only at runtime, i.e. it
/// needs to be flashed manually once per device. To make this easier, it is expected to be stored
/// in JSON format in the `config` data partition. See README.md for details on how to flash the
/// configuration.
///
/// If there is no valid JSON or no valid `config` data partition, a default configuration is
/// provided (which isn't very useful, but at least doesn't prevent the device from starting).
#[derive(Debug, Default)]
pub struct Config {
    /// Wifi SSID to connect to
    pub wifi_ssid: String,
    /// Wifi password
    pub wifi_password: SensitiveString,
    /// Mixpanel project token for analytics (optional)
    pub mp_token: Option<String>,
    /// Vereinsflieger API username
    pub vf_username: String,
    /// MD5 (hex) of Vereinsflieger API password
    pub vf_password_md5: SensitiveString,
    /// Vereinsflieger API appkey
    pub vf_appkey: SensitiveString,
    /// Vereinsflieger API cid (optional)
    pub vf_cid: Option<u32>,
    /// Vereinsflieger article id for purchase
    pub vf_article_id: ArticleId,
}

impl FromJsonObject for Config {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        match &*key {
            "wifi-ssid" => self.wifi_ssid = json.read().await?,
            "wifi-password" => self.wifi_password = json.read().await?,
            // Don't use telemetry in debug builds, unless explicitly specified
            #[cfg(not(debug_assertions))]
            "mp-token" => self.mp_token = Some(json.read().await?),
            #[cfg(debug_assertions)]
            "mp-token-debug" => self.mp_token = Some(json.read().await?),
            "vf-username" => self.vf_username = json.read().await?,
            "vf-password-md5" => self.vf_password_md5 = json.read().await?,
            "vf-appkey" => self.vf_appkey = json.read().await?,
            "vf-cid" => self.vf_cid = Some(json.read().await?),
            "vf-article-id" => self.vf_article_id = json.read().await?,
            _ => _ = json.read_any().await?,
        }
        Ok(())
    }
}

impl Config {
    /// Read configuration from `config` flash data partition
    pub async fn read() -> Self {
        let mut storage = FlashStorage::new();

        // Read partition table (at 0x8000 by default)
        let table = PartitionTable::default();
        debug!("Config: Reading partition table at 0x{:x}", table.addr);

        // Look up config data partition (custom partition type 0x54, subtype 0x44)
        let config_offset = if let Some(offset) = table
            .iter_storage(&mut storage, false)
            .flatten()
            .find(|partition| partition.type_ == PartitionType::User(0x54, 0x44))
            .map(|partition| partition.offset)
        {
            debug!("Config: Found config partition at offset 0x{:x}", offset);
            offset
        } else {
            warn!("Config: Unable to find config partition");
            return Self::default();
        };

        // Read first sector (4 kb) of config data partition
        let mut bytes = [0; FlashStorage::SECTOR_SIZE as usize];
        if let Err(_err) = storage.read(config_offset, &mut bytes) {
            warn!("Config: Unable to read config partition");
            return Self::default();
        }

        // Parse JSON config
        let config = match json::Reader::new(&bytes[..]).read().await {
            Ok(config) => config,
            Err(err) => {
                warn!(
                    "Config: Unable to parse configuration in config partition: {}",
                    err
                );
                return Self::default();
            }
        };

        debug!("Config: System configuration: {:?}", config);
        info!("Config: Configuration loaded from config partition");
        config
    }
}
