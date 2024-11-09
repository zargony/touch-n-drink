use crate::article::{ArticleId, Articles};
use crate::buzzer::Buzzer;
use crate::display::Display;
use crate::error::Error;
use crate::keypad::{Key, Keypad};
use crate::nfc::Nfc;
use crate::screen;
use crate::user::{UserId, Users};
use crate::vereinsflieger::Vereinsflieger;
use crate::wifi::Wifi;
use alloc::string::String;
use core::convert::Infallible;
use core::fmt;
use embassy_futures::select::{select, Either};
use embassy_time::{with_timeout, Duration, TimeoutError, Timer};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::i2c::I2c;
use log::info;
use rand_core::RngCore;

/// How long to show the splash screen if no key is pressed
const SPLASH_TIMEOUT: Duration = Duration::from_secs(5);

/// How long to wait for network to become available
const NETWORK_TIMEOUT: Duration = Duration::from_secs(30);

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

/// User interface
pub struct Ui<'a, RNG, I2C, IRQ> {
    rng: RNG,
    display: &'a mut Display<I2C>,
    keypad: &'a mut Keypad<'a, 3, 4>,
    nfc: &'a mut Nfc<I2C, IRQ>,
    buzzer: &'a mut Buzzer<'a>,
    wifi: &'a Wifi,
    vereinsflieger: &'a mut Vereinsflieger<'a>,
    articles: &'a mut Articles<1>,
    users: &'a mut Users,
}

impl<'a, RNG: RngCore, I2C: I2c, IRQ: Wait<Error = Infallible>> Ui<'a, RNG, I2C, IRQ> {
    /// Create user interface with given human interface devices
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rng: RNG,
        display: &'a mut Display<I2C>,
        keypad: &'a mut Keypad<'a, 3, 4>,
        nfc: &'a mut Nfc<I2C, IRQ>,
        buzzer: &'a mut Buzzer<'a>,
        wifi: &'a Wifi,
        vereinsflieger: &'a mut Vereinsflieger<'a>,
        articles: &'a mut Articles<1>,
        users: &'a mut Users,
    ) -> Self {
        Self {
            rng,
            display,
            keypad,
            nfc,
            buzzer,
            wifi,
            vereinsflieger,
            articles,
            users,
        }
    }

    /// Save power by turning off devices not needed during idle
    pub async fn power_save(&mut self) -> Result<(), Error> {
        info!("UI: Power saving...");

        self.display.turn_off().await?;
        Ok(())
    }

    /// Show splash screen and wait for keypress or timeout
    pub async fn show_splash(&mut self) -> Result<(), Error> {
        info!("UI: Displaying splash screen");

        self.display.screen(&screen::Splash).await?;

        match with_timeout(SPLASH_TIMEOUT, self.keypad.read()).await {
            // Key pressed
            Ok(_key) => Ok(()),
            // User interaction timeout
            Err(TimeoutError) => Err(Error::UserTimeout),
        }
    }

    /// Show error screen and wait for keypress or timeout
    pub async fn show_error<M: fmt::Display>(
        &mut self,
        message: M,
        interactive: bool,
    ) -> Result<(), Error> {
        info!("UI: Displaying error: {}", message);

        self.display.screen(&screen::Failure::new(message)).await?;
        if interactive {
            let _ = self.buzzer.error().await;
        }

        // Wait at least 1s without responding to keypad
        let min_time = Duration::from_secs(1);
        Timer::after(min_time).await;

        let wait_cancel = async { while self.keypad.read().await != Key::Cancel {} };
        match with_timeout(USER_TIMEOUT - min_time, wait_cancel).await {
            // Cancel key cancels
            Ok(()) => Ok(()),
            // User interaction timeout
            Err(TimeoutError) => Err(Error::UserTimeout),
        }
    }

    /// Wait for network to become available (if not already). Show a waiting screen and allow to
    /// cancel
    pub async fn wait_network_up(&mut self) -> Result<(), Error> {
        if self.wifi.is_up() {
            return Ok(());
        }

        info!("UI: Waiting for network to become available...");

        self.display
            .screen(&screen::PleaseWait::WifiConnecting)
            .await?;

        let wait_cancel = async { while self.keypad.read().await != Key::Cancel {} };
        match with_timeout(NETWORK_TIMEOUT, select(self.wifi.wait_up(), wait_cancel)).await {
            // Network has become available
            Ok(Either::First(())) => Ok(()),
            // Cancel key cancels
            Ok(Either::Second(())) => Err(Error::Cancel),
            // Timeout waiting for network
            Err(TimeoutError) => Err(Error::NoNetwork),
        }
    }

    /// Refresh article and user information
    pub async fn refresh_articles_and_users(&mut self) -> Result<(), Error> {
        // Wait for network to become available (if not already)
        self.wait_network_up().await?;

        info!("UI: Refreshing articles and users...");

        self.display
            .screen(&screen::PleaseWait::ApiQuerying)
            .await?;
        let mut vf = self.vereinsflieger.connect().await?;

        // Show authenticated user information when debugging
        #[cfg(debug_assertions)]
        vf.get_user_information().await?;

        // Refresh article information
        vf.refresh_articles(self.articles).await?;

        // Refresh user information
        vf.refresh_users(self.users).await?;

        Ok(())
    }

    /// Initialize user interface
    pub async fn init(&mut self) -> Result<(), Error> {
        // Wait for network to become available (if not already)
        self.wait_network_up().await?;

        // Refresh articles and users
        self.refresh_articles_and_users().await?;

        Ok(())
    }

    /// Run the user interface flow
    pub async fn run(&mut self) -> Result<(), Error> {
        // Wait for id card and verify identification
        let user_id = self.authenticate_user().await?;
        let user = self.users.get(user_id);

        // Ask for number of drinks
        let name = user.map_or(String::new(), |u| u.name.clone());
        let num_drinks = self.get_number_of_drinks(&name).await?;

        // Get article information
        let article_id = self.articles.id(0).ok_or(Error::ArticleNotFound)?.clone();
        let article = self.articles.get(0).ok_or(Error::ArticleNotFound)?;

        // Calculate total price. It's ok to cast num_drinks to f32 as it's always a small number.
        #[allow(clippy::cast_precision_loss)]
        let amount = num_drinks as f32;
        let total_price = article.price * amount;

        // Show total price and ask for confirmation
        self.confirm_purchase(num_drinks, total_price).await?;

        // Store purchase
        self.purchase(&article_id, amount, user_id, total_price)
            .await?;

        // Show success and affirm to take drinks
        self.show_success(num_drinks).await?;

        Ok(())
    }
}

impl<'a, RNG: RngCore, I2C: I2c, IRQ: Wait<Error = Infallible>> Ui<'a, RNG, I2C, IRQ> {
    /// Authentication: wait for id card, read it and look up the associated user. On idle timeout,
    /// enter power saving (turn off display). Any key pressed leaves power saving (turn on
    /// display).
    async fn authenticate_user(&mut self) -> Result<UserId, Error> {
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
                // Id card detected
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
            saving_power = false;
            // Look up user id by detected NFC uid
            if let Some(id) = self.users.id(&uid) {
                // User found, authorized
                info!("UI: NFC card {} identified as user {}", uid, id);
                let _ = self.buzzer.confirm().await;
                break Ok(id);
            }
            // User not found, unauthorized
            info!("UI: NFC card {} unknown, rejecting", uid);
            let _ = self.buzzer.deny().await;
        }
    }

    /// Ask for number of drinks
    async fn get_number_of_drinks(&mut self, name: &str) -> Result<usize, Error> {
        info!("UI: Asking for number of drinks...");

        self.display
            .screen(&screen::NumberOfDrinks::new(&mut self.rng, name))
            .await?;
        loop {
            #[allow(clippy::match_same_arms)]
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await {
                // Any digit 1..=9 selects number of drinks
                Ok(Key::Digit(n)) if (1..=9).contains(&n) => return Ok(n as usize),
                // Ignore any other digit
                Ok(Key::Digit(_)) => (),
                // Cancel key cancels
                Ok(Key::Cancel) => return Err(Error::Cancel),
                // Ignore any other key
                Ok(_) => (),
                // User interaction timeout
                Err(TimeoutError) => return Err(Error::UserTimeout),
            }
        }
    }

    /// Show total price and ask for confirmation
    async fn confirm_purchase(&mut self, num_drinks: usize, total_price: f32) -> Result<(), Error> {
        info!(
            "UI: Asking for purchase confirmation of {} drinks, {:.02} EUR...",
            num_drinks, total_price
        );

        self.display
            .screen(&screen::Checkout::new(num_drinks, total_price))
            .await?;
        loop {
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await {
                // Enter key confirms purchase
                Ok(Key::Enter) => return Ok(()),
                // Cancel key cancels
                Ok(Key::Cancel) => return Err(Error::Cancel),
                // Ignore any other key
                Ok(_) => (),
                // User interaction timeout
                Err(TimeoutError) => return Err(Error::UserTimeout),
            }
        }
    }

    /// Purchase the given article
    async fn purchase(
        &mut self,
        article_id: &ArticleId,
        amount: f32,
        user_id: UserId,
        total_price: f32,
    ) -> Result<(), Error> {
        // Wait for network to become available (if not already)
        self.wait_network_up().await?;

        info!(
            "UI: Purchasing {}x {}, {:.02} EUR for user {}...",
            amount, article_id, total_price, user_id
        );

        self.display
            .screen(&screen::PleaseWait::ApiQuerying)
            .await?;
        let mut vf = self.vereinsflieger.connect().await?;

        // Store purchase
        vf.purchase(article_id, amount, user_id, total_price)
            .await?;

        Ok(())
    }

    /// Show success screen and wait for keypress or timeout
    async fn show_success(&mut self, num_drinks: usize) -> Result<(), Error> {
        info!("UI: Displaying success, {} drinks", num_drinks);

        self.display
            .screen(&screen::Success::new(num_drinks))
            .await?;
        let _ = self.buzzer.confirm().await;

        // Wait at least 1s without responding to keypad
        let min_time = Duration::from_secs(1);
        Timer::after(min_time).await;

        let wait_cancel = async { while self.keypad.read().await != Key::Enter {} };
        match with_timeout(USER_TIMEOUT - min_time, wait_cancel).await {
            // Enter key continues
            Ok(()) => Ok(()),
            // User interaction timeout
            Err(TimeoutError) => Err(Error::UserTimeout),
        }
    }
}
