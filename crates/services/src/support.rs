use anyhow::Result;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct DiagnosticReport {
    pub summary: String,
    pub details: Vec<String>,
}

fn test_dns() -> String {
    match ("example.com", 80).to_socket_addrs() {
        Ok(iter) => {
            let ips: Vec<String> = iter.map(|a| a.ip().to_string()).collect();
            format!("DNS OK: example.com -> {:?}", ips)
        }
        Err(e) => format!("DNS FAIL: {}", e),
    }
}

fn test_tcp(addr: &str) -> String {
    let timeout = Duration::from_millis(800);
    match TcpStream::connect_timeout(
        &addr
            .parse()
            .unwrap_or_else(|_| "1.1.1.1:53".parse().unwrap()),
        timeout,
    ) {
        Ok(_) => format!("TCP OK: {}", addr),
        Err(e) => format!("TCP FAIL: {} ({})", addr, e),
    }
}

pub fn network_diagnostics() -> Result<DiagnosticReport> {
    let mut details = Vec::new();
    details.push(test_dns());
    details.push(test_tcp("1.1.1.1:53"));
    details.push(test_tcp("8.8.8.8:53"));

    let failures = details.iter().filter(|d| d.contains("FAIL")).count();
    let summary = if failures == 0 {
        "Network looks healthy".into()
    } else {
        format!("Network issues detected: {} checks failed", failures)
    };

    Ok(DiagnosticReport { summary, details })
}
