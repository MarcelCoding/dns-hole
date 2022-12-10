use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use sqlx::PgPool;
use tokio::net::{TcpListener, UdpSocket};
use tokio::time::sleep;
use tracing::{error, info};
use trust_dns_server::authority::{Catalog, ZoneType};
use trust_dns_server::client::rr::{LowerName, Name};
use trust_dns_server::resolver::config::NameServerConfigGroup;
use trust_dns_server::store::forwarder::{ForwardAuthority, ForwardConfig};
use trust_dns_server::ServerFuture;

use crate::blacklist::Blacklist;
use crate::cli::Cli;
use crate::stats::Stats;

mod blacklist;
mod cli;
mod stats;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt::init();

  let cli = Cli::parse();

  let pool = PgPool::connect(&cli.db_url).await?;
  let blacklist = Blacklist::new(pool);
  blacklist.update().await?;
  // blacklist.fetch_list("https://raw.githubusercontent.com/RPiList/specials/master/Blocklisten/Win10Telemetry".parse()?).await?;

  info!("{}", blacklist.len().await?);

  let auth = Arc::new(
    ForwardAuthority::try_from_config(
      Name::root(),
      ZoneType::Forward,
      &ForwardConfig {
        name_servers: NameServerConfigGroup::from_ips_https(
          &cli.dns_https_ip,
          cli.dns_https_port,
          cli.dns_dns_name,
          true,
        ),
        options: None,
      },
    )
    .unwrap(),
  );

  let mut catalog = Catalog::new();
  catalog.upsert(LowerName::new(&Name::root()), Box::new(auth));

  let stats = Stats::new(
    &cli.stats_url,
    cli.stats_bucket,
    cli.stats_org,
    &cli.stats_token,
    catalog,
    blacklist,
  );

  let mut server = ServerFuture::new(stats.clone());

  for addr in cli.udp_listen_addr {
    info!("Listening on {}/udp", addr);
    server.register_socket(UdpSocket::bind(addr).await?);
  }

  for addr in cli.tcp_listen_addr {
    info!("Listening on {}/tcp", addr);
    server.register_listener(TcpListener::bind(addr).await?, Duration::from_secs(10));
  }

  tokio::spawn(async move {
    loop {
      if let Err(err) = stats.flush().await {
        error!("Error: {}", err);
      }
      sleep(Duration::from_secs(10)).await;
    }
  });

  server.block_until_done().await?;

  Ok(())
}
