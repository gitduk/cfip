use std::net::IpAddr;

use anyhow::{Context, Result};
use ipnetwork::IpNetwork;
use rand::seq::SliceRandom;

const CF_IPV4_URL: &str = "https://www.cloudflare.com/ips-v4/";
const CF_IPV6_URL: &str = "https://www.cloudflare.com/ips-v6/";

pub async fn fetch_ip_ranges(include_ipv6: bool) -> Result<Vec<IpNetwork>> {
    let mut ranges = Vec::new();

    let body = reqwest::get(CF_IPV4_URL)
        .await
        .context("获取 IPv4 地址段失败")?
        .text()
        .await?;

    for line in body.lines() {
        let line = line.trim();
        if !line.is_empty() {
            if let Ok(net) = line.parse::<IpNetwork>() {
                ranges.push(net);
            }
        }
    }

    if include_ipv6 {
        let body = reqwest::get(CF_IPV6_URL)
            .await
            .context("获取 IPv6 地址段失败")?
            .text()
            .await?;

        for line in body.lines() {
            let line = line.trim();
            if !line.is_empty() {
                if let Ok(net) = line.parse::<IpNetwork>() {
                    ranges.push(net);
                }
            }
        }
    }

    if ranges.is_empty() {
        anyhow::bail!("未获取到任何 Cloudflare IP 段");
    }

    Ok(ranges)
}

pub fn sample_ips(ranges: &[IpNetwork]) -> Vec<IpAddr> {
    let mut rng = rand::thread_rng();
    let mut ips = Vec::new();

    for &network in ranges {
        let prefix = network.prefix();
        let hosts: Vec<IpAddr> = network.iter().collect();

        // 排除网络地址和广播地址 (仅 IPv4 且子网 >= /31)
        let usable: Vec<IpAddr> = if hosts.len() > 2 {
            hosts[1..hosts.len() - 1].to_vec()
        } else {
            hosts
        };

        if usable.is_empty() {
            continue;
        }

        let sample_count = match network {
            IpNetwork::V4(_) => match prefix {
                32 => 1,
                31..=32 => usable.len(),
                25..=30 => 2,
                21..=24 => 5,
                17..=20 => 10,
                _ => 20,
            },
            IpNetwork::V6(_) => 5,
        };

        let sample_count = sample_count.min(usable.len());

        if sample_count >= usable.len() {
            ips.extend(usable);
        } else {
            // 随机采样不重复的 IP
            let mut indices: Vec<usize> = (0..usable.len()).collect();
            indices.partial_shuffle(&mut rng, sample_count);
            for &idx in &indices[..sample_count] {
                ips.push(usable[idx]);
            }
        }
    }

    ips.shuffle(&mut rng);
    ips
}
