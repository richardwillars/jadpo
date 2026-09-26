pub fn format_source(source: &str) -> String {
    let mut output = String::new();
    let mut indent = 0usize;
    let mut blank_pending = false;

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            blank_pending = !output.is_empty();
            continue;
        }
        if blank_pending && !output.ends_with("\n\n") {
            output.push('\n');
        }
        blank_pending = false;

        let leading_closes = line
            .chars()
            .take_while(|character| matches!(character, '}' | ')' | ']'))
            .count();
        indent = indent.saturating_sub(leading_closes);
        let signature_continuation =
            usize::from(line.starts_with("fails ") || line.starts_with("-> "));
        output.push_str(&"    ".repeat(indent + signature_continuation));
        output.push_str(line);
        output.push('\n');

        let (opens, closes) = structural_delimiters(line);
        indent = indent
            .saturating_add(opens)
            .saturating_sub(closes.saturating_sub(leading_closes));
    }

    output
}

fn structural_delimiters(line: &str) -> (usize, usize) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    let characters = line.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < characters.len() {
        let character = characters[index];
        if !quoted && character == '/' && characters.get(index + 1) == Some(&'/') {
            break;
        }
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
        } else if character == '"' {
            quoted = true;
        } else if matches!(character, '{' | '(' | '[') {
            opens += 1;
        } else if matches!(character, '}' | ')' | ']') {
            closes += 1;
        }
        index += 1;
    }
    (opens, closes)
}

#[cfg(test)]
mod tests {
    use super::format_source;

    #[test]
    fn formats_indentation_blank_lines_comments_and_is_idempotent() {
        let source = "enum State {\nready\nfailed {\nreason: Text\n}\n}\n\n\n// keep { braces } in comments\ntest \"works\" {\nassert true\n}\n";
        let expected = "enum State {\n    ready\n    failed {\n        reason: Text\n    }\n}\n\n// keep { braces } in comments\ntest \"works\" {\n    assert true\n}\n";
        assert_eq!(format_source(source), expected);
        assert_eq!(format_source(expected), expected);
    }

    #[test]
    fn preserves_multiline_callable_signature_structure() {
        let source = "action register_customer(\ninput: RegisterCustomer\n)\nfails InviteCodeRejected\n-> RegistrationAccepted\n{\nreturn input\n}\n";
        let expected = "action register_customer(\n    input: RegisterCustomer\n)\n    fails InviteCodeRejected\n    -> RegistrationAccepted\n{\n    return input\n}\n";
        assert_eq!(format_source(source), expected);
        assert_eq!(format_source(expected), expected);
    }
}
