use anyhow::Result;
use colored::Colorize;
use comfy_table::{Cell, Color, Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};

use crate::score::ScoredResult;

fn format_speed(bps: f64) -> String {
    let mbps = bps / 1_048_576.0;
    if mbps >= 1.0 {
        format!("{:.2} MB/s", mbps)
    } else {
        let kbps = bps / 1024.0;
        format!("{:.1} KB/s", kbps)
    }
}

fn latency_color(ms: f64) -> Color {
    if ms < 100.0 {
        Color::Green
    } else if ms < 200.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn speed_color(bps: f64) -> Color {
    let mbps = bps / 1_048_576.0;
    if mbps > 5.0 {
        Color::Green
    } else if mbps > 1.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

pub fn print_results(results: &[ScoredResult], count: usize) {
    let display = &results[..count.min(results.len())];

    if display.is_empty() {
        println!("{}", "未找到可用的 IP".red());
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["排名", "IP 地址", "延迟", "丢包率", "速度", "综合分"]);

    for (i, r) in display.iter().enumerate() {
        let rank = format!("#{}", i + 1);
        let ms = r.latency.as_secs_f64() * 1000.0;
        let latency_str = format!("{:.1} ms", ms);
        let loss_str = format!("{:.0}%", r.loss_rate * 100.0);
        let speed_str = format_speed(r.speed_bps);
        let score_str = format!("{:.2}", r.score);

        let loss_color = if r.loss_rate == 0.0 {
            Color::Green
        } else {
            Color::Yellow
        };

        table.add_row(vec![
            Cell::new(rank),
            Cell::new(r.ip.to_string()),
            Cell::new(latency_str).fg(latency_color(ms)),
            Cell::new(loss_str).fg(loss_color),
            Cell::new(speed_str).fg(speed_color(r.speed_bps)),
            Cell::new(score_str),
        ]);
    }

    println!("\n{table}\n");

    // 输出最优 IP 方便复制
    if let Some(best) = display.first() {
        println!(
            "{}  {}",
            "最优 IP:".green().bold(),
            best.ip.to_string().white().bold()
        );
    }
}

pub fn write_csv(results: &[ScoredResult], path: &str) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(["IP", "延迟(ms)", "丢包率(%)", "速度(MB/s)", "综合分"])?;

    for r in results {
        let ms = r.latency.as_secs_f64() * 1000.0;
        let mbps = r.speed_bps / 1_048_576.0;
        wtr.write_record([
            r.ip.to_string(),
            format!("{:.1}", ms),
            format!("{:.0}", r.loss_rate * 100.0),
            format!("{:.2}", mbps),
            format!("{:.4}", r.score),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}
