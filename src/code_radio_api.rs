use crate::models::{
    code_radio::{CodeRadioMessage, Remote},
    server_sent_events::{Np, SeverSentEventsChannelMessage},
};
use anyhow::{anyhow, Result};
use eventsource_client::{Client, SSE::Event};
use futures_util::{FutureExt, Stream, StreamExt, TryStreamExt};
use std::{pin::Pin, time::Duration};

const REST_API_URL: &str =
    "https://coderadio-admin-v2.freecodecamp.org/api/nowplaying_static/coderadio.json";
const SERVER_SENT_EVENTS_API_URL: &str =
    "https://coderadio-admin-v2.freecodecamp.org/api/live/nowplaying/sse?cf_connect=%7B%22subs%22%3A%7B%22station%3Acoderadio%22%3A%7B%7D%7D%7D";

/// Get a `CodeRadioMessage` with REST API.
pub async fn get_message() -> Result<CodeRadioMessage> {
    let message: CodeRadioMessage = reqwest::get(REST_API_URL).await?.json().await?;
    Ok(message)
}

/// Get a `CodeRadioMessage` stream with Server-Sent Events API.
pub fn get_message_stream() -> Pin<Box<dyn Stream<Item = Result<CodeRadioMessage>>>> {
    let sse_client = eventsource_client::ClientBuilder::for_url(SERVER_SENT_EVENTS_API_URL)
        .unwrap()
        .reconnect(
            eventsource_client::ReconnectOptions::reconnect(true)
                .retry_initial(false)
                .delay(Duration::from_secs(1))
                .backoff_factor(2)
                .delay_max(Duration::from_secs(20))
                .build(),
        )
        .build();

    let mut sse_stream = sse_client.stream();

    sse_stream.next().now_or_never(); // Poll once to start connecting immediately

    let sse_message_stream = sse_stream
        .try_filter_map(|response| async move {
            if let Event(event) = response {
                if let Ok(message) =
                    serde_json::from_str::<SeverSentEventsChannelMessage<Np>>(&event.data)
                {
                    return Ok(Some(message.r#pub.data.np));
                }
            }
            Ok(None)
        })
        .map_err(|error| anyhow!("Server-Sent Events Error: {:#?}", error))
        .into_stream();

    Box::pin(sse_message_stream)
}

/// Get all stations with REST API.
pub async fn get_stations() -> Result<Vec<Remote>> {
    let message = get_message().await?;
    let stations = get_stations_from_message(&message);
    Ok(stations)
}

/// Get all stations from an existing `CodeRadioMessage`.
pub fn get_stations_from_message(message: &CodeRadioMessage) -> Vec<Remote> {
    let mut stations: Vec<Remote> = Vec::new();
    for remote in &message.station.remotes {
        stations.push(remote.clone());
    }
    for mount in &message.station.mounts {
        stations.push(mount.clone().into());
    }
    stations.sort_by_key(|s| s.id);
    stations
}
