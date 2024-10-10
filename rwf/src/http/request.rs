//! HTTP request.

use std::fmt::Debug;
use std::marker::Unpin;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Deserializer, Value};
use tokio::io::{AsyncRead, AsyncReadExt};

use super::{Cookies, Error, Head, Params, Response, ToParameter};
use crate::controller::{Session, SessionId};

/// HTTP request.
///
/// The request is fully loaded into memory. It's safe to clone
/// since the contents are behind an [`std::sync::Arc`].
#[derive(Debug, Clone, Default)]
pub struct Request {
    head: Head,
    session: Option<Session>,
    inner: Arc<Inner>,
    params: Option<Arc<Params>>,
}

#[derive(Debug, Default, Clone)]
struct Inner {
    body: Vec<u8>,
    cookies: Cookies,
    peer: Option<SocketAddr>,
}

impl Request {
    /// Read the request in its entirety from a stream.
    pub async fn read(peer: SocketAddr, mut stream: impl AsyncRead + Unpin) -> Result<Self, Error> {
        let head = Head::read(&mut stream).await?;
        let content_length = head.content_length().unwrap_or(0);
        let mut body = vec![0u8; content_length];
        stream
            .read_exact(&mut body)
            .await
            .map_err(|_| Error::MalformedRequest("incorrect content length"))?;

        let cookies = head.cookies();

        Ok(Request {
            head,
            params: None,
            session: cookies.get_session()?,
            inner: Arc::new(Inner {
                body,
                peer: Some(peer),
                cookies,
            }),
        })
    }

    /// Get the request's source IP address.
    pub fn peer(&self) -> &SocketAddr {
        self.inner
            .peer
            .as_ref()
            .expect("peer is not set on the request")
    }

    /// Set params on the request.
    pub fn with_params(mut self, params: Arc<Params>) -> Self {
        self.params = Some(params);
        self
    }

    pub fn head(&self) -> &Head {
        &self.head
    }

    pub fn head_mut(&mut self) -> &mut Head {
        &mut self.head
    }

    /// Extract a parameter from the provided path.
    pub fn parameter<T: ToParameter>(&self, name: &str) -> Result<Option<T>, Error> {
        if let Some(ref params) = self.params {
            if let Some(parameter) = params.parameter(self.path().base(), name) {
                return Ok(Some(T::to_parameter(&parameter)?));
            }
        }

        Ok(None)
    }

    /// Request's body as bytes.
    ///
    /// It's the job of the caller to handle encoding if any.
    pub fn body(&self) -> &[u8] {
        &self.inner.body
    }

    /// Request's body as JSON value.
    pub fn json_raw(&self) -> Result<Value, serde_json::Error> {
        self.json()
    }

    /// Request's body as HTML.
    /// UTF-8 encoding is assumed, and all incompatible characters are dropped.
    pub fn html(&self) -> String {
        String::from_utf8_lossy(self.body()).to_string()
    }

    /// Request's body deserialized from JSON into a particular Rust type.
    pub fn json<'a, T: Deserialize<'a>>(&'a self) -> Result<T, serde_json::Error> {
        let mut deserializer = Deserializer::from_slice(self.body());
        T::deserialize(&mut deserializer)
    }

    /// Request's cookies.
    pub fn cookies(&self) -> &Cookies {
        &self.inner.cookies
    }

    /// Request's session.
    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn session_id(&self) -> Option<SessionId> {
        self.session
            .as_ref()
            .map(|session| session.session_id.clone())
    }

    pub fn set_session(mut self, session: Option<Session>) -> Self {
        self.session = session;
        self
    }

    pub fn upgrade_websocket(&self) -> bool {
        self.headers()
            .get("connection")
            .map(|v| v.to_lowercase().contains("upgrade"))
            == Some(true)
            && self.headers().get("upgrade").map(|v| v.to_lowercase())
                == Some(String::from("websocket"))
    }

    pub fn login(&self, user_id: i64) -> Response {
        let mut session = self
            .session()
            .map(|s| s.clone())
            .unwrap_or(Session::empty());
        session.session_id = SessionId::Authenticated(user_id);
        Response::new().set_session(session)
    }

    pub fn logout(&self) -> Response {
        let mut session = self
            .session()
            .map(|s| s.clone())
            .unwrap_or(Session::empty());
        session.session_id = SessionId::default();
        Response::new().set_session(session)
    }
}

impl Deref for Request {
    type Target = Head;

    fn deref(&self) -> &Self::Target {
        &self.head
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_response() {
        #[derive(Deserialize)]
        struct Hello {
            hello: String,
        }

        let body = ("GET / HTTP/1.1\r\n".to_owned()
            + "Content-Type: application/json\r\n"
            + "Accept: */*\r\n"
            + "Content-Length: 18\r\n"
            + "\r\n"
            + r#"{"hello": "world"}"#)
            .as_bytes()
            .to_vec();
        let response = Request::read("127.0.0.1:1337".parse().unwrap(), &body[..])
            .await
            .expect("response");
        let json = response.json::<Hello>().expect("deserialize body");
        assert_eq!(json.hello, "world");
    }
}