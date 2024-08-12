use crate::{GIT_SHA_STR, VERSION_STR};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{fonts, FontRenderer};

const SPLASH_TITLE_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>();
const SPLASH_VERSION_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_profont10_tr>();
const FOOTER_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_5x7_tf>();
const BIG_CHAR_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_luBS24_tr>();

/// Screen display error
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E> {
    /// Display error
    DisplayError(E),
    /// Font render error
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
            Point::new(63, 30),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        SPLASH_VERSION_FONT.render_aligned(
            VERSION_STR,
            Point::new(63, 30 + 12),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        Footer("", GIT_SHA_STR).draw(target)?;
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

/// Common footer (bottom 7 lines 57..64)
pub struct Footer(&'static str, &'static str);

impl Screen for Footer {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        if !self.0.is_empty() {
            FOOTER_FONT.render_aligned(
                self.0,
                Point::new(0, 63),
                VerticalPosition::Baseline,
                HorizontalAlignment::Left,
                FontColor::Transparent(BinaryColor::On),
                target,
            )?;
        }
        if !self.1.is_empty() {
            FOOTER_FONT.render_aligned(
                self.1,
                Point::new(127, 63),
                VerticalPosition::Baseline,
                HorizontalAlignment::Right,
                FontColor::Transparent(BinaryColor::On),
                target,
            )?;
        }
        Ok(())
    }
}
