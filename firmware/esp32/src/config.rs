use common::config::Config;
use esp_bootloader_esp_idf::partitions::{self, DataPartitionSubType, PartitionType};
use esp_storage::FlashStorage;
use log::{debug, info, warn};

// Config partition type and name
const PARTITION_TYPE: PartitionType = PartitionType::Data(DataPartitionSubType::Undefined);
const PARTITION_NAME: &str = "config";

/// Read configuration from `config` flash data partition
pub fn read(flash: &mut FlashStorage) -> Config {
    // Read partition table
    let mut buf = [0; partitions::PARTITION_TABLE_MAX_LEN];
    let table = match partitions::read_partition_table(flash, &mut buf) {
        Ok(table) => {
            debug!("Config: Read partition table with {} entries", table.len());
            table
        }
        Err(err) => {
            warn!("Config: Unable to read partition table: {err}");
            return Config::default();
        }
    };

    // Look up config data partition and flash region
    let mut region = if let Some(part) = table.iter().find(|part| {
        part.partition_type() == PARTITION_TYPE && part.label_as_str() == PARTITION_NAME
    }) {
        debug!("Config: Found config partition at 0x{:x}", part.offset());
        part.as_embedded_storage(flash)
    } else {
        warn!("Config: No config partition found, using default configuration");
        return Config::default();
    };

    let config = Config::read(&mut region);
    info!("Config: Configuration loaded from config partition");
    config
}
