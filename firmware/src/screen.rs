use crate::article::{Article, Articles};
use crate::{GIT_SHA_STR, VERSION_STR};
use alloc::format;
use core::fmt;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::AnchorPoint;
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::iso_8859_15::{FONT_5X7, FONT_6X10, FONT_6X12, FONT_7X13_BOLD};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use rand_core::RngCore;

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
static GREETINGS: [&str; 20] = [
    "Hi", "Hallo", "Hey", "Tach", "Servus", "Moin", "Hej", "Olá", "Ciao", "Yo", "Ahoi", "Hola",
    "Salut", "Cheers", "Salve", "Hoi", "Hiya", "Sup", "Hiho", "Oi",
];

const SPLASH_VERSION_STYLE: MonoTextStyle<BinaryColor> =
    MonoTextStyle::new(&FONT_6X12, BinaryColor::On);
const TITLE_STYLE: MonoTextStyle<BinaryColor> =
    MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);
const MEDIUM_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
const SMALL_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_5X7, BinaryColor::On);
const FOOTER_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_5X7, BinaryColor::On);

/// Screen dimension
const SCREEN: Rectangle = Rectangle::new(Point::zero(), Size::new(128, 64));

/// Number of characters that fit in a line
const MEDIUM_CHARS_PER_LINE: usize =
    (SCREEN.size.width / MEDIUM_STYLE.font.character_size.width) as usize;

/// Left aligned point on screen
const fn left(y: i32) -> Point {
    Point::new(0, y)
}

/// Horizontally centered point on screen
const fn center(y: i32) -> Point {
    Point::new((SCREEN.size.width.cast_signed() - 1) / 2, y)
}

/// Right aligned point on screen
const fn right(y: i32) -> Point {
    Point::new(SCREEN.size.width.cast_signed() - 1, y)
}

/// Left aligned text
fn text<'a>(
    text: &'a str,
    x: i32,
    y: i32,
    style: MonoTextStyle<'a, BinaryColor>,
) -> Text<'a, MonoTextStyle<'a, BinaryColor>> {
    Text::with_alignment(text, Point::new(x, y), style, Alignment::Left)
}

/// Horizontally centered text
fn text_centered<'a>(
    text: &'a str,
    y: i32,
    style: MonoTextStyle<'a, BinaryColor>,
) -> Text<'a, MonoTextStyle<'a, BinaryColor>> {
    Text::with_alignment(text, center(y), style, Alignment::Center)
}

/// Right aligned text
fn text_right<'a>(
    text: &'a str,
    y: i32,
    style: MonoTextStyle<'a, BinaryColor>,
) -> Text<'a, MonoTextStyle<'a, BinaryColor>> {
    Text::with_alignment(text, right(y), style, Alignment::Right)
}

/// Draw common footer (bottom 7 lines, 57..64)
fn footer<D: DrawTarget<Color = BinaryColor>>(
    text_left: &str,
    text_right: &str,
    target: &mut D,
) -> Result<(), D::Error> {
    Text::with_text_style(
        text_left,
        SCREEN.anchor_point(AnchorPoint::BottomLeft),
        FOOTER_STYLE,
        TextStyleBuilder::new()
            .alignment(Alignment::Left)
            .baseline(Baseline::Bottom)
            .build(),
    )
    .draw(target)?;
    Text::with_text_style(
        text_right,
        SCREEN.anchor_point(AnchorPoint::BottomRight),
        FOOTER_STYLE,
        TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Bottom)
            .build(),
    )
    .draw(target)?;
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

/// Trim prefixes from text
fn trim_prefixes<'a>(text: &'a str, prefixes: &[&str]) -> &'a str {
    let mut result = text;
    for prefix in prefixes {
        if result.starts_with(prefix) {
            result = &result[prefix.len()..];
        }
    }
    result = result.trim_start();
    if result.is_empty() { text } else { result }
}

/// Draw user greeting (top 10 lines, 0..10)
fn greeting<D: DrawTarget<Color = BinaryColor>>(
    random: u32,
    name: &str,
    target: &mut D,
) -> Result<(), D::Error> {
    let greeting = GREETINGS[random as usize % GREETINGS.len()];
    // Trim name if it's too long to display
    let name = trim(name, MEDIUM_CHARS_PER_LINE - greeting.len() - 1);
    text_centered(&format!("{greeting} {name}"), 7, MEDIUM_STYLE).draw(target)?;
    Ok(())
}

/// Generic screen that can be displayed
pub trait Screen {
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error>;
}

/// Splash screen
pub struct Splash;

impl Screen for Splash {
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        Image::new(&LOGO, left(13)).draw(target)?;
        text_centered(&format!("v{VERSION_STR}"), 13 + 30, SPLASH_VERSION_STYLE).draw(target)?;
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
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        text_centered("FEHLER!", 25, TITLE_STYLE).draw(target)?;
        text_centered(&format!("{}", self.message), 25 + 12, SMALL_STYLE).draw(target)?;
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
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        text_centered("Stand By...", 25, TITLE_STYLE).draw(target)?;
        text_centered(
            match self {
                Self::WifiConnecting => "WLAN Verbindung\nwird aufgebaut",
                Self::UpdatingData => "Daten-Aktualisierung",
                Self::Purchasing => "Zahlung wird\nbearbeitet",
                Self::SubmittingTelemetry => "Daten-Übertragung",
            },
            25 + 12,
            MEDIUM_STYLE,
        )
        .draw(target)?;
        if let Self::WifiConnecting = self {
            footer("* Abbruch", "", target)?;
        }
        Ok(())
    }
}

/// Prompt to scan id card
pub struct ScanId;

impl Screen for ScanId {
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        text_centered("Mitgliedsausweis\nscannen", 25, TITLE_STYLE).draw(target)?;
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
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        greeting(self.greeting, self.name, target)?;

        // Safe to unwrap since conversion always succeeds for these small numbers
        let num_articles = i32::try_from(self.articles.count_ids()).unwrap();
        let y0 = 40 + num_articles * -5;
        for (idx, _article_id, article) in self.articles.iter() {
            // Safe to unwrap since conversion always succeeds for these small numbers
            let y = y0 + i32::try_from(idx).unwrap() * 12;
            text(&format!("{}:", idx + 1), 0, y, TITLE_STYLE).draw(target)?;
            let article_name = trim_prefixes(&article.name, &["Getränke", "Getränk"]);
            text(trim(article_name, 13), 16, y, TITLE_STYLE).draw(target)?;
            text_right(&format!("{:.02}", article.price), y, SMALL_STYLE).draw(target)?;
        }
        footer(
            "* Abbruch",
            &format!("1-{} Weiter", self.articles.count_ids()),
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
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        text_centered(
            &format!(
                "{} {:.02}",
                trim(&self.article.name, MEDIUM_CHARS_PER_LINE - 3),
                self.article.price
            ),
            22,
            MEDIUM_STYLE,
        )
        .draw(target)?;
        text_centered("Anzahl wählen", 22 + 16, TITLE_STYLE).draw(target)?;
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
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        text_centered(
            &format!(
                "{}x {}",
                self.amount,
                trim(&self.article.name, MEDIUM_CHARS_PER_LINE - 3)
            ),
            22,
            MEDIUM_STYLE,
        )
        .draw(target)?;
        text_centered(
            &format!("{:.02} EUR", self.total_price),
            22 + 16,
            TITLE_STYLE,
        )
        .draw(target)?;
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
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        text_centered("Affirm!", 25, TITLE_STYLE).draw(target)?;
        text_centered(
            &format!("{} Getränke genehmigt", self.amount),
            25 + 12,
            SMALL_STYLE,
        )
        .draw(target)?;
        Ok(())
    }
}
