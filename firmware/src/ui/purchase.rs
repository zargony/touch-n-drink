use super::common::{MEDIUM_STYLE, SMALL_STYLE, TITLE_STYLE};
use super::{Error, Frontend, FrontendResources, Keypad, UiContent, UiInteraction};
use crate::article::{Article, Articles};
use crate::util::RectangleExt;
use alloc::format;
use alloc::vec::Vec;
use embassy_time::Timer;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Alignment, Text};
use embedded_layout::layout::linear::{LinearLayout, spacing};
use embedded_layout::prelude::*;
use log::info;
use rand_core::RngCore;

/// User greetings (choosen randomly)
static GREETINGS: [&str; 20] = [
    "Hi", "Hallo", "Hey", "Tach", "Servus", "Moin", "Hej", "Olá", "Ciao", "Yo", "Ahoi", "Hola",
    "Salut", "Cheers", "Salve", "Hoi", "Hiya", "Sup", "Hiho", "Oi",
];

/// Prompt to select article
pub struct SelectArticle<'a> {
    greeting: &'static str,
    name: &'a str,
    articles: &'a Articles,
}

impl<'a> SelectArticle<'a> {
    pub fn new<RNG: RngCore>(mut rng: RNG, name: &'a str, articles: &'a Articles) -> Self {
        let greeting = GREETINGS[rng.next_u32() as usize % GREETINGS.len()];
        Self {
            greeting,
            name,
            articles,
        }
    }
}

impl UiContent for SelectArticle<'_> {
    const FOOTER_LEFT: &'static str = "* Abbruch";
    // FIXME: Should show actual selectable numbers instead "1-9" fixed
    const FOOTER_RIGHT: &'static str = "1-9 Weiter";

    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        let (greeting_box, articles_box) = target
            .bounding_box()
            .header(MEDIUM_STYLE.font.character_size.height);

        let greeting_text = format!("{} {}", self.greeting, self.name);
        let greeting_width = greeting_text.len() * MEDIUM_STYLE.font.character_size.width as usize;
        let mut greeting = Text::new(&greeting_text, Point::zero(), MEDIUM_STYLE);
        if greeting_width > greeting_box.size.width as usize {
            greeting = greeting.align_to(&greeting_box, horizontal::Left, vertical::Top);
        } else {
            greeting = greeting.align_to(&greeting_box, horizontal::Center, vertical::Top);
        }
        greeting.draw(target)?;

        if self.articles.iter().count() == 0 {
            Text::with_alignment(
                "Keine Artikel\nverfügbar",
                Point::zero(),
                TITLE_STYLE,
                Alignment::Center,
            )
            .align_to(&articles_box, horizontal::Center, vertical::Center)
            .draw(target)?;
            return Ok(());
        }

        let articles: Vec<_> = self
            .articles
            .iter()
            .map(|(idx, _article_id, article)| {
                (
                    format!("{}:{}", idx + 1, article.name),
                    format!("{:.02}", article.price),
                )
            })
            .collect();
        let mut articles: Vec<_> = articles
            .iter()
            .map(|(desc_text, price_text)| {
                let desc = Text::new(desc_text, Point::zero(), TITLE_STYLE);
                let price = Text::new(price_text, Point::zero(), SMALL_STYLE);
                LinearLayout::horizontal(Chain::new(desc).append(price))
                    .with_alignment(vertical::Center)
                    .with_spacing(spacing::DistributeFill(articles_box.size.width))
                    .arrange()
            })
            .collect();

        LinearLayout::vertical(Views::new(&mut articles))
            .with_alignment(horizontal::Left)
            .with_spacing(spacing::Tight)
            .arrange()
            .align_to(&articles_box, horizontal::Center, vertical::Center)
            .draw(target)?;
        Ok(())
    }
}

impl<FE: Frontend> UiInteraction<FE> for SelectArticle<'_> {
    type Output = usize;

    async fn run(
        &mut self,
        frontend: &mut FrontendResources<'_, FE>,
    ) -> Result<Self::Output, Error<FE>> {
        info!("UI: Asking to select article...");

        let num_articles = self.articles.count_ids();
        loop {
            match frontend.keypad.read().await.map_err(Error::Keypad)? {
                // Any digit 1..=num_articles selects article
                ch @ '1'..='9' => {
                    if let Some(n) = ch.to_digit(10)
                        && n as usize <= num_articles
                    {
                        break Ok(n as usize - 1);
                    }
                }
                // Cancel key cancels
                '*' => break Err(Error::Cancelled),
                // Ignore any other key
                _ => (),
            }
        }
    }
}

/// Ask for amount to purchase
pub struct EnterAmount<'a> {
    article: &'a Article,
}

impl<'a> EnterAmount<'a> {
    pub fn new(article: &'a Article) -> Self {
        Self { article }
    }
}

impl UiContent for EnterAmount<'_> {
    const FOOTER_LEFT: &'static str = "* Abbruch";
    const FOOTER_RIGHT: &'static str = "1-9 Weiter";

    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        let article_text = format!("{}: {:.02}", self.article.name, self.article.price);
        let article = Text::new(&article_text, Point::zero(), MEDIUM_STYLE);

        let select = Text::new("Anzahl wählen", Point::zero(), TITLE_STYLE);

        LinearLayout::vertical(Chain::new(article).append(select))
            .with_alignment(horizontal::Center)
            .with_spacing(spacing::FixedMargin(4))
            .arrange()
            .align_to(&target.bounding_box(), horizontal::Center, vertical::Center)
            .draw(target)?;

        Ok(())
    }
}

impl<FE: Frontend> UiInteraction<FE> for EnterAmount<'_> {
    type Output = usize;

    async fn run(
        &mut self,
        frontend: &mut FrontendResources<'_, FE>,
    ) -> Result<Self::Output, Error<FE>> {
        info!(
            "UI: Asking to enter amount for {}, {:.02} EUR...",
            self.article.name, self.article.price
        );

        loop {
            match frontend.keypad.read().await.map_err(Error::Keypad)? {
                // Any digit 1..=9 selects amount
                ch @ '1'..='9' => {
                    if let Some(n) = ch.to_digit(10) {
                        break Ok(n as usize);
                    }
                }
                // Cancel key cancels
                '*' => break Err(Error::Cancelled),
                // Ignore any other key
                _ => (),
            }
        }
    }
}

/// Confirm purchase (show total price and ask for confirmation)
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

impl UiContent for Checkout<'_> {
    const FOOTER_LEFT: &'static str = "* Abbruch";
    const FOOTER_RIGHT: &'static str = "# BEZAHLEN";

    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        let articles_text = format!("{}x {}", self.amount, self.article.name);
        let articles = Text::new(&articles_text, Point::zero(), MEDIUM_STYLE);

        let price_text = format!("{:.02} EUR", self.total_price);
        let price = Text::new(&price_text, Point::zero(), TITLE_STYLE);

        LinearLayout::vertical(Chain::new(articles).append(price))
            .with_alignment(horizontal::Center)
            .with_spacing(spacing::FixedMargin(4))
            .arrange()
            .align_to(&target.bounding_box(), horizontal::Center, vertical::Center)
            .draw(target)?;

        Ok(())
    }
}

impl<FE: Frontend> UiInteraction<FE> for Checkout<'_> {
    type Output = ();

    async fn run(
        &mut self,
        frontend: &mut FrontendResources<'_, FE>,
    ) -> Result<Self::Output, Error<FE>> {
        info!(
            "UI: Asking for purchase confirmation of {}x {}, {:.02} EUR...",
            self.amount, self.article.name, self.total_price
        );

        loop {
            match frontend.keypad.read().await.map_err(Error::Keypad)? {
                // Cancel key cancels
                '*' => break Err(Error::Cancelled),
                // Enter key confirms purchase
                '#' => break Ok(()),
                // Ignore any other key
                _ => (),
            }
        }
    }
}

/// Show success and wait for keypress
pub struct Success {
    amount: usize,
}

impl Success {
    pub fn new(amount: usize) -> Self {
        Self { amount }
    }
}

impl UiContent for Success {
    const FOOTER_RIGHT: &'static str = "# Roger";

    fn draw<D: DrawTarget<Color = BinaryColor>>(&self, target: &mut D) -> Result<(), D::Error> {
        let title = Text::new("Affirm!", Point::zero(), TITLE_STYLE);

        let affirm_text = format!("{} Getränke genehmigt", self.amount);
        let affirm = Text::new(&affirm_text, Point::zero(), SMALL_STYLE);

        LinearLayout::vertical(Chain::new(title).append(affirm))
            .with_alignment(horizontal::Center)
            .with_spacing(spacing::FixedMargin(4))
            .arrange()
            .align_to(&target.bounding_box(), horizontal::Center, vertical::Center)
            .draw(target)?;

        Ok(())
    }
}

impl<FE: Frontend> UiInteraction<FE> for Success {
    type Output = ();

    async fn run(
        &mut self,
        frontend: &mut FrontendResources<'_, FE>,
    ) -> Result<Self::Output, Error<FE>> {
        info!("UI: Displaying success, {} items", self.amount);

        // Wait at least 1s without responding to keypad
        Timer::after_secs(1).await;

        // Wait for enter key to be pressed
        while frontend.keypad.read().await.map_err(Error::Keypad)? != '#' {}
        Ok(())
    }
}
