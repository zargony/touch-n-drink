use crate::buzzer::Buzzer;
use crate::display::{self, Display};
use crate::keypad::{Key, Keypad};
use crate::nfc::{self, Nfc, Uid};
use crate::screen;
use core::convert::Infallible;
use embassy_futures::select::{select, Either};
use embassy_time::{with_timeout, Duration, TimeoutError};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::i2c::I2c;
use log::info;

/// How long to show the splash screen if no key is pressed
const SPLASH_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for user input. Actions are cancelled if the user does nothing for this duration.
#[cfg(not(debug_assertions))]
const USER_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(debug_assertions)]
const USER_TIMEOUT: Duration = Duration::from_secs(5);

/// Timeout for initial screen. Power saving is activated if no action is taken for this duration.
#[cfg(not(debug_assertions))]
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
#[cfg(debug_assertions)]
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// User interface error
#[derive(Debug)]
#[non_exhaustive]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    /// Failed to output to display
    #[allow(dead_code)]
    DisplayError(display::Error),
    /// NFC reader error
    #[allow(dead_code)]
    NFCError(nfc::Error),
    /// User cancel request
    Cancel,
    /// User interaction timeout
    Timeout,
}

impl From<display::Error> for Error {
    fn from(err: display::Error) -> Self {
        Self::DisplayError(err)
    }
}

impl From<nfc::Error> for Error {
    fn from(err: nfc::Error) -> Self {
        Self::NFCError(err)
    }
}

impl From<TimeoutError> for Error {
    fn from(_err: TimeoutError) -> Self {
        Self::Timeout
    }
}

/// User interface
pub struct Ui<'a, I2C, IRQ> {
    display: Display<I2C>,
    keypad: Keypad<'a, 3, 4>,
    nfc: Nfc<I2C, IRQ>,
    buzzer: Buzzer<'a>,
}

impl<'a, I2C: I2c, IRQ: Wait<Error = Infallible>> Ui<'a, I2C, IRQ> {
    /// Create user interface with given human interface devices
    pub fn new(
        display: Display<I2C>,
        keypad: Keypad<'a, 3, 4>,
        nfc: Nfc<I2C, IRQ>,
        buzzer: Buzzer<'a>,
    ) -> Self {
        Self {
            display,
            keypad,
            nfc,
            buzzer,
        }
    }

    /// Save power by turning off devices not needed during idle
    pub async fn power_save(&mut self) -> Result<(), Error> {
        info!("UI: Power saving...");

        self.display.turn_off().await?;
        Ok(())
    }

    /// Show splash screen and wait for keypress or timeout
    pub async fn show_splash_screen(&mut self) -> Result<(), Error> {
        self.display.screen(&screen::Splash).await?;
        let _key = with_timeout(SPLASH_TIMEOUT, self.keypad.read()).await?;
        Ok(())
    }

    /// Wait for id card and verify identification
    pub async fn read_id_card(&mut self) -> Result<Uid, Error> {
        info!("UI: Waiting for NFC card...");

        let mut saving_power = false;
        loop {
            // Show scan prompt unless power saving is active
            if !saving_power {
                self.display.screen(&screen::ScanId).await?;
            }
            // Wait for id card read, keypress or timeout
            let uid = match with_timeout(IDLE_TIMEOUT, select(self.nfc.read(), self.keypad.read()))
                .await
            {
                // Id card read
                Ok(Either::First(res)) => res?,
                // Key pressed while saving power, leave power saving
                Ok(Either::Second(_key)) if saving_power => {
                    saving_power = false;
                    continue;
                }
                // Idle timeout, enter power saving
                Err(TimeoutError) if !saving_power => {
                    self.power_save().await?;
                    saving_power = true;
                    continue;
                }
                // Otherwise, do nothing
                _ => continue,
            };
            info!("UI: Detected NFC card: {}", uid);
            let _ = self.buzzer.short_confirmation().await;
            // TODO: Verify identification and return user information
            return Ok(uid);
        }
    }

    /// Ask for number of drinks
    pub async fn get_number_of_drinks(&mut self) -> Result<usize, Error> {
        info!("UI: Asking for number of drinks...");

        self.display.screen(&screen::NumberOfDrinks).await?;
        loop {
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await? {
                // Any digit 1..=9 selects number of drinks
                Key::Digit(n) if (1..=9).contains(&n) => return Ok(n as usize),
                // Ignore any other digit
                Key::Digit(_) => (),
                // Cancel key cancels
                Key::Cancel => return Err(Error::Cancel),
                // Ignore any other key
                _ => (),
            }
        }
    }

    /// Confirm purchase
    pub async fn checkout(&mut self, num_drinks: usize, total_price: f32) -> Result<(), Error> {
        info!(
            "UI: Asking for checkout of {} drinks, {:.02} EUR...",
            num_drinks, total_price
        );

        self.display
            .screen(&screen::Checkout::new(num_drinks, total_price))
            .await?;
        loop {
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await? {
                // Enter key confirms purchase
                Key::Enter => return Ok(()),
                // Cancel key cancels
                Key::Cancel => return Err(Error::Cancel),
                // Ignore any other key
                _ => (),
            }
        }
    }

    /// Run the user interface flow
    pub async fn run(&mut self) -> Result<(), Error> {
        let _uid = self.read_id_card().await?;
        let num_drinks = self.get_number_of_drinks().await?;
        let total_price = 1.0 * num_drinks as f32;
        self.checkout(num_drinks, total_price).await?;

        // TODO: Process payment
        let _ = screen::Success::new(num_drinks);
        self.display
            .screen(&screen::Failure::new("Not implemented yet"))
            .await?;
        let _ = self.buzzer.error().await;
        let _key = self.keypad.read().await;
        Ok(())
    }
}
