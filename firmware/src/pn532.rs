// Mute pedantic clippy warnings caused by original code copied from pn532 crate
#![allow(
    clippy::cast_possible_truncation,
    clippy::if_not_else,
    clippy::items_after_statements,
    clippy::range_plus_one
)]

use core::convert::Infallible;
use core::fmt::Debug;
use embassy_time::{with_timeout, Duration};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::i2c::{I2c, Operation};
use log::warn;
use pn532::i2c::{I2C_ADDRESS, PN532_I2C_READY};
use pn532::requests::BorrowedRequest;

pub use pn532::requests::{Command, SAMMode};
pub use pn532::{Error, Request};

/// Reasponse buffer size (32 is the PN532 default)
pub const BUFFER_SIZE: usize = 32;

/// Command ACK timeout
const ACK_TIMEOUT: Duration = Duration::from_millis(50);

/// Command response timeout
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(50);

const PREAMBLE: [u8; 3] = [0x00, 0x00, 0xFF];
const POSTAMBLE: u8 = 0x00;
const ACK: [u8; 6] = [0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00];
const HOST_TO_PN532: u8 = 0xD4;
const PN532_TO_HOST: u8 = 0xD5;

/// PN532 interface
/// This is mostly a re-implementation of `pn532::Interface`, but with fully asynchronous handling
// TODO: Switch to `pn532::Interface` once the pn532 crate supports embedded-hal-async 1.0
pub trait Interface {
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
pub struct I2CInterfaceWithIrq<I2C, IRQ> {
    i2c: I2C,
    irq: IRQ,
}

impl<I2C: I2c, IRQ: Wait> I2CInterfaceWithIrq<I2C, IRQ> {
    pub fn new(i2c: I2C, irq: IRQ) -> Self {
        Self { i2c, irq }
    }
}

impl<I2C: I2c, IRQ: Wait<Error = Infallible>> Interface for I2CInterfaceWithIrq<I2C, IRQ> {
    type Error = I2C::Error;

    async fn write(&mut self, frame: &[u8]) -> Result<(), Self::Error> {
        self.i2c.write(I2C_ADDRESS, frame).await
    }

    async fn wait_ready(&mut self) -> Result<(), Self::Error> {
        // Wait for IRQ line to become low (ready). Instead of busy waiting, we use hardware
        // interrupt driven asynchronous waiting.
        // Note: Always safe to unwrap because of `IRQ: Wait<Error=Infallible>` bound
        self.irq.wait_for_low().await.unwrap();
        Ok(())
    }

    async fn read(&mut self, frame: &mut [u8]) -> Result<(), Self::Error> {
        let mut status = [0];
        self.i2c
            .transaction(
                I2C_ADDRESS,
                &mut [Operation::Read(&mut status), Operation::Read(frame)],
            )
            .await?;
        // Status in a read frame should always indicate ready since `read` is always called after
        // `wait_ready`. But sometimes it doesn't, which we ignore for now.
        if status[0] != PN532_I2C_READY {
            warn!("PN532: read while not ready");
        }
        Ok(())
    }
}

/// PN532 driver
/// This is mostly a re-implementation of `pn532::Pn532`, but with fully asynchronous handling
// TODO: Switch to `pn532::Pn532` once the pn532 crate supports embedded-hal-async 1.0
#[derive(Debug)]
pub struct Pn532<I, const N: usize = BUFFER_SIZE> {
    interface: I,
    buf: [u8; N],
}

impl<I: Interface, const N: usize> Pn532<I, N> {
    /// Create PN532 driver
    /// Like `pn532::Pn532::new`
    pub fn new(interface: I) -> Self {
        Self {
            interface,
            buf: [0; N],
        }
    }

    /// Send PN532 command
    /// Like `pn532::Pn532::send`, but fully asynchronous
    pub async fn send(&mut self, request: BorrowedRequest<'_>) -> Result<(), Error<I::Error>> {
        let data_len = request.data.len();
        let frame_len = 2 + data_len as u8; // frame identifier + command + data

        let mut data_sum = HOST_TO_PN532.wrapping_add(request.command as u8); // sum(command + data + frame identifier)
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
    pub async fn receive_ack(&mut self) -> Result<(), Error<I::Error>> {
        let mut ack_buf = [0; 6];
        self.interface.read(&mut ack_buf).await?;
        if ack_buf != ACK {
            Err(Error::BadAck)
        } else {
            Ok(())
        }
    }

    /// Receive PN532 response
    /// Like `pn532::Pn532::receive_response`, but fully asynchronous
    pub async fn receive_response(
        &mut self,
        sent_command: Command,
        response_len: usize,
    ) -> Result<&[u8], Error<I::Error>> {
        let response_buf = &mut self.buf[..response_len + 9];
        response_buf.fill(0); // zero out buf
        self.interface.read(response_buf).await?;
        let expected_response_command = sent_command as u8 + 1;
        parse_response(response_buf, expected_response_command)
    }

    /// Send PN532 ACK frame to abort the current process
    /// Like `pn532::Pn532::abort`, but fully asynchronous
    pub async fn abort(&mut self) -> Result<(), Error<I::Error>> {
        self.interface.write(&ACK).await?;
        Ok(())
    }

    /// Send PN532 request and wait for ack and response.
    /// Like `pn532::Pn532::process`, but fully asynchronous
    pub async fn process<'a>(
        &mut self,
        request: impl Into<BorrowedRequest<'a>>,
        response_len: usize,
    ) -> Result<&[u8], Error<I::Error>> {
        self.process_timeout(request, response_len, RESPONSE_TIMEOUT)
            .await
    }

    /// Send PN532 request and wait for ack and response.
    /// Like `pn532::Pn532::process`, but fully asynchronous and with timeout
    pub async fn process_timeout<'a>(
        &mut self,
        request: impl Into<BorrowedRequest<'a>>,
        response_len: usize,
        timeout: Duration,
    ) -> Result<&[u8], Error<I::Error>> {
        let request = request.into();
        let sent_command = request.command;

        with_timeout(ACK_TIMEOUT, async {
            self.send(request).await?;
            self.interface.wait_ready().await?;
            self.receive_ack().await
        })
        .await
        .map_err(|_| Error::TimeoutAck)??;

        with_timeout(timeout, async {
            self.interface.wait_ready().await?;
            self.receive_response(sent_command, response_len).await
        })
        .await
        .map_err(|_| Error::TimeoutResponse)?
    }
}

/// Parse PN532 response
/// Like `pn532::protocol::parse_response`
fn parse_response<E: Debug>(
    response_buf: &[u8],
    expected_response_command: u8,
) -> Result<&[u8], Error<E>> {
    if response_buf[0..3] != PREAMBLE {
        return Err(Error::BadResponseFrame);
    }
    // Check length & length checksum
    let frame_len = response_buf[3];
    if (frame_len.wrapping_add(response_buf[4])) != 0 {
        return Err(Error::CrcError);
    }
    if frame_len == 0 {
        return Err(Error::BadResponseFrame);
    }
    if frame_len == 1 {
        // 6.2.1.5 Error frame
        return Err(Error::Syntax);
    }
    match response_buf.get(5 + frame_len as usize + 1) {
        None => {
            return Err(Error::BufTooSmall);
        }
        Some(&POSTAMBLE) => {}
        Some(_) => {
            return Err(Error::BadResponseFrame);
        }
    }

    if response_buf[5] != PN532_TO_HOST || response_buf[6] != expected_response_command {
        return Err(Error::BadResponseFrame);
    }
    // Check frame checksum value matches bytes
    let checksum = response_buf[5..5 + frame_len as usize + 1]
        .iter()
        .fold(0u8, |s, &b| s.wrapping_add(b));
    if checksum != 0 {
        return Err(Error::CrcError);
    }
    // Adjust response buf and return it
    Ok(&response_buf[7..5 + frame_len as usize])
}
