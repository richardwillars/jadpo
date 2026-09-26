use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => formatter.write_str("error"),
            Self::Warning => formatter.write_str("warning"),
            Self::Note => formatter.write_str("note"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    pub source: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub primary: Option<SourceSpan>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            primary: None,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            primary: None,
            notes: Vec::new(),
        }
    }

    pub fn to_json(&self) -> String {
        let (source, range) = self.primary.as_ref().map_or_else(
            || ("null".to_owned(), "null".to_owned()),
            |primary| {
                (
                    json_string(&primary.source),
                    format!("{{\"start\":{},\"end\":{}}}", primary.start, primary.end),
                )
            },
        );
        let notes = self
            .notes
            .iter()
            .map(|note| json_string(note))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"severity\":{},\"code\":{},\"message\":{},\"source\":{source},\"range\":{range},\"notes\":[{notes}]}}",
            json_string(&self.severity.to_string()),
            json_string(self.code),
            json_string(&self.message),
        )
    }
}

pub fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character <= '\u{001f}' => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "{}[{}]: {}",
            self.severity, self.code, self.message
        )?;

        if let Some(primary) = &self.primary {
            writeln!(
                formatter,
                "  at {}:{}..{}",
                primary.source, primary.start, primary.end
            )?;
        }

        for note in &self.notes {
            writeln!(formatter, "  note: {note}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, SourceSpan};

    #[test]
    fn emits_stable_json_with_explicit_nullable_location_fields() {
        let mut diagnostic =
            Diagnostic::error("TEST_CODE", "bad \"value\"\nnext").with_note("fix\\path");
        diagnostic.primary = Some(SourceSpan {
            source: "example.jadpo".to_owned(),
            start: 4,
            end: 9,
        });

        assert_eq!(
            diagnostic.to_json(),
            "{\"severity\":\"error\",\"code\":\"TEST_CODE\",\"message\":\"bad \\\"value\\\"\\nnext\",\"source\":\"example.jadpo\",\"range\":{\"start\":4,\"end\":9},\"notes\":[\"fix\\\\path\"]}"
        );
        assert_eq!(
            Diagnostic::warning("TEST_WARNING", "careful").to_json(),
            "{\"severity\":\"warning\",\"code\":\"TEST_WARNING\",\"message\":\"careful\",\"source\":null,\"range\":null,\"notes\":[]}"
        );
    }
}
