use crate::article::ArticleId;
use crate::util::SensitiveString;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use embedded_storage::ReadStorage;
use log::{debug, warn};
use serde::Deserialize;
use serde_with::{Bytes, serde_as};

/// Maximum size of configuration data
const MAX_CONFIG_SIZE: usize = 8192;

/// System configuration
///
/// System configuration is stored in a separate storage, usually a flash data partition called
/// `config`. This way, it stays unaffected by firmware updates via USB or OTA. Currently,
/// configuration is read-only at runtime, i.e. it needs to be flashed manually once per device.
/// To make this easier, it is expected to be stored in JSON format. See README.md for details on
/// how to flash the configuration.
///
/// If there is no valid JSON, a default configuration is provided (which isn't very useful, but
/// at least doesn't prevent the device from starting).
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
    /// Read configuration from the given storage
    pub fn read<S: ReadStorage>(storage: &mut S) -> Self {
        let mut bytes = vec![0; storage.capacity().min(MAX_CONFIG_SIZE)].into_boxed_slice();
        if let Err(_err) = storage.read(0, &mut bytes) {
            warn!("Config: Unable to read configuration");
            return Self::default();
        }

        // Parse JSON config, ignore trailing junk
        let mut de = serde_json::Deserializer::from_slice(&bytes);
        let config = match Deserialize::deserialize(&mut de) {
            Ok(config) => config,
            Err(err) => {
                warn!("Config: Unable to parse configuration: {err}");
                return Self::default();
            }
        };

        debug!("Config: System configuration: {config:?}");
        config
    }
}
