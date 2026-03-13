use crate::article::ArticleId;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;
use core::ops::Deref;
use embedded_storage::{ReadStorage, Storage};
use esp_bootloader_esp_idf::partitions;
use log::{debug, info, warn};
use serde::Deserialize;
use serde_with::{Bytes, serde_as};

// Config partition type (custom partition type 0x54, subtype 0x44)
const CONFIG_PARTITION_TYPE: [u8; 2] = [0x54, 0x44];

/// String with sensitive content (debug and display output redacted)
#[derive(Default, Deserialize)]
#[serde(transparent)]
pub struct SensitiveString(String);

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
#[serde_as]
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    /// Wifi country
    #[serde_as(as = "Option<Bytes>")]
    pub wifi_country: Option<[u8; 2]>,
    /// Wifi SSID to connect to
    pub wifi_ssid: String,
    /// Wifi password
    pub wifi_password: SensitiveString,
    /// Mixpanel project token for analytics (optional)
    // Use a different token in debug builds
    #[cfg_attr(debug_assertions, serde(rename = "mp-token-debug"))]
    pub mp_token: Option<String>,
    /// Vereinsflieger API username
    pub vf_username: String,
    /// MD5 (hex) of Vereinsflieger API password
    pub vf_password_md5: SensitiveString,
    /// Vereinsflieger API appkey
    pub vf_appkey: SensitiveString,
    /// Vereinsflieger API cid (optional)
    pub vf_cid: Option<u32>,
    /// Vereinsflieger article ids for purchase
    pub vf_article_ids: Vec<ArticleId>,
}

impl Config {
    /// Read configuration from `config` flash data partition
    pub fn read<S: Storage>(storage: &mut S) -> Self {
        // Read partition table
        let mut buf = [0; partitions::PARTITION_TABLE_MAX_LEN];
        let table = match partitions::read_partition_table(storage, &mut buf) {
            Ok(table) => {
                debug!("Config: Read partition table with {} entries", table.len());
                table
            }
            Err(err) => {
                warn!("Config: Unable to read partition table: {err}");
                return Self::default();
            }
        };

        // Look up config data partition and flash region
        let mut region = if let Some(part) = table
            .iter()
            .find(|part| [part.raw_type(), part.raw_subtype()] == CONFIG_PARTITION_TYPE)
        {
            debug!("Config: Found config partition at 0x{:x}", part.offset());
            part.as_embedded_storage(storage)
        } else {
            warn!("Config: No config partition found, using default configuration");
            return Self::default();
        };

        // Read config data flash region
        let mut bytes = vec![0; region.capacity()];
        if let Err(_err) = region.read(0, &mut bytes) {
            warn!("Config: Unable to read config partition");
            return Self::default();
        }

        // Parse JSON config, ignore trailing junk
        let mut de = serde_json::Deserializer::from_slice(&bytes);
        let config = match Deserialize::deserialize(&mut de) {
            Ok(config) => config,
            Err(err) => {
                warn!("Config: Unable to parse configuration in config partition: {err}");
                return Self::default();
            }
        };

        debug!("Config: System configuration: {config:?}");
        info!("Config: Configuration loaded from config partition");
        config
    }
}
