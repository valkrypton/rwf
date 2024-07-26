use std::collections::HashMap;
use std::marker::Unpin;
use tokio::io::{AsyncRead, AsyncReadExt};

use super::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Version {
    Http1,
    Http2,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Head {
    method: String,
    path: String,
    version: String,
    headers: HashMap<String, String>,
}

impl Head {
    pub async fn read(mut stream: impl AsyncRead + Unpin) -> Result<Self, Error> {
        let request = Self::read_line(&mut stream)
            .await?
            .split(" ")
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let method = request
            .get(0)
            .ok_or(Error::MalformedRequest("method"))?
            .to_string();
        let path = request
            .get(1)
            .ok_or(Error::MalformedRequest("path"))?
            .to_string();
        let version = request
            .get(2)
            .ok_or(Error::MalformedRequest("version"))?
            .to_string();
        let mut headers = HashMap::new();

        loop {
            let header = Self::read_line(&mut stream).await?;
            if header.is_empty() {
                break;
            } else {
                let header = header
                    .split(":")
                    .map(|s| s.trim().to_string())
                    .collect::<Vec<_>>();
                let name = header
                    .get(0)
                    .ok_or(Error::MalformedRequest("header name"))?
                    .to_lowercase();
                let value = header
                    .get(1)
                    .ok_or(Error::MalformedRequest("header value"))?
                    .clone();
                headers.insert(name, value);
            }
        }

        Ok(Head {
            method,
            path,
            version,
            headers,
        })
    }

    pub fn http2(&self) -> bool {
        self.version == "HTTP/2"
    }

    pub fn http1(&self) -> bool {
        self.version == "HTTP/1.1"
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn content_length(&self) -> Option<usize> {
        if let Some(cl) = self.headers.get("content-length") {
            if let Ok(cl) = cl.parse::<usize>() {
                Some(cl)
            } else {
                None
            }
        } else {
            None
        }
    }

    async fn read_line(mut stream: impl AsyncRead + Unpin) -> Result<String, std::io::Error> {
        let mut buf = Vec::new();
        let (mut cr, mut lf) = (false, false);

        loop {
            let b = stream.read_u8().await?;

            if (b == '\r' as u8) {
                cr = true;
            } else if (b == '\n' as u8) {
                lf = true;
            } else {
                buf.push(b);
            }

            if cr && lf {
                break;
            }
        }

        Ok(String::from_utf8_lossy(&buf).to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_read_line() {
        let mut line = b"Content-Type: application/json\r\n";
        let result = Head::read_line(&line[..]).await.expect("read_line");
        assert_eq!(result, "Content-Type: application/json");
    }

    #[tokio::test]
    async fn test_parse_header() {
        let body = ("GET / HTTP/1.1\r\n".to_owned()
            + "Content-Type: application/json\r\n"
            + "Accept: */*\r\n"
            + "Content-Length: 4\r\n"
            + "\r\n"
            + "hello")
            .as_bytes()
            .to_vec();
        let head = Head::read(&body[..]).await.expect("head");
        assert!(head.http1());
        assert_eq!(head.method(), "GET");
        assert_eq!(head.content_length(), Some(4));
    }
}