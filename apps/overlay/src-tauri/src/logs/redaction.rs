fn replace_token_after_prefix(input: &str, prefix: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(prefix) {
        let (before, after_before) = remaining.split_at(index);
        output.push_str(before);
        output.push_str(prefix);

        let value_start = prefix.len();
        let after_prefix = &after_before[value_start..];
        let value_len = after_prefix
            .find(char::is_whitespace)
            .unwrap_or(after_prefix.len());
        output.push_str("<redacted>");
        remaining = &after_prefix[value_len..];
    }

    output.push_str(remaining);
    output
}

fn replace_quoted_value(input: &str, key: &str) -> String {
    let pattern = format!("{key}=\"");
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(&pattern) {
        let (before, after_before) = remaining.split_at(index);
        output.push_str(before);
        output.push_str(&pattern);

        let after_pattern = &after_before[pattern.len()..];
        if let Some(end_quote) = after_pattern.find('"') {
            output.push_str("<redacted>\"");
            remaining = &after_pattern[end_quote + 1..];
        } else {
            output.push_str("<redacted>");
            remaining = "";
        }
    }

    output.push_str(remaining);
    output
}

fn replace_parenthesized_value(input: &str, key: &str) -> String {
    let pattern = format!("{key}=(");
    let mut output = String::with_capacity(input.len());
    let mut remaining = input;

    while let Some(index) = remaining.find(&pattern) {
        let (before, after_before) = remaining.split_at(index);
        output.push_str(before);
        output.push_str(&pattern);

        let after_pattern = &after_before[pattern.len()..];
        if let Some(end_paren) = after_pattern.find(')') {
            output.push_str("<redacted>)");
            remaining = &after_pattern[end_paren + 1..];
        } else {
            output.push_str("<redacted>");
            remaining = "";
        }
    }

    output.push_str(remaining);
    output
}

pub fn redact_sensitive(input: &str) -> String {
    let mut redacted = input.to_owned();
    for prefix in [
        "AUTH_PASSWORD=",
        "-AUTH_PASSWORD=",
        "epicuserid=",
        "-epicuserid=",
        "epicusername=",
        "-epicusername=",
    ] {
        redacted = replace_token_after_prefix(&redacted, prefix);
    }

    for key in ["DSRToken", "JoinPassword", "JoinCredentials"] {
        redacted = replace_quoted_value(&redacted, key);
    }

    replace_parenthesized_value(&redacted, "ConnectionID")
}

#[cfg(test)]
mod tests {
    use super::redact_sensitive;

    #[test]
    fn redacts_epic_launch_and_join_secrets() {
        let line = "Command line: -AUTH_PASSWORD=5139003f31b04a6ba73e914a8860125a -epicuserid=7efc351e447043c4be4447da51b790e4 JoinPassword=\"UM1H9ZNCNIYWKZSC\" JoinCredentials=\"10PJB3O2SM6ZR8JH:UM1H9ZNCNIYWKZSC\"";

        let redacted = redact_sensitive(line);

        assert!(!redacted.contains("5139003f31b04a6ba73e914a8860125a"));
        assert!(!redacted.contains("7efc351e447043c4be4447da51b790e4"));
        assert!(!redacted.contains("UM1H9ZNCNIYWKZSC"));
        assert!(redacted.contains("AUTH_PASSWORD=<redacted>"));
        assert!(redacted.contains("epicuserid=<redacted>"));
        assert!(redacted.contains("JoinPassword=\"<redacted>\""));
        assert!(redacted.contains("JoinCredentials=\"<redacted>\""));
    }

    #[test]
    fn redacts_jwt_like_tokens() {
        let line = "DSRToken=\"aaa.bbb.ccc\" ConnectionID=(aaa.bbb.ccc)";

        let redacted = redact_sensitive(line);

        assert_eq!(
            redacted,
            "DSRToken=\"<redacted>\" ConnectionID=(<redacted>)"
        );
    }
}
