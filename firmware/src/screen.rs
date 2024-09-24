use crate::{GIT_SHA_STR, VERSION_STR};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{fonts, FontRenderer};

/// Touch 'n Drink bi-color logo
#[rustfmt::skip]
#[allow(clippy::unreadable_literal)]
static LOGO: ImageRaw<BinaryColor> = ImageRaw::new(&[
    0b11111111, 0b11000111, 0b11100011, 0b11001111, 0b00011111, 0b10001111, 0b00011110, 0b00111101, 0b11110011, 0b11000011, 0b11111100, 0b01111111, 0b10001111, 0b01111100, 0b11110111, 0b10001111,
    0b10000000, 0b01001100, 0b00110010, 0b01001001, 0b00110000, 0b11001001, 0b00010010, 0b00100101, 0b00010010, 0b01000011, 0b11111110, 0b01111111, 0b11001111, 0b01111100, 0b11110111, 0b10001111,
    0b10000000, 0b01011000, 0b00011010, 0b01001001, 0b01100000, 0b01101001, 0b00010010, 0b00100101, 0b00010010, 0b01000011, 0b11111111, 0b01111111, 0b11101111, 0b01111100, 0b11110111, 0b10011110,
    0b11110011, 0b11010001, 0b10001010, 0b01001001, 0b01000110, 0b00101001, 0b00010010, 0b00100101, 0b00001010, 0b01000011, 0b11111111, 0b01111111, 0b11101111, 0b01111110, 0b11110111, 0b10011110,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001001, 0b00101001, 0b00010010, 0b00111101, 0b00001010, 0b01000011, 0b11001111, 0b01111001, 0b11101111, 0b01111110, 0b11110111, 0b10111100,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001001, 0b00101001, 0b00010010, 0b00011001, 0b00001010, 0b01000011, 0b11001111, 0b01111001, 0b11101111, 0b01111110, 0b11110111, 0b10111100,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001001, 0b11101001, 0b00010010, 0b00011001, 0b00001010, 0b01000011, 0b11001111, 0b01111001, 0b11101111, 0b01111110, 0b11110111, 0b11111000,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001000, 0b00001001, 0b00010010, 0b00000001, 0b00100110, 0b01000011, 0b11001111, 0b01111001, 0b11101111, 0b01111111, 0b11110111, 0b11111000,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001000, 0b00001001, 0b11110010, 0b00000001, 0b00100110, 0b01000011, 0b11001111, 0b01111111, 0b11101111, 0b01111111, 0b11110111, 0b11110000,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001000, 0b00001000, 0b00000010, 0b00000001, 0b00110110, 0b01000011, 0b11001111, 0b01111111, 0b11001111, 0b01111111, 0b11110111, 0b11110000,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001000, 0b00001000, 0b00000010, 0b00000001, 0b00110110, 0b01000011, 0b11001111, 0b01111111, 0b11001111, 0b01111111, 0b11110111, 0b11111000,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001000, 0b00001001, 0b11110010, 0b00000001, 0b00110010, 0b01000011, 0b11001111, 0b01111111, 0b11101111, 0b01111111, 0b11110111, 0b11111000,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001001, 0b11101001, 0b00010010, 0b00000001, 0b00110010, 0b01000011, 0b11001111, 0b01111001, 0b11101111, 0b01111111, 0b11110111, 0b10111000,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001001, 0b00101001, 0b00010010, 0b00000001, 0b00101000, 0b01000011, 0b11001111, 0b01111001, 0b11101111, 0b01111011, 0b11110111, 0b10111100,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001001, 0b00101001, 0b00010010, 0b00000001, 0b00101000, 0b01000011, 0b11001111, 0b01111001, 0b11101111, 0b01111011, 0b11110111, 0b10111100,
    0b00010010, 0b00010010, 0b01001010, 0b01001001, 0b01001001, 0b00101001, 0b00010010, 0b00000001, 0b00101000, 0b01000011, 0b11001111, 0b01111001, 0b11101111, 0b01111011, 0b11110111, 0b10011110,
    0b00010010, 0b00010001, 0b10001010, 0b00110001, 0b01000110, 0b00101001, 0b00010010, 0b00000001, 0b00101000, 0b01000011, 0b11111111, 0b01111001, 0b11101111, 0b01111011, 0b11110111, 0b10011110,
    0b00010010, 0b00011000, 0b00011011, 0b00000011, 0b01100000, 0b01101001, 0b00010010, 0b00000001, 0b00100100, 0b01000011, 0b11111111, 0b01111001, 0b11101111, 0b01111001, 0b11110111, 0b10011110,
    0b00010010, 0b00001100, 0b00110001, 0b10000110, 0b00110000, 0b11001001, 0b00010010, 0b00000001, 0b00100100, 0b01000011, 0b11111110, 0b01111001, 0b11101111, 0b01111001, 0b11110111, 0b10001111,
    0b00011110, 0b00000111, 0b11100000, 0b11111100, 0b00011111, 0b10001111, 0b00011110, 0b00000001, 0b11100111, 0b11000011, 0b11111100, 0b01111001, 0b11101111, 0b01111001, 0b11110111, 0b10001111,
], 128);

const SPLASH_VERSION_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_profont10_tr>();
const TOTAL_PRICE_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_8x13B_tf>();
const TITLE_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_8x13_tf>();
const SMALL_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_5x7_tf>();
const FOOTER_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_5x7_tf>();

/// Screen display error
pub type Error<E> = u8g2_fonts::Error<E>;

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
        Image::new(&LOGO, Point::new(0, 13))
            .draw(target)
            .map_err(Error::DisplayError)?;
        SPLASH_VERSION_FONT.render_aligned(
            VERSION_STR,
            Point::new(63, 30 + 12),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        #[cfg(not(debug_assertions))]
        Footer::new("", GIT_SHA_STR).draw(target)?;
        #[cfg(debug_assertions)]
        Footer::new("(DEBUG)", GIT_SHA_STR).draw(target)?;
        Ok(())
    }
}

/// Wait while a lengthy action is in progress
pub enum PleaseWait {
    WifiConnecting,
}

impl Screen for PleaseWait {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        TITLE_FONT.render_aligned(
            "Stand By...",
            Point::new(63, 26),
            VerticalPosition::Baseline,
            HorizontalAlignment::Center,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
        SMALL_FONT.render_aligned(
            match self {
                Self::WifiConnecting => "WLAN Verbindung\nwird aufgebaut",
            },
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

/// Prompt to scan id card
pub struct ScanId;

impl Screen for ScanId {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        TITLE_FONT.render_aligned(
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
        TITLE_FONT.render_aligned(
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
        TITLE_FONT.render_aligned(
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
        TOTAL_PRICE_FONT.render_aligned(
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
        TITLE_FONT.render_aligned(
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
        TITLE_FONT.render_aligned(
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
