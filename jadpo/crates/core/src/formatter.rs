use jadpo_syntax::{lex, parse, Declaration, Token, TokenKind};
use std::path::Path;

pub fn format_source(source: &str) -> String {
    let reordered_source = reorder_delivery_items(&reorder_route_items(source));
    let source = reordered_source.as_str();
    let mut output = String::new();
    let mut indent = 0usize;
    let mut blank_pending = false;

    for raw_line in source.lines() {
        let line = trim_for_format(raw_line);
        if line.is_empty() {
            blank_pending = !output.is_empty();
            continue;
        }
        if blank_pending && !output.ends_with("\n\n") {
            output.push('\n');
        }
        blank_pending = false;

        let line = canonicalize_spacing(&line);
        let leading_closes = line
            .chars()
            .take_while(|character| matches!(character, '}' | ')' | ']'))
            .count();
        indent = indent.saturating_sub(leading_closes);
        let signature_continuation =
            usize::from(line.starts_with("fails ") || line.starts_with("-> "));
        output.push_str(&"    ".repeat(indent + signature_continuation));
        output.push_str(&line);
        output.push('\n');

        let (opens, closes) = structural_delimiters(&line);
        indent = indent
            .saturating_add(opens)
            .saturating_sub(closes.saturating_sub(leading_closes));
    }

    output
}

/// Only syntactically complete closed descriptors are reordered. Preserve raw
/// authored tokens/comments and the ordered cursor; never render checked facts
/// or discard invalid input as though it were a valid descriptor.
fn reorder_delivery_items(source: &str) -> String {
    let parsed = parse(Path::new("<formatter>"), source);
    if !parsed.diagnostics.is_empty() {
        return source.to_owned();
    }
    let mut replacements = Vec::new();
    for declaration in &parsed.file.declarations {
        let Declaration::Job(job) = declaration else {
            continue;
        };
        let Some(delivery) = &job.delivery else {
            continue;
        };
        let selection = closed_delivery_block(
            source,
            &parsed.tokens,
            delivery.selection.range,
            &[
                "name",
                "entity",
                "identity",
                "due",
                "before",
                "open",
                "visible",
                "unsent",
                "required_owner",
                "owner_visible",
                "order_by",
                "limit",
                "continuation",
            ],
            &[],
        );
        let hooks = closed_delivery_block(
            source,
            &parsed.tokens,
            delivery.hooks.range,
            &["create", "patch", "supplied"],
            &[],
        );
        let payload = closed_delivery_block(
            source,
            &parsed.tokens,
            delivery.service.payload.range,
            &["idempotency_key", "from", "to", "todo_title", "due_at"],
            &[],
        );
        let service = closed_delivery_block(
            source,
            &parsed.tokens,
            delivery.service.range,
            &[
                "operation",
                "intent",
                "input",
                "output",
                "payload_version",
                "payload",
            ],
            &[(delivery.service.payload.range, payload)],
        );
        let authority = closed_delivery_block(
            source,
            &parsed.tokens,
            delivery.authority.range,
            &["validator", "membership", "role", "permit"],
            &[],
        );
        let completion = closed_delivery_block(
            source,
            &parsed.tokens,
            delivery.completion.range,
            &["name", "field", "time"],
            &[],
        );
        let body = closed_delivery_block(
            source,
            &parsed.tokens,
            delivery.range,
            &["selection", "hooks", "service", "authority", "completion"],
            &[
                (delivery.selection.range, selection),
                (delivery.hooks.range, hooks),
                (delivery.service.range, service),
                (delivery.authority.range, authority),
                (delivery.completion.range, completion),
            ],
        );
        replacements.push((delivery.range, body));
    }
    replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut output = source.to_owned();
    for (range, body) in replacements {
        output.replace_range(range.start..range.end, &body);
    }
    output
}

fn closed_delivery_block(
    source: &str,
    tokens: &[Token],
    range: jadpo_syntax::TextRange,
    keys: &[&str],
    children: &[(jadpo_syntax::TextRange, String)],
) -> String {
    let tokens = tokens
        .iter()
        .filter(|token| token.range.start >= range.start && token.range.end <= range.end)
        .filter(|token| !token.kind.is_trivia())
        .collect::<Vec<_>>();
    let Some(open_index) = tokens
        .iter()
        .position(|token| token.kind == TokenKind::LeftBrace)
    else {
        return source[range.start..range.end].to_owned();
    };
    let open = tokens[open_index];
    let mut depth = 0usize;
    let mut close = None;
    let mut items = Vec::new();
    for (index, token) in tokens.iter().enumerate().skip(open_index + 1) {
        if depth == 0 && token.kind == TokenKind::RightBrace {
            close = Some(*token);
            break;
        }
        if depth == 0
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.kind == TokenKind::Colon)
        {
            if let Some(rank) = keys.iter().position(|key| *key == token.text(source)) {
                items.push((rank, token.range.start));
            }
        }
        match token.kind {
            TokenKind::LeftBrace | TokenKind::LeftParen | TokenKind::LeftBracket => depth += 1,
            TokenKind::RightBrace | TokenKind::RightParen | TokenKind::RightBracket => {
                depth = depth.saturating_sub(1)
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        return source[range.start..range.end].to_owned();
    };
    if items.len() != keys.len() {
        return source[range.start..range.end].to_owned();
    }
    let starts = items
        .iter()
        .map(|(_, marker)| attached_route_item_start(source, *marker, open.range.end))
        .collect::<Vec<_>>();
    let mut ordered = Vec::new();
    for (index, (rank, _)) in items.iter().enumerate() {
        let start = starts[index];
        let end = starts.get(index + 1).copied().unwrap_or(close.range.start);
        let mut text = source[start..end].to_owned();
        let mut nested = children
            .iter()
            .filter(|(range, _)| range.start >= start && range.end <= end)
            .collect::<Vec<_>>();
        nested.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, body) in nested {
            text.replace_range(range.start - start..range.end - start, body);
        }
        ordered.push((*rank, text));
    }
    ordered.sort_by_key(|(rank, _)| *rank);
    let mut body = source[range.start..open.range.end].to_owned();
    body.push('\n');
    // Any nonattached leading trivia remains in the block, not dropped.
    let prefix = source[open.range.end..starts[0]].trim();
    if !prefix.is_empty() {
        body.push_str(prefix);
        body.push('\n');
    }
    for (_, text) in ordered {
        body.push_str(text.trim());
        body.push('\n');
    }
    body.push_str(&source[close.range.start..range.end]);
    body
}

fn reorder_route_items(source: &str) -> String {
    let parsed = parse(Path::new("<formatter>"), source);
    if !parsed.diagnostics.is_empty() {
        return source.to_owned();
    }

    let mut replacements = Vec::new();
    for declaration in &parsed.file.declarations {
        let Declaration::Route(route) = declaration else {
            continue;
        };
        let route_tokens = parsed
            .tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| {
                token.range.start >= route.range.start && token.range.end <= route.range.end
            })
            .collect::<Vec<_>>();
        let Some((open_index, open)) = route_tokens
            .iter()
            .copied()
            .find(|(_, token)| token.kind == TokenKind::LeftBrace)
        else {
            continue;
        };

        let mut nesting = 0usize;
        let mut close = None;
        let mut items = Vec::new();
        for (_, token) in route_tokens
            .iter()
            .copied()
            .filter(|(index, _)| *index > open_index)
        {
            if nesting == 0 && token.kind == TokenKind::RightBrace {
                close = Some(token);
                break;
            }
            if nesting == 0 {
                if let Some(rank) = route_item_rank(token.kind) {
                    items.push((rank, token.range.start));
                }
            }
            match token.kind {
                TokenKind::LeftBrace | TokenKind::LeftParen | TokenKind::LeftBracket => {
                    nesting += 1;
                }
                TokenKind::RightBrace | TokenKind::RightParen | TokenKind::RightBracket => {
                    nesting = nesting.saturating_sub(1);
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            continue;
        };
        if items.is_empty() {
            continue;
        }

        let starts = items
            .iter()
            .map(|(_, marker)| attached_route_item_start(source, *marker, open.range.end))
            .collect::<Vec<_>>();
        let prefix_comments = route_tokens
            .iter()
            .map(|(_, token)| *token)
            .filter(|token| {
                token.kind == TokenKind::LineComment
                    && token.range.start >= open.range.end
                    && token.range.end <= starts[0]
            })
            .collect::<Vec<_>>();

        let mut sorted = Vec::new();
        for (index, (rank, _)) in items.iter().enumerate() {
            let start = starts[index];
            let end = starts.get(index + 1).copied().unwrap_or(close.range.start);
            let first = parsed.tokens.iter().find(|token| {
                token.range.start >= start
                    && token.range.end <= end
                    && !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof)
            });
            let last = parsed.tokens.iter().rev().find(|token| {
                token.range.start >= start
                    && token.range.end <= end
                    && !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof)
            });
            if let (Some(first), Some(last)) = (first, last) {
                sorted.push((*rank, source[first.range.start..last.range.end].to_owned()));
            }
        }
        sorted.sort_by_key(|(rank, _)| *rank);

        let mut body = String::from("\n");
        for (index, comment) in prefix_comments.iter().enumerate() {
            if index > 0 {
                body.push('\n');
            }
            body.push_str(comment.text(source));
        }
        if !prefix_comments.is_empty() {
            body.push('\n');
        }
        for (_, item) in sorted {
            body.push_str(&item);
            body.push('\n');
        }
        replacements.push((open.range.end, close.range.start, body));
    }

    replacements.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    let mut output = source.to_owned();
    for (start, end, replacement) in replacements {
        output.replace_range(start..end, &replacement);
    }
    output
}

fn route_item_rank(kind: TokenKind) -> Option<u8> {
    match kind {
        TokenKind::Auth => Some(0),
        TokenKind::Deadline => Some(1),
        TokenKind::Path => Some(2),
        TokenKind::Query => Some(3),
        TokenKind::Headers => Some(4),
        TokenKind::Input => Some(5),
        TokenKind::Output => Some(6),
        TokenKind::Success => Some(7),
        TokenKind::Run | TokenKind::Action => Some(8),
        _ => None,
    }
}

fn attached_route_item_start(source: &str, marker: usize, body_start: usize) -> usize {
    let mut start = source[..marker]
        .rfind('\n')
        .map_or(0, |newline| newline + 1)
        .max(body_start);
    if !source[start..marker]
        .trim_matches(|character| matches!(character, ' ' | '\t' | '\r'))
        .is_empty()
    {
        return marker;
    }

    loop {
        if start <= body_start {
            break;
        }
        let previous_end = start.saturating_sub(1);
        let previous_start = source[..previous_end]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        let previous_line = source[previous_start..previous_end]
            .trim_matches(|character| matches!(character, ' ' | '\t' | '\r'));
        if previous_line.is_empty() || previous_line.starts_with("//") {
            start = previous_start.max(body_start);
        } else {
            break;
        }
    }
    start
}

fn trim_for_format(raw_line: &str) -> String {
    let line = raw_line.trim_start_matches(|character| matches!(character, ' ' | '\t'));
    let lexed = lex(Path::new("<formatter>"), line);
    if let Some(comment) = lexed
        .tokens
        .iter()
        .find(|token| token.kind == TokenKind::LineComment)
    {
        format!(
            "{}{}",
            line[..comment.range.start]
                .trim_end_matches(|character| matches!(character, ' ' | '\t')),
            &line[comment.range.start..]
        )
    } else {
        line.trim_end_matches(|character| matches!(character, ' ' | '\t'))
            .to_owned()
    }
}

fn canonicalize_spacing(line: &str) -> String {
    if let Some(authority) = line.strip_prefix("egress:") {
        return format!("egress: {}", authority.trim());
    }
    // Route paths are a context-sensitive lexer token. Once a whitespace
    // perturbation puts the path on its own physical line, it lexes as
    // ordinary slash and identifier tokens here, but remains one route-path
    // token in the full source. Preserve its compact spelling.
    if line.starts_with('/') && !line.chars().any(char::is_whitespace) {
        return line.to_owned();
    }
    let lexed = lex(Path::new("<formatter>"), line);
    if !lexed.diagnostics.is_empty() {
        // Keep malformed or incomplete text intact for editor buffers. In
        // particular, an unknown character may not have a lexer token.
        return line.to_owned();
    }
    let tokens = lexed
        .tokens
        .into_iter()
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof))
        .collect::<Vec<_>>();
    let generic_angles = generic_angles(&tokens, line);
    let unary_minus = unary_minus_tokens(&tokens);
    let mut output = String::new();

    for (index, token) in tokens.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1).and_then(|i| tokens.get(i)) {
            if needs_space(
                previous,
                token,
                line,
                generic_angles[index - 1],
                generic_angles[index],
                unary_minus[index - 1],
                unary_minus[index],
            ) {
                output.push(' ');
            }
        }
        output.push_str(token.text(line));
    }

    output
}

fn generic_angles(tokens: &[Token], source: &str) -> Vec<bool> {
    let mut result = vec![false; tokens.len()];
    let mut depth = 0usize;

    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::LeftAngle => {
                let follows_type_name = index
                    .checked_sub(1)
                    .and_then(|previous| tokens.get(previous))
                    .filter(|previous| previous.kind == TokenKind::Identifier)
                    .and_then(|previous| previous.text(source).chars().next())
                    .is_some_and(char::is_uppercase);
                if depth > 0 || follows_type_name {
                    result[index] = true;
                    depth += 1;
                }
            }
            TokenKind::RightAngle if depth > 0 => {
                result[index] = true;
                depth -= 1;
            }
            _ => {}
        }
    }

    result
}

fn unary_minus_tokens(tokens: &[Token]) -> Vec<bool> {
    tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            token.kind == TokenKind::Minus
                && index
                    .checked_sub(1)
                    .and_then(|previous| tokens.get(previous))
                    .map_or(true, |previous| {
                        matches!(
                            previous.kind,
                            TokenKind::LeftBrace
                                | TokenKind::LeftParen
                                | TokenKind::LeftBracket
                                | TokenKind::Comma
                                | TokenKind::Colon
                                | TokenKind::Equal
                                | TokenKind::EqualEqual
                                | TokenKind::BangEqual
                                | TokenKind::LessEqual
                                | TokenKind::GreaterEqual
                                | TokenKind::Plus
                                | TokenKind::Minus
                                | TokenKind::Star
                                | TokenKind::Slash
                                | TokenKind::Percent
                                | TokenKind::Arrow
                                | TokenKind::FatArrow
                                | TokenKind::And
                                | TokenKind::Or
                                | TokenKind::Not
                                | TokenKind::Return
                        )
                    })
        })
        .collect()
}

fn needs_space(
    previous: &Token,
    current: &Token,
    source: &str,
    previous_is_generic_angle: bool,
    current_is_generic_angle: bool,
    previous_is_unary_minus: bool,
    current_is_unary_minus: bool,
) -> bool {
    use TokenKind as K;

    if current.kind == K::LineComment {
        return true;
    }

    if matches!(previous.kind, K::IntegerLiteral | K::DecimalLiteral)
        && current.kind == K::Identifier
        && matches!(current.text(source), "ms" | "s" | "m" | "h" | "d")
    {
        return false;
    }

    if current.kind == K::RightBrace {
        return previous.kind != K::LeftBrace;
    }

    if matches!(
        current.kind,
        K::RightParen | K::RightBracket | K::Comma | K::Dot | K::Question | K::Colon
    ) || matches!(previous.kind, K::LeftParen | K::LeftBracket | K::Dot)
    {
        return false;
    }

    if current.kind == K::LeftBrace && previous.kind == K::RightAngle && previous_is_generic_angle {
        return true;
    }

    if (current.kind == K::RightAngle && current_is_generic_angle)
        || (previous.kind == K::LeftAngle && previous_is_generic_angle)
        || (current.kind == K::LeftAngle && current_is_generic_angle)
        || (previous.kind == K::RightAngle && previous_is_generic_angle)
    {
        return false;
    }

    if current.kind == K::LeftParen {
        return !matches!(previous.kind, K::Identifier | K::RightParen);
    }

    if previous_is_unary_minus {
        return false;
    }

    if current_is_unary_minus
        && matches!(
            previous.kind,
            K::LeftBrace | K::LeftParen | K::LeftBracket | K::Dot
        )
    {
        return false;
    }

    if is_binary_operator(current.kind) || is_binary_operator(previous.kind) {
        return true;
    }

    if current.kind == K::LeftBrace || previous.kind == K::LeftBrace {
        return true;
    }

    true
}

fn is_binary_operator(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Equal
            | TokenKind::EqualEqual
            | TokenKind::BangEqual
            | TokenKind::LessEqual
            | TokenKind::GreaterEqual
            | TokenKind::LeftAngle
            | TokenKind::RightAngle
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Percent
            | TokenKind::Arrow
            | TokenKind::FatArrow
            | TokenKind::And
            | TokenKind::Or
    )
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
    use jadpo_syntax::{lex, parse, TokenKind};
    use std::fs;
    use std::path::Path;

    fn compile_pass_files(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(directory).expect("read compile-pass fixtures") {
            let path = entry.expect("read compile-pass entry").path();
            if path.is_dir() {
                compile_pass_files(&path, files);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "jadpo")
            {
                files.push(path);
            }
        }
    }

    fn formatted_golden(repository: &Path, source: &Path) -> std::path::PathBuf {
        let fixture = source
            .file_name()
            .expect("fixture filename")
            .to_string_lossy();
        repository
            .join("tests/formatter/golden")
            .join(format!("{fixture}.formatted"))
    }

    fn perturb_indentation_and_line_endings(source: &str) -> String {
        let mut perturbed = String::new();
        for line in source.split_inclusive('\n') {
            let has_newline = line.ends_with('\n');
            let content = line.strip_suffix('\n').unwrap_or(line);
            if content.trim().is_empty() {
                if has_newline {
                    perturbed.push_str(" \t\r\n");
                }
                continue;
            }
            perturbed.push_str("\t  \t");
            perturbed.push_str(content.trim_start_matches(|c| matches!(c, ' ' | '\t')));
            if has_newline {
                perturbed.push_str("\r\n");
            }
        }
        perturbed
    }

    fn perturb_horizontal_whitespace(source: &str) -> String {
        let path = Path::new("<formatter-whitespace>");
        let lexed = lex(path, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let mut perturbed = String::new();
        let mut previous_token = None;
        for token in lexed.tokens {
            if token.kind == TokenKind::Whitespace {
                for character in token.text(source).chars() {
                    match character {
                        '\r' => {}
                        '\n' if previous_token == Some(TokenKind::LineComment) => {
                            perturbed.push_str("\r\n\t  ")
                        }
                        '\n' => perturbed.push_str(" \t\r\n\t  "),
                        ' ' | '\t' => perturbed.push_str(" \t\t"),
                        other => perturbed.push(other),
                    }
                }
            } else if token.kind != TokenKind::Eof {
                perturbed.push_str(token.text(source));
                previous_token = Some(token.kind);
            }
        }
        perturbed
    }

    fn add_excess_blank_lines(source: &str) -> String {
        let mut expanded = String::from("\n\n");
        for line in source.split_inclusive('\n') {
            expanded.push_str(line);
            if line
                .trim_matches(|character| matches!(character, ' ' | '\t' | '\r' | '\n'))
                .is_empty()
            {
                expanded.push('\n');
            }
        }
        expanded.push_str("\n\n");
        expanded
    }

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

    #[test]
    fn canonicalizes_token_spacing_without_rewriting_literals_or_comments() {
        let source = "type   Label =   Text{min_length : 1,  max_length: 20}\nfunction  describe(label:Label)->Text { return Text(\"a  b { // c\") } // keep  { spacing }  \n";
        let expected = "type Label = Text { min_length: 1, max_length: 20 }\nfunction describe(label: Label) -> Text { return Text(\"a  b { // c\") } // keep  { spacing }  \n";
        assert_eq!(format_source(source), expected);
        assert_eq!(format_source(expected), expected);
    }

    #[test]
    fn distinguishes_generic_angles_from_comparison_operators_and_duration_units() {
        let source = "function compare(left:Int, right:Int)->Bool { return left<right }\noutput Page { items: Map<Text,List<Todo>> }\n";
        let expected = "function compare(left: Int, right: Int) -> Bool { return left < right }\noutput Page { items: Map<Text, List<Todo>> }\n";
        assert_eq!(format_source(source), expected);

        for unit in ["ms", "s", "m", "h", "d"] {
            let source = format!("var delay = 5   {unit}\n");
            let expected = format!("var delay = 5{unit}\n");
            assert_eq!(format_source(&source), expected, "duration unit {unit}");
        }
    }

    #[test]
    fn normalizes_whitespace_for_every_surface_family_in_the_rule_matrix() {
        let cases = [
            (
                "module and visibility",
                "module demo.api\n\nimport demo.types { User }\npublic type UserId = Uuid {}\n",
            ),
            (
                "locales and every route method",
                "locales {\n    default: \"en-GB\"\n    supported: [\"en-GB\", \"fr-FR\"]\n    unsupported: fallback_to_default\n}\nroute GET /items {\n    auth: none\n    run: list_items()\n}\nroute POST /items {\n    auth: none\n    input: ItemInput\n    output: ItemOutput\n    run: create_item(input)\n}\nroute PUT /items {\n    auth: none\n    input: ItemInput\n    output: ItemOutput\n    run: replace_item(input)\n}\nroute PATCH /items {\n    auth: none\n    input: ItemInput\n    output: ItemOutput\n    run: patch_item(input)\n}\nroute DELETE /items {\n    auth: none\n    run: delete_item()\n}\n",
            ),
            (
                "constrained, object, and enum types",
                "type Label = Text {\n    min_length: 1\n    max_length: 32\n}\ntype Summary = Object {\n    label: Label\n}\nenum State {\n    ready\n    failed { reason: Text }\n}\n",
            ),
            (
                "application, principal, and configuration",
                "application Demo {\n    authentication {\n        principal: Principal\n        revocation {\n            mode: bounded\n            maximum_delay: 5m\n        }\n    }\n}\nprincipal Principal {\n    user { subject: Text user_id: User.id }\n    service { subject: Text service_id: Service.id }\n}\nconfig Settings { api_key: Secret<Text> { binding: \"API_KEY\" secret: true } }\n",
            ),
            (
                "API-key authentication topology",
                "entity Operator { id: Uuid identity subject: Text unique enabled: Bool }\nentity Worker { id: Uuid identity subject: Text unique operator_id: Operator.id enabled: Bool }\nfailure Disabled { kind: Rejected code: \"disabled\" }\nauthentication api_bearer {\n    transport { bearer: authorization_header }\n    validators { service_key { mode: api_key principal: service secret: config.signing_key audience: \"service\" owner: Worker.operator_id } }\n    resolution service {\n        authority: Worker.subject\n        active: enabled == true\n        mappings { subject -> Principal.service.subject id -> Principal.service.service_id }\n        inactive: Disabled\n    }\n}\n",
            ),
            (
                "entity, references, and policy",
                "enum NoteRole {\n    owner\n}\nentity Note {\n    id: Uuid { generated: identity }\n    owner_id: User.id {\n        role: NoteRole.owner\n        immutable: true\n    }\n    identity: id\n    policy {\n        NoteRole.owner: [create, read, update, delete]\n    }\n}\n",
            ),
            (
                "legacy persistence declaration alternatives",
                "type Customer = Object {\n    id: Uuid\n    owner_id: User.id?\n    name: Text\n}\npersist Customer {\n    identity: id\n    unique: name\n    index: owner_id\n    references owner_id: User.id as: owner on_delete: set_null\n}\n",
            ),
            (
                "failures and callables",
                "failure InvalidName {\n    kind: InvalidValue\n    code: \"invalid_name\"\n}\nfunction normalize(value: Text) -> Text {\n    return value\n}\naction register(input: Registration)\n    fails InvalidName\n    -> Accepted\n{\n    return normalize(input.name)\n}\n",
            ),
            (
                "statements and operators",
                "action decide(input: Int) -> Bool {\n    var mut result = input\n    if result >= 0 and not false {\n        result = result + 1 * 2\n    } else {\n        reject InvalidName\n    }\n    return result != 0\n}\n",
            ),
            (
                "persistence expressions",
                "action save(input: SaveInput) -> Todo {\n    var item = attempt create Todo { owner_id: input.owner_id title: input.title }\n    var found = attempt query optional Todo { where: id == input.id }\n    var changed = attempt update required Todo { where: id == item.id set: { title: input.title } missing: TodoMissing conflict: TodoConflict }\n    var deleted = attempt delete required Todo { where: id == item.id missing: TodoMissing conflict: TodoConflict }\n    return item\n}\n",
            ),
            (
                "routes, fixtures, and authored tests",
                "route POST /todos/{owner_id} {\n    auth: none\n    path: { owner_id: User.id }\n    input: CreateTodo\n    output: Todo\n    success: created\n    run: create_todo(input)\n}\nfixture primary_database {}\ntest \"creates a todo\" {\n    assert true\n}\n",
            ),
            (
                "comments and string literals",
                "// retain { comment text }\nfunction label(value: Text) -> Text { return Text(\" two  spaces { // literal text \") } // retain trailing  { text }\n",
            ),
        ];

        for (surface, expected) in cases {
            let parsed = parse(Path::new("formatter-surface.jadpo"), expected);
            assert!(
                parsed.diagnostics.is_empty(),
                "canonical {surface} did not parse: {:?}",
                parsed.diagnostics
            );
            assert_eq!(format_source(expected), expected, "canonical {surface}");
            for (variation, changed) in [
                (
                    "extra horizontal spaces and tabs",
                    perturb_horizontal_whitespace(expected),
                ),
                (
                    "incorrect indentation and CRLF line endings",
                    perturb_indentation_and_line_endings(expected),
                ),
                ("excess blank lines", add_excess_blank_lines(expected)),
            ] {
                let changed_parse = parse(Path::new("formatter-surface.jadpo"), &changed);
                assert!(
                    changed_parse.diagnostics.is_empty(),
                    "{variation} made {surface} unparsable: {:?}",
                    changed_parse.diagnostics
                );
                assert_eq!(
                    format_source(&changed),
                    expected,
                    "{variation} changed exact output for {surface}"
                );
            }
        }
    }

    #[test]
    fn formats_checked_service_contract_and_preserves_its_parsed_body() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let path = repository.join("tests/compile/pass/170_checked_service_operation.jadpo");
        let source = fs::read_to_string(&path).expect("service fixture should be readable");
        let formatted = format_source(&source);

        assert_eq!(format_source(&formatted), formatted);
        let before = parse(&path, &source);
        let after = parse(&path, &formatted);
        assert!(before.diagnostics.is_empty(), "{:#?}", before.diagnostics);
        assert!(after.diagnostics.is_empty(), "{:#?}", after.diagnostics);
        let normalized_body = |syntax: &jadpo_syntax::ParsedSyntax| {
            syntax.file.services[0]
                .items
                .iter()
                .map(|item| (item.path.clone(), item.key.clone(), item.value.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(normalized_body(&before), normalized_body(&after));
    }

    #[test]
    fn formats_grammar_alternatives_missing_from_compile_pass_fixtures() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let source_path =
            repository.join("tests/formatter/cases/uncovered-grammar-alternatives.jadpo");
        let expected_path =
            repository.join("tests/formatter/cases/uncovered-grammar-alternatives.jadpo.formatted");
        let source = fs::read_to_string(&source_path).expect("read formatter variation case");
        let expected =
            fs::read_to_string(&expected_path).expect("read formatter variation expectation");
        let path = Path::new("uncovered-grammar-alternatives.jadpo");

        assert!(
            parse(path, &source).diagnostics.is_empty(),
            "focused grammar alternatives did not parse: {:?}",
            parse(path, &source).diagnostics
        );
        assert_eq!(format_source(&source), expected);
        assert_eq!(format_source(&expected), expected);

        for (variation, changed) in [
            (
                "extra horizontal spaces and tabs",
                perturb_horizontal_whitespace(&source),
            ),
            (
                "incorrect indentation and CRLF line endings",
                perturb_indentation_and_line_endings(&source),
            ),
            ("excess blank lines", add_excess_blank_lines(&source)),
        ] {
            let parsed = parse(path, &changed);
            assert!(
                parsed.diagnostics.is_empty(),
                "{variation} made the focused grammar alternatives unparsable: {:?}",
                parsed.diagnostics
            );
            assert_eq!(
                format_source(&changed),
                expected,
                "{variation} changed exact output for focused grammar alternatives"
            );
        }
    }

    #[test]
    fn formatter_rule_matrix_covers_each_named_grammar_production() {
        use std::collections::HashSet;

        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let grammar = fs::read_to_string(repository.join("docs/grammar-v0.1.md"))
            .expect("read accepted grammar");
        let matrix = fs::read_to_string(repository.join("docs/formatter-rules.md"))
            .expect("read formatter rule matrix");

        let mut productions = HashSet::new();
        let mut in_ebnf = false;
        let mut pending = None;
        for line in grammar.lines() {
            if line.trim() == "```ebnf" {
                in_ebnf = true;
                pending = None;
                continue;
            }
            if in_ebnf && line.trim() == "```" {
                in_ebnf = false;
                pending = None;
                continue;
            }
            if in_ebnf {
                let trimmed = line.trim();
                if let Some((left, _)) = trimmed.split_once('=') {
                    let name = left.trim();
                    if !name.is_empty()
                        && name
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || character == '_')
                    {
                        productions.insert(name.to_owned());
                    }
                    pending = None;
                } else if trimmed.starts_with('=') {
                    if let Some(name) = pending.take() {
                        productions.insert(name);
                    }
                } else if !trimmed.is_empty()
                    && trimmed
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
                {
                    pending = Some(trimmed.to_owned());
                } else {
                    pending = None;
                }
            } else if let Some((name, _)) = line.trim().split_once(" ::=") {
                // The optional module extension uses named productions in a
                // plain-text grammar block rather than an EBNF fence.
                if name.ends_with("-declaration") {
                    productions.insert(name.replace('-', "_"));
                }
            }
        }

        let coverage = matrix
            .split("## Grammar coverage matrix")
            .nth(1)
            .expect("grammar coverage section")
            .split("## Executable checks")
            .next()
            .expect("end of grammar coverage section");
        let mut covered = HashSet::new();
        let mut linked_rows = 0usize;
        for line in coverage.lines().filter(|line| line.starts_with('|')) {
            let cells = line.split('|').collect::<Vec<_>>();
            if cells.len() < 4 {
                continue;
            }
            let row_productions = cells[1]
                .split('`')
                .skip(1)
                .step_by(2)
                .map(|name| name.replace('-', "_"))
                .filter(|name| productions.contains(name))
                .collect::<Vec<_>>();
            if row_productions.is_empty() {
                continue;
            }

            let case_cell = cells[3];
            let mut exact_cases = 0usize;
            for link in case_cell.split("](").skip(1) {
                let target = link
                    .split_once(')')
                    .map(|(target, _)| target)
                    .unwrap_or(link);
                if target.ends_with(".jadpo.formatted") {
                    let path = repository.join("docs").join(target);
                    assert!(
                        path.exists(),
                        "missing exact-output case {}",
                        path.display()
                    );
                    exact_cases += 1;
                }
            }
            assert!(
                exact_cases > 0,
                "grammar production row has no linked exact-output snapshot: {row_productions:?}"
            );
            linked_rows += 1;
            covered.extend(row_productions);
        }

        let missing = productions
            .difference(&covered)
            .cloned()
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "grammar productions missing from the formatter matrix: {missing:?}"
        );
        assert!(
            linked_rows > 0,
            "formatter matrix contains no exact-output rows"
        );
    }

    #[test]
    fn formats_every_compile_pass_fixture_to_its_exact_golden_output() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let corpus = repository.join("tests/compile/pass");
        let mut files = Vec::new();
        compile_pass_files(&corpus, &mut files);
        files.sort();
        assert!(!files.is_empty(), "compile-pass grammar corpus is empty");

        for path in files {
            let source = fs::read_to_string(&path).expect("read formatter source fixture");
            let golden_path = formatted_golden(&repository, &path);
            let expected = fs::read_to_string(&golden_path).unwrap_or_else(|error| {
                panic!(
                    "missing exact formatter expectation {}: {error}",
                    golden_path.display()
                )
            });
            let source_parse = parse(&path, &source);
            let expected_parse = parse(&path, &expected);
            assert!(
                source_parse.diagnostics.is_empty(),
                "source fixture {} did not parse: {:?}",
                path.display(),
                source_parse.diagnostics
            );
            assert!(
                expected_parse.diagnostics.is_empty(),
                "formatted fixture {} did not parse: {:?}",
                golden_path.display(),
                expected_parse.diagnostics
            );

            let tokens = |text: &str| {
                let mut tokens = lex(&path, text)
                    .tokens
                    .into_iter()
                    .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof))
                    .map(|token| (format!("{:?}", token.kind), token.text(text).to_owned()))
                    .collect::<Vec<_>>();
                tokens.sort();
                tokens
            };
            assert_eq!(
                tokens(&source),
                tokens(&expected),
                "golden output changed tokens, comments, or string bytes for {}",
                path.display()
            );
            assert_eq!(
                format_source(&source),
                expected,
                "unexpected exact formatted output for {}",
                path.display()
            );
            assert_eq!(
                format_source(&expected),
                expected,
                "golden output is not idempotent for {}",
                path.display()
            );

            for (variation, changed) in [
                (
                    "extra horizontal spaces and tabs",
                    perturb_horizontal_whitespace(&source),
                ),
                (
                    "incorrect indentation and CRLF line endings",
                    perturb_indentation_and_line_endings(&source),
                ),
                ("excess blank lines", add_excess_blank_lines(&source)),
            ] {
                let changed_parse = parse(&path, &changed);
                assert!(
                    changed_parse.diagnostics.is_empty(),
                    "{variation} made {} unparsable: {:?}",
                    path.display(),
                    changed_parse.diagnostics
                );
                assert_eq!(
                    tokens(&source),
                    tokens(&changed),
                    "{variation} changed comments, strings, or syntax tokens in {}",
                    path.display()
                );
                assert_eq!(
                    format_source(&changed),
                    expected,
                    "{variation} changed exact canonical output for {}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn missing_line_breaks_do_not_change_the_parsed_token_stream() {
        let path = Path::new("line-breaks.jadpo");
        let multiline = "type Label = Text {}\n\nfunction greeting() -> Label {\n    return Label(\"hello\")\n}\n";
        let compact = multiline.replace('\n', " ");
        let formatted = format_source(&compact);
        let compact_parse = parse(path, &compact);
        let formatted_parse = parse(path, &formatted);
        assert!(
            compact_parse.diagnostics.is_empty(),
            "{:?}",
            compact_parse.diagnostics
        );
        assert!(
            formatted_parse.diagnostics.is_empty(),
            "{:?}",
            formatted_parse.diagnostics
        );

        let tokens = |source: &str| {
            lex(path, source)
                .tokens
                .into_iter()
                .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof))
                .map(|token| (token.kind, token.text(source).to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(tokens(&multiline), tokens(&formatted));
        assert_eq!(format_source(&formatted), formatted);
    }

    #[test]
    fn preserves_compact_route_paths_when_whitespace_splits_route_headers() {
        let source = "route\nGET\n/items/{item_id}\n{\nauth: none\npath: { item_id: Uuid }\nrun: get_item(path.item_id)\n}\n";
        let formatted = format_source(source);
        let path = Path::new("route-path.jadpo");
        let tokens = |source: &str| {
            lex(path, source)
                .tokens
                .into_iter()
                .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Eof))
                .map(|token| (token.kind, token.text(source).to_owned()))
                .collect::<Vec<_>>()
        };

        assert_eq!(tokens(source), tokens(&formatted));
        assert!(parse(path, &formatted).diagnostics.is_empty());
        assert_eq!(format_source(&formatted), formatted);
    }

    #[test]
    fn orders_route_items_and_keeps_leading_comments_with_their_item() {
        let source = "route POST /items {\n    run: create_item(input)\n    // output contract\n    output: Item\n    input: CreateItem\n    auth: none\n    success: created\n}\n";
        let expected = "route POST /items {\n    auth: none\n    input: CreateItem\n    // output contract\n    output: Item\n    success: created\n    run: create_item(input)\n}\n";
        let path = Path::new("route-order.jadpo");
        let before = parse(path, source);
        let after = parse(path, expected);
        assert!(before.diagnostics.is_empty(), "{:?}", before.diagnostics);
        assert!(after.diagnostics.is_empty(), "{:?}", after.diagnostics);
        assert_eq!(format_source(source), expected);
        assert_eq!(format_source(expected), expected);

        let route = |parsed: &jadpo_syntax::ParsedSyntax| match &parsed.file.declarations[0] {
            jadpo_syntax::Declaration::Route(route) => route.clone(),
            declaration => panic!("expected route, got {declaration:?}"),
        };
        let original = route(&before);
        let formatted = route(&after);
        assert_eq!(original.method, formatted.method);
        assert_eq!(original.path, formatted.path);
        assert_eq!(original.success, formatted.success);
        assert_eq!(
            original.input.as_ref().unwrap().path[0].text,
            formatted.input.as_ref().unwrap().path[0].text
        );
        assert_eq!(
            original.output.as_ref().unwrap().path[0].text,
            formatted.output.as_ref().unwrap().path[0].text
        );
        assert_eq!(
            original.run.as_ref().unwrap().callee.path[0].text,
            formatted.run.as_ref().unwrap().callee.path[0].text
        );
    }
}
