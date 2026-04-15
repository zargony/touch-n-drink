mod auth;
mod common;
mod error;
mod purchase;
mod splash;

use crate::article::{Article, ArticleId};
use crate::ota::{self, Ota};
use crate::telemetry::Event;
use crate::user::{User, UserId};
use crate::util::RectangleExt;
use crate::{Buzzer, Context, DeviceTypes, Display, Network, Updater};
use alloc::string::ToString;
use alloc::vec::Vec;
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
enum Error<D: DeviceTypes> {
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
    Display(D::DisplayError),
    /// Keypad error
    #[display("Keypad: {_0}")]
    Keypad(D::KeypadError),
    /// NFC reader error
    #[display("NFC: {_0}")]
    Nfc(D::NfcError),
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
struct ErrorWithUser<D: DeviceTypes>(Error<D>, Option<UserId>);

impl<D: DeviceTypes> From<Error<D>> for ErrorWithUser<D> {
    fn from(err: Error<D>) -> Self {
        Self(err, None)
    }
}

impl<D: DeviceTypes> ErrorWithUser<D> {
    /// Try running the provided closure and associate the given user id with any error returned
    #[expect(dead_code)]
    fn try_as_user<R, F>(user_id: UserId, f: F) -> Result<R, Self>
    where
        F: FnOnce() -> Result<R, Error<D>>,
    {
        f().map_err(|err| Self(err, Some(user_id)))
    }

    /// Try running the provided closure and associate the given user id with any error returned
    async fn try_as_user_async<R, F>(user_id: UserId, f: F) -> Result<R, Self>
    where
        F: AsyncFnOnce() -> Result<R, Error<D>>,
    {
        f().await.map_err(|err| Self(err, Some(user_id)))
    }
}

/// User interface frontend
pub(crate) struct Frontend<'fe, 'dev, D: DeviceTypes> {
    display: &'fe mut D::Display<'dev>,
    keypad: &'fe mut D::Keypad<'dev>,
    nfc: &'fe mut D::NfcReader<'dev>,
    buzzer: &'fe mut D::Buzzer<'dev>,
}

impl<'fe, 'dev, D: DeviceTypes> From<&'fe mut Context<'dev, D>> for Frontend<'fe, 'dev, D> {
    fn from(ctx: &'fe mut Context<'dev, D>) -> Self {
        Self {
            display: ctx.dev.display,
            keypad: ctx.dev.keypad,
            nfc: ctx.dev.nfc,
            buzzer: ctx.dev.buzzer,
        }
    }
}

/// User interface content
pub(crate) trait UiContent {
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
pub(crate) trait UiInteraction {
    /// Type of value returned by this interaction
    type Output;

    /// Timeout after which the user interaction is cancelled
    const TIMEOUT: Option<Duration> = Some(DEFAULT_TIMEOUT);

    /// Run user interaction and return the determined value
    async fn run<D: DeviceTypes>(
        &mut self,
        frontend: &mut Frontend<'_, '_, D>,
    ) -> Result<Self::Output, Error<D>>;
}

/// User interface
struct UserInterface<UI>(pub UI);

impl<UI: UiContent> UserInterface<UI> {
    /// Draw user interface
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
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

impl<UI: UiContent + UiInteraction> UserInterface<UI> {
    /// Run user interface (draw content, wait for interaction, return determined value)
    async fn run<D: DeviceTypes>(
        &mut self,
        ctx: &mut Context<'_, D>,
    ) -> Result<UI::Output, Error<D>> {
        // Draw user interface
        self.draw(ctx.dev.display).map_err(Error::Display)?;
        ctx.dev.display.flush().await.map_err(Error::Display)?;

        // Run user interaction with or without timeout
        if let Some(timeout) = UI::TIMEOUT {
            with_timeout(timeout, self.0.run(&mut Frontend::from(ctx)))
                .await
                .map_err(|TimeoutError| Error::UserTimeout)?
        } else {
            self.0.run(&mut Frontend::from(ctx)).await
        }
    }

    /// Run user interface (draw content, wait for interaction, return determined value). Ignores
    /// user interaction timeout errors and returns ok instead (useful for a final step).
    async fn run_timeout_ok<D: DeviceTypes>(
        &mut self,
        ctx: &mut Context<'_, D>,
    ) -> Result<(), Error<D>>
    where
        UI: UiInteraction<Output = ()>,
    {
        match self.run(ctx).await {
            Err(Error::UserTimeout) => Ok(()),
            res => res,
        }
    }
}

/// Display waiting screen
async fn show_please_wait<D: DeviceTypes>(
    display: &mut D::Display<'_>,
    reason: common::PleaseWait,
) -> Result<(), Error<D>> {
    UserInterface(reason)
        .draw(display)
        .map_err(Error::Display)?;
    display.flush().await.map_err(Error::Display)?;
    Ok(())
}

/// Wait for network to become available (if not already)
async fn wait_network_up<D: DeviceTypes>(ctx: &mut Context<'_, D>) -> Result<(), Error<D>> {
    if ctx.dev.network.is_up() {
        return Ok(());
    }

    info!("UI: Waiting for network to become available...");
    show_please_wait(ctx.dev.display, common::PleaseWait::WifiConnecting).await?;

    ctx.dev.network.wait_up().await;

    Ok(())
}

/// Run main user interface loop
pub(crate) async fn run<D: DeviceTypes>(ctx: &mut Context<'_, D>) -> ! {
    // Track system start
    Event::SystemStart.track(&mut ctx.telemetry);

    // Show splash screen for a while
    let _ = UserInterface(splash::Splash).run_timeout_ok(ctx).await;

    // Run initialization flow once
    loop {
        #[expect(clippy::match_same_arms)]
        match run_init_flow(ctx).await {
            // Success: continue
            Ok(()) => break,
            // User interaction timeout: continue
            Err(Error::UserTimeout) => break,
            // User cancelled: continue
            Err(Error::Cancelled) => break,
            // Display error to user and try again
            Err(err) => {
                error!("Initialization error: {err}");
                Event::Error(None, err.to_string()).track(&mut ctx.telemetry);
                let _ = submit_telemetry(ctx).await;
                let _ = UserInterface(error::ErrorMessage::new(err, false))
                    .run_timeout_ok(ctx)
                    .await;
            }
        }
    }

    // Run main user interface flow forever
    loop {
        match run_main_flow(ctx).await {
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
                Event::Error(opt_user_id, error.to_string()).track(&mut ctx.telemetry);
                let _ = submit_telemetry(ctx).await;
                let _ = UserInterface(error::ErrorMessage::new(error, opt_user_id.is_some()))
                    .run_timeout_ok(ctx)
                    .await;
            }
        }
    }
}

/// Run initialization flow
async fn run_init_flow<D: DeviceTypes>(ctx: &mut Context<'_, D>) -> Result<(), Error<D>> {
    // Run daily schedule
    run_schedule(ctx).await?;

    Ok(())
}

/// Run main user interface flow
async fn run_main_flow<D: DeviceTypes>(ctx: &mut Context<'_, D>) -> Result<(), ErrorWithUser<D>> {
    // Submit telemetry data if needed
    submit_telemetry(ctx).await?;

    // Wait for user authentication or schedule time
    let schedule_timer = ctx.schedule.timer();
    let (user_id, user) = match select(authenticate_user(ctx), schedule_timer).await {
        // User authenticated
        Either::First(res) => res?,
        // Schedule time
        Either::Second(()) => {
            run_schedule(ctx).await?;
            return Ok(());
        }
    };

    // Run purchase flow with authorized user
    ErrorWithUser::try_as_user_async(user_id, async || {
        run_purchase_flow(ctx, user_id, &user).await
    })
    .await
}

/// Run daily schedule
async fn run_schedule<D: DeviceTypes>(ctx: &mut Context<'_, D>) -> Result<(), Error<D>> {
    info!("UI: Running schedule...");

    // Schedule next event
    ctx.schedule.schedule_next();

    // Check for OTA update
    if !D::Updater::recently_restarted() {
        check_ota_update(ctx).await?;
    }

    // Refresh article and user information
    refresh_articles_and_users(ctx).await?;

    Ok(())
}

/// Check for OTA update
async fn check_ota_update<D: DeviceTypes>(ctx: &mut Context<'_, D>) -> Result<(), Error<D>> {
    // Wait for network to become available (if not already)
    wait_network_up(ctx).await?;

    info!("UI: Checking for OTA update...");
    show_please_wait(ctx.dev.display, common::PleaseWait::UpdateCheck).await?;

    // Check for latest release
    let mut ota = Ota::new(&mut ctx.http);
    let new_version = {
        match ota.check().await.map_err(Error::Ota)? {
            Some(new_version) => new_version,
            None => return Ok(()),
        }
    };

    // Don't actually apply OTA update in debug mode or when no updater is available
    if ctx.dev.updater.is_none() || cfg!(debug_assertions) {
        warn!("UI: Automatic OTA update unavailable. Please update manually.");
        return Ok(());
    }

    // Do automatic OTA update when updater is available
    if let Some(updater) = ctx.dev.updater.as_mut() {
        show_please_wait(ctx.dev.display, common::PleaseWait::UpdatingFirmware).await?;

        // Download and apply OTA update
        ota.update(*updater, &new_version)
            .await
            .map_err(Error::Ota)?;

        // OTA update completed, restart system
        D::Updater::restart();
    }

    Ok(())
}

/// Refresh article and user information
async fn refresh_articles_and_users<D: DeviceTypes>(
    ctx: &mut Context<'_, D>,
) -> Result<(), Error<D>> {
    // Wait for network to become available (if not already)
    wait_network_up(ctx).await?;

    info!("UI: Refreshing articles and users...");
    show_please_wait(ctx.dev.display, common::PleaseWait::UpdatingData).await?;

    // Connect to Vereinsflieger API
    let mut vf = ctx.vereinsflieger.connect(&mut ctx.http).await?;

    // Set current date and time based on time gathered from API response
    if let Some(time_reference) = vf.last_response_time() {
        ctx.rtc.set_by_reference(time_reference);
    }

    // Refresh article information
    debug!("UI: Refreshing articles...");
    let today = ctx.rtc.today().ok_or(Error::CurrentTimeNotSet)?;
    ctx.articles.clear();
    let total_articles = vf
        .get_articles(async |article| {
            ctx.articles.update_vereinsflieger_article(article, today);
        })
        .await?;
    info!(
        "UI: Refreshed {} of {} articles",
        ctx.articles.count(),
        total_articles
    );

    // Refresh user information
    debug!("UI: Refreshing users...");
    ctx.users.clear();
    let total_users = vf
        .get_users(async |user| {
            ctx.users.update_vereinsflieger_user(user);
        })
        .await?;
    info!(
        "UI: Refreshed {} of {} users",
        ctx.users.count(),
        total_users
    );

    Event::DataRefreshed(
        ctx.articles.count(),
        ctx.users.count_uids(),
        ctx.users.count(),
    )
    .track(&mut ctx.telemetry);

    Ok(())
}

/// Submit telemetry data if needed
async fn submit_telemetry<D: DeviceTypes>(ctx: &mut Context<'_, D>) -> Result<(), Error<D>> {
    if !ctx.telemetry.needs_flush() {
        return Ok(());
    }

    // Wait for network to become available (if not already)
    wait_network_up(ctx).await?;

    info!("UI: Submitting telemetry data...");
    show_please_wait(ctx.dev.display, common::PleaseWait::SubmittingTelemetry).await?;

    // Submit telemetry data, ignore any error
    let _ = ctx.telemetry.flush(&mut ctx.http, &ctx.rtc).await;

    Ok(())
}

/// Wait for user authentication
async fn authenticate_user<D: DeviceTypes>(
    ctx: &mut Context<'_, D>,
) -> Result<(UserId, User), ErrorWithUser<D>> {
    loop {
        let uid = UserInterface(auth::Authentication).run(ctx).await?;

        // Look up user by detected NFC uid
        if let Some((user_id, user)) = ctx.users.get_by_uid(&uid) {
            // User found, authorized
            info!("UI: NFC card {uid} identified as user {user_id}");
            Event::UserAuthenticated(user_id, uid).track(&mut ctx.telemetry);
            ctx.dev.buzzer.confirm().await;
            break Ok((user_id, user.clone()));
        }

        // User not found, unauthorized
        info!("UI: NFC card {uid} unknown, rejecting");
        Event::AuthenticationFailed(uid).track(&mut ctx.telemetry);
        ctx.dev.buzzer.deny().await;
        Timer::after_secs(1).await;
    }
}

/// Run purchase flow with given user
async fn run_purchase_flow<D: DeviceTypes>(
    ctx: &mut Context<'_, D>,
    user_id: UserId,
    user: &User,
) -> Result<(), Error<D>> {
    // Collect list of articles
    // FIXME: Needs to clone because lifetime issue if we borrow ctx.articles and call run(ctx)
    let articles: Vec<(ArticleId, Article)> = ctx
        .articles
        .iter()
        .map(|(article_id, article)| (article_id.clone(), article.clone()))
        .collect();

    // Ask for article to purchase
    let article_idx = UserInterface(purchase::SelectArticle::new(
        ctx.dev.rng,
        &user.name,
        &articles,
    ))
    .run(ctx)
    .await?;

    // Get article information
    let (article_id, article) = articles.get(article_idx).ok_or(Error::ArticleNotFound)?;

    // Ask for amount to purchase
    let amount = UserInterface(purchase::EnterAmount::new(article))
        .run(ctx)
        .await?;

    // Calculate total price. It's ok to cast amount to f32 as it's always a small number.
    #[expect(clippy::cast_precision_loss)]
    let total_price = article.price * amount as f32;

    // Show total price and ask for confirmation
    UserInterface(purchase::Checkout::new(article, amount, total_price))
        .run(ctx)
        .await?;

    // Submit purchase
    #[expect(clippy::cast_precision_loss)]
    submit_purchase(ctx, article_id, amount as f32, user_id, total_price).await?;

    // Show success and affirm to take items
    ctx.dev.buzzer.confirm().await;
    UserInterface(purchase::Success::new(amount))
        .run_timeout_ok(ctx)
        .await?;

    Ok(())
}

/// Submit purchase for the given article
async fn submit_purchase<D: DeviceTypes>(
    ctx: &mut Context<'_, D>,
    article_id: &ArticleId,
    amount: f32,
    user_id: UserId,
    total_price: f32,
) -> Result<(), Error<D>> {
    // Wait for network to become available (if not already)
    wait_network_up(ctx).await?;

    // Submit purchase
    info!("UI: Purchasing {amount}x {article_id}, {total_price:.02} EUR for user {user_id}...");
    show_please_wait(ctx.dev.display, common::PleaseWait::Purchasing).await?;

    // Connect to Vereinsflieger API
    let mut vf = ctx.vereinsflieger.connect(&mut ctx.http).await?;

    // Submit purchase
    let today = ctx.rtc.today().ok_or(Error::CurrentTimeNotSet)?;
    vf.purchase(today, article_id, amount, user_id, total_price)
        .await?;
    Event::ArticlePurchased(user_id, article_id.clone(), amount, total_price)
        .track(&mut ctx.telemetry);

    Ok(())
}
