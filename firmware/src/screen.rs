use crate::{GIT_SHA_STR, VERSION_STR};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{fonts, FontRenderer};

const SPLASH_TITLE_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>();
const SPLASH_VERSION_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_profont10_tr>();
const DEFAULT_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_8x13_tf>();
const BOLD_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_8x13B_tf>();
const SMALL_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_5x7_tf>();
const FOOTER_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_5x7_tf>();

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
        Footer::new("", GIT_SHA_STR).draw(target)?;
        Ok(())
    }
}

/// Prompt to scan id card
pub struct ScanId;

impl Screen for ScanId {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        DEFAULT_FONT.render_aligned(
            "Mitgliedsausweis\nscannen",
            Point::new(63, 26),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        Ok(())
    }
}

/// Prompt to ask for number of drinks
pub struct NumberOfDrinks;

impl Screen for NumberOfDrinks {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        DEFAULT_FONT.render_aligned(
            "Anzahl Getränke\nwählen",
            Point::new(63, 26),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        Footer::new("* Abbruch", "1-9 Weiter").draw(target)?;
        Ok(())
    }
}

/// Checkout (confirm purchase)
pub struct Checkout {
    num_drinks: usize,
    total_price: f32,
}

impl Checkout {
    pub fn new(num_drinks: usize, total_price: f32) -> Self {
        Self {
            num_drinks,
            total_price,
        }
    }
}

impl Screen for Checkout {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        DEFAULT_FONT.render_aligned(
            format_args!(
                "{} {}",
                self.num_drinks,
                if self.num_drinks == 1 {
                    "Getränk"
                } else {
                    "Getränke"
                }
            ),
            Point::new(63, 25),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        BOLD_FONT.render_aligned(
            format_args!("{:.02} EUR", self.total_price),
            Point::new(63, 25 + 16),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        Footer::new("* Abbruch", "# BEZAHLEN").draw(target)?;
        Ok(())
    }
}

/// Success screen
pub struct Success {
    num_drinks: usize,
}

impl Success {
    pub fn new(num_drinks: usize) -> Self {
        Self { num_drinks }
    }
}

impl Screen for Success {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        DEFAULT_FONT.render_aligned(
            "Affirm!",
            Point::new(63, 27),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        SMALL_FONT.render_aligned(
            format_args!("{} Getränke entnehmen", self.num_drinks),
            Point::new(63, 27 + 12),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        Footer::new("", "# Ok").draw(target)?;
        Ok(())
    }
}

/// Error screen
pub struct Failure<'a> {
    message: &'a str,
}

impl<'a> Failure<'a> {
    pub fn new(message: &'a str) -> Self {
        Self { message }
    }
}

impl<'a> Screen for Failure<'a> {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        DEFAULT_FONT.render_aligned(
            "FEHLER!",
            Point::new(63, 27),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        SMALL_FONT.render_aligned(
            self.message,
            Point::new(63, 27 + 12),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        Footer::new("* Abbruch", "").draw(target)?;
        Ok(())
    }
}

/// Common footer (bottom 7 lines 57..64)
struct Footer<'a> {
    left: &'a str,
    right: &'a str,
}

impl<'a> Footer<'a> {
    fn new(left: &'a str, right: &'a str) -> Self {
        Self { left, right }
    }

    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        if !self.left.is_empty() {
            FOOTER_FONT.render_aligned(
                self.left,
                Point::new(0, 63),
                VerticalPosition::Baseline,
                HorizontalAlignment::Left,
                FontColor::Transparent(BinaryColor::On),
                target,
            )?;
        }
        if !self.right.is_empty() {
            FOOTER_FONT.render_aligned(
                self.right,
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
