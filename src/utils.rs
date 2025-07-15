use once_cell::sync::Lazy;
use regex::Regex;

static SLUG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[^a-zA-Z0-9]+").expect("Failed to compile regex for slugify"));

pub fn slugify(s: &str) -> String {
    SLUG_REGEX.replace_all(s, "-").to_string().to_lowercase()
}

pub fn room_alias_like(alias: &str) -> bool {
    let parts: Vec<&str> = alias.split(':').collect();
    parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() && !alias.starts_with('!')
}

pub fn is_valid_room_id(room_id: &str) -> bool {
    if !room_id.starts_with('!') {
        return false;
    }

    let colon_pos = match room_id.rfind(':') {
        Some(pos) => pos,
        None => return false,
    };

    let localpart = &room_id[1..colon_pos];
    let domain = &room_id[colon_pos + 1..];

    if !is_valid_localpart(localpart) {
        return false;
    }

    is_valid_domain(domain)
}

fn is_valid_localpart(localpart: &str) -> bool {
    if localpart.is_empty() || localpart.len() > 255 {
        return false;
    }

    localpart
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '=' || c == '-')
}

fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() {
        return false;
    }

    if let Some(colon_pos) = domain.rfind(':') {
        let hostname = &domain[..colon_pos];
        let port_str = &domain[colon_pos + 1..];

        if let Ok(port) = port_str.parse::<u16>() {
            if port == 0 {
                return false;
            }
        } else {
            return false;
        }

        is_valid_hostname(hostname)
    } else {
        is_valid_hostname(domain)
    }
}

fn is_valid_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }

    if hostname.starts_with('.')
        || hostname.ends_with('.')
        || hostname.starts_with('-')
        || hostname.ends_with('-')
    {
        return false;
    }

    let labels: Vec<&str> = hostname.split('.').collect();

    for label in labels {
        if label.is_empty() || label.len() > 63 {
            return false;
        }

        if !label.chars().next().unwrap().is_ascii_alphanumeric()
            || !label.chars().last().unwrap().is_ascii_alphanumeric()
        {
            return false;
        }

        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }

    true
}
