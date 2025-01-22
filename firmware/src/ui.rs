use crate::article::{Article, ArticleId, Articles};
use crate::buzzer::Buzzer;
use crate::display::Display;
use crate::error::{Error, ErrorKind};
use crate::http::Http;
use crate::keypad::{Key, Keypad};
use crate::nfc::Nfc;
use crate::schedule::Daily;
use crate::screen;
use crate::telemetry::{Event, Telemetry};
use crate::user::{UserId, Users};
use crate::vereinsflieger::Vereinsflieger;
use crate::wifi::Wifi;
use alloc::string::{String, ToString};
use core::convert::Infallible;
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
const USER_TIMEOUT: Duration = Duration::from_secs(10);

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
    http: &'a mut Http<'a>,
    vereinsflieger: &'a mut Vereinsflieger<'a>,
    articles: &'a mut Articles,
    users: &'a mut Users,
    telemetry: &'a mut Telemetry<'a>,
    schedule: &'a mut Daily,
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
        http: &'a mut Http<'a>,
        vereinsflieger: &'a mut Vereinsflieger<'a>,
        articles: &'a mut Articles,
        users: &'a mut Users,
        telemetry: &'a mut Telemetry<'a>,
        schedule: &'a mut Daily,
    ) -> Self {
        Self {
            rng,
            display,
            keypad,
            nfc,
            buzzer,
            wifi,
            http,
            vereinsflieger,
            articles,
            users,
            telemetry,
            schedule,
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

        let _ = with_timeout(SPLASH_TIMEOUT, self.keypad.read()).await;
        Ok(())
    }

    /// Show error screen and wait for keypress or timeout
    pub async fn show_error(&mut self, error: &Error) -> Result<(), Error> {
        info!("UI: Displaying error: {}", error);

        self.display.screen(&screen::Failure::new(error)).await?;

        // Sound the error buzzer if the error was caused by a user's interaction
        if error.user_id().is_some() {
            let _ = self.buzzer.error().await;
        }

        self.telemetry
            .track(Event::Error(error.user_id(), error.to_string()));

        // Wait at least 1s without responding to keypad
        let min_time = Duration::from_secs(1);
        Timer::after(min_time).await;

        let wait_cancel = async { while self.keypad.read().await != Key::Cancel {} };
        match with_timeout(USER_TIMEOUT - min_time, wait_cancel).await {
            // Cancel key cancels
            Ok(()) => Ok(()),
            // User interaction timeout
            Err(TimeoutError) => Err(ErrorKind::UserTimeout)?,
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
            Ok(Either::Second(())) => Err(ErrorKind::Cancel)?,
            // Timeout waiting for network
            Err(TimeoutError) => Err(ErrorKind::NoNetwork)?,
        }
    }

    /// Refresh article and user information
    pub async fn refresh_articles_and_users(&mut self) -> Result<(), Error> {
        // Wait for network to become available (if not already)
        self.wait_network_up().await?;

        info!("UI: Refreshing articles and users...");

        self.display
            .screen(&screen::PleaseWait::UpdatingData)
            .await?;

        // Connect to Vereinsflieger API
        let mut vf = self.vereinsflieger.connect(self.http).await?;

        // Show authenticated user information when debugging
        #[cfg(debug_assertions)]
        vf.get_user_information().await?;

        // Refresh article information
        vf.refresh_articles(self.articles).await?;

        // Refresh user information
        vf.refresh_users(self.users).await?;

        // Close connection to Vereinsflieger API
        drop(vf);

        self.telemetry.track(Event::DataRefreshed(
            self.articles.count(),
            self.users.count_uids(),
            self.users.count(),
        ));

        // Submit telemetry data if needed
        self.submit_telemetry().await?;

        Ok(())
    }

    /// Submit telemetry data if needed
    pub async fn submit_telemetry(&mut self) -> Result<(), Error> {
        if !self.telemetry.needs_flush() {
            return Ok(());
        }

        // Wait for network to become available (if not already)
        self.wait_network_up().await?;

        info!("UI: Submitting telemetry data...");

        self.display
            .screen(&screen::PleaseWait::SubmittingTelemetry)
            .await?;

        // Submit telemetry data, ignore any error
        let _ = self.telemetry.flush(self.http).await;

        Ok(())
    }

    /// Initialize user interface
    pub async fn init(&mut self) -> Result<(), Error> {
        // Show splash screen for a while
        self.show_splash().await?;

        // Wait for network to become available (if not already)
        self.wait_network_up().await?;

        // Refresh articles and users
        self.refresh_articles_and_users().await?;

        Ok(())
    }

    /// Run the user interface flow
    pub async fn run(&mut self) -> Result<(), Error> {
        // Submit telemetry data if needed
        self.submit_telemetry().await?;

        // Either wait for id card read or schedule time
        let schedule_timer = self.schedule.timer();
        let user_id = match select(self.authenticate_user(), schedule_timer).await {
            // Id card read
            Either::First(res) => res?,
            // Schedule time
            Either::Second(()) => {
                self.schedule().await?;
                return Ok(());
            }
        };

        Error::try_with_async(user_id, async {
            // Get user information
            let user = self.users.get(user_id);
            let user_name = user.map_or(String::new(), |u| u.name.clone());

            // Ask for article to purchase
            let article_idx = self.select_article(&user_name).await?;

            // Get article information
            let article_id = self
                .articles
                .id(article_idx)
                .ok_or(ErrorKind::ArticleNotFound)?
                .clone();
            let article = self
                .articles
                .get(&article_id)
                .ok_or(ErrorKind::ArticleNotFound)?
                .clone();

            // Ask for amount to purchase
            let amount = self.select_amount().await?;

            // Calculate total price. It's ok to cast amount to f32 as it's always a small number.
            #[allow(clippy::cast_precision_loss)]
            let total_price = article.price * amount as f32;

            // Show total price and ask for confirmation
            self.confirm_purchase(&article, amount, total_price).await?;

            // Store purchase
            #[allow(clippy::cast_precision_loss)]
            self.purchase(&article_id, amount as f32, user_id, total_price)
                .await?;

            // Submit telemetry data if needed
            self.submit_telemetry().await?;

            // Show success and affirm to take items
            self.show_success(amount).await?;

            Ok(())
        })
        .await
    }

    /// Run schedule
    pub async fn schedule(&mut self) -> Result<(), Error> {
        if self.schedule.is_expired() {
            info!("UI: Running schedule...");

            // Schedule next event
            self.schedule.schedule_next();

            // Refresh article and user information
            self.refresh_articles_and_users().await?;
        }
        Ok(())
    }
}

impl<RNG: RngCore, I2C: I2c, IRQ: Wait<Error = Infallible>> Ui<'_, RNG, I2C, IRQ> {
    /// Authentication: wait for id card, read it and look up the associated user. On idle timeout,
    /// enter power saving (turn off display). Any key pressed leaves power saving (turn on
    /// display).
    async fn authenticate_user(&mut self) -> Result<UserId, Error> {
        info!("UI: Waiting for NFC card...");

        loop {
            self.display.screen(&screen::ScanId).await?;

            // Wait for id card read or timeout
            #[allow(clippy::single_match_else)]
            let uid = match with_timeout(IDLE_TIMEOUT, self.nfc.read()).await {
                // Id card detected
                Ok(res) => res?,
                // Idle timeout, enter power saving
                Err(TimeoutError) => {
                    self.power_save().await?;
                    // Wait for id card read or keypress
                    match select(self.nfc.read(), self.keypad.read()).await {
                        // Id card detected
                        Either::First(res) => res?,
                        // Key pressed while saving power, leave power saving
                        Either::Second(_key) => continue,
                    }
                }
            };

            // Look up user id by detected NFC uid
            if let Some(user_id) = self.users.id(&uid) {
                // User found, authorized
                info!("UI: NFC card {} identified as user {}", uid, user_id);
                self.telemetry.track(Event::UserAuthenticated(user_id, uid));
                let _ = self.buzzer.confirm().await;
                break Ok(user_id);
            }

            // User not found, unauthorized
            info!("UI: NFC card {} unknown, rejecting", uid);
            self.telemetry.track(Event::AuthenticationFailed(uid));
            let _ = self.buzzer.deny().await;
        }
    }

    /// Ask for article to purchase
    async fn select_article(&mut self, name: &str) -> Result<usize, Error> {
        info!("UI: Asking to select article...");

        self.display
            .screen(&screen::SelectArticle::new(
                &mut self.rng,
                name,
                self.articles,
            ))
            .await?;
        let num_articles = self.articles.count_ids();
        loop {
            #[allow(clippy::match_same_arms)]
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await {
                // Any digit 1..=num_articles selects article
                Ok(Key::Digit(n)) if n >= 1 && n as usize <= num_articles => {
                    break Ok(n as usize - 1)
                }
                // Ignore any other digit
                Ok(Key::Digit(_)) => (),
                // Cancel key cancels
                Ok(Key::Cancel) => Err(ErrorKind::Cancel)?,
                // Ignore any other key
                Ok(_) => (),
                // User interaction timeout
                Err(TimeoutError) => Err(ErrorKind::UserTimeout)?,
            }
        }
    }

    /// Ask for amount to purchase
    async fn select_amount(&mut self) -> Result<usize, Error> {
        info!("UI: Asking to enter amount...");

        self.display.screen(&screen::EnterAmount).await?;
        loop {
            #[allow(clippy::match_same_arms)]
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await {
                // Any digit 1..=9 selects amount
                Ok(Key::Digit(n)) if (1..=9).contains(&n) => break Ok(n as usize),
                // Ignore any other digit
                Ok(Key::Digit(_)) => (),
                // Cancel key cancels
                Ok(Key::Cancel) => Err(ErrorKind::Cancel)?,
                // Ignore any other key
                Ok(_) => (),
                // User interaction timeout
                Err(TimeoutError) => Err(ErrorKind::UserTimeout)?,
            }
        }
    }

    /// Show total price and ask for confirmation
    async fn confirm_purchase(
        &mut self,
        article: &Article,
        amount: usize,
        total_price: f32,
    ) -> Result<(), Error> {
        info!(
            "UI: Asking for purchase confirmation of {}x {}, {:.02} EUR...",
            amount, article.name, total_price
        );

        self.display
            .screen(&screen::Checkout::new(article, amount, total_price))
            .await?;
        loop {
            match with_timeout(USER_TIMEOUT, self.keypad.read()).await {
                // Enter key confirms purchase
                Ok(Key::Enter) => break Ok(()),
                // Cancel key cancels
                Ok(Key::Cancel) => Err(ErrorKind::Cancel)?,
                // Ignore any other key
                Ok(_) => (),
                // User interaction timeout
                Err(TimeoutError) => Err(ErrorKind::UserTimeout)?,
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

        self.display.screen(&screen::PleaseWait::Purchasing).await?;

        // Connect to Vereinsflieger API
        let mut vf = self.vereinsflieger.connect(self.http).await?;

        // Store purchase
        vf.purchase(article_id, amount, user_id, total_price)
            .await?;
        self.telemetry.track(Event::ArticlePurchased(
            user_id,
            article_id.clone(),
            amount,
            total_price,
        ));

        Ok(())
    }

    /// Show success screen and wait for keypress or timeout
    async fn show_success(&mut self, amount: usize) -> Result<(), Error> {
        info!("UI: Displaying success, {} items", amount);

        self.display.screen(&screen::Success::new(amount)).await?;
        let _ = self.buzzer.confirm().await;

        // Wait at least 1s without responding to keypad
        let min_time = Duration::from_secs(1);
        Timer::after(min_time).await;

        let wait_cancel = async { while self.keypad.read().await != Key::Enter {} };
        match with_timeout(USER_TIMEOUT - min_time, wait_cancel).await {
            // Enter key continues
            Ok(()) => Ok(()),
            // User interaction timeout
            Err(TimeoutError) => Err(ErrorKind::UserTimeout)?,
        }
    }
}
