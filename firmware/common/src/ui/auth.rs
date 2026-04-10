use super::common::TITLE_STYLE;
use super::{Error, Frontend, FrontendResources, UiContent, UiInteraction};
use crate::nfc::Uid;
use crate::{Display, Keypad, NfcReader};
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, TimeoutError, with_timeout};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Alignment, Text};
use embedded_layout::prelude::*;
use log::info;

/// Timeout for scanning id card. Power saving is activated if no action is taken for this duration.
#[cfg(not(debug_assertions))]
const TIMEOUT: Duration = Duration::from_secs(300);
#[cfg(debug_assertions)]
const TIMEOUT: Duration = Duration::from_secs(10);

/// Wait for a user to scan its id card, read it and look up the associated user (authentication).
/// On idle timeout, enter power saving (turn off display). Any key pressed leaves power saving
/// (turn on display).
#[must_use]
pub struct Authentication;

impl UiContent for Authentication {
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        Text::with_alignment(
            "Mitgliedsausweis\nscannen",
            Point::zero(),
            TITLE_STYLE,
            Alignment::Center,
        )
        .align_to(&target.bounding_box(), horizontal::Center, vertical::Center)
        .draw(target)?;
        Ok(())
    }
}

impl<FE: Frontend> UiInteraction<FE> for Authentication {
    type Output = Uid;
    // No user interaction timeout since there is no user when waiting for a user
    const TIMEOUT: Option<Duration> = None;

    async fn run(
        &mut self,
        frontend: &mut FrontendResources<'_, FE>,
    ) -> Result<Self::Output, Error<FE>> {
        info!("UI: Waiting for NFC card...");

        loop {
            // Wait for id card read or timeout
            #[expect(clippy::single_match_else)]
            match with_timeout(TIMEOUT, frontend.nfc.read()).await {
                // Id card detected
                Ok(res) => break res.map_err(Error::Nfc),

                // Idle timeout, enter power saving
                Err(TimeoutError) => {
                    info!("UI: Power saving...");
                    frontend.display.power_save().await;

                    // While saving power, wait for id card read or keypress
                    match select(frontend.keypad.read(), frontend.nfc.read()).await {
                        // Key pressed, leave power saving
                        Either::First(Ok(_key)) => {
                            frontend.display.flush().await.map_err(Error::Display)?;
                        }
                        Either::First(Err(err)) => break Err(Error::Keypad(err)),
                        // Id card detected
                        Either::Second(res) => break res.map_err(Error::Nfc),
                    }
                }
            }
        }
    }
}
