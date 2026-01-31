use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "cfip", about = "Cloudflare 优选 IP 工具", version)]
pub struct Config {
    /// 显示结果数量
    #[arg(short = 'n', long = "count", default_value_t = 10)]
    pub count: usize,

    /// 延迟测试并发数
    #[arg(short = 't', long = "threads", default_value_t = 200)]
    pub threads: usize,

    /// 速度测试 IP 数量
    #[arg(short = 's', long = "speed-count", default_value_t = 10)]
    pub speed_count: usize,

    /// 测试端口
    #[arg(short = 'p', long = "port", default_value_t = 443)]
    pub port: u16,

    /// TCP 超时 (毫秒)
    #[arg(long = "timeout", default_value_t = 1000)]
    pub timeout_ms: u64,

    /// 延迟上限 (毫秒)
    #[arg(long = "latency-limit", default_value_t = 300)]
    pub latency_limit_ms: u64,

    /// 每个 IP 测试次数
    #[arg(long = "ping-times", default_value_t = 10)]
    pub ping_times: usize,

    /// 下载测试大小 (字节)
    #[arg(long = "download-size", default_value_t = 10_485_760)]
    pub download_size: usize,

    /// 速度测试 URL
    #[arg(
        long = "test-url",
        default_value = "https://speed.cloudflare.com/__down"
    )]
    pub test_url: String,

    /// 输出 CSV 文件路径
    #[arg(short = 'o', long = "output")]
    pub output: Option<String>,

    /// 包含 IPv6
    #[arg(short = '6', long = "ipv6")]
    pub ipv6: bool,
}
