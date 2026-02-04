use anyhow::{Result, anyhow};
use colored::Colorize;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const CLOUDFLARE_API_BASE_URL: &str = "https://api.cloudflare.com/client/v4";

#[derive(Debug, Deserialize)]
struct CloudflareResponse<T> {
    success: bool,
    errors: Vec<CloudflareError>,
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    code: u16,
    message: String,
}

impl std::fmt::Display for CloudflareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Code: {}, Message: {}", self.code, self.message)
    }
}

#[derive(Debug, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DnsRecord {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub content: String,
    pub proxied: bool,
    pub ttl: u16,
}

#[derive(Debug, Deserialize, Serialize)]
struct UpdateDnsRecordRequest {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    proxied: bool,
    ttl: u16,
}

pub async fn get_zone_id_by_name(
    client: &Client,
    api_token: &str,
    domain_name: &str,
) -> Result<Option<String>> {
    let url = format!("{}/zones?name={}", CLOUDFLARE_API_BASE_URL, domain_name);

    let response = client
        .get(&url)
        .bearer_auth(api_token)
        .send()
        .await?
        .json::<CloudflareResponse<Vec<Zone>>>()
        .await?;

    if !response.success {
        let errors_str: Vec<String> = response.errors.iter().map(|e| e.to_string()).collect();
        return Err(anyhow!(
            "Failed to get Zone ID for '{}': {}",
            domain_name,
            errors_str.join(", ")
        ));
    }

    if let Some(zones) = response.result {
        if let Some(zone) = zones.into_iter().next() {
            println!(
                "Name='{}', Status='{}', ID='{}'\n",
                zone.name.green(),
                zone.status.green(),
                zone.id.green()
            );
            return Ok(Some(zone.id));
        }
    }

    Ok(None)
}

pub async fn get_dns_record_id(
    client: &Client,
    api_token: &str,
    zone_id: &str,
    record_name: &str,
    record_type: &str,
) -> Result<Option<String>> {
    let url = format!(
        "{}/zones/{}/dns_records?type={}&name={}",
        CLOUDFLARE_API_BASE_URL, zone_id, record_type, record_name
    );

    let response = client
        .get(&url)
        .bearer_auth(api_token)
        .send()
        .await?
        .json::<CloudflareResponse<Vec<DnsRecord>>>()
        .await?;

    if !response.success {
        let errors_str: Vec<String> = response.errors.iter().map(|e| e.to_string()).collect();
        return Err(anyhow!(
            "Failed to get DNS record ID: {}",
            errors_str.join(", ")
        ));
    }

    if let Some(records) = response.result {
        if let Some(record) = records.into_iter().next() {
            return Ok(Some(record.id));
        }
    }

    Ok(None)
}

pub async fn update_dns_record(
    client: &Client,
    api_token: &str,
    zone_id: &str,
    record_id: &str,
    record_name: &str,
    new_ip: &str,
    record_type: &str,
    proxied: bool,
    ttl: u16,
) -> Result<()> {
    let url = format!(
        "{}/zones/{}/dns_records/{}",
        CLOUDFLARE_API_BASE_URL, zone_id, record_id
    );

    let request_body = UpdateDnsRecordRequest {
        record_type: record_type.to_string(),
        name: record_name.to_string(),
        content: new_ip.to_string(),
        proxied,
        ttl,
    };

    let response = client
        .put(&url)
        .bearer_auth(api_token)
        .json(&request_body)
        .send()
        .await?
        .json::<CloudflareResponse<DnsRecord>>()
        .await?;

    if !response.success {
        let errors_str: Vec<String> = response.errors.iter().map(|e| e.to_string()).collect();
        return Err(anyhow!(
            "Failed to update DNS record: {}",
            errors_str.join(", ")
        ));
    }

    Ok(())
}
