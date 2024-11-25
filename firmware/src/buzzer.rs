use core::fmt;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::{AnyPin, OutputPin};
use esp_hal::ledc::{channel, timer, LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::peripheral::Peripheral;
use esp_hal::peripherals;
use esp_hal::prelude::*;
use log::{debug, info};

/// PWM duty cycle to use for tones (percentage, 0-100)
/// For an active low buzzer, 75% duty cycle means 25% active time.
/// 50% produces max volume, 100% is off
#[cfg(not(debug_assertions))]
const TONE_DUTY_CYCLE: u8 = 75;
#[cfg(debug_assertions)]
const TONE_DUTY_CYCLE: u8 = 95;

/// Buzzer error
#[derive(Debug)]
pub enum Error {
    /// PWM timer error
    Timer(timer::Error),
    /// PWM channel error
    Channel(channel::Error),
}

impl From<timer::Error> for Error {
    fn from(err: timer::Error) -> Self {
        Self::Timer(err)
    }
}

impl From<channel::Error> for Error {
    fn from(err: channel::Error) -> Self {
        Self::Channel(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timer(_err) => write!(f, "PWM timer error"),
            Self::Channel(_err) => write!(f, "PWM channel error"),
        }
    }
}

/// Passive buzzer (driven by PWM signal on GPIO)
pub struct Buzzer<'a> {
    ledc: Ledc<'a>,
    pin: AnyPin,
}

impl<'a> Buzzer<'a> {
    /// Create new buzzer driver
    pub fn new(ledc: impl Peripheral<P = peripherals::LEDC> + 'a, pin: impl OutputPin) -> Self {
        debug!("Buzzer: Initializing PWM controller...");

        let mut ledc = Ledc::new(ledc);
        ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

        info!("Buzzer: PWM controller initialized");
        Self {
            ledc,
            pin: pin.degrade(),
        }
    }

    /// Drive the buzzer with a PWM signal of given frequency and duty cycle
    pub fn drive(&mut self, frequency: u32, duty_pct: u8) -> Result<(), Error> {
        // debug!("Buzzer: driving {} Hz at {}%", frequency, duty_pct);
        let mut timer = self.ledc.timer::<LowSpeed>(timer::Number::Timer0);
        timer.configure(timer::config::Config {
            duty: timer::config::Duty::Duty13Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: frequency.Hz(),
        })?;
        let mut channel = self.ledc.channel(channel::Number::Channel0, &mut self.pin);
        channel.configure(channel::config::Config {
            timer: &timer,
            duty_pct,
            pin_config: channel::config::PinConfig::PushPull,
        })?;
        Ok(())
    }

    /// Stop driving the buzzer
    pub fn off(&mut self) -> Result<(), Error> {
        // To turn off the buzzer, use 100% duty cycle so the output keeps staying high
        self.drive(1, 100)?;
        Ok(())
    }

    /// Output the given tone for given duration
    pub async fn tone(&mut self, frequency: u32, duration: Duration) -> Result<(), Error> {
        // debug!("Buzzer: playing {} Hz for {}", frequency, duration);
        self.drive(frequency, TONE_DUTY_CYCLE)?;
        Timer::after(duration).await;
        self.off()?;
        Ok(())
    }

    /// Output startup/testing tone
    pub async fn startup(&mut self) -> Result<(), Error> {
        debug!("Buzzer: Playing startup tone");
        self.tone(3136, Duration::from_millis(1000)).await // G7
    }

    /// Output a short confirmation tone
    pub async fn confirm(&mut self) -> Result<(), Error> {
        debug!("Buzzer: Playing confirm tone");
        self.tone(3136, Duration::from_millis(100)).await // G7
    }

    /// Output a long denying tone
    pub async fn deny(&mut self) -> Result<(), Error> {
        debug!("Buzzer: Playing deny tone");
        self.tone(392, Duration::from_millis(500)).await?; // G4
        Timer::after(Duration::from_millis(1000)).await;
        Ok(())
    }

    /// Output an error tone
    pub async fn error(&mut self) -> Result<(), Error> {
        debug!("Buzzer: Playing error tone");
        self.tone(784, Duration::from_millis(200)).await?; // G5
        Timer::after(Duration::from_millis(10)).await;
        self.tone(587, Duration::from_millis(200)).await?; // D5
        Timer::after(Duration::from_millis(10)).await;
        self.tone(392, Duration::from_millis(500)).await?; // G4
        Timer::after(Duration::from_millis(1000)).await;
        Ok(())
    }
}
