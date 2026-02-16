use serde_json::Value;

pub(crate) fn parse_u64(value: &Value) -> Option<u64> {
    match value {
        Value::String(s) => s.parse::<u64>().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

pub(crate) fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

pub(crate) fn get_nested_string(value: &Value, keys: &[&str]) -> String {
    let mut current = value;
    for key in keys {
        let Some(next) = current.get(*key) else {
            return String::new();
        };
        current = next;
    }
    value_to_string(current)
}

pub(crate) fn shorten_addr(value: &str) -> String {
    if value.len() > 12 {
        format!("{}...{}", &value[..6], &value[value.len() - 4..])
    } else {
        value.to_owned()
    }
}

pub(crate) fn with_optional_ledger_version(path: &str, ledger_version: Option<u64>) -> String {
    match ledger_version {
        Some(version) => {
            let separator = if path.contains('?') { '&' } else { '?' };
            format!("{path}{separator}ledger_version={version}")
        }
        None => path.to_owned(),
    }
}
