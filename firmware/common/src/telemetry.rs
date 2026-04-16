use crate::mixpanel::{self, Event as MixpanelEvent, Mixpanel};
use crate::{GIT_SHA_STR, VERSION_STR, article, nfc, time, user};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use chrono::{DateTime, Utc};
use derive_more::{Display, From};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, TrySendError};
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer, with_deadline};
use embedded_nal_async::{Dns, TcpConnect};
use log::{debug, info, warn};
use reqwless::client::HttpClient;
use serde::Serialize;
use serde_with::{DisplayFromStr, serde_as};

/// Time to wait for more events to buffer before submitting to server
const WAIT_MORE_TIMEOUT: Duration = Duration::from_secs(30);

/// Max number of events to buffer before submitting to server
const MAX_BUFFER_EVENTS: usize = 10;

/// Telemetry event channel. Used to receive events to track from other tasks.
static CHANNEL: Channel<CriticalSectionRawMutex, (Instant, Event), 32> = Channel::new();

/// Telemetry flush signal. Used to force submitting events now
static FLUSH: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Telemetry idle signal. Can be used to wait for all events being submitted
static IDLE: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Force to submit telemetry events immediately and wait for completion
pub async fn flush() {
    FLUSH.signal(());
    IDLE.wait().await;
}

/// Telemetry error
#[derive(Debug, Display, From)]
#[must_use]
enum Error {
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
#[derive(Debug, Clone)]
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
    pub fn track(self) {
        debug!("Telemetry: Tracking event {self:?}");
        match CHANNEL.try_send((Instant::now(), self)) {
            Ok(()) => (),
            Err(TrySendError::Full((_time, event))) => {
                warn!(
                    "Telemetry: Event queue overflow, dropping event '{}'!",
                    event.event_name()
                );
            }
        }
    }

    /// Track event and submit immediately
    pub fn track_now(self) {
        self.track();
        FLUSH.signal(());
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

    /// Mixpanel event for Mixpanel submission
    fn mixpanel_event<'a>(
        &'a self,
        token: &'a str,
        time: DateTime<Utc>,
        device_id: &'a str,
    ) -> MixpanelEvent<'a, MixpanelEventProperties<'a>> {
        let user_or_device_id = if let Some(user_id) = self.user_id() {
            user_id.to_string()
        } else {
            device_id.to_string()
        };
        MixpanelEvent::new(
            self.event_name(),
            token,
            time,
            user_or_device_id,
            self.mixpanel_properties(device_id),
        )
    }
}

/// Telemetry for tracking events
#[must_use]
pub struct Telemetry {
    mp_token: Option<String>,
    device_id: String,
    events: Vec<(Instant, Event)>,
}

impl Telemetry {
    /// Create new telemetry
    pub fn new(mp_token: Option<&str>, device_id: &str) -> Self {
        if let Some(token) = mp_token {
            info!("Telemetry: Initialized with Mixpanel token {token}");
        } else {
            warn!("Telemetry: Disabled! No Mixpanel token.");
        }
        Self {
            mp_token: mp_token.map(ToString::to_string),
            device_id: device_id.to_string(),
            events: Vec::with_capacity(MAX_BUFFER_EVENTS),
        }
    }

    /// Run telemetry submission
    pub async fn run<T: TcpConnect, D: Dns>(&mut self, http: &mut HttpClient<'_, T, D>) -> ! {
        loop {
            // Receive and buffer events
            match select(self.receive(), FLUSH.wait()).await {
                Either::First(()) => (),
                Either::Second(()) => {
                    debug!("Telemetry: Flush received, submitting now");
                    FLUSH.reset();
                }
            }

            // Submit buffered events to server
            loop {
                match self.submit(http).await {
                    Ok(()) => break,
                    Err(err) => {
                        warn!("Telemetry: Submission failed, retrying in 10s: {err}");
                        Timer::after_secs(10).await;
                    }
                }
            }
        }
    }
}

impl Telemetry {
    /// Receive event and buffer it for submission (fills self.events)
    async fn receive(&mut self) {
        // Signal idle when there is no event right now
        if self.events.is_empty() && CHANNEL.is_empty() {
            IDLE.signal(());
        }

        // Wait for first event to buffer
        let (time, event) = CHANNEL.receive().await;
        self.events.push((time, event));
        IDLE.reset();

        // Deadline for waiting is time of first buffered event + timeout
        let deadline = time + WAIT_MORE_TIMEOUT;

        // Wait for either enough events or timeout
        while let Ok((time, event)) = with_deadline(deadline, CHANNEL.receive()).await {
            self.events.push((time, event));
            if self.events.len() >= MAX_BUFFER_EVENTS {
                break;
            }
        }
    }

    /// Submit tracked events to server (drains self.events)
    async fn submit<T: TcpConnect, D: Dns>(
        &mut self,
        http: &mut HttpClient<'_, T, D>,
    ) -> Result<(), Error> {
        if self.events.is_empty() {
            return Ok(());
        }

        if let Some(ref token) = self.mp_token {
            debug!("Telemetry: Submitting {} events...", self.events.len());

            let mut mixpanel = Mixpanel::new(token);
            let mut mp = mixpanel.connect(http).await?;

            let events = self
                .events
                .iter()
                .map(|(time, event)| -> Result<_, Error> {
                    let time = time::instant_to_datetime(*time).ok_or(Error::CurrentTimeNotSet)?;
                    Ok(event.mixpanel_event(token, time, &self.device_id))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            // TODO: Pass events iterator without allocating a temporary collection vector?
            mp.submit(&events).await?;

            debug!("Telemetry: Submission successful");
        }

        self.events.clear();
        Ok(())
    }
}
