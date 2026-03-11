//! Basic network diagnostics for the Fix/Support mode.
//!
//! Runs quick, non-invasive connectivity checks (DNS resolution, TCP connect)
//! against well-known endpoints. Used to give the user a fast "is my internet
//! working?" answer before diving into more specific troubleshooting.

use anyhow::Result;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Summary of network health checks with per-test detail lines.
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

/// Run DNS + TCP connectivity checks and return a pass/fail report.
pub fn network_diagnostics() -> Result<DiagnosticReport> {
    let details = vec![test_dns(), test_tcp("1.1.1.1:53"), test_tcp("8.8.8.8:53")];

    let failures = details.iter().filter(|d| d.contains("FAIL")).count();
    let summary = if failures == 0 {
        "Network looks healthy".into()
    } else {
        format!("Network issues detected: {} checks failed", failures)
    };

    Ok(DiagnosticReport { summary, details })
}
