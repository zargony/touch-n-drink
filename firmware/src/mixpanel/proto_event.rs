use crate::json::{self, FromJsonObject, ToJson};
use crate::telemetry;
use crate::time::DateTimeExt;
use alloc::string::String;
use embassy_time::Instant;
use embedded_io_async::{BufRead, Write};

/// `track` request
#[derive(Debug)]
pub struct TrackRequest<'a> {
    pub token: &'a str,
    pub device_id: &'a str,
    pub events: &'a [(Instant, telemetry::Event)],
}

impl ToJson for TrackRequest<'_> {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        json.write_array(self.events.iter().map(|(time, event)| Event {
            token: self.token,
            device_id: self.device_id,
            time,
            telemetry: event,
        }))
        .await
    }
}

/// `track` response
#[derive(Debug, Default)]
pub struct TrackResponse {
    pub error: String,
    pub status: u32,
}

impl FromJsonObject for TrackResponse {
    type Context<'ctx> = ();

    async fn read_next<R: BufRead>(
        &mut self,
        _key: String,
        json: &mut json::Reader<R>,
        _context: &Self::Context<'_>,
    ) -> Result<(), json::Error<R::Error>> {
        // FIXME: Mixpanel returns an empty body on success, which is not a valid JSON object
        json.skip_any().await
    }
}

/// Event
#[derive(Debug)]
struct Event<'a> {
    token: &'a str,
    device_id: &'a str,
    time: &'a Instant,
    telemetry: &'a telemetry::Event,
}

impl ToJson for Event<'_> {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        json.write_object()
            .await?
            .field("event", self.telemetry.event_name())
            .await?
            .field("properties", EventProperties { event: self })
            .await?
            .finish()
            .await
    }
}

/// Event properties
#[derive(Debug)]
struct EventProperties<'a> {
    event: &'a Event<'a>,
}

impl ToJson for EventProperties<'_> {
    async fn to_json<W: Write>(
        &self,
        json: &mut json::Writer<W>,
    ) -> Result<(), json::Error<W::Error>> {
        // Convert relative `Instant` time to absolute `DateTime` (needs current time set)
        let time = self
            .event
            .time
            .to_datetime()
            .ok_or(json::Error::InvalidType)?;

        let mut object = json.write_object().await?;

        // Reserved properties, see https://docs.mixpanel.com/docs/data-structure/property-reference/reserved-properties
        object
            .field("token", self.event.token)
            .await?
            .field("time", time.timestamp_millis())
            .await?;
        // Use user id as distinct id if event is associated with a user, use device id otherwise
        match self.event.telemetry.user_id() {
            Some(user_id) => object.field("distinct_id", user_id).await?,
            None => object.field("distinct_id", self.event.device_id).await?,
        };

        // Global custom properties
        object
            .field("firmware_version", crate::VERSION_STR)
            .await?
            .field("firmware_git_sha", crate::GIT_SHA_STR)
            .await?
            .field("device_id", self.event.device_id)
            .await?;
        // Event-specific custom properties
        self.event
            .telemetry
            .add_event_attributes(&mut object)
            .await?;

        object.finish().await
    }
}
