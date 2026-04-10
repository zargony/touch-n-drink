use derive_more::Display;
use esp_bootloader_esp_idf::ota::OtaImageState;
use esp_bootloader_esp_idf::ota_updater::OtaUpdater;
use esp_bootloader_esp_idf::partitions::{self, FlashRegion};
use esp_hal::rtc_cntl::SocResetReason;
use esp_hal::system;
use esp_hal::time::{Duration, Instant};
use esp_storage::FlashStorage;
use log::debug;

/// Required buffer size for firmware updater
pub const BUFFER_SIZE: usize = partitions::PARTITION_TABLE_MAX_LEN;

/// Firmware updater error
#[derive(Debug, Display)]
#[must_use]
pub enum Error {
    /// Invalid partition setup
    #[display("Invalid partition setup: {_0}")]
    InvalidPartitionSetup(partitions::Error),
    /// Failed to activate next partition
    #[display("Failed to activate partition: {_0}")]
    FailedToActivatePartition(partitions::Error),
}

/// Firmware updater
pub struct Updater<'u> {
    ota_updater: OtaUpdater<'u, FlashStorage<'u>>,
}

impl<'u> common::Updater for Updater<'u> {
    const FIRMWARE_VARIANT: &'static str = "esp32c3";

    type Error = Error;
    type Region<'r>
        = FlashRegion<'r, FlashStorage<'u>>
    where
        Self: 'r;

    fn region(&mut self) -> Result<Self::Region<'_>, Self::Error> {
        let current_subtype = self
            .ota_updater
            .selected_partition()
            .map_err(Error::InvalidPartitionSetup)?;
        let (region, subtype) = self
            .ota_updater
            .next_partition()
            .map_err(Error::InvalidPartitionSetup)?;
        debug!("Updater: Current partition: {current_subtype:?}, next partition: {subtype:?}");

        Ok(region)
    }

    fn commit(&mut self) -> Result<(), Self::Error> {
        // Mark the written partition active so bootloader will use it on next restart
        debug!("Updater: Activating next partition");
        self.ota_updater
            .activate_next_partition()
            .map_err(Error::FailedToActivatePartition)?;
        // FIXME: Explicitly set the state of the new image slot to valid because
        //  - the bootloader has no rollback support
        //  - esp-bootloader-esp-ids 0.4.0 retains the old state of whatever was in the
        //    ota data slot before, which seems wrong as we just flashed a new image
        let _ = self.ota_updater.set_current_ota_state(OtaImageState::Valid);

        Ok(())
    }

    fn cancel(&mut self) {
        // Do nothing to cancel an update (i.e. don't activate the next partition so the current
        // one stays active)
        // TODO: Erase partition because of potentially bad firmware image in next partition?
    }

    fn restart() -> ! {
        system::software_reset()
    }

    fn recently_restarted() -> bool {
        // Check if system was recently restarted. Only takes software restarts into account,
        // no other reasons like power on or watchdog failure.
        system::reset_reason() == Some(SocResetReason::CoreSw)
            && Instant::now().duration_since_epoch() < Duration::from_secs(120)
    }
}

impl<'u> Updater<'u> {
    /// Create new firmware updater
    pub fn new(
        flash: &'u mut FlashStorage<'u>,
        buffer: &'u mut [u8; BUFFER_SIZE],
    ) -> Result<Self, Error> {
        let ota_updater = OtaUpdater::new(flash, buffer).map_err(Error::InvalidPartitionSetup)?;
        Ok(Self { ota_updater })
    }
}
