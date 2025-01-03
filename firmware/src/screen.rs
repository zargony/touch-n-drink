use crate::{GIT_SHA_STR, VERSION_STR};
use core::fmt;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use rand_core::RngCore;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{fonts, Content, FontRenderer};

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
const TITLE_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_8x13B_tf>();
const MEDIUM_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_6x10_tf>();
const SMALL_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_5x7_tf>();
const FOOTER_FONT: FontRenderer = FontRenderer::new::<fonts::u8g2_font_5x7_tf>();

/// Screen width
const WIDTH: i32 = 128;

/// Horizontal position for centering
const HCENTER: i32 = WIDTH / 2;

/// Screen height
const HEIGHT: i32 = 64;

/// Number of characters that fit in a line
const MEDIUM_CHARS_PER_LINE: i32 = WIDTH / 6;

/// Screen display error
pub type Error<E> = u8g2_fonts::Error<E>;

/// Generic screen that can be displayed
pub trait Screen {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>>;
}

/// Draw centered text on given line
fn centered<D: DrawTarget<Color = BinaryColor>>(
    font: &FontRenderer,
    y: i32,
    content: impl Content,
    target: &mut D,
) -> Result<(), Error<D::Error>> {
    font.render_aligned(
        content,
        Point::new(HCENTER, y),
        VerticalPosition::Baseline,
        HorizontalAlignment::Center,
        FontColor::Transparent(BinaryColor::On),
        target,
    )?;
    Ok(())
}

/// Draw common footer (bottom 7 lines, 57..64)
fn footer<D: DrawTarget<Color = BinaryColor>>(
    left: &str,
    right: &str,
    target: &mut D,
) -> Result<(), Error<D::Error>> {
    if !left.is_empty() {
        FOOTER_FONT.render_aligned(
            left,
            Point::new(0, HEIGHT - 1),
            VerticalPosition::Baseline,
            HorizontalAlignment::Left,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
    }
    if !right.is_empty() {
        FOOTER_FONT.render_aligned(
            right,
            Point::new(WIDTH - 1, HEIGHT - 1),
            VerticalPosition::Baseline,
            HorizontalAlignment::Right,
            FontColor::Transparent(BinaryColor::On),
            target,
        )?;
    }
    Ok(())
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
        centered(
            &SPLASH_VERSION_FONT,
            13 + 29,
            format_args!("v{VERSION_STR}"),
            target,
        )?;
        #[cfg(not(debug_assertions))]
        footer("", GIT_SHA_STR, target)?;
        #[cfg(debug_assertions)]
        footer("(DEBUG)", GIT_SHA_STR, target)?;
        Ok(())
    }
}

/// Failure screen
pub struct Failure<M> {
    message: M,
}

impl<M: fmt::Display> Failure<M> {
    pub fn new(message: M) -> Self {
        Self { message }
    }
}

impl<M: fmt::Display> Screen for Failure<M> {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        centered(&TITLE_FONT, 26, "FEHLER!", target)?;
        centered(
            &SMALL_FONT,
            26 + 12,
            format_args!("{}", self.message),
            target,
        )?;
        footer("* Abbruch", "", target)?;
        Ok(())
    }
}

/// Wait while a lengthy action is in progress
pub enum PleaseWait {
    WifiConnecting,
    UpdatingData,
    Purchasing,
    SubmittingTelemetry,
}

impl Screen for PleaseWait {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        centered(&TITLE_FONT, 26, "Stand By...", target)?;
        centered(
            &MEDIUM_FONT,
            26 + 12,
            match self {
                Self::WifiConnecting => "WLAN Verbindung\nwird aufgebaut",
                Self::UpdatingData => "Daten-Aktualisierung",
                Self::Purchasing => "Zahlung wird\nbearbeitet",
                Self::SubmittingTelemetry => "Daten-Übertragung",
            },
            target,
        )?;
        if let Self::WifiConnecting = self {
            footer("* Abbruch", "", target)?;
        }
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
        centered(&TITLE_FONT, 26, "Mitgliedsausweis\nscannen", target)?;
        Ok(())
    }
}

/// Prompt to ask for number of drinks
pub struct NumberOfDrinks<'a> {
    greeting: u32,
    name: &'a str,
}

impl<'a> NumberOfDrinks<'a> {
    pub fn new<RNG: RngCore>(rng: &mut RNG, name: &'a str) -> Self {
        Self {
            greeting: rng.next_u32(),
            name,
        }
    }
}

impl Screen for NumberOfDrinks<'_> {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        static GREETINGS: [&str; 9] = [
            "Hi", "Hallo", "Hey", "Tach", "Servus", "Moin", "Hej", "Olá", "Ciao",
        ];

        // Trim name if it's too long to display
        let greeting = GREETINGS[self.greeting as usize % GREETINGS.len()];
        let greeting_len = greeting.len() + 1;
        let name = if self.name.len() + greeting_len > MEDIUM_CHARS_PER_LINE as usize {
            &self.name[..(MEDIUM_CHARS_PER_LINE as usize - greeting_len)]
        } else {
            self.name
        };

        centered(&MEDIUM_FONT, 8, format_args!("{greeting} {name}"), target)?;
        centered(&TITLE_FONT, 8 + 22, "Anzahl Getränke\nwählen", target)?;
        footer("* Abbruch", "1-9 Weiter", target)?;
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
        centered(
            &MEDIUM_FONT,
            26,
            format_args!(
                "{} {}",
                self.num_drinks,
                if self.num_drinks == 1 {
                    "Getränk"
                } else {
                    "Getränke"
                }
            ),
            target,
        )?;
        centered(
            &TITLE_FONT,
            26 + 16,
            format_args!("{:.02} EUR", self.total_price),
            target,
        )?;
        footer("* Abbruch", "# BEZAHLEN", target)?;
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
        centered(&TITLE_FONT, 26, "Affirm!", target)?;
        centered(
            &SMALL_FONT,
            26 + 12,
            format_args!("{} Getränke genehmigt", self.num_drinks),
            target,
        )?;
        footer("", "# Ok", target)?;
        Ok(())
    }
}
