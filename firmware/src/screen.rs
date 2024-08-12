use crate::{GIT_SHA_STR, VERSION_STR};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{fonts, FontRenderer};

const SPLASH_TITLE_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>();
const SPLASH_VERSION_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_profont12_tr>();
const SPLASH_FOOTER_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_profont11_tr>();
const BIG_CHAR_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_luBS24_tr>();

/// Screen display error
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// Display error
    #[allow(dead_code)]
    DisplayError(E),
    /// Font render error
    #[allow(dead_code)]
    FontRenderError(u8g2_fonts::Error<E>),
}

impl<E> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Self::DisplayError(err)
    }
}

impl<E> From<u8g2_fonts::Error<E>> for Error<E> {
    fn from(err: u8g2_fonts::Error<E>) -> Self {
        Self::FontRenderError(err)
    }
}

/// Generic screen that can be displayed
pub trait Screen {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>>;
}

/// Splash screen
pub struct Splash;

impl Screen for Splash {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        // TODO: Temporary title, replace with proper bitmap logo
        SPLASH_TITLE_FONT.render_aligned(
            "Touch'n Drink",
            Point::new(63, 28),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        SPLASH_VERSION_FONT.render_aligned(
            VERSION_STR,
            Point::new(63, 28 + 12),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        SPLASH_FOOTER_FONT.render_aligned(
            GIT_SHA_STR,
            Point::new(127, 63),
            VerticalPosition::Baseline,
            HorizontalAlignment::Right,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        Ok(())
    }
}

/// Big centered character
pub struct BigCenteredChar(pub char);

impl Screen for BigCenteredChar {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        BIG_CHAR_FONT.render_aligned(
            self.0,
            Point::new(63, 44),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        Ok(())
    }
}
