use crate::display::{self, Display};
use crate::keypad::{self, Key, Keypad};
use crate::screen;
use embassy_time::{with_timeout, Duration, TimeoutError};
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::i2c::I2c;
use embedded_hal_async::digital::Wait;
use log::info;

const SPLASH_TIMEOUT: Duration = Duration::from_secs(5);
const USER_TIMEOUT: Duration = Duration::from_secs(5);

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

    /// Wait for input of a single digit and show it on screen
    pub async fn get_single_digit(&mut self) -> Result<Key, Error<IN, OUT>> {
        let key = with_timeout(USER_TIMEOUT, self.keypad.read()).await??;
        let ch = key.as_char();
        self.display.screen(screen::BigCenteredChar(ch))?;
        Ok(key)
    }

    /// Testing user interface flow
    pub async fn test(&mut self) -> Result<(), Error<IN, OUT>> {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Displaying {
            Splash,
            Key,
            Off,
        }

        let mut displaying = Displaying::Splash;
        loop {
            match self.get_single_digit().await {
                Ok(key) => {
                    info!("UI: Key pressed: {:?}", key);
                    displaying = Displaying::Key;
                }
                Err(Error::Timeout) if displaying == Displaying::Key => {
                    self.show_splash_screen().await?;
                    displaying = Displaying::Splash;
                }
                Err(Error::Timeout) if displaying == Displaying::Splash => {
                    self.power_save()?;
                    displaying = Displaying::Off;
                }
                Err(Error::Timeout) => (),
                Err(err) => return Err(err),
            }
        }
    }
}
