use crate::AnalyzedProject;
use jadpo_diagnostics::{Diagnostic, DiagnosticFact};
use jadpo_syntax::{
    ConfigDefault, ConfigDefaultKind, ConfigFieldDeclaration, Constraint, ConstraintKind,
    Declaration, TypeDeclaration, TypeReference,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigurationField {
    pub name: String,
    pub binding: String,
    pub type_name: String,
    pub secret: bool,
    pub default: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalConfigurationState {
    Set,
    Default,
    Missing,
    Invalid,
}

impl LocalConfigurationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Set => "set",
            Self::Default => "default",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
        }
    }

    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Set | Self::Default)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalConfigurationStatus {
    pub field: String,
    pub binding: String,
    pub state: LocalConfigurationState,
}

pub fn configuration_fields(project: &AnalyzedProject) -> Vec<ConfigurationField> {
    project
        .syntax
        .sources
        .iter()
        .flat_map(|source| source.file.declarations.iter())
        .filter_map(|declaration| {
            let Declaration::Config(configuration) = declaration else {
                return None;
            };
            Some(configuration.fields.iter())
        })
        .flatten()
        .filter_map(|field| {
            Some(ConfigurationField {
                name: field.name.text.clone(),
                binding: unquote(&field.binding.as_ref()?.text),
                type_name: type_name(&field.field_type),
                secret: field.secret,
                default: field.default.as_ref().map(default_text),
            })
        })
        .collect()
}

pub fn check_local_configuration(
    project_path: &Path,
    project: &AnalyzedProject,
) -> Result<Vec<LocalConfigurationStatus>, Diagnostic> {
    let values = read_local_values(project_path)?;
    let declarations = configuration_declarations(project);
    let mut statuses = Vec::new();
    for (field, public) in declarations {
        let state = match values.get(&public.binding) {
            Some(value) if validate_value(project, field, value) => LocalConfigurationState::Set,
            Some(_) => LocalConfigurationState::Invalid,
            None if public.default.is_some() => LocalConfigurationState::Default,
            None => LocalConfigurationState::Missing,
        };
        statuses.push(LocalConfigurationStatus {
            field: public.name,
            binding: public.binding,
            state,
        });
    }
    Ok(statuses)
}

pub fn local_configuration_environment(
    project_path: &Path,
    project: &AnalyzedProject,
) -> Result<BTreeMap<String, String>, Diagnostic> {
    let values = read_local_values(project_path)?;
    let mut environment = BTreeMap::new();
    for (field, public) in configuration_declarations(project) {
        match values.get(&public.binding) {
            Some(value) if validate_value(project, field, value) => {
                environment.insert(public.binding, value.clone());
            }
            Some(_) => {
                return Err(Diagnostic::error("CONFIG_VALUE_INVALID")
                    .with_fact(DiagnosticFact::Field(public.name)));
            }
            None if public.default.is_some() => {}
            None => {
                return Err(Diagnostic::error("CONFIG_VALUE_MISSING")
                    .with_fact(DiagnosticFact::Field(public.name)));
            }
        }
    }
    Ok(environment)
}

pub fn set_local_configuration(
    project_path: &Path,
    project: &AnalyzedProject,
    field_name: &str,
    value: &str,
) -> Result<ConfigurationField, Diagnostic> {
    let declarations = configuration_declarations(project);
    let Some((field, public)) = declarations
        .into_iter()
        .find(|(_, field)| field.name == field_name)
    else {
        return Err(Diagnostic::error("CONFIG_FIELD_UNKNOWN")
            .with_fact(DiagnosticFact::Field(field_name.to_owned())));
    };
    if value.contains(['\n', '\r', '\0']) || !validate_value(project, field, value) {
        return Err(Diagnostic::error("CONFIG_VALUE_INVALID")
            .with_fact(DiagnosticFact::Field(public.name.clone())));
    }

    let path = project_root(project_path).join(".env.local");
    reject_unsafe_target(&path)?;
    let original = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return Err(Diagnostic::error("CONFIG_LOCAL_READ_FAILED")),
    };
    let original_text = String::from_utf8(original.clone())
        .map_err(|_| Diagnostic::error("CONFIG_LOCAL_INVALID_ENCODING"))?;
    let updated = update_binding(&original_text, &public.binding, value)?;

    let current = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return Err(Diagnostic::error("CONFIG_LOCAL_READ_FAILED")),
    };
    ensure_unchanged(&original, &current)?;
    atomic_write(&path, updated.as_bytes(), &original)?;
    Ok(public)
}

fn ensure_unchanged(original: &[u8], current: &[u8]) -> Result<(), Diagnostic> {
    if current != original {
        return Err(Diagnostic::error("CONFIG_LOCAL_CHANGED"));
    }
    Ok(())
}

fn configuration_declarations(
    project: &AnalyzedProject,
) -> Vec<(&ConfigFieldDeclaration, ConfigurationField)> {
    project
        .syntax
        .sources
        .iter()
        .flat_map(|source| source.file.declarations.iter())
        .filter_map(|declaration| {
            let Declaration::Config(configuration) = declaration else {
                return None;
            };
            Some(configuration.fields.iter())
        })
        .flatten()
        .filter_map(|field| {
            Some((
                field,
                ConfigurationField {
                    name: field.name.text.clone(),
                    binding: unquote(&field.binding.as_ref()?.text),
                    type_name: type_name(&field.field_type),
                    secret: field.secret,
                    default: field.default.as_ref().map(default_text),
                },
            ))
        })
        .collect()
}

fn read_local_values(project_path: &Path) -> Result<BTreeMap<String, String>, Diagnostic> {
    let path = project_root(project_path).join(".env.local");
    reject_unsafe_target(&path)?;
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(_) => return Err(Diagnostic::error("CONFIG_LOCAL_READ_FAILED")),
    };
    parse_local_values(&source)
}

fn parse_local_values(source: &str) -> Result<BTreeMap<String, String>, Diagnostic> {
    let mut values = BTreeMap::new();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((raw_name, raw_value)) = line.split_once('=') else {
            return Err(Diagnostic::error("CONFIG_LOCAL_SYNTAX"));
        };
        let name = raw_name.trim();
        if !valid_binding_name(name) || values.contains_key(name) {
            return Err(Diagnostic::error("CONFIG_LOCAL_SYNTAX"));
        }
        values.insert(name.to_owned(), parse_env_value(raw_value.trim())?);
    }
    Ok(values)
}

fn parse_env_value(value: &str) -> Result<String, Diagnostic> {
    if !value.starts_with('"') {
        return Ok(value.to_owned());
    }
    if !value.ends_with('"') || value.len() < 2 {
        return Err(Diagnostic::error("CONFIG_LOCAL_SYNTAX"));
    }
    let mut decoded = String::new();
    let mut characters = value[1..value.len() - 1].chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some('t') => decoded.push('\t'),
            Some('"') => decoded.push('"'),
            Some('\\') => decoded.push('\\'),
            _ => return Err(Diagnostic::error("CONFIG_LOCAL_SYNTAX")),
        }
    }
    Ok(decoded)
}

fn update_binding(source: &str, binding: &str, value: &str) -> Result<String, Diagnostic> {
    let encoded = encode_env_value(value);
    let mut output = String::new();
    let mut replaced = false;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let matches = trimmed
            .split_once('=')
            .is_some_and(|(name, _)| name.trim() == binding);
        if matches {
            if replaced {
                return Err(Diagnostic::error("CONFIG_LOCAL_SYNTAX"));
            }
            output.push_str(binding);
            output.push('=');
            output.push_str(&encoded);
            output.push('\n');
            replaced = true;
        } else {
            output.push_str(line);
        }
    }
    if !replaced {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(binding);
        output.push('=');
        output.push_str(&encoded);
        output.push('\n');
    }
    Ok(output)
}

fn encode_env_value(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

fn atomic_write(path: &Path, contents: &[u8], expected: &[u8]) -> Result<(), Diagnostic> {
    atomic_write_before_promotion(path, contents, expected, || {})
}

fn atomic_write_before_promotion(
    path: &Path,
    contents: &[u8],
    expected: &[u8],
    before_promotion: impl FnOnce(),
) -> Result<(), Diagnostic> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|_| Diagnostic::error("CONFIG_LOCAL_WRITE_FAILED"))?;
    let temporary = parent.join(format!(".env.local.jadpo.{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| Diagnostic::error("CONFIG_LOCAL_WRITE_FAILED"))?;
    if file.write_all(contents).is_err() || file.sync_all().is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(Diagnostic::error("CONFIG_LOCAL_WRITE_FAILED"));
    }
    drop(file);
    before_promotion();
    // Staging and fsync can take time. Reject edits observed after that work,
    // while preserving the external writer's file. This is not an atomic CAS:
    // an uncooperative writer can still race the final check and rename.
    let checked = (|| {
        reject_unsafe_target(path)?;
        let current = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(_) => return Err(Diagnostic::error("CONFIG_LOCAL_READ_FAILED")),
        };
        ensure_unchanged(expected, &current)
    })();
    if let Err(diagnostic) = checked {
        let _ = fs::remove_file(&temporary);
        return Err(diagnostic);
    }
    if fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(Diagnostic::error("CONFIG_LOCAL_WRITE_FAILED"));
    }
    Ok(())
}

fn reject_unsafe_target(path: &Path) -> Result<(), Diagnostic> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(Diagnostic::error("CONFIG_LOCAL_UNSAFE_TARGET"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Diagnostic::error("CONFIG_LOCAL_READ_FAILED")),
    }
}

fn validate_value(project: &AnalyzedProject, field: &ConfigFieldDeclaration, value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    let types = type_declarations(project);
    let mut current = type_name(&field.field_type);
    let mut constraints = Vec::new();
    let mut visited = BTreeSet::new();
    while visited.insert(current.clone()) {
        let Some(declaration) = types.get(&current) else {
            break;
        };
        constraints.extend(declaration.constraints.iter());
        current = type_name(&declaration.parent);
    }
    let representation_valid = match current.as_str() {
        "Text" => true,
        "Bool" => matches!(value, "true" | "false"),
        "Int" => value.parse::<i64>().is_ok(),
        "Decimal" => value.parse::<f64>().is_ok_and(f64::is_finite),
        "Uuid" => valid_uuid(value),
        "Email" => valid_email(value),
        "Url" => valid_url(value),
        "IpAddress" => value.parse::<IpAddr>().is_ok(),
        "Duration" => valid_duration(value),
        "Instant" => valid_instant(value),
        "CalendarDate" => valid_date(value),
        _ => false,
    };
    representation_valid
        && constraints
            .iter()
            .all(|constraint| valid_constraint(value, constraint))
}

fn valid_constraint(value: &str, constraint: &Constraint) -> bool {
    match constraint.kind {
        ConstraintKind::MinLength => constraint
            .value
            .text
            .parse::<usize>()
            .is_ok_and(|minimum| value.chars().count() >= minimum),
        ConstraintKind::MaxLength => constraint
            .value
            .text
            .parse::<usize>()
            .is_ok_and(|maximum| value.chars().count() <= maximum),
        ConstraintKind::Min => value
            .parse::<f64>()
            .ok()
            .zip(constraint.value.text.parse::<f64>().ok())
            .is_some_and(|(value, minimum)| value >= minimum),
        ConstraintKind::Max => value
            .parse::<f64>()
            .ok()
            .zip(constraint.value.text.parse::<f64>().ok())
            .is_some_and(|(value, maximum)| value <= maximum),
        ConstraintKind::Pattern => {
            unquote(&constraint.value.text) == "[a-z0-9_]+"
                && !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        }
        ConstraintKind::Format => match unquote(&constraint.value.text).as_str() {
            "email" => valid_email(value),
            _ => false,
        },
    }
}

fn type_declarations(project: &AnalyzedProject) -> BTreeMap<String, &TypeDeclaration> {
    project
        .syntax
        .sources
        .iter()
        .flat_map(|source| source.file.declarations.iter())
        .filter_map(|declaration| {
            let Declaration::Type(declaration) = declaration else {
                return None;
            };
            Some((declaration.name.text.clone(), declaration))
        })
        .collect()
}

fn type_name(reference: &TypeReference) -> String {
    reference
        .path
        .iter()
        .map(|name| name.text.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn default_text(default: &ConfigDefault) -> String {
    match default.kind {
        ConfigDefaultKind::String => unquote(&default.text),
        ConfigDefaultKind::Integer
        | ConfigDefaultKind::Decimal
        | ConfigDefaultKind::Boolean
        | ConfigDefaultKind::Duration => default.text.clone(),
    }
}

fn unquote(value: &str) -> String {
    let value = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value);
    if value.contains('\\') {
        parse_env_value(&format!("\"{value}\"")).unwrap_or_else(|_| value.to_owned())
    } else {
        value.to_owned()
    }
}

fn valid_binding_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
        && matches!(value.as_bytes()[14], b'1'..=b'5')
        && matches!(
            value.as_bytes()[19].to_ascii_lowercase(),
            b'8' | b'9' | b'a' | b'b'
        )
}

fn valid_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !value.chars().any(char::is_whitespace)
}

fn valid_url(value: &str) -> bool {
    let remainder = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"));
    remainder.is_some_and(|remainder| {
        let authority = remainder.split(['/', '?', '#']).next().unwrap_or("");
        !authority.is_empty()
            && authority != "."
            && !authority.starts_with('.')
            && !authority.ends_with('.')
            && !value.chars().any(char::is_whitespace)
    })
}

fn valid_duration(value: &str) -> bool {
    let canonical = value.strip_prefix('-').unwrap_or(value);
    if let Some(mut rest) = canonical.strip_prefix("PT") {
        if rest.is_empty() {
            return false;
        }
        let mut found = false;
        let mut milliseconds = 0.0;
        for (suffix, multiplier) in [('H', 3_600_000.0), ('M', 60_000.0), ('S', 1_000.0)] {
            let Some(position) = rest.find(suffix) else {
                continue;
            };
            let number = &rest[..position];
            let valid = if suffix == 'S' {
                let mut pieces = number.split('.');
                let whole = pieces.next().unwrap_or_default();
                let fraction = pieces.next();
                !whole.is_empty()
                    && whole.bytes().all(|byte| byte.is_ascii_digit())
                    && fraction.map_or(true, |fraction| {
                        !fraction.is_empty()
                            && fraction.len() <= 3
                            && fraction.bytes().all(|byte| byte.is_ascii_digit())
                    })
                    && pieces.next().is_none()
            } else {
                !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
            };
            if !valid {
                return false;
            }
            let Ok(component) = number.parse::<f64>() else {
                return false;
            };
            milliseconds += component * multiplier;
            found = true;
            rest = &rest[position + 1..];
        }
        return found
            && rest.is_empty()
            && milliseconds.is_finite()
            && milliseconds.fract() == 0.0
            && milliseconds <= 9_007_199_254_740_991.0;
    }
    for (suffix, multiplier) in [
        ("ms", 1.0),
        ("s", 1_000.0),
        ("m", 60_000.0),
        ("h", 3_600_000.0),
        ("d", 86_400_000.0),
    ] {
        if let Some(number) = value.strip_suffix(suffix) {
            return !number.is_empty()
                && number.parse::<f64>().is_ok_and(|number| {
                    let milliseconds = number * multiplier;
                    milliseconds.is_finite()
                        && milliseconds.fract() == 0.0
                        && milliseconds.abs() <= 9_007_199_254_740_991.0
                });
        }
    }
    false
}

fn valid_date(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if !(parts.len() == 3
        && parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit())))
    {
        return false;
    }
    let Ok(year) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(month) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(day) = parts[2].parse::<u32>() else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

fn valid_instant_clock(value: &str) -> bool {
    let (clock, fraction) = value
        .split_once('.')
        .map_or((value, None), |(clock, fraction)| (clock, Some(fraction)));
    if fraction.is_some_and(|fraction| {
        fraction.is_empty()
            || fraction.len() > 3
            || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return false;
    }
    let parts = clock.split(':').collect::<Vec<_>>();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.len() != 2 || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let Ok(hour) = parts[0].parse::<u8>() else {
        return false;
    };
    let Ok(minute) = parts[1].parse::<u8>() else {
        return false;
    };
    let Ok(second) = parts[2].parse::<u8>() else {
        return false;
    };
    hour <= 23 && minute <= 59 && second <= 59
}

fn valid_instant(value: &str) -> bool {
    let Some((date, time_and_zone)) = value.split_once('T') else {
        return false;
    };
    if !valid_date(date) {
        return false;
    }
    let (clock, offset) = if let Some(clock) = time_and_zone.strip_suffix('Z') {
        (clock, "Z")
    } else if time_and_zone.len() >= 6 {
        time_and_zone.split_at(time_and_zone.len() - 6)
    } else {
        return false;
    };
    if !valid_instant_clock(clock) {
        return false;
    }
    if offset == "Z" {
        return true;
    }
    let bytes = offset.as_bytes();
    bytes.len() == 6
        && matches!(bytes[0], b'+' | b'-')
        && bytes[3] == b':'
        && bytes[1..3].iter().all(u8::is_ascii_digit)
        && bytes[4..6].iter().all(u8::is_ascii_digit)
        && offset[1..3].parse::<u8>().is_ok_and(|hour| hour <= 23)
        && offset[4..6].parse::<u8>().is_ok_and(|minute| minute <= 59)
}

fn project_root(project_path: &Path) -> PathBuf {
    if project_path.is_dir() {
        project_path.to_owned()
    } else {
        project_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_local_values, update_binding, valid_date, valid_duration, valid_instant, valid_url,
        valid_uuid,
    };

    #[test]
    fn updates_one_binding_without_exposing_or_reordering_others() {
        let source = "# local\nFIRST=\"one\"\nSECOND=\"two\"\n";
        let updated = update_binding(source, "SECOND", "new value").expect("update");
        assert_eq!(updated, "# local\nFIRST=\"one\"\nSECOND=\"new value\"\n");
        let values = parse_local_values(&updated).expect("parse");
        assert_eq!(values.get("SECOND").map(String::as_str), Some("new value"));
    }

    #[test]
    fn validates_textual_configuration_representations_strictly() {
        assert!(valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!valid_uuid("550e8400-e29b-01d4-a716-446655440000"));
        assert!(valid_url("https://example.com/path"));
        assert!(!valid_url("https://"));
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2023-02-29"));
        assert!(valid_instant("2024-02-29T23:59:59Z"));
        assert!(valid_instant("2024-02-29T23:59:59+01:00"));
        assert!(!valid_instant("2024-02-29T23:59:59"));
        assert!(valid_duration("PT1H30M"));
        assert!(valid_duration("-PT1.25S"));
        assert!(valid_duration("1.5h"));
        assert!(valid_duration("-250ms"));
        assert!(!valid_duration("P1D"));
        assert!(!valid_duration("PT"));
        assert!(!valid_duration("PT0.0001S"));
        assert!(!valid_duration("PT999999999999999999H"));
        assert!(!valid_duration("0.0001s"));
    }
}

#[cfg(test)]
mod validation_revision_guard {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Local(PathBuf);
    impl Local {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "jadpo-staged-config-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn temporary(&self) -> PathBuf {
            self.0
                .join(format!(".env.local.jadpo.{}.tmp", std::process::id()))
        }
    }
    impl Drop for Local {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn edit_after_staging_is_preserved_and_owned_temporary_is_cleaned() {
        let root = Local::new();
        let path = root.0.join(".env.local");
        fs::write(&path, b"KEY=original\n").unwrap();
        let diagnostic = super::atomic_write_before_promotion(
            &path,
            b"KEY=replacement\n",
            b"KEY=original\n",
            || {
                assert_eq!(fs::read(root.temporary()).unwrap(), b"KEY=replacement\n");
                fs::write(&path, b"KEY=concurrent\n").unwrap();
            },
        )
        .unwrap_err();
        assert_eq!(diagnostic.code, "CONFIG_LOCAL_CHANGED");
        assert_eq!(fs::read(&path).unwrap(), b"KEY=concurrent\n");
        assert!(!root.temporary().exists());
    }

    #[test]
    fn newly_created_file_after_staging_is_preserved() {
        let root = Local::new();
        let path = root.0.join(".env.local");
        let diagnostic =
            super::atomic_write_before_promotion(&path, b"KEY=replacement\n", b"", || {
                fs::write(&path, b"KEY=concurrent\n").unwrap();
            })
            .unwrap_err();
        assert_eq!(diagnostic.code, "CONFIG_LOCAL_CHANGED");
        assert_eq!(fs::read(&path).unwrap(), b"KEY=concurrent\n");
        assert!(!root.temporary().exists());
    }

    #[test]
    fn foreign_staging_file_is_neither_removed_nor_promoted() {
        let root = Local::new();
        let path = root.0.join(".env.local");
        fs::write(&path, b"KEY=original\n").unwrap();
        fs::write(root.temporary(), b"foreign staging contents").unwrap();
        let diagnostic = super::atomic_write_before_promotion(
            &path,
            b"KEY=replacement\n",
            b"KEY=original\n",
            || panic!("must not own this stage"),
        )
        .unwrap_err();
        assert_eq!(diagnostic.code, "CONFIG_LOCAL_WRITE_FAILED");
        assert_eq!(fs::read(&path).unwrap(), b"KEY=original\n");
        assert_eq!(
            fs::read(root.temporary()).unwrap(),
            b"foreign staging contents"
        );
    }

    #[test]
    fn concurrent_configuration_change_is_rejected_without_disclosing_values() {
        let diagnostic = super::ensure_unchanged(b"KEY=old-secret", b"KEY=new-secret").unwrap_err();
        assert_eq!(diagnostic.code, "CONFIG_LOCAL_CHANGED");
        let rendered = format!("{diagnostic:?}");
        assert!(!rendered.contains("old-secret"));
        assert!(!rendered.contains("new-secret"));
        assert!(super::ensure_unchanged(b"KEY=same", b"KEY=same").is_ok());
    }
}
