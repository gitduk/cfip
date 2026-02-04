mod cloudflare;
mod config;
mod ip;
mod output;
mod ping;
mod score;
mod speed; // Add cloudflare module

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use dotenvy::dotenv;
use reqwest::Client;
use std::env;
use std::io::{self, Write}; // Import io and Write // Import reqwest::Client

use config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let mut config = Config::parse();

    config.cloudflare_api_token = env::var("CLOUDFLARE_API_TOKEN").ok();
    config.cloudflare_zone_id = env::var("CLOUDFLARE_ZONE_ID").ok();
    config.cloudflare_record_name = env::var("CLOUDFLARE_RECORD_NAME").ok();
    config.cloudflare_record_type = env::var("CLOUDFLARE_RECORD_TYPE")
        .ok()
        .or(Some("A".to_string())); // Default to "A" record
    config.cloudflare_proxied = env::var("CLOUDFLARE_PROXIED")
        .ok()
        .and_then(|s| s.parse::<bool>().ok())
        .or(Some(false)); // Default to not proxied

    // --- Cloudflare DNS Update Pre-checks and Dynamic ZONE_ID Fetching (Optimization Start) ---
    let mut needs_cloudflare_update = true; // Flag to control if we proceed with CF update

    if config.cloudflare_api_token.is_none() {
        println!(
            "{}",
            "警告: 环境变量 CLOUDFLARE_API_TOKEN 未设置，将跳过 Cloudflare DNS 更新。".yellow()
        );
        needs_cloudflare_update = false;
    }
    if config.cloudflare_record_name.is_none() {
        println!(
            "{}",
            "警告: 环境变量 CLOUDFLARE_RECORD_NAME 未设置，将跳过 Cloudflare DNS 更新。".yellow()
        );
        needs_cloudflare_update = false;
    }

    if needs_cloudflare_update && config.cloudflare_zone_id.is_none() {
        println!("{}", "* 解析 Zone ID".cyan().bold());
        if let (Some(api_token), Some(record_name)) =
            (&config.cloudflare_api_token, &config.cloudflare_record_name)
        {
            let client = Client::new();
            // Extract root domain from record_name for zone lookup
            let domain_parts: Vec<&str> = record_name.split('.').collect();
            if domain_parts.len() >= 2 {
                let root_domain = format!(
                    "{}.{}",
                    domain_parts[domain_parts.len() - 2],
                    domain_parts[domain_parts.len() - 1]
                );
                match cloudflare::get_zone_id_by_name(&client, api_token, &root_domain).await {
                    Ok(Some(zone_id)) => {
                        config.cloudflare_zone_id = Some(zone_id);
                    }
                    Ok(None) => {
                        eprintln!(
                            "{} 未找到域名 '{}' 对应的 Zone ID，请检查 CLOUDFLARE_API_TOKEN 权限或 CLOUDFLARE_RECORD_NAME 是否正确。",
                            "错误".red(),
                            root_domain
                        );
                        needs_cloudflare_update = false;
                    }
                    Err(e) => {
                        eprintln!("{} 获取 Zone ID 失败: {}", "错误".red(), e);
                        needs_cloudflare_update = false;
                    }
                }
            } else {
                eprintln!(
                    "{} CLOUDFLARE_RECORD_NAME '{}' 格式不正确，无法提取根域名，将跳过 Cloudflare DNS 更新。",
                    "错误".red(),
                    record_name
                );
                needs_cloudflare_update = false;
            }
        }
    } else if needs_cloudflare_update && config.cloudflare_zone_id.is_none() {
        // This case should not be hit if needs_cloudflare_update is correctly managed, but as a fallback
        println!(
            "{}",
            "警告: CLOUDFLARE_ZONE_ID 未设置，且未能动态获取，将跳过 Cloudflare DNS 更新。"
                .yellow()
        );
        needs_cloudflare_update = false;
    }
    // --- Cloudflare DNS Update Pre-checks and Dynamic ZONE_ID Fetching (Optimization End) ---

    // 1. 获取 Cloudflare IP 段
    let ranges = ip::fetch_ip_ranges(config.ipv6).await?;

    // 2. 随机采样 IP
    let ips = ip::sample_ips(&ranges);

    // 3. 延迟测试
    println!("{}", "* 延迟测试".cyan().bold());
    let ping_results = ping::test_latency(&ips, &config).await?;
    println!(
        "{}",
        format!("延迟测试完成，{} 个 IP 通过筛选。\n", ping_results.len()).green()
    );

    // Recommendation for proxy testing if latency is very low
    if !ping_results.is_empty() {
        let min_latency_ms = ping_results
            .iter()
            .map(|r| r.avg_latency.as_millis())
            .min()
            .unwrap_or(u128::MAX); // Get the minimum latency in milliseconds

        if min_latency_ms < 5 {
            println!("{}", "提示: 检测到极低延迟 (小于 5ms)。\n如果您正在使用代理测试，建议关闭代理以获得更准确的 Cloudflare 优选 IP。\n".yellow());
        }
    }

    if ping_results.is_empty() {
        println!(
            "{}",
            "没有 IP 通过延迟筛选，请尝试增大 --latency-limit".red()
        );
        return Ok(());
    }

    // 4. 速度测试
    println!("{}", "* 速度测试".cyan().bold());
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
        println!("{}", format!("结果已保存到 '{}'。", path.green()).green());
    }

    // 7. Cloudflare DNS 更新
    if needs_cloudflare_update {
        // Safe to unwrap here because needs_cloudflare_update is true and checks before would have caught None
        let api_token = config.cloudflare_api_token.as_ref().unwrap();
        let zone_id = config.cloudflare_zone_id.as_ref().unwrap();
        let record_name = config.cloudflare_record_name.as_ref().unwrap();
        let record_type = config.cloudflare_record_type.as_ref().unwrap();
        let proxied = config.cloudflare_proxied.unwrap();

        if scored.is_empty() {
            println!("{}", "没有找到最优 IP，跳过 Cloudflare DNS 更新。".yellow());
            return Ok(());
        }

        let optimal_ip_str: String;

        if config.quiet {
            if let Some(ip) = scored.first().map(|s| s.ip.to_string()) {
                optimal_ip_str = ip;
                println!(
                    "{}",
                    format!(
                        "Quiet模式启用，自动选择最优 IP: '{}' 进行更新。",
                        optimal_ip_str.green()
                    )
                    .cyan()
                );
            } else {
                println!("{}", "没有找到最优 IP，跳过 Cloudflare DNS 更新。".yellow());
                return Ok(());
            }
        } else {
            println!("\n{}", "Cloudflare DNS 更新选项:".cyan().bold());
            println!(
                "选择一个 IP 地址来更新 '{}' ({}):",
                record_name.green(),
                record_type.green()
            );

            let display_count = scored.len().min(config.count);
            for (i, entry) in scored.iter().take(display_count).enumerate() {
                println!(
                    "{}. {} (延迟: {}ms, 速度: {:.2}MB/s, 丢包率: {}%)",
                    i + 1,
                    entry.ip.to_string().green(),
                    entry.latency.as_millis(), // Use .as_millis() for Duration
                    entry.speed_bps / 8_000_000.0, // Correctly convert bits per second to MB/s
                    entry.loss_rate * 100.0
                );
            }
            println!("{}. {}", "0".yellow(), "取消更新".yellow());

            let mut selected_index = None;
            let mut retries = 3;

            while selected_index.is_none() && retries > 0 {
                print!(
                    "{}",
                    format!("请输入选择的数字 (0-{}): ", display_count).cyan()
                );
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                let input = input.trim();

                if let Ok(choice) = input.parse::<usize>() {
                    if choice == 0 {
                        println!("{}", "取消 Cloudflare DNS 更新。".yellow());
                        return Ok(());
                    } else if choice > 0 && choice <= display_count {
                        selected_index = Some(choice - 1); // Adjust to 0-based index
                    } else {
                        println!("{}", "无效输入，请选择列表中的数字或 '0' 取消。".red());
                        retries -= 1;
                    }
                } else {
                    println!("{}", "无效输入，请输入数字。".red());
                    retries -= 1;
                }
            }

            optimal_ip_str = if let Some(index) = selected_index {
                scored[index].ip.to_string()
            } else {
                eprintln!("{}", "没有有效选择，跳过 Cloudflare DNS 更新。".red());
                return Ok(());
            };

            println!(
                "{}",
                format!("将使用 IP '{}' 更新 DNS 记录。", optimal_ip_str.green()).cyan()
            );
        }

        let client = Client::new();
        match cloudflare::get_dns_record_id(&client, api_token, zone_id, record_name, record_type)
            .await
        {
            Ok(Some(record_id)) => {
                match cloudflare::update_dns_record(
                    &client,
                    api_token,
                    zone_id,
                    &record_id,
                    record_name,
                    &optimal_ip_str,
                    record_type,
                    proxied,
                    1, // TTL 1 for auto
                )
                .await
                {
                    Ok(_) => println!("{}", "Cloudflare DNS 记录更新成功！".green()),
                    Err(e) => {
                        eprintln!("{} Cloudflare DNS 记录更新失败: {}", "错误".red().bold(), e)
                    }
                }
            }
            Ok(None) => eprintln!(
                "{} 未找到匹配的 Cloudflare DNS 记录 '{}' 类型 '{}'。",
                "错误".red().bold(),
                record_name,
                record_type
            ),
            Err(e) => eprintln!(
                "{} 获取 Cloudflare DNS 记录 ID 失败: {}",
                "错误".red().bold(),
                e
            ),
        }
    } else {
        println!(
            "{}",
            "因 Cloudflare 配置缺失或不完整，已跳过 DNS 更新。".yellow()
        );
    }

    Ok(())
}
