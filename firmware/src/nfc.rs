use core::convert::Infallible;
use core::fmt::{self, Debug};
use embassy_time::{with_timeout, Duration, Timer};
use embedded_hal::i2c::I2c;
use embedded_hal_async::digital::Wait;
use log::{debug, info, warn};
use pn532::i2c::{I2C_ADDRESS, PN532_I2C_READY};
use pn532::requests::{BorrowedRequest, Command, SAMMode};
use pn532::{Error as Pn532Error, Request};

/// NFC reader buffer size (32 is the PN532 default)
const BUFFER_SIZE: usize = 32;

/// NFC reader ACK timeout
const ACK_TIMEOUT: Duration = Duration::from_millis(50);

/// NFC reader default command timeout
const COMMAND_TIMEOUT: Duration = Duration::from_millis(50);

/// NFC reader read loop timeout
const READ_TIMEOUT: Duration = Duration::from_millis(100);

/// NFC reader read loop sleep
const READ_SLEEP: Duration = Duration::from_millis(900);

const PREAMBLE: [u8; 3] = [0x00, 0x00, 0xFF];
const POSTAMBLE: u8 = 0x00;
const ACK: [u8; 6] = [0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00];
const HOST_TO_PN532: u8 = 0xD4;
const PN532_TO_HOST: u8 = 0xD5;

/// NFC reader error
#[derive(Debug)]
pub enum Error {
    /// PN532 error (with static interface error type)
    #[allow(dead_code)]
    Pn532(Pn532Error<embedded_hal::i2c::ErrorKind>),
}

impl<E: embedded_hal::i2c::Error> From<Pn532Error<E>> for Error {
    fn from(err: Pn532Error<E>) -> Self {
        // Convert generic Pn532Error::InterfaceError(E: embedded_hal::i2c::Error) to non-generic
        // Pn532Error::InterfaceError(embedded_hal::i2c::ErrorKind) to avoid generics in this type
        match err {
            Pn532Error::BadAck => Self::Pn532(Pn532Error::BadAck),
            Pn532Error::BadResponseFrame => Self::Pn532(Pn532Error::BadResponseFrame),
            Pn532Error::Syntax => Self::Pn532(Pn532Error::Syntax),
            Pn532Error::CrcError => Self::Pn532(Pn532Error::CrcError),
            Pn532Error::BufTooSmall => Self::Pn532(Pn532Error::BufTooSmall),
            Pn532Error::TimeoutAck => Self::Pn532(Pn532Error::TimeoutAck),
            Pn532Error::TimeoutResponse => Self::Pn532(Pn532Error::TimeoutResponse),
            Pn532Error::InterfaceError(e) => Self::Pn532(Pn532Error::InterfaceError(e.kind())),
        }
    }
}

/// PN532 interface
/// This is mostly a re-implementation of `pn532::Interface`, but with fully asynchronous handling
// TODO: Switch to `pn532::Interface` once the pn532 crate supports embedded-hal-async 1.0
trait Interface {
    type Error: Debug;
    async fn write(&mut self, frame: &[u8]) -> Result<(), Self::Error>;
    async fn wait_ready(&mut self) -> Result<(), Self::Error>;
    async fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error>;
}

/// PN532 I2C interface with ready status interrupt
/// This is mostly a re-implementation of `pn532::i2c::I2CInterfaceWithIrq`, but with fully
/// asynchronous handling
// TODO: Switch to `pn532::i2c::I2CInterfaceWithIrq` once the pn532 crate supports embedded-hal-async 1.0
#[derive(Debug)]
struct I2CInterfaceWithIrq<I2C, IRQ> {
    i2c: I2C,
    irq: IRQ,
}

impl<I2C: I2c, IRQ: Wait> I2CInterfaceWithIrq<I2C, IRQ> {
    fn new(i2c: I2C, irq: IRQ) -> Self {
        Self { i2c, irq }
    }
}

impl<I2C: I2c, IRQ: Wait<Error = Infallible>> Interface for I2CInterfaceWithIrq<I2C, IRQ> {
    type Error = I2C::Error;

    async fn write(&mut self, frame: &[u8]) -> Result<(), Self::Error> {
        self.i2c.write(I2C_ADDRESS, frame)
    }

    async fn wait_ready(&mut self) -> Result<(), Self::Error> {
        // Wait for IRQ line to become low (ready). Instead of busy waiting, we use hardware
        // interrupt driven asynchronous waiting.
        // Note: Always safe to unwrap because of `IRQ: Wait<Error=Infallible>` bound
        self.irq.wait_for_low().await.unwrap();
        Ok(())
    }

    async fn read(&mut self, frame: &mut [u8]) -> Result<(), Self::Error> {
        // FIXME: Find a way to drop the first byte (ready status) without copying
        // It would be more efficient to use a transaction with separate read operations for status
        // and frame, but somehow this results in AckCheckFailed errors with embedded-hal 1.0
        // self.i2c.transaction(I2C_ADDRESS, &mut [Operation::Read(&mut buf), Operation::Read(frame)])?;
        let mut buf = [0; BUFFER_SIZE + 1];
        // TODO: I2C communication should be asynchronous as well
        self.i2c.read(I2C_ADDRESS, &mut buf[..frame.len() + 1])?;
        debug_assert_eq!(buf[0], PN532_I2C_READY, "PN532 read while not ready");
        frame.copy_from_slice(&buf[1..frame.len() + 1]);
        Ok(())
    }
}

/// PN532 driver
/// This is mostly a re-implementation of `pn532::Pn532`, but with fully asynchronous handling
// TODO: Switch to `pn532::Pn532` once the pn532 crate supports embedded-hal-async 1.0
#[derive(Debug)]
struct Pn532<I> {
    interface: I,
    buf: [u8; BUFFER_SIZE],
}

impl<I: Interface> Pn532<I> {
    /// Create PN532 driver
    /// Like `pn532::Pn532::new`
    fn new(interface: I) -> Self {
        Self {
            interface,
            buf: [0; BUFFER_SIZE],
        }
    }

    /// Send PN532 command
    /// Like `pn532::Pn532::send`, but fully asynchronous
    async fn send(&mut self, request: BorrowedRequest<'_>) -> Result<(), Pn532Error<I::Error>> {
        let data_len = request.data.len();
        let frame_len = 2 + data_len as u8;

        let mut data_sum = HOST_TO_PN532.wrapping_add(request.command as u8);
        for &byte in request.data {
            data_sum = data_sum.wrapping_add(byte);
        }

        const fn to_checksum(sum: u8) -> u8 {
            (!sum).wrapping_add(1)
        }

        self.buf[0] = PREAMBLE[0];
        self.buf[1] = PREAMBLE[1];
        self.buf[2] = PREAMBLE[2];
        self.buf[3] = frame_len;
        self.buf[4] = to_checksum(frame_len);
        self.buf[5] = HOST_TO_PN532;
        self.buf[6] = request.command as u8;

        self.buf[7..7 + data_len].copy_from_slice(request.data);

        self.buf[7 + data_len] = to_checksum(data_sum);
        self.buf[8 + data_len] = POSTAMBLE;

        self.interface.write(&self.buf[..9 + data_len]).await?;
        Ok(())
    }

    /// Receive PN532 ACK
    /// Like `pn532::Pn532::receive_ack`, but fully asynchronous
    async fn receive_ack(&mut self) -> Result<(), Pn532Error<I::Error>> {
        let mut ack_buf = [0; 6];
        self.interface.read(&mut ack_buf).await?;
        if ack_buf != ACK {
            return Err(Pn532Error::BadAck);
        }
        Ok(())
    }

    /// Receive PN532 response
    /// Like `pn532::Pn532::receive_response`, but fully asynchronous
    async fn receive_response(
        &mut self,
        sent_command: Command,
        response_len: usize,
    ) -> Result<&[u8], Pn532Error<I::Error>> {
        let response_buf = &mut self.buf[..response_len + 9];
        response_buf.fill(0);
        self.interface.read(response_buf).await?;
        let expected_response_command = sent_command as u8 + 1;
        Self::parse_response(response_buf, expected_response_command)
    }

    /// Send PN532 ACK frame to abort the current process
    /// Like `pn532::Pn532::abort`, but fully asynchronous
    async fn abort(&mut self) -> Result<(), Pn532Error<I::Error>> {
        self.interface.write(&ACK).await?;
        Ok(())
    }

    /// Parse PN532 response
    /// Like `pn532::protocol::parse_response`
    fn parse_response<E: Debug>(
        response_buf: &[u8],
        expected_response_command: u8,
    ) -> Result<&[u8], Pn532Error<E>> {
        if response_buf[0..3] != PREAMBLE {
            return Err(Pn532Error::BadResponseFrame);
        }
        let frame_len = response_buf[3];
        if (frame_len.wrapping_add(response_buf[4])) != 0 {
            return Err(Pn532Error::CrcError);
        }
        if frame_len == 0 {
            return Err(Pn532Error::BadResponseFrame);
        }
        if frame_len == 1 {
            return Err(Pn532Error::Syntax);
        }
        match response_buf.get(5 + frame_len as usize + 1) {
            None => return Err(Pn532Error::BufTooSmall),
            Some(&POSTAMBLE) => (),
            Some(_) => return Err(Pn532Error::BadResponseFrame),
        }

        if response_buf[5] != PN532_TO_HOST || response_buf[6] != expected_response_command {
            return Err(Pn532Error::BadResponseFrame);
        }
        let checksum = response_buf[5..5 + frame_len as usize + 1]
            .iter()
            .fold(0u8, |s, &b| s.wrapping_add(b));
        if checksum != 0 {
            return Err(Pn532Error::CrcError);
        }
        Ok(&response_buf[7..5 + frame_len as usize])
    }

    /// Send PN532 request and wait for ack and response.
    /// Like `pn532::Pn532::process`, but fully asynchronous and with timeout
    async fn process<'a>(
        &mut self,
        request: impl Into<BorrowedRequest<'a>>,
        response_len: usize,
        timeout: Duration,
    ) -> Result<&[u8], Pn532Error<I::Error>> {
        let request = request.into();
        let sent_command = request.command;
        with_timeout(ACK_TIMEOUT, async {
            self.send(request).await?;
            self.interface.wait_ready().await?;
            self.receive_ack().await
        })
        .await
        .map_err(|_| Pn532Error::TimeoutAck)??;
        with_timeout(timeout, async {
            self.interface.wait_ready().await?;
            self.receive_response(sent_command, response_len).await
        })
        .await
        .map_err(|_| Pn532Error::TimeoutResponse)?
    }
}

/// NFC reader
#[derive(Debug)]
pub struct Nfc<I2C, IRQ> {
    driver: Pn532<I2CInterfaceWithIrq<I2C, IRQ>>,
}

impl<I2C: I2c, IRQ: Wait<Error = Infallible>> Nfc<I2C, IRQ> {
    /// Create NFC driver and initialize NFC hardware
    pub async fn new(i2c: I2C, irq: IRQ) -> Result<Self, Error> {
        debug!("NFC: Initializing PN532...");

        let mut driver = Pn532::new(I2CInterfaceWithIrq::new(i2c, irq));

        // Abort any currently running command (just in case), ignore any error
        let _ = driver.abort().await;

        // Configure PN532 as initiator (normal mode)
        driver
            .process(
                // SAMConfiguration request (PN532 §7.2.10)
                &Request::sam_configuration(SAMMode::Normal, true),
                0,
                COMMAND_TIMEOUT,
            )
            .await?;

        // Query PN532 version and capabilities
        let version_response = driver
            .process(
                // GetFirmwareVersion request (PN532 §7.2.2)
                &Request::GET_FIRMWARE_VERSION,
                4,
                COMMAND_TIMEOUT,
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
                .process(
                    // InListPassiveTarget request (PN532 §7.3.5)
                    &Request::INLIST_ONE_ISO_A_TARGET,
                    BUFFER_SIZE - 9, // max response length
                    READ_TIMEOUT,
                )
                .await
            {
                Ok(res) => res,
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
                .process(
                    // InRelease request (PN532 §7.3.11)
                    &Request::RELEASE_TAG_1,
                    1,
                    COMMAND_TIMEOUT,
                )
                .await
            {
                warn!("NFC: Failed to release target: {:?}", err);
            }

            // Return UID if retrieved, continue looping otherwise
            if let Some(uid) = maybe_uid {
                return Ok(uid);
            }
        }
    }
}

/// NFC UID Error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidUid;

/// NFC UID
#[derive(Debug, Clone, PartialEq, Eq)]
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

fn write_hex_bytes(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
    for b in bytes {
        write!(f, "{:02x}", *b)?;
    }
    Ok(())
}

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Single(ref bytes) => write_hex_bytes(f, bytes),
            Self::Double(ref bytes) => write_hex_bytes(f, bytes),
            Self::Triple(ref bytes) => write_hex_bytes(f, bytes),
        }
    }
}

impl AsRef<[u8]> for Uid {
    fn as_ref(&self) -> &[u8] {
        match *self {
            Self::Single(ref bytes) => bytes,
            Self::Double(ref bytes) => bytes,
            Self::Triple(ref bytes) => bytes,
        }
    }
}
