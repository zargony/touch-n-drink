use crate::article::{Article, Articles};
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

/// User greetings (chosen randomly)
static GREETINGS: [&str; 9] = [
    "Hi", "Hallo", "Hey", "Tach", "Servus", "Moin", "Hej", "Olá", "Ciao",
];

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
const MEDIUM_CHARS_PER_LINE: usize = WIDTH as usize / 6;

/// Screen display error
pub type Error<E> = u8g2_fonts::Error<E>;

/// Generic screen that can be displayed
pub trait Screen {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>>;
}

/// Draw left aligned text on given line
fn left<D: DrawTarget<Color = BinaryColor>>(
    font: &FontRenderer,
    x: i32,
    y: i32,
    content: impl Content,
    target: &mut D,
) -> Result<(), Error<D::Error>> {
    font.render_aligned(
        content,
        Point::new(x, y),
        VerticalPosition::Baseline,
        HorizontalAlignment::Left,
        FontColor::Transparent(BinaryColor::On),
        target,
    )?;
    Ok(())
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

/// Draw right aligned text on given line
fn right<D: DrawTarget<Color = BinaryColor>>(
    font: &FontRenderer,
    y: i32,
    content: impl Content,
    target: &mut D,
) -> Result<(), Error<D::Error>> {
    font.render_aligned(
        content,
        Point::new(WIDTH - 1, y),
        VerticalPosition::Baseline,
        HorizontalAlignment::Right,
        FontColor::Transparent(BinaryColor::On),
        target,
    )?;
    Ok(())
}

/// Draw common footer (bottom 7 lines, 57..64)
fn footer<D: DrawTarget<Color = BinaryColor>>(
    content_left: impl Content,
    content_right: impl Content,
    target: &mut D,
) -> Result<(), Error<D::Error>> {
    left(&FOOTER_FONT, 0, HEIGHT - 1, content_left, target)?;
    right(&FOOTER_FONT, HEIGHT - 1, content_right, target)?;
    Ok(())
}

/// Trim text if it's too long
fn trim(text: &str, max_len: usize) -> &str {
    if text.len() > max_len {
        &text[..max_len]
    } else {
        text
    }
}

/// Draw user greeting (top 10 lines, 0..10)
fn greeting<D: DrawTarget<Color = BinaryColor>>(
    random: u32,
    name: &str,
    target: &mut D,
) -> Result<(), Error<D::Error>> {
    let greeting = GREETINGS[random as usize % GREETINGS.len()];
    // Trim name if it's too long to display
    let name = trim(name, MEDIUM_CHARS_PER_LINE - greeting.len() - 1);
    centered(&MEDIUM_FONT, 8, format_args!("{greeting} {name}"), target)
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

/// Prompt to select article
pub struct SelectArticle<'a> {
    greeting: u32,
    name: &'a str,
    articles: &'a Articles,
}

impl<'a> SelectArticle<'a> {
    pub fn new<RNG: RngCore>(mut rng: RNG, name: &'a str, articles: &'a Articles) -> Self {
        Self {
            greeting: rng.next_u32(),
            name,
            articles,
        }
    }
}

impl Screen for SelectArticle<'_> {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        greeting(self.greeting, self.name, target)?;

        // Safe to unwrap since conversion always succeeds for these small numbers
        let num_articles = i32::try_from(self.articles.count_ids()).unwrap();
        let y0 = 40 + num_articles * -5;
        for (idx, _article_id, article) in self.articles.iter() {
            // Safe to unwrap since conversion always succeeds for these small numbers
            let y = y0 + i32::try_from(idx).unwrap() * 12;
            left(&TITLE_FONT, 0, y, format_args!("{}:", idx + 1), target)?;
            left(&TITLE_FONT, 20, y, trim(&article.name, 11), target)?;
            right(
                &SMALL_FONT,
                y,
                format_args!("{:.02}", article.price),
                target,
            )?;
        }
        footer(
            "* Abbruch",
            format_args!("1-{} Weiter", self.articles.count_ids()),
            target,
        )?;
        Ok(())
    }
}

/// Prompt to enter amount
pub struct EnterAmount<'a> {
    article: &'a Article,
}

impl<'a> EnterAmount<'a> {
    pub fn new(article: &'a Article) -> Self {
        Self { article }
    }
}

impl Screen for EnterAmount<'_> {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        centered(
            &MEDIUM_FONT,
            23,
            format_args!(
                "{} {:.02}",
                trim(&self.article.name, MEDIUM_CHARS_PER_LINE - 3),
                self.article.price
            ),
            target,
        )?;
        centered(&TITLE_FONT, 23 + 16, "Anzahl wählen", target)?;
        footer("* Abbruch", "1-9 Weiter", target)?;
        Ok(())
    }
}

/// Checkout (confirm purchase)
pub struct Checkout<'a> {
    article: &'a Article,
    amount: usize,
    total_price: f32,
}

impl<'a> Checkout<'a> {
    pub fn new(article: &'a Article, amount: usize, total_price: f32) -> Self {
        Self {
            article,
            amount,
            total_price,
        }
    }
}

impl Screen for Checkout<'_> {
    fn draw<D: DrawTarget<Color = BinaryColor>>(
        &self,
        target: &mut D,
    ) -> Result<(), Error<D::Error>> {
        centered(
            &MEDIUM_FONT,
            23,
            format_args!(
                "{}x {}",
                self.amount,
                trim(&self.article.name, MEDIUM_CHARS_PER_LINE - 3)
            ),
            target,
        )?;
        centered(
            &TITLE_FONT,
            23 + 16,
            format_args!("{:.02} EUR", self.total_price),
            target,
        )?;
        footer("* Abbruch", "# BEZAHLEN", target)?;
        Ok(())
    }
}

/// Success screen
pub struct Success {
    amount: usize,
}

impl Success {
    pub fn new(amount: usize) -> Self {
        Self { amount }
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
            format_args!("{} Getränke genehmigt", self.amount),
            target,
        )?;
        footer("", "# Ok", target)?;
        Ok(())
    }
}
