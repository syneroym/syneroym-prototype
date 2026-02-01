use anyhow::{Result, anyhow};
use tls_parser::{TlsMessage, TlsMessageHandshake, parse_tls_plaintext};
use tracing::{error};

pub fn is_tls_client_hello(buf: &[u8]) -> bool {
    // TLS record starts with:
    // - 0x16 (Handshake)
    // - 0x03 0x00 to 0x03 0x03 (SSL/TLS version)
    buf.len() >= 3 && buf[0] == 0x16 && buf[1] == 0x03
}

pub fn extract_sni(buf: &[u8]) -> Result<String> {
    let (_, tls_record) =
        parse_tls_plaintext(buf).map_err(|e| anyhow!("Failed to parse TLS: {:?}", e))?;

    // Look for ClientHello message
    for msg in tls_record.msg {
        if let TlsMessage::Handshake(TlsMessageHandshake::ClientHello(client_hello)) = msg {
            // Parse extensions from raw bytes
            if let Some(ext_bytes) = client_hello.ext {
                // Use parse_tls_extensions to parse the extension bytes
                match tls_parser::parse_tls_extensions(ext_bytes) {
                    Ok((_, extensions)) => {
                        for ext in extensions {
                            if let tls_parser::TlsExtension::SNI(sni_list) = ext
                                && !sni_list.is_empty()
                            {
                                // SNI entry is (type, hostname_bytes)
                                let hostname = std::str::from_utf8(sni_list[0].1)
                                    .map_err(|e| anyhow!("Invalid SNI hostname: {}", e))?;
                                return Ok(hostname.to_string());
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse TLS extensions: {:?}", e);
                    }
                }
            }
        }
    }

    Err(anyhow!("No SNI found in TLS ClientHello"))
}

pub fn extract_host_from_http(buf: &[u8]) -> Result<String> {
    // Use lossy conversion to handle potential binary body data in the peek buffer
    let http_text = String::from_utf8_lossy(buf);

    // Parse HTTP headers line by line
    for line in http_text.lines() {
        if line.len() > 5 && line[..5].eq_ignore_ascii_case("host:") {
            let host = line[5..].trim();
            // Remove port if present
            let hostname = host.split(':').next().unwrap_or(host);
            return Ok(hostname.to_string());
        }
    }

    Err(anyhow!("No Host header found in HTTP request"))
}

pub fn extract_service_from_host(host: &str) -> Result<String> {
    let hostname = host.split(':').next().unwrap_or(host);
    let parts: Vec<&str> = hostname.split('.').collect();
    if parts.len() > 1 {
        Ok(parts[0].to_string())
    } else {
        Err(anyhow!("service name not found in host: {}", host))
    }
}
