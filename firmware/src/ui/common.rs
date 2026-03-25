use super::UiContent;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::iso_8859_15::{FONT_5X7, FONT_6X10, FONT_7X13_BOLD};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};
use embedded_layout::layout::linear::{LinearLayout, spacing};
use embedded_layout::prelude::*;

/// Title text style
pub const TITLE_STYLE: MonoTextStyle<BinaryColor> =
    MonoTextStyle::new(&FONT_7X13_BOLD, BinaryColor::On);

/// Medium text style
pub const MEDIUM_STYLE: MonoTextStyle<BinaryColor> =
    MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

/// Small text style
pub const SMALL_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_5X7, BinaryColor::On);

/// Footer text style
pub const FOOTER_STYLE: MonoTextStyle<BinaryColor> = MonoTextStyle::new(&FONT_5X7, BinaryColor::On);

/// Height of footer
pub const FOOTER_HEIGHT: u32 = FOOTER_STYLE.font.character_size.height;

/// Common footer with left and right text at the bottom of the draw target
pub struct Footer<'a>(pub &'a str, pub &'a str);

impl Drawable for Footer<'_> {
    type Color = BinaryColor;
    type Output = ();

    fn draw<D: DrawTarget<Color = Self::Color>>(
        &self,
        target: &mut D,
    ) -> Result<Self::Output, D::Error> {
        Text::with_text_style(
            self.0,
            Point::zero(),
            FOOTER_STYLE,
            TextStyleBuilder::new()
                .alignment(Alignment::Left)
                .baseline(Baseline::Bottom)
                .build(),
        )
        .align_to(&target.bounding_box(), horizontal::Left, vertical::Bottom)
        .draw(target)?;
        Text::with_text_style(
            self.1,
            Point::zero(),
            FOOTER_STYLE,
            TextStyleBuilder::new()
                .alignment(Alignment::Right)
                .baseline(Baseline::Bottom)
                .build(),
        )
        .align_to(&target.bounding_box(), horizontal::Right, vertical::Bottom)
        .draw(target)?;
        Ok(())
    }
}

/// Wait while a lengthy action is in progress
pub enum PleaseWait {
    WifiConnecting,
    UpdateCheck,
    UpdatingFirmware,
    UpdatingData,
    Purchasing,
    SubmittingTelemetry,
}

impl UiContent for PleaseWait {
    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        let title = Text::new("Stand By...", Point::zero(), TITLE_STYLE);

        let message_text = match self {
            Self::WifiConnecting => "WLAN Verbindung\nwird aufgebaut",
            Self::UpdateCheck => "Suche Updates",
            Self::UpdatingFirmware => "Lade\nFirmware-Update",
            Self::UpdatingData => "Daten-Aktualisierung",
            Self::Purchasing => "Zahlung wird\nbearbeitet",
            Self::SubmittingTelemetry => "Daten-Übertragung",
        };
        let message =
            Text::with_alignment(message_text, Point::zero(), MEDIUM_STYLE, Alignment::Center);

        LinearLayout::vertical(Chain::new(title).append(message))
            .with_alignment(horizontal::Center)
            .with_spacing(spacing::FixedMargin(4))
            .arrange()
            .align_to(&target.bounding_box(), horizontal::Center, vertical::Center)
            .draw(target)?;

        Ok(())
    }
}
