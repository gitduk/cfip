mod config;
mod ip;
mod output;
mod ping;
mod score;
mod speed;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();

    // 1. 获取 Cloudflare IP 段
    println!("{}", "正在获取 Cloudflare IP 段...".cyan());
    let ranges = ip::fetch_ip_ranges(config.ipv6).await?;
    println!("获取到 {} 个 CIDR 段", ranges.len());

    // 2. 随机采样 IP
    let ips = ip::sample_ips(&ranges);
    println!("采样得到 {} 个 IP 地址\n", ips.len());

    // 3. Phase 1: 延迟测试
    println!("{}", "Phase 1: 延迟测试".cyan().bold());
    let ping_results = ping::test_latency(&ips, &config).await?;
    println!("延迟测试完成，{} 个 IP 通过筛选\n", ping_results.len());

    if ping_results.is_empty() {
        println!(
            "{}",
            "没有 IP 通过延迟筛选，请尝试增大 --latency-limit".red()
        );
        return Ok(());
    }

    // 4. Phase 2: 速度测试
    println!("{}", "Phase 2: 速度测试".green().bold());
    let speed_results = speed::test_speed(&ping_results, &config).await?;

    if speed_results.is_empty() {
        println!("{}", "没有 IP 通过速度测试".red());
        return Ok(());
    }

    // 5. 综合评分
    let scored = score::calculate_scores(&speed_results);

    // 6. 输出结果
    output::print_results(&scored, config.count);

    if let Some(ref path) = config.output {
        output::write_csv(&scored, path)?;
        println!("结果已保存到 {}", path.green());
    }

    Ok(())
}
