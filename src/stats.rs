use std::io::Write;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::header::{AUTHORIZATION, CONTENT_ENCODING};
use reqwest::{Client, Url};
use serde::Serialize;
use tokio::sync::Mutex;
use trust_dns_server::client::op::ResponseCode;
use trust_dns_server::client::rr::{DNSClass, LowerName, RecordType};
use trust_dns_server::server::{Protocol, Request, RequestHandler, ResponseHandler, ResponseInfo};

const BUFFER_SIZE: usize = 128;

#[derive(Clone)]
struct Entry {
  timestamp: SystemTime,
  src: IpAddr,
  protocol: Protocol,
  query: LowerName,
  query_type: RecordType,
  query_class: DNSClass,
  response_code: ResponseCode,
}

pub(crate) struct Stats<T>(Arc<InnerStats<T>>);

struct InnerStats<T> {
  endpoint: Url,
  query: InfluxWriteQuery,
  auth: String,
  client: Client,
  buffer: Mutex<Vec<Entry>>,
  delegate: T,
}

#[derive(Serialize)]
enum WritePrecision {
  // #[serde(rename = "s")]
  // Seconds,
  #[serde(rename = "ms")]
  Milliseconds,
  // #[serde(rename = "us")]
  // Microseconds,
  // #[serde(rename = "ns")]
  // Nanoseconds,
}

#[derive(Serialize)]
struct InfluxWriteQuery {
  bucket: String,
  org: String,
  precision: WritePrecision,
}

impl Entry {
  fn write<W: Write>(&self, w: &mut W) -> anyhow::Result<()> {
    let timestamp = self.timestamp.duration_since(UNIX_EPOCH)?.as_millis();

    writeln!(
      w,
      "queries,src={},protocol={},query={},type={},class={},response_code=\"{}\" {}",
      self.src,
      self.protocol,
      self.query,
      self.query_type,
      self.query_class,
      self.response_code,
      timestamp
    )?;

    Ok(())
  }
}

impl<T: RequestHandler> Stats<T> {
  pub(crate) fn new(endpoint: &Url, bucket: String, org: String, token: &str, delegate: T) -> Self {
    Self(Arc::new(InnerStats {
      endpoint: endpoint.join("api/v2/write").unwrap(),
      auth: format!("Token {}", token),
      client: Client::new(),
      buffer: Mutex::new(Vec::with_capacity(BUFFER_SIZE)),
      query: InfluxWriteQuery {
        bucket,
        org,
        precision: WritePrecision::Milliseconds,
      },
      delegate,
    }))
  }

  async fn push(&self, entry: Entry) {
    self.0.buffer.lock().await.push(entry);
  }

  pub(crate) async fn flush(&self) -> anyhow::Result<()> {
    let entries = {
      let mut guard = self.0.buffer.lock().await;
      let buffer = guard.to_vec();
      *guard = Vec::with_capacity(BUFFER_SIZE);
      buffer
    };

    if entries.is_empty() {
      return Ok(());
    }

    let mut buf = Vec::new();
    {
      let mut encoder = GzEncoder::new(&mut buf, Compression::default());

      for entry in entries {
        entry.write(&mut encoder)?;
      }
    }

    self
      .0
      .client
      .post(self.0.endpoint.clone())
      .query(&self.0.query)
      .header(CONTENT_ENCODING, "gzip")
      .header(AUTHORIZATION, &self.0.auth)
      .body(buf)
      .send()
      .await?
      .error_for_status()?;

    Ok(())
  }

  pub(crate) fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[async_trait]
impl<T: RequestHandler> RequestHandler for Stats<T> {
  async fn handle_request<R: ResponseHandler>(
    &self,
    request: &Request,
    response_handle: R,
  ) -> ResponseInfo {
    let timestamp = SystemTime::now();

    let response = self
      .0
      .delegate
      .handle_request(request, response_handle)
      .await;

    {
      let entry = Entry {
        timestamp,
        src: request.src().ip(),
        protocol: request.protocol(),
        query: request.query().name().to_owned(),
        query_type: request.query().query_type(),
        query_class: request.query().query_class(),
        response_code: response.response_code(),
      };

      self.push(entry).await;
    }

    response
  }
}
