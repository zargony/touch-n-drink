use crate::http::Http;
use crate::mixpanel::{self, Mixpanel};
use crate::{article, json, nfc, user};
use alloc::collections::VecDeque;
use alloc::string::String;
use embassy_time::{Duration, Instant};
use embedded_io_async::Write;
use log::{debug, info, warn};

/// Time after which events are flushed even when queue isn't filled yet
const MAX_BUFFER_DURATION: Duration = Duration::from_secs(30);

/// Max number of events to buffer before flushing
const MAX_BUFFER_EVENTS: usize = 10;

/// Telemetry error
pub type Error = mixpanel::Error;

/// Telemetry event
#[derive(Debug)]
pub enum Event {
    /// System start
    SystemStart,
    /// Articles and users refreshed (article count, NFC uid count, user count)
    DataRefreshed(usize, usize, usize),
    /// User authentication failed (NFC uid)
    AuthenticationFailed(nfc::Uid),
    /// User authentication successful (user id, NFC uid)
    UserAuthenticated(user::UserId, nfc::Uid),
    /// Article purchased (user id, article id, amount, total price)
    ArticlePurchased(user::UserId, article::ArticleId, f32, f32),
    /// Error occured (optional user id, error message)
    Error(Option<user::UserId>, String),
}

impl Event {
    /// Event name as reported to server
    pub fn event_name(&self) -> &'static str {
        match self {
            Event::SystemStart => "system_start",
            Event::DataRefreshed(..) => "data_refreshed",
            Event::AuthenticationFailed(..) => "authentication_failed",
            Event::UserAuthenticated(..) => "user_authenticated",
            Event::ArticlePurchased(..) => "article_purchased",
            Event::Error(..) => "error",
        }
    }

    /// User id associated with this event, if any
    pub fn user_id(&self) -> Option<user::UserId> {
        #[allow(clippy::match_same_arms)]
        match self {
            Event::SystemStart => None,
            Event::DataRefreshed(..) => None,
            Event::AuthenticationFailed(..) => None,
            Event::UserAuthenticated(user_id, ..) => Some(*user_id),
            Event::ArticlePurchased(user_id, ..) => Some(*user_id),
            Event::Error(user_id, ..) => *user_id,
        }
    }

    /// Add event attributes to given JSON object
    pub async fn add_event_attributes<W: Write>(
        &self,
        object: &mut json::ObjectWriter<'_, W>,
    ) -> Result<(), json::Error<W::Error>> {
        match self {
            Event::SystemStart => (),
            Event::DataRefreshed(article_count, uid_count, user_count) => {
                object
                    .field("article_count", article_count)
                    .await?
                    .field("uid_count", uid_count)
                    .await?
                    .field("user_count", user_count)
                    .await?;
            }
            Event::AuthenticationFailed(uid) => {
                object.field("uid", uid).await?;
            }
            Event::UserAuthenticated(_user_id, uid) => {
                object.field("uid", uid).await?;
            }
            Event::ArticlePurchased(_user_id, article_id, amount, total_price) => {
                object
                    .field("article_id", article_id)
                    .await?
                    .field("amount", amount)
                    .await?
                    .field("total_price", total_price)
                    .await?;
            }
            Event::Error(_user_id, message) => {
                object.field("error_message", message).await?;
            }
        }
        Ok(())
    }
}

/// Telemetry for tracking events
#[derive(Debug)]
pub struct Telemetry<'a> {
    mixpanel: Option<Mixpanel<'a>>,
    events: VecDeque<(Instant, Event)>,
    last_flush: Instant,
}

impl<'a> Telemetry<'a> {
    /// Create new telemetry
    pub fn new(mp_token: Option<&'a str>, device_id: &'a str) -> Self {
        let mixpanel = if let Some(token) = mp_token {
            info!("Telemetry: Initialized with Mixpanel token {token}");
            Some(Mixpanel::new(token, device_id))
        } else {
            warn!("Telemetry: Disabled! No Mixpanel token.");
            None
        };
        Self {
            mixpanel,
            events: VecDeque::new(),
            last_flush: Instant::now(),
        }
    }

    /// Track event
    pub fn track(&mut self, event: Event) {
        if self.mixpanel.is_some() {
            debug!("Telemetry: tracking event {event:?}");
            self.events.push_back((Instant::now(), event));
        }
    }

    /// Returns true if buffer has filled up or time has ran out and events should be submitted
    pub fn needs_flush(&mut self) -> bool {
        (self.last_flush.elapsed() >= MAX_BUFFER_DURATION && !self.events.is_empty())
            || self.events.len() >= MAX_BUFFER_EVENTS
    }

    /// Submit tracked events to server
    pub async fn flush(&mut self, http: &mut Http<'_>) -> Result<(), Error> {
        if self.events.is_empty() {
            return Ok(());
        }

        if let Some(ref mut mixpanel) = self.mixpanel {
            debug!("Telemetry: Flushing {} events...", self.events.len());

            let mut mp = mixpanel.connect(http).await?;
            let events = self.events.make_contiguous();
            mp.submit(events).await?;

            debug!("Telemetry: Flush successful");
            self.events.clear();
            self.last_flush = Instant::now();
        }

        Ok(())
    }
}
