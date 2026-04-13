use super::common::{SMALL_STYLE, TITLE_STYLE};
use super::{DeviceTypes, Error, Frontend, UiContent, UiInteraction};
use crate::util::RectangleExt;
use crate::{Buzzer, Keypad};
use alloc::format;
use core::fmt;
use embassy_time::{Duration, Timer};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use embedded_layout::layout::linear::LinearLayout;
use embedded_layout::prelude::*;
use embedded_text::TextBox;
use embedded_text::alignment::HorizontalAlignment;
use embedded_text::style::{HeightMode, TextBoxStyleBuilder};
use log::info;

/// How long to show the error message if no key is pressed
const TIMEOUT: Duration = Duration::from_secs(60);

/// Error message
#[must_use]
pub struct ErrorMessage<M> {
    message: M,
    as_user: bool,
}

impl<M: fmt::Display> ErrorMessage<M> {
    pub fn new(message: M, as_user: bool) -> Self {
        Self { message, as_user }
    }
}

impl<M: fmt::Display> UiContent for ErrorMessage<M> {
    const FOOTER_LEFT: &str = "* Abbruch";

    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        let (_title_box, message_box) = target
            .bounding_box()
            .header(TITLE_STYLE.font.character_size.height);

        let title = Text::new("FEHLER!", Point::zero(), TITLE_STYLE);

        let message_text = format!("{}", self.message);
        let message = TextBox::with_textbox_style(
            &message_text,
            message_box,
            SMALL_STYLE,
            TextBoxStyleBuilder::new()
                .alignment(HorizontalAlignment::Center)
                .height_mode(HeightMode::FitToText)
                .build(),
        );

        LinearLayout::vertical(Chain::new(title).append(message))
            .with_alignment(horizontal::Center)
            .arrange()
            .align_to(&target.bounding_box(), horizontal::Center, vertical::Center)
            .draw(target)?;

        Ok(())
    }
}

impl<M: fmt::Display> UiInteraction for ErrorMessage<M> {
    type Output = ();
    const TIMEOUT: Option<Duration> = Some(TIMEOUT);

    async fn run<D: DeviceTypes>(
        &mut self,
        frontend: &mut Frontend<'_, '_, D>,
    ) -> Result<Self::Output, Error<D>> {
        info!("UI: Displaying error: {}", self.message);

        // Sound the error buzzer if the error was caused by a user's interaction
        if self.as_user {
            frontend.buzzer.error().await;
        }

        // Wait at least 1s without responding to keypad
        Timer::after_secs(1).await;

        // Wait for cancel key to be pressed
        while frontend.keypad.read().await.map_err(Error::Keypad)? != '*' {}
        Ok(())
    }
}
