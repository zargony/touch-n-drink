use crate::mixpanel::{self, Event as MixpanelEvent, Mixpanel};
use crate::{GIT_SHA_STR, VERSION_STR, article, nfc, time, user};
use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use derive_more::{Display, From};
use embassy_time::{Duration, Instant};
use embedded_nal_async::{Dns, TcpConnect};
use log::{debug, info, warn};
use reqwless::client::HttpClient;
use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};

/// Time after which events are flushed even when queue isn't filled yet
const MAX_BUFFER_DURATION: Duration = Duration::from_secs(30);

/// Max number of events to buffer before flushing
const MAX_BUFFER_EVENTS: usize = 10;

/// Telemetry error
#[derive(Debug, Display, From)]
#[must_use]
pub enum Error {
    /// Mixpanel error
    #[from]
    #[display("Mixpanel: {_0}")]
    Mixpanel(mixpanel::Error),
    /// Current time is required but not set
    #[display("Unknown current time")]
    CurrentTimeNotSet,
}

/// Custom event properties for Mixpanel submission
#[derive(Debug, Serialize)]
struct MixpanelEventProperties<'a> {
    // Global custom properties
    firmware_version: &'static str,
    firmware_git_sha: &'static str,
    device_id: &'a str,
    // Event-specific custom properties
    #[serde(flatten)]
    extra: MixpanelEventPropertiesExtra<'a>,
}

/// Event-specific custom properties for Mixpanel submission
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum MixpanelEventPropertiesExtra<'a> {
    /// No event-specific properties
    None,
    /// Event-specific custom properties (data refresh)
    DataRefresh {
        article_count: usize,
        uid_count: usize,
        user_count: usize,
    },
    /// Event-specific custom properties (authentication)
    Authentication {
        #[serde_as(as = "DisplayFromStr")]
        uid: &'a nfc::Uid,
    },
    /// Event-specific custom properties (purchase)
    Purchase {
        article_id: &'a str,
        amount: f32,
        total_price: f32,
    },
    /// Event-specific custom properties (error)
    Error { error_message: &'a str },
}

/// Telemetry event
#[derive(Debug)]
#[must_use]
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
    /// Track event
    pub fn track(self, telemetry: &mut Telemetry<'_>) {
        telemetry.track(self);
    }
}

impl Event {
    /// Event name as reported to server
    fn event_name(&self) -> &'static str {
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
    fn user_id(&self) -> Option<user::UserId> {
        #[expect(clippy::match_same_arms)]
        match self {
            Event::SystemStart => None,
            Event::DataRefreshed(..) => None,
            Event::AuthenticationFailed(..) => None,
            Event::UserAuthenticated(user_id, ..) => Some(*user_id),
            Event::ArticlePurchased(user_id, ..) => Some(*user_id),
            Event::Error(user_id, ..) => *user_id,
        }
    }

    /// Custom event properties for Mixpanel submission
    fn mixpanel_properties<'a>(&'a self, device_id: &'a str) -> MixpanelEventProperties<'a> {
        MixpanelEventProperties {
            firmware_version: VERSION_STR,
            firmware_git_sha: GIT_SHA_STR,
            device_id,
            extra: match self {
                Self::SystemStart => MixpanelEventPropertiesExtra::None,
                Self::DataRefreshed(article_count, uid_count, user_count) => {
                    MixpanelEventPropertiesExtra::DataRefresh {
                        article_count: *article_count,
                        uid_count: *uid_count,
                        user_count: *user_count,
                    }
                }
                Self::AuthenticationFailed(uid) | Self::UserAuthenticated(_, uid) => {
                    MixpanelEventPropertiesExtra::Authentication { uid }
                }
                Self::ArticlePurchased(_, article_id, amount, total_price) => {
                    MixpanelEventPropertiesExtra::Purchase {
                        article_id,
                        amount: *amount,
                        total_price: *total_price,
                    }
                }
                Self::Error(_, error_message) => {
                    MixpanelEventPropertiesExtra::Error { error_message }
                }
            },
        }
    }
}

/// Telemetry for tracking events
#[must_use]
pub struct Telemetry<'a> {
    mixpanel: Option<Mixpanel<'a>>,
    device_id: &'a str,
    events: VecDeque<(Instant, Event)>,
    last_flush: Instant,
}

impl<'a> Telemetry<'a> {
    /// Create new telemetry
    pub fn new(mp_token: Option<&'a str>, device_id: &'a str) -> Self {
        let mixpanel = if let Some(token) = mp_token {
            info!("Telemetry: Initialized with Mixpanel token {token}");
            Some(Mixpanel::new(token))
        } else {
            warn!("Telemetry: Disabled! No Mixpanel token.");
            None
        };
        Self {
            mixpanel,
            device_id,
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
    ///
    /// # Errors
    ///
    /// An error will be returned if events couldn't be submitted.
    pub async fn flush<T: TcpConnect, D: Dns>(
        &mut self,
        http: &mut HttpClient<'_, T, D>,
    ) -> Result<(), Error> {
        if self.events.is_empty() {
            return Ok(());
        }

        if let Some(ref mut mixpanel) = self.mixpanel {
            debug!("Telemetry: Flushing {} events...", self.events.len());

            let token = mixpanel.token().to_string();
            let mut mp = mixpanel.connect(http).await?;

            let events = self
                .events
                .iter()
                .map(|(time, event)| -> Result<_, Error> {
                    let time = time::instant_to_datetime(*time).ok_or(Error::CurrentTimeNotSet)?;
                    if let Some(user_id) = event.user_id() {
                        Ok(MixpanelEvent::new(
                            event.event_name(),
                            &token,
                            time,
                            user_id.to_string(),
                            event.mixpanel_properties(self.device_id),
                        ))
                    } else {
                        Ok(MixpanelEvent::new(
                            event.event_name(),
                            &token,
                            time,
                            self.device_id,
                            event.mixpanel_properties(self.device_id),
                        ))
                    }
                })
                .collect::<Result<Vec<_>, Error>>()?;
            // TODO: Pass events iterator without allocating a temporary collection vector?
            mp.submit(&events).await?;

            debug!("Telemetry: Flush successful");
            self.events.clear();
            self.last_flush = Instant::now();
        }

        Ok(())
    }
}
