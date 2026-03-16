use crate::article::ArticleId;
use crate::util::SensitiveString;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use embedded_storage::{ReadStorage, Storage};
use esp_bootloader_esp_idf::partitions::{self, DataPartitionSubType, PartitionType};
use log::{debug, info, warn};
use serde::Deserialize;
use serde_with::{Bytes, serde_as};

// Config partition type and name
const PARTITION_TYPE: PartitionType = PartitionType::Data(DataPartitionSubType::Undefined);
const PARTITION_NAME: &str = "config";

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
        let mut region = if let Some(part) = table.iter().find(|part| {
            part.partition_type() == PARTITION_TYPE && part.label_as_str() == PARTITION_NAME
        }) {
            debug!("Config: Found config partition at 0x{:x}", part.offset());
            part.as_embedded_storage(storage)
        } else {
            warn!("Config: No config partition found, using default configuration");
            return Self::default();
        };

        // Read config data flash region
        let mut bytes = vec![0; region.capacity()].into_boxed_slice();
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
