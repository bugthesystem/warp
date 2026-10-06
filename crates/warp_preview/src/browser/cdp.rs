//! A small blocking client for the Chrome DevTools protocol, over the WebSocket a browser opens
//! with `--remote-debugging-port`.

use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::TcpStream;
use std::time::Duration;

use instant::Instant;

use serde_json::{Map, Value, json};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message as WsMessage, WebSocket};

/// How long one read waits before the caller gets a chance to do other work.
const READ_POLL: Duration = Duration::from_millis(30);

/// How long a command waits for its response.
const CALL_TIMEOUT: Duration = Duration::from_secs(15);

/// A message from the browser.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Response {
        id: u64,
        result: Result<Value, String>,
    },
    Event {
        method: String,
        params: Value,
        session: Option<String>,
    },
}

/// Reads a message the browser sent. `None` for text that is not a protocol message.
pub fn parse_message(text: &str) -> Option<Message> {
    let value: Value = serde_json::from_str(text).ok()?;
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        let result = match value.get("error") {
            Some(error) => Err(error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("The browser reported an error")
                .to_owned()),
            None => Ok(value.get("result").cloned().unwrap_or(Value::Null)),
        };
        return Some(Message::Response { id, result });
    }
    Some(Message::Event {
        method: value.get("method")?.as_str()?.to_owned(),
        params: value.get("params").cloned().unwrap_or(Value::Null),
        session: value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// Writes a command, addressed to `session` when given.
pub fn command_message(id: u64, method: &str, params: Value, session: Option<&str>) -> String {
    let mut message = Map::new();
    message.insert("id".to_owned(), json!(id));
    message.insert("method".to_owned(), json!(method));
    message.insert("params".to_owned(), params);
    if let Some(session) = session {
        message.insert("sessionId".to_owned(), json!(session));
    }
    Value::Object(message).to_string()
}

/// The browser's DevTools address from a line it prints on startup.
pub fn devtools_url(line: &str) -> Option<&str> {
    let start = line.find("ws://")?;
    Some(line[start..].trim())
}

pub struct Cdp {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    next_id: u64,
    /// Events that arrived while waiting for a response.
    events: VecDeque<Message>,
}

impl Cdp {
    pub fn connect(url: &str) -> Result<Self, String> {
        let (socket, _) =
            tungstenite::connect(url).map_err(|err| format!("Couldn't reach Chromium: {err}"))?;
        if let MaybeTlsStream::Plain(stream) = socket.get_ref() {
            stream
                .set_read_timeout(Some(READ_POLL))
                .map_err(|err| err.to_string())?;
        }
        Ok(Self {
            socket,
            next_id: 1,
            events: VecDeque::new(),
        })
    }

    /// Sends a command without waiting for its response, returning its id.
    pub fn send(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.socket
            .send(WsMessage::text(command_message(
                id, method, params, session,
            )))
            .map_err(|err| format!("Lost the connection to Chromium: {err}"))?;
        Ok(id)
    }

    /// Sends a command and waits for its result. Events that arrive meanwhile are kept for
    /// [`Self::next_event`].
    pub fn call(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, String> {
        let id = self.send(method, params, session)?;
        let deadline = Instant::now() + CALL_TIMEOUT;
        while Instant::now() < deadline {
            match self.read()? {
                Some(Message::Response { id: got, result }) if got == id => {
                    return result.map_err(|err| format!("{method} failed: {err}"));
                }
                Some(event @ Message::Event { .. }) => self.events.push_back(event),
                Some(Message::Response { .. }) | None => {}
            }
        }
        Err(format!("Chromium didn't answer {method}"))
    }

    /// The next event, waiting briefly. `None` when nothing arrived.
    pub fn next_event(&mut self) -> Result<Option<Message>, String> {
        if let Some(event) = self.events.pop_front() {
            return Ok(Some(event));
        }
        loop {
            match self.read()? {
                Some(event @ Message::Event { .. }) => return Ok(Some(event)),
                Some(Message::Response { .. }) => continue,
                None => return Ok(None),
            }
        }
    }

    fn read(&mut self) -> Result<Option<Message>, String> {
        match self.socket.read() {
            Ok(WsMessage::Text(text)) => Ok(parse_message(&text)),
            Ok(WsMessage::Close(_)) => Err("Chromium closed the connection".to_owned()),
            Ok(_) => Ok(None),
            Err(tungstenite::Error::Io(err))
                if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
            {
                Ok(None)
            }
            Err(err) => Err(format!("Lost the connection to Chromium: {err}")),
        }
    }

    pub fn close(mut self) {
        let _ = self.socket.close(None);
    }
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
