use std::net::IpAddr;
use std::time::{Duration, Instant};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::config::Config;
use crate::ping::PingResult;

#[derive(Debug, Clone)]
pub struct SpeedResult {
    pub ip: IpAddr,
    pub avg_latency: Duration,
    pub loss_rate: f64,
    pub speed_bps: f64,
}

pub async fn test_speed(ping_results: &[PingResult], config: &Config) -> Result<Vec<SpeedResult>> {
    let count = config.speed_count.min(ping_results.len());
    let candidates = &ping_results[..count];

    let pb = ProgressBar::new(count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.green/white} {pos}/{len} 速度测试中...")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut results = Vec::with_capacity(count);
    let url = format!("{}?bytes={}", config.test_url, config.download_size);
    let test_duration = Duration::from_secs(10);

    for candidate in candidates {
        let ip = candidate.ip;
        pb.set_message(format!("{}", ip));

        match test_download(&url, ip, config.port, test_duration).await {
            Ok(speed_bps) => {
                results.push(SpeedResult {
                    ip,
                    avg_latency: candidate.avg_latency,
                    loss_rate: candidate.loss_rate,
                    speed_bps,
                });
            }
            Err(_) => {
                // IP 速度测试失败，跳过
            }
        }

        pb.inc(1);
    }

    pb.finish_and_clear();
    Ok(results)
}

async fn test_download(url: &str, ip: IpAddr, port: u16, max_duration: Duration) -> Result<f64> {
    let client = reqwest::Client::builder()
        .resolve("speed.cloudflare.com", (ip, port).into())
        .timeout(max_duration)
        .danger_accept_invalid_certs(true)
        .build()?;

    let start = Instant::now();
    let response = client.get(url).send().await?;

    let mut total_bytes: u64 = 0;
    let mut stream = response;

    while let Some(chunk) = stream.chunk().await? {
        total_bytes += chunk.len() as u64;
        if start.elapsed() >= max_duration {
            break;
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    if elapsed < 0.001 || total_bytes == 0 {
        anyhow::bail!("下载数据不足");
    }

    Ok(total_bytes as f64 / elapsed)
}
