use std::net::{IpAddr, SocketAddr};

use clap::Parser;
use reqwest::Url;

#[derive(Parser)]
pub(crate) struct Cli {
  #[arg(long, env = "DNS_HOLE_UDP_LISTEN_ADDRS", default_value = "0.0.0.0:53")]
  pub(crate) udp_listen_addr: Vec<SocketAddr>,
  #[arg(long, env = "DNS_HOLE_TCP_LISTEN_ADDRS", default_value = "0.0.0.0:53")]
  pub(crate) tcp_listen_addr: Vec<SocketAddr>,
  #[arg(short = 'u', long, env = "DNS_HOLE_STATS_URL")]
  pub(crate) stats_url: Url,
  #[arg(short = 't', long, env = "DNS_HOLE_STATS_TOKEN")]
  pub(crate) stats_token: String,
  #[arg(
    short = 'b',
    long,
    env = "DNS_HOLE_STATS_BUCKET",
    default_value = "dns-hole"
  )]
  pub(crate) stats_bucket: String,
  #[arg(short = 'o', long, env = "DNS_HOLE_STATS_ORG")]
  pub(crate) stats_org: String,
  #[arg(short = 'i', long, env = "DNS_HOLE_DNS_HTTPS_IPS", default_values = ["1.1.1.2", "1.0.0.2", "2606:4700:4700::1112", "2606:4700:4700::1002"])]
  pub(crate) dns_https_ip: Vec<IpAddr>,
  #[arg(
    short = 'p',
    long,
    env = "DNS_HOLE_DNS_HTTP_PORT",
    default_value_t = 443
  )]
  pub(crate) dns_https_port: u16,
  #[arg(
    short = 'd',
    long,
    env = "DNS_HOLE_DNS_DNS_NAME",
    default_value = "security.cloudflare-dns.com"
  )]
  pub(crate) dns_dns_name: String,
  #[arg(long, env = "DNS_HOLE_DB_URL")]
  pub(crate) db_url: String,
}
