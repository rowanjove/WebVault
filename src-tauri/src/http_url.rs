use anyhow::Result;
use url::Url;

/// Accept only http(s) URLs with a host. Rejects Chromium switch injection (`--flag`).
pub fn require_http_url(raw: &str) -> Result<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        anyhow::bail!("URL is empty");
    }
    if trimmed.starts_with("--") {
        anyhow::bail!("URL must be http or https");
    }
    let parsed = Url::parse(trimmed).map_err(|_| anyhow::anyhow!("Invalid URL"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        anyhow::bail!("URL must be http or https");
    }
    if parsed.host_str().map(|h| h.is_empty()).unwrap_or(true) {
        anyhow::bail!("URL is missing a host");
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_http_url() {
        assert!(require_http_url("https://example.com/login").is_ok());
        assert!(require_http_url("http://127.0.0.1:8080/").is_ok());
        assert!(require_http_url("--gpu-launcher=cmd.exe").is_err());
        assert!(require_http_url("file:///C:/secret.txt").is_err());
        assert!(require_http_url("javascript:alert(1)").is_err());
        assert!(require_http_url("").is_err());
    }
}
