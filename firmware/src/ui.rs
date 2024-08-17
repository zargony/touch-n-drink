use crate::display::{self, Display};
use crate::keypad::{self, Key, Keypad};
use crate::screen;
use embassy_time::{with_timeout, Duration, TimeoutError};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::i2c::I2c;
use embedded_hal_async::digital::Wait;
use log::info;

/// How long to display the splash screen if no key is pressed
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
const IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// User interface error
#[derive(Debug)]
#[non_exhaustive]
#[allow(clippy::enum_variant_names)]
pub enum Error<IN: InputPin, OUT: OutputPin> {
    /// Failed to output to display
    #[allow(dead_code)]
    DisplayError(display::Error),
    /// Failed to read keypad
    KeypadError(keypad::Error<IN, OUT>),
    /// User cancel request
    Cancel,
    /// User interaction timeout
    Timeout,
}

impl<IN: InputPin, OUT: OutputPin> From<display::Error> for Error<IN, OUT> {
    fn from(err: display::Error) -> Self {
        Self::DisplayError(err)
    }
}

impl<IN: InputPin, OUT: OutputPin> From<keypad::Error<IN, OUT>> for Error<IN, OUT> {
    fn from(err: keypad::Error<IN, OUT>) -> Self {
        Self::KeypadError(err)
    }
}

impl<IN: InputPin, OUT: OutputPin> From<TimeoutError> for Error<IN, OUT> {
    fn from(_err: TimeoutError) -> Self {
        Self::Timeout
    }
}

/// User interface
pub struct Ui<I2C, IN, OUT> {
    display: Display<I2C>,
    keypad: Keypad<IN, OUT, 3, 4>,
}

impl<I2C, IN, OUT> Ui<I2C, IN, OUT>
where
    I2C: I2c,
    IN: InputPin + Wait,
    OUT: OutputPin,
{
    /// Create user interface with given human interface devices
    pub fn new(display: Display<I2C>, keypad: Keypad<IN, OUT, 3, 4>) -> Self {
        Self { display, keypad }
    }

    /// Save power by turning off devices not needed during idle
    pub fn power_save(&mut self) -> Result<(), Error<IN, OUT>> {
        info!("UI: Power saving...");
        self.display.turn_off()?;
        Ok(())
    }

    /// Show splash screen and wait for keypress or timeout
    pub async fn show_splash_screen(&mut self) -> Result<(), Error<IN, OUT>> {
        self.display.screen(screen::Splash)?;
        let _key = with_timeout(SPLASH_TIMEOUT, self.keypad.read()).await??;
        Ok(())
    }

    /// Wait for id card and verify identification
    pub async fn read_id_card(&mut self) -> Result<Key, Error<IN, OUT>> {
        self.display.screen(screen::ScanId)?;
        let mut saving_power = false;
        loop {
            // TODO: Poll card reader here
            let id = match with_timeout(IDLE_TIMEOUT, self.keypad.read()).await {
                Ok(res) => res,
                // On idle timeout, enter power saving
                Err(TimeoutError) if !saving_power => {
                    self.power_save()?;
                    saving_power = true;
                    continue;
                }
                Err(TimeoutError) => continue,
            }?;
            // TODO: Verify identification and return user information
            return Ok(id);
        }
    }

    /// Ask for number of drinks
    pub async fn get_number_of_drinks(&mut self) -> Result<usize, Error<IN, OUT>> {
        self.display.screen(screen::NumberOfDrinks)?;
        loop {
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await?? {
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
    pub async fn checkout(
        &mut self,
        num_drinks: usize,
        total_price: f32,
    ) -> Result<(), Error<IN, OUT>> {
        self.display
            .screen(screen::Checkout::new(num_drinks, total_price))?;
        loop {
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await?? {
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
    pub async fn run(&mut self) -> Result<(), Error<IN, OUT>> {
        let _id = self.read_id_card().await?;
        let num_drinks = self.get_number_of_drinks().await?;
        let total_price = 1.0 * num_drinks as f32;
        self.checkout(num_drinks, total_price).await?;

        // TODO: Process payment
        let _ = screen::Success::new(num_drinks);
        self.display
            .screen(screen::Failure::new("Not implemented yet"))?;
        let _key = self.keypad.read().await?;
        Ok(())
    }
}
