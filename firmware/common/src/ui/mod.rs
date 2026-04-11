mod auth;
mod common;
mod error;
mod purchase;
mod splash;

use crate::article::ArticleId;
use crate::ota::{self, Ota};
use crate::telemetry::Event;
use crate::user::{User, UserId};
use crate::util::RectangleExt;
use crate::{
    Backend, BackendResources, Buzzer, Display, Frontend, FrontendResources, Network, Updater,
};
use alloc::string::ToString;
use derive_more::{Display, From};
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, TimeoutError, Timer, with_timeout};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use log::{debug, error, info, warn};

/// Default timeout after which a user interaction is cancelled by default
#[cfg(not(debug_assertions))]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(debug_assertions)]
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// User interface error
#[derive(Debug, Display, From)]
enum Error<FE: Frontend> {
    /// User interaction timeout
    #[display("Timeout waiting for user input")]
    UserTimeout,
    /// User cancelled the flow
    #[display("User cancelled")]
    Cancelled,
    /// The specified article was not found
    #[display("Article not found")]
    ArticleNotFound,
    /// Current time is required but not set
    #[display("Unknown current time")]
    CurrentTimeNotSet,
    /// Display output error
    #[display("Display: {_0}")]
    Display(FE::DisplayError),
    /// Keypad error
    #[display("Keypad: {_0}")]
    Keypad(FE::KeypadError),
    /// NFC reader error
    #[display("NFC: {_0}")]
    Nfc(FE::NfcError),
    /// Vereinsflieger API error
    #[display("Vereinsflieger: {_0}")]
    #[from]
    Vereinsflieger(crate::vereinsflieger::Error),
    /// OTA error
    #[display("OTA: {_0}")]
    #[from]
    Ota(ota::Error),
}

/// User interface error with optional user id
#[derive(Display)]
#[display("{_0}")]
struct ErrorWithUser<FE: Frontend>(Error<FE>, Option<UserId>);

impl<FE: Frontend> From<Error<FE>> for ErrorWithUser<FE> {
    fn from(err: Error<FE>) -> Self {
        Self(err, None)
    }
}

impl<FE: Frontend> ErrorWithUser<FE> {
    /// Try running the provided closure and associate the given user id with any error returned
    #[expect(dead_code)]
    pub fn try_as_user<R, F>(user_id: UserId, f: F) -> Result<R, Self>
    where
        F: FnOnce() -> Result<R, Error<FE>>,
    {
        f().map_err(|err| Self(err, Some(user_id)))
    }

    /// Try running the provided closure and associate the given user id with any error returned
    pub async fn try_as_user_async<R, F>(user_id: UserId, f: F) -> Result<R, Self>
    where
        F: AsyncFnOnce() -> Result<R, Error<FE>>,
    {
        f().await.map_err(|err| Self(err, Some(user_id)))
    }
}

/// User interface content
pub trait UiContent {
    /// Footer left text
    const FOOTER_LEFT: &str = "";

    /// Footer right text
    const FOOTER_RIGHT: &str = "";

    /// Draw user interface content
    ///
    /// # Errors
    ///
    /// An error will be returned if the content could not be drawn to the given target.
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error>;
}

/// User interface interaction
pub trait UiInteraction<FE: Frontend> {
    /// Type of value returned by this interaction
    type Output;

    /// Timeout after which the user interaction is cancelled
    const TIMEOUT: Option<Duration> = Some(DEFAULT_TIMEOUT);

    /// Run user interaction and return the determined value
    async fn run(
        &mut self,
        frontend: &mut FrontendResources<'_, FE>,
    ) -> Result<Self::Output, Error<FE>>;
}

/// User interface
struct UserInterface<UI>(pub UI);

impl<UI: UiContent> UserInterface<UI> {
    /// Run user interface (draw content, wait for interaction, return determined value). Ignores
    /// user interaction timeout errors and returns ok instead (useful for a final step).
    async fn run_timeout_ok<FE: Frontend>(
        &mut self,
        frontend: &mut FrontendResources<'_, FE>,
    ) -> Result<(), Error<FE>>
    where
        UI: UiInteraction<FE, Output = ()>,
    {
        match self.run(frontend).await {
            Err(Error::UserTimeout) => Ok(()),
            res => res,
        }
    }

    /// Run user interface (draw content, wait for interaction, return determined value)
    async fn run<FE: Frontend>(
        &mut self,
        frontend: &mut FrontendResources<'_, FE>,
    ) -> Result<UI::Output, Error<FE>>
    where
        UI: UiInteraction<FE>,
    {
        // Draw user interface
        self.draw(&mut frontend.display).map_err(Error::Display)?;
        frontend.display.flush().await.map_err(Error::Display)?;

        // Run user interaction with or without timeout
        if let Some(timeout) = UI::TIMEOUT {
            with_timeout(timeout, self.0.run(frontend))
                .await
                .map_err(|TimeoutError| Error::UserTimeout)?
        } else {
            self.0.run(frontend).await
        }
    }

    /// Draw user interface
    pub fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        // Clear screen
        target.clear(BinaryColor::Off)?;

        // Draw content full-screen (no footer)
        if UI::FOOTER_LEFT.is_empty() && UI::FOOTER_RIGHT.is_empty() {
            self.0.draw(target)?;

        // Draw footer and content in a clipped area
        } else {
            let (content_box, footer_box) = target.bounding_box().footer(common::FOOTER_HEIGHT);

            // Draw footer
            let mut footer_area = target.cropped(&footer_box);
            common::Footer(UI::FOOTER_LEFT, UI::FOOTER_RIGHT).draw(&mut footer_area)?;

            // Draw content
            let mut content_area = target.clipped(&content_box);
            self.0.draw(&mut content_area)?;
        }

        Ok(())
    }
}

/// Display waiting screen
async fn show_please_wait<FE: Frontend>(
    frontend: &mut FrontendResources<'_, FE>,
    reason: common::PleaseWait,
) -> Result<(), Error<FE>> {
    UserInterface(reason)
        .draw(&mut frontend.display)
        .map_err(Error::Display)?;
    frontend.display.flush().await.map_err(Error::Display)?;
    Ok(())
}

/// Wait for network to become available (if not already)
async fn wait_network_up<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> Result<(), Error<FE>> {
    if backend.network.is_up() {
        return Ok(());
    }

    info!("UI: Waiting for network to become available...");
    show_please_wait(frontend, common::PleaseWait::WifiConnecting).await?;

    backend.network.wait_up().await;

    Ok(())
}

/// Run main user interface loop
pub async fn run<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> ! {
    // Track system start
    Event::SystemStart.track(&mut backend.telemetry);

    // Show splash screen for a while
    let _ = UserInterface(splash::Splash).run_timeout_ok(frontend).await;

    // Run initialization flow once
    loop {
        #[expect(clippy::match_same_arms)]
        match run_init_flow(frontend, backend).await {
            // Success: continue
            Ok(()) => break,
            // User interaction timeout: continue
            Err(Error::UserTimeout) => break,
            // User cancelled: continue
            Err(Error::Cancelled) => break,
            // Display error to user and try again
            Err(err) => {
                error!("Initialization error: {err}");
                Event::Error(None, err.to_string()).track(&mut backend.telemetry);
                let _ = submit_telemetry(frontend, backend).await;
                let _ = UserInterface(error::ErrorMessage::new(err, false))
                    .run_timeout_ok(frontend)
                    .await;
            }
        }
    }

    // Run main user interface flow forever
    loop {
        match run_main_flow(frontend, backend).await {
            // Success: start over again
            Ok(()) => (),
            // User interaction timeout: start over again
            Err(ErrorWithUser(Error::UserTimeout, _)) => {
                info!("Timeout waiting for user, starting over...");
            }
            // User cancelled: start over again
            Err(ErrorWithUser(Error::Cancelled, _)) => info!("User cancelled, starting over..."),
            // Display error to user and start over again
            Err(ErrorWithUser(error, opt_user_id)) => {
                error!("Error: {error}");
                Event::Error(opt_user_id, error.to_string()).track(&mut backend.telemetry);
                let _ = submit_telemetry(frontend, backend).await;
                let _ = UserInterface(error::ErrorMessage::new(error, opt_user_id.is_some()))
                    .run_timeout_ok(frontend)
                    .await;
            }
        }
    }
}

/// Run initialization flow
async fn run_init_flow<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> Result<(), Error<FE>> {
    // Run daily schedule
    run_schedule(frontend, backend).await?;

    Ok(())
}

/// Run main user interface flow
async fn run_main_flow<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> Result<(), ErrorWithUser<FE>> {
    // Submit telemetry data if needed
    submit_telemetry(frontend, backend).await?;

    // Wait for user authentication or schedule time
    let schedule_timer = backend.schedule.timer();
    let (user_id, user) = match select(authenticate_user(frontend, backend), schedule_timer).await {
        // User authenticated
        Either::First(res) => res?,
        // Schedule time
        Either::Second(()) => {
            run_schedule(frontend, backend).await?;
            return Ok(());
        }
    };

    // Run purchase flow with authorized user
    ErrorWithUser::try_as_user_async(user_id, async || {
        run_purchase_flow(frontend, backend, user_id, &user).await
    })
    .await
}

/// Run daily schedule
async fn run_schedule<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> Result<(), Error<FE>> {
    info!("UI: Running schedule...");

    // Schedule next event
    backend.schedule.schedule_next();

    // Check for OTA update
    if !BE::Updater::recently_restarted() {
        check_ota_update(frontend, backend).await?;
    }

    // Refresh article and user information
    refresh_articles_and_users(frontend, backend).await?;

    Ok(())
}

/// Check for OTA update
async fn check_ota_update<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> Result<(), Error<FE>> {
    // Wait for network to become available (if not already)
    wait_network_up(frontend, backend).await?;

    info!("UI: Checking for OTA update...");
    show_please_wait(frontend, common::PleaseWait::UpdateCheck).await?;

    // Check for latest release
    let mut ota = Ota::new(&mut backend.http);
    let new_version = {
        match ota.check().await.map_err(Error::Ota)? {
            Some(new_version) => new_version,
            None => return Ok(()),
        }
    };

    // Don't actually apply OTA update in debug mode or when no updater is available
    if backend.updater.is_none() || cfg!(debug_assertions) {
        warn!("UI: Automatic OTA update unavailable. Please update manually.");
        return Ok(());
    }

    // Do automatic OTA update when updater is available
    if let Some(ref mut updater) = backend.updater {
        show_please_wait(frontend, common::PleaseWait::UpdatingFirmware).await?;

        // Download and apply OTA update
        ota.update(updater, &new_version)
            .await
            .map_err(Error::Ota)?;

        // OTA update completed, restart system
        BE::Updater::restart();
    }

    Ok(())
}

/// Refresh article and user information
async fn refresh_articles_and_users<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> Result<(), Error<FE>> {
    // Wait for network to become available (if not already)
    wait_network_up(frontend, backend).await?;

    info!("UI: Refreshing articles and users...");
    show_please_wait(frontend, common::PleaseWait::UpdatingData).await?;

    // Connect to Vereinsflieger API
    let mut vf = backend.vereinsflieger.connect(&mut backend.http).await?;

    // Set current date and time based on time gathered from API response
    if let Some(time_reference) = vf.last_response_time() {
        backend.rtc.set_by_reference(time_reference);
    }

    // Refresh article information
    debug!("UI: Refreshing articles...");
    let today = backend.rtc.today().ok_or(Error::CurrentTimeNotSet)?;
    backend.articles.clear();
    let total_articles = vf
        .get_articles(async |article| {
            backend
                .articles
                .update_vereinsflieger_article(article, today);
        })
        .await?;
    info!(
        "UI: Refreshed {} of {} articles",
        backend.articles.count(),
        total_articles
    );

    // Refresh user information
    debug!("UI: Refreshing users...");
    backend.users.clear();
    let total_users = vf
        .get_users(async |user| {
            backend.users.update_vereinsflieger_user(user);
        })
        .await?;
    info!(
        "UI: Refreshed {} of {} users",
        backend.users.count(),
        total_users
    );

    Event::DataRefreshed(
        backend.articles.count(),
        backend.users.count_uids(),
        backend.users.count(),
    )
    .track(&mut backend.telemetry);

    Ok(())
}

/// Submit telemetry data if needed
async fn submit_telemetry<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> Result<(), Error<FE>> {
    if !backend.telemetry.needs_flush() {
        return Ok(());
    }

    // Wait for network to become available (if not already)
    wait_network_up(frontend, backend).await?;

    info!("UI: Submitting telemetry data...");
    show_please_wait(frontend, common::PleaseWait::SubmittingTelemetry).await?;

    // Submit telemetry data, ignore any error
    let _ = backend
        .telemetry
        .flush(&mut backend.http, &backend.rtc)
        .await;

    Ok(())
}

/// Wait for user authentication
async fn authenticate_user<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
) -> Result<(UserId, User), ErrorWithUser<FE>> {
    loop {
        let uid = UserInterface(auth::Authentication).run(frontend).await?;

        // Look up user by detected NFC uid
        if let Some((user_id, user)) = backend.users.get_by_uid(&uid) {
            // User found, authorized
            info!("UI: NFC card {uid} identified as user {user_id}");
            Event::UserAuthenticated(user_id, uid).track(&mut backend.telemetry);
            frontend.buzzer.confirm().await;
            break Ok((user_id, user.clone()));
        }

        // User not found, unauthorized
        info!("UI: NFC card {uid} unknown, rejecting");
        Event::AuthenticationFailed(uid).track(&mut backend.telemetry);
        frontend.buzzer.deny().await;
        Timer::after_secs(1).await;
    }
}

/// Run purchase flow with given user
async fn run_purchase_flow<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
    user_id: UserId,
    user: &User,
) -> Result<(), Error<FE>> {
    // Ask for article to purchase
    let article_idx = UserInterface(purchase::SelectArticle::new(
        &mut backend.rng,
        &user.name,
        &backend.articles,
    ))
    .run(frontend)
    .await?;

    // Get article information
    let (article_id, article) = backend
        .articles
        .get_by_index(article_idx)
        .ok_or(Error::ArticleNotFound)?;
    let article_id = article_id.clone();
    let article = article.clone();

    // Ask for amount to purchase
    let amount = UserInterface(purchase::EnterAmount::new(&article))
        .run(frontend)
        .await?;

    // Calculate total price. It's ok to cast amount to f32 as it's always a small number.
    #[expect(clippy::cast_precision_loss)]
    let total_price = article.price * amount as f32;

    // Show total price and ask for confirmation
    UserInterface(purchase::Checkout::new(&article, amount, total_price))
        .run(frontend)
        .await?;

    // Submit purchase
    #[expect(clippy::cast_precision_loss)]
    submit_purchase(
        frontend,
        backend,
        article_id,
        amount as f32,
        user_id,
        total_price,
    )
    .await?;

    // Show success and affirm to take items
    frontend.buzzer.confirm().await;
    UserInterface(purchase::Success::new(amount))
        .run_timeout_ok(frontend)
        .await?;

    Ok(())
}

/// Submit purchase for the given article
async fn submit_purchase<FE: Frontend, BE: Backend>(
    frontend: &mut FrontendResources<'_, FE>,
    backend: &mut BackendResources<'_, BE>,
    article_id: ArticleId,
    amount: f32,
    user_id: UserId,
    total_price: f32,
) -> Result<(), Error<FE>> {
    // Wait for network to become available (if not already)
    wait_network_up(frontend, backend).await?;

    // Submit purchase
    info!("UI: Purchasing {amount}x {article_id}, {total_price:.02} EUR for user {user_id}...");
    show_please_wait(frontend, common::PleaseWait::Purchasing).await?;

    // Connect to Vereinsflieger API
    let mut vf = backend.vereinsflieger.connect(&mut backend.http).await?;

    // Submit purchase
    let today = backend.rtc.today().ok_or(Error::CurrentTimeNotSet)?;
    vf.purchase(today, &article_id, amount, user_id, total_price)
        .await?;
    Event::ArticlePurchased(user_id, article_id, amount, total_price).track(&mut backend.telemetry);

    Ok(())
}
