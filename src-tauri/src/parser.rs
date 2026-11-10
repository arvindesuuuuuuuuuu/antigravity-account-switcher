use base64::prelude::*;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct ParsedIdentity {
    pub name: String,
    pub email: String,
    pub tier: String,
    pub picture: Option<String>,
}

pub fn parse_user_status(raw_val: &str) -> Result<ParsedIdentity, String> {
    let b1 = BASE64_STANDARD
        .decode(raw_val.trim())
        .map_err(|e| format!("Failed to base64 decode userStatus outer: {}", e))?;

    // Search for the inner base64 string
    let b64_regex = Regex::new(r"[A-Za-z0-9+/=]{80,}").unwrap();
    let b1_str = String::from_utf8_lossy(&b1);
    
    let proto_bytes = if let Some(m) = b64_regex.find(&b1_str) {
        let mut inner_b64 = m.as_str().to_string();
        let pad = (4 - inner_b64.len() % 4) % 4;
        inner_b64.push_str(&"=".repeat(pad));
        BASE64_STANDARD
            .decode(&inner_b64)
            .map_err(|e| format!("Failed to decode inner base64 payload: {}", e))?
    } else {
        b1
    };

    // Extract email
    let email_re = Regex::new(r"[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+").unwrap();
    let email = if let Some(m) = email_re.find(&String::from_utf8_lossy(&proto_bytes)) {
        m.as_str().to_string()
    } else {
        "unknown@google.com".to_string()
    };

    // Extract name
    let mut name = "Antigravity User".to_string();
    if let Some(email_idx) = proto_bytes.windows(email.len()).position(|w| w == email.as_bytes()) {
        let start = email_idx.saturating_sub(60);
        let prefix = &proto_bytes[start..email_idx];
        let name_re = Regex::new(r"[A-Z][a-zA-Z0-9 ]{2,30}").unwrap();
        let prefix_str = String::from_utf8_lossy(prefix);
        if let Some(m) = name_re.find_iter(&prefix_str).last() {
            name = m.as_str().trim().to_string();
        }
    }

    // Extract Tier
    let proto_str = String::from_utf8_lossy(&proto_bytes);
    let tier = if proto_str.contains("g1-ultra-tier") {
        "Google AI Ultra".to_string()
    } else if proto_str.contains("g1-pro-tier") {
        "Google AI Pro".to_string()
    } else {
        "Free Tier".to_string()
    };

    // Extract Picture
    let pic_re = Regex::new(r"https://lh3\.googleusercontent\.com/[^\s\x00-\x1f\x7f-\xff]+").unwrap();
    let picture = pic_re.find(&proto_str).map(|m| m.as_str().to_string());

    Ok(ParsedIdentity {
        name,
        email,
        tier,
        picture,
    })
}
