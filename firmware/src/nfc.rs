// Use custom pn532 driver instead of pn532 crate
use crate::pn532;

use crate::json::{self, ToJson};
use alloc::string::ToString;
use const_hex::FromHex;
use core::convert::Infallible;
use core::fmt::{self, Debug};
use core::str::FromStr;
use embassy_time::{Duration, Timer};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::i2c::I2c;
use embedded_io_async::Write;
use log::{debug, info, warn};
use pn532::{Error as Pn532Error, I2CInterfaceWithIrq, Pn532, Request, SAMMode};

/// Response buffer size (32 is the PN532 default)
const BUFFER_SIZE: usize = 64;

/// NFC reader read loop timeout
const READ_TIMEOUT: Duration = Duration::from_millis(100);

/// NFC reader read loop sleep
const READ_SLEEP: Duration = Duration::from_millis(400);

/// NFC reader error
// Basically a PN532 error with static interface error type to avoid generics in this type
#[derive(Debug)]
pub struct Error(Pn532Error<embedded_hal_async::i2c::ErrorKind>);

impl<E: embedded_hal_async::i2c::Error> From<Pn532Error<E>> for Error {
    fn from(err: Pn532Error<E>) -> Self {
        // Convert generic Pn532Error::InterfaceError(E: embedded_hal::i2c::Error) to non-generic
        // Pn532Error::InterfaceError(embedded_hal::i2c::ErrorKind) to avoid generics in this type
        match err {
            Pn532Error::BadAck => Self(Pn532Error::BadAck),
            Pn532Error::BadResponseFrame => Self(Pn532Error::BadResponseFrame),
            Pn532Error::Syntax => Self(Pn532Error::Syntax),
            Pn532Error::CrcError => Self(Pn532Error::CrcError),
            Pn532Error::BufTooSmall => Self(Pn532Error::BufTooSmall),
            Pn532Error::TimeoutAck => Self(Pn532Error::TimeoutAck),
            Pn532Error::TimeoutResponse => Self(Pn532Error::TimeoutResponse),
            Pn532Error::InterfaceError(e) => Self(Pn532Error::InterfaceError(e.kind())),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Pn532Error::BadAck => write!(f, "Bad ACK"),
            Pn532Error::BadResponseFrame => write!(f, "Bad response frame"),
            Pn532Error::Syntax => write!(f, "Syntax error"),
            Pn532Error::CrcError => write!(f, "CRC error"),
            Pn532Error::BufTooSmall => write!(f, "Buffer too small"),
            Pn532Error::TimeoutAck => write!(f, "ACK timeout"),
            Pn532Error::TimeoutResponse => write!(f, "Response timeout"),
            Pn532Error::InterfaceError(_err) => write!(f, "Bus error"),
        }
    }
}

/// NFC reader
#[derive(Debug)]
pub struct Nfc<I2C, IRQ> {
    driver: Pn532<I2CInterfaceWithIrq<I2C, IRQ>, BUFFER_SIZE>,
}

impl<I2C: I2c, IRQ: Wait<Error = Infallible>> Nfc<I2C, IRQ> {
    /// Create NFC driver and initialize NFC hardware
    pub async fn new(i2c: I2C, irq: IRQ) -> Result<Self, Error> {
        debug!("NFC: Initializing PN532...");

        let mut driver = Pn532::new_async(I2CInterfaceWithIrq { i2c, irq });

        // Abort any currently running command (just in case), ignore any error
        let _ = driver.abort().await;

        // Configure PN532 as initiator (normal mode)
        driver
            .process_async(
                // SAMConfiguration request (PN532 §7.2.10)
                &Request::sam_configuration(SAMMode::Normal, true),
                0,
            )
            .await?;

        // Query PN532 version and capabilities
        let version_response = driver
            .process_async(
                // GetFirmwareVersion request (PN532 §7.2.2)
                &Request::GET_FIRMWARE_VERSION,
                4,
            )
            .await?;
        // GetFirmwareVersion response (PN532 §7.2.2)
        // - 1 byte: IC version (0x32 for PN532)
        // - 1 byte: Version of firmware
        // - 1 byte: Revision of firmware
        // - 1 byte: Supported functionality bitmask
        //           - Bit 0: ISO/IEC 14443 Type A
        //           - Bit 1: ISO/IEC 14443 Type B
        //           - Bit 2: ISO 18092
        debug!(
            "NFC: PN532 IC 0x{:02x}, Firmware {}.{}, Support 0x{:02x}",
            version_response[0], version_response[1], version_response[2], version_response[3]
        );

        info!("NFC: PN532 initialized");
        Ok(Self { driver })
    }

    /// Wait for NFC target and read identification
    pub async fn read(&mut self) -> Result<Uid, Error> {
        loop {
            // Abort any currently running command, ignore any error
            let _ = self.driver.abort().await;

            // Sleep for some time before starting next detection
            Timer::after(READ_SLEEP).await;

            // Detect any ISO/IEC14443 Type A target in passive mode
            let list_response = match self
                .driver
                .process_timeout_async(
                    // InListPassiveTarget request (PN532 §7.3.5)
                    &Request::INLIST_ONE_ISO_A_TARGET,
                    BUFFER_SIZE - 9, // max response length
                    READ_TIMEOUT,
                )
                .await
            {
                Ok(bytes) => bytes,
                // On timeout (no target detected), restart detection
                Err(Pn532Error::TimeoutResponse) => continue,
                // Error listing targets, cancel loop and return
                Err(err) => return Err(err.into()),
            };

            // InListPassiveTarget response (PN532 §7.3.5, ISO/IEC 14443 Type A)
            // - 1 byte: number of detected targets (should be 1, as limited by request)
            // - for each detected target:
            //   - 1 byte: target number (0x01 for first target)
            //   - 2 bytes: SENS_RES
            //   - 1 byte: SEL_RES
            //   - 1 byte: NFCID1tLength (typically 4 or 7)
            //   - NFCID1tLength bytes: NFCID1t
            //   - 1 byte (optional): ATSLength
            //   - ATSLength bytes (optional): ATS data
            if list_response.len() < 6 {
                warn!(
                    "NFC: Target list short response ({} < 6)",
                    list_response.len()
                );
                continue;
            }
            if list_response[0] < 1 {
                warn!("NFC: Target list empty");
                continue;
            }
            debug_assert_eq!(list_response[1], 1, "NFC: First target number must be 1");

            // Extract and parse UID, truncate tail on short response
            let nfcid = &list_response[6..];
            let nfcid_len = (list_response[5] as usize).min(nfcid.len());
            let nfcid = &nfcid[..nfcid_len];
            let maybe_uid = match Uid::try_from(nfcid) {
                Ok(uid) => Some(uid),
                Err(_err) => {
                    warn!("NFC: Target has invalid NFCID: {:02x?}", nfcid);
                    None
                }
            };

            // Release the detected target, ignore any error
            // Note: needs to be always done, even if any requests to communicate with the target
            // has failed, as it's required to release the target to be able to find the next
            if let Err(err) = self
                .driver
                .process_async(
                    // InRelease request (PN532 §7.3.11)
                    &Request::RELEASE_TAG_1,
                    1,
                )
                .await
            {
                warn!("NFC: Failed to release target: {:?}", err);
            }

            // Return UID if retrieved, continue looping otherwise
            if let Some(uid) = maybe_uid {
                debug!("NFC: Detected NFC card: {}", uid);
                return Ok(uid);
            }
        }
    }
}

/// NFC UID Error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidUid;

/// NFC UID
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Uid {
    /// Single Size UID (4 bytes), Mifare Classic
    Single([u8; 4]),
    /// Double Size UID (7 bytes), NXP NTAG Series
    Double([u8; 7]),
    /// Triple Size UID (10 bytes), not used yet
    Triple([u8; 10]),
}

impl TryFrom<&[u8]> for Uid {
    type Error = InvalidUid;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        match bytes.len() {
            // Note: Always safe to unwrap because of matching length check
            4 => Ok(Self::Single(bytes.try_into().unwrap())),
            7 => Ok(Self::Double(bytes.try_into().unwrap())),
            10 => Ok(Self::Triple(bytes.try_into().unwrap())),
            _ => Err(InvalidUid),
        }
    }
}

impl FromStr for Uid {
    type Err = InvalidUid;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.len() {
            8 => {
                let bytes = <[u8; 4]>::from_hex(s).map_err(|_e| InvalidUid)?;
                Ok(Self::Single(bytes))
            }
            14 => {
                let bytes = <[u8; 7]>::from_hex(s).map_err(|_e| InvalidUid)?;
                Ok(Self::Double(bytes))
            }
            20 => {
                let bytes = <[u8; 10]>::from_hex(s).map_err(|_e| InvalidUid)?;
                Ok(Self::Triple(bytes))
            }
            _ => Err(InvalidUid),
        }
    }
}

fn write_hex_bytes(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for b in bytes {
        write!(f, "{:02x}", *b)?;
    }
    Ok(())
}

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single(bytes) => write_hex_bytes(f, bytes),
            Self::Double(bytes) => write_hex_bytes(f, bytes),
            Self::Triple(bytes) => write_hex_bytes(f, bytes),
        }
    }
}

impl AsRef<[u8]> for Uid {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Single(bytes) => bytes,
            Self::Double(bytes) => bytes,
            Self::Triple(bytes) => bytes,
        }
    }
}

impl ToJson for Uid {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        json.write(self.to_string()).await
    }
}
