use std::net::IpAddr;
use std::time::Duration;

use crate::speed::SpeedResult;

#[derive(Debug, Clone)]
pub struct ScoredResult {
    pub ip: IpAddr,
    pub latency: Duration,
    pub loss_rate: f64,
    pub speed_bps: f64,
    pub score: f64,
}

pub fn calculate_scores(results: &[SpeedResult]) -> Vec<ScoredResult> {
    if results.is_empty() {
        return Vec::new();
    }

    if results.len() == 1 {
        let r = &results[0];
        return vec![ScoredResult {
            ip: r.ip,
            latency: r.avg_latency,
            loss_rate: r.loss_rate,
            speed_bps: r.speed_bps,
            score: 1.0,
        }];
    }

    let latencies: Vec<f64> = results
        .iter()
        .map(|r| r.avg_latency.as_secs_f64())
        .collect();
    let speeds: Vec<f64> = results.iter().map(|r| r.speed_bps).collect();

    let lat_min = latencies.iter().cloned().fold(f64::INFINITY, f64::min);
    let lat_max = latencies.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let spd_min = speeds.iter().cloned().fold(f64::INFINITY, f64::min);
    let spd_max = speeds.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let lat_range = lat_max - lat_min;
    let spd_range = spd_max - spd_min;

    let mut scored: Vec<ScoredResult> = results
        .iter()
        .map(|r| {
            let lat = r.avg_latency.as_secs_f64();
            let spd = r.speed_bps;

            // 归一化: 延迟越低越好, 速度越高越好
            let lat_score = if lat_range > 0.0 {
                1.0 - (lat - lat_min) / lat_range
            } else {
                1.0
            };
            let spd_score = if spd_range > 0.0 {
                (spd - spd_min) / spd_range
            } else {
                1.0
            };

            let score = lat_score * 0.3 + spd_score * 0.7;

            ScoredResult {
                ip: r.ip,
                latency: r.avg_latency,
                loss_rate: r.loss_rate,
                speed_bps: r.speed_bps,
                score,
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}
