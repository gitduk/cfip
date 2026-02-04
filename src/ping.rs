use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct PingResult {
    pub ip: IpAddr,
    pub avg_latency: Duration,
    pub loss_rate: f64,
}

async fn tcp_ping(ip: IpAddr, port: u16, timeout: Duration) -> Option<Duration> {
    let addr = SocketAddr::new(ip, port);
    let start = Instant::now();
    match tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
        Ok(Ok(_stream)) => Some(start.elapsed()),
        _ => None,
    }
}

pub async fn test_latency(ips: &[IpAddr], config: &Config) -> Result<Vec<PingResult>> {
    let total = ips.len();
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.green/white} {pos}/{len} 延迟测试中...")
            .unwrap()
            .progress_chars("=> "),
    );

    let semaphore = Arc::new(Semaphore::new(config.threads));
    let timeout = Duration::from_millis(config.timeout_ms);
    let latency_limit = Duration::from_millis(config.latency_limit_ms);
    let ping_times = config.ping_times;
    let port = config.port;
    let pb = Arc::new(pb);

    let mut handles = Vec::with_capacity(total);

    for &ip in ips {
        let sem = semaphore.clone();
        let pb = pb.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            let mut successes = Vec::new();
            let mut failures = 0usize;

            for _ in 0..ping_times {
                match tcp_ping(ip, port, timeout).await {
                    Some(latency) => successes.push(latency),
                    None => failures += 1,
                }
            }

            pb.inc(1);

            let total_attempts = ping_times;
            let loss_rate = failures as f64 / total_attempts as f64;

            if successes.is_empty() {
                return None;
            }

            // 丢包率超过 50% 视为不可用
            if loss_rate > 0.5 {
                return None;
            }

            let avg = successes.iter().sum::<Duration>() / successes.len() as u32;

            if avg > latency_limit {
                return None;
            }

            Some(PingResult {
                ip,
                avg_latency: avg,
                loss_rate,
            })
        });

        handles.push(handle);
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(Some(result)) = handle.await {
            results.push(result);
        }
    }

    pb.finish_and_clear();

    results.sort_by(|a, b| a.avg_latency.cmp(&b.avg_latency));
    Ok(results)
}
