use embassy_time::{Duration, Timer};
use esp_hal::clock::Clocks;
use esp_hal::gpio::AnyPin;
use esp_hal::ledc::{channel, timer, LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::peripheral::Peripheral;
use esp_hal::peripherals::LEDC;
use esp_hal::prelude::*;
use log::{debug, info};

/// Buzzer error
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// PWM timer error
    Timer(timer::Error),
    /// PWM channel error
    #[allow(dead_code)]
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

/// Passive buzzer (driven by PWM signal on GPIO)
pub struct Buzzer<'a> {
    ledc: Ledc<'a>,
    pin: AnyPin<'a>,
}

impl<'a> Buzzer<'a> {
    /// Create new buzzer driver
    pub fn new(
        ledc: impl Peripheral<P = LEDC> + 'a,
        clock_control_config: &'a Clocks<'_>,
        pin: AnyPin<'a>,
    ) -> Self {
        debug!("Buzzer: Initializing PWM controller...");

        let mut ledc = Ledc::new(ledc, clock_control_config);
        ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

        info!("Buzzer: PWM controller initialized");
        Self { ledc, pin }
    }

    /// Drive the buzzer with a PWM signal of given frequency and duty cycle
    pub fn drive(&mut self, frequency: u32, duty_pct: u8) -> Result<(), Error> {
        // debug!("Buzzer: driving {} Hz at {}%", frequency, duty_pct);
        let mut timer = self.ledc.get_timer::<LowSpeed>(timer::Number::Timer0);
        timer.configure(timer::config::Config {
            duty: timer::config::Duty::Duty13Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: frequency.Hz(),
        })?;
        let mut channel = self
            .ledc
            .get_channel(channel::Number::Channel0, &mut self.pin);
        channel.configure(channel::config::Config {
            timer: &timer,
            duty_pct,
            pin_config: channel::config::PinConfig::PushPull,
        })?;
        Ok(())
    }

    /// Output the given tone for given duration
    pub async fn tone(&mut self, frequency: u32, duration: Duration) -> Result<(), Error> {
        // debug!("Buzzer: playing {} Hz for {}", frequency, duration);
        self.drive(frequency, 50)?;
        Timer::after(duration).await;
        // To turn off the buzzer, use 100% duty cycle so the output keeps staying high
        self.drive(1, 100)?;
        Ok(())
    }

    /// Output startup/testing tone
    pub async fn startup(&mut self) -> Result<(), Error> {
        debug!("Buzzer: Playing startup tone");
        self.tone(3000, Duration::from_millis(1000)).await
    }

    /// Output a short confirmation tone
    pub async fn short_confirmation(&mut self) -> Result<(), Error> {
        debug!("Buzzer: Playing short confirmation tone");
        self.tone(3000, Duration::from_millis(100)).await
    }

    /// Output an error tone
    pub async fn error(&mut self) -> Result<(), Error> {
        debug!("Buzzer: Playing error tone");
        self.tone(800, Duration::from_millis(200)).await?;
        Timer::after(Duration::from_millis(50)).await;
        self.tone(600, Duration::from_millis(200)).await?;
        Timer::after(Duration::from_millis(50)).await;
        self.tone(400, Duration::from_millis(500)).await?;
        Ok(())
    }
}
