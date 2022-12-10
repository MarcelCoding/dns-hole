use std::borrow::Borrow;
use std::str::FromStr;

use futures_util::stream::TryStreamExt;
use reqwest::Client;
use sqlx::FromRow;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;
use tracing::error;
use trust_dns_server::client::rr::LowerName;
use trust_dns_server::resolver::Name;
use uuid::Uuid;

#[derive(FromRow)]
struct Source {
  id: Uuid,
  url: String,
  last_updated: Option<OffsetDateTime>,
}

#[derive(FromRow)]
struct Count {
  count: i64,
}

#[derive(FromRow)]
struct Blocked {
  blocked: bool,
}

pub(crate) struct Blacklist {
  client: Client,
  pool: PgPool,
}

impl Blacklist {
  pub(crate) fn new(pool: PgPool) -> Self {
    Self {
      client: Client::new(),
      pool,
    }
  }

  pub(crate) async fn update(&self) -> anyhow::Result<()> {
    let sources = sqlx::query_as::<_, Source>("select id, url, last_updated from source")
      .fetch_all(&self.pool)
      .await?;

    let time = OffsetDateTime::now_utc() - Duration::days(1);

    for source in sources {
      if let Some(last_updated) = source.last_updated {
        if last_updated > time {
          continue;
        }
      }

      self.update_source(&source).await?;
    }

    Ok(())
  }

  async fn update_source(&self, source: &Source) -> anyhow::Result<()> {
    let response = self
      .client
      .get(&source.url)
      .send()
      .await?
      .error_for_status()?;

    fn convert_err(err: reqwest::Error) -> std::io::Error {
      todo!()
    }
    let reader = StreamReader::new(response.bytes_stream().map_err(convert_err));
    let mut lines = reader.lines();

    let mut tx = self.pool.begin().await?;

    while let Some(line) = lines.next_line().await? {
      if line.is_empty() || line.starts_with('#') {
        continue;
      }

      match LowerName::from_str(&line) {
        Ok(name) => {
          sqlx::query("insert into blacklist (domain, source) values ($1::varchar(255), $2::uuid)")
            .bind(format!("{}", name))
            .bind(&source.id)
            .execute(&mut tx)
            .await?;
        }
        Err(err) => error!("Unable to parse domain: {}", err),
      };
    }

    sqlx::query("update source set last_updated = current_timestamp where id = $1::uuid")
      .bind(&source.id)
      .execute(&mut tx)
      .await?;

    tx.commit().await?;

    Ok(())
  }

  pub(crate) async fn is_blocked(&self, name: &Name) -> anyhow::Result<bool> {
    let blocked = sqlx::query_as::<_, Blocked>(
      "select exists(select 1 from blacklist where domain=$1::varchar(255))",
    )
    .bind(format!("{}", name))
    .fetch_one(&self.pool)
    .await?
    .blocked;

    Ok(blocked)
  }

  pub(crate) async fn len(&self) -> anyhow::Result<i64> {
    let count = sqlx::query_as::<_, Count>("select count(*), 1 from blacklist")
      .fetch_one(&self.pool)
      .await?
      .count;

    Ok(count)
  }
}
