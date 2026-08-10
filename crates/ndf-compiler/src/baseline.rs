use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use ndf_core::identity::NodeId;
use ndf_core::model::{Diagnostic, DiagnosticSeverity, SourceSpan};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BaselineEntry {
    pub fingerprint: String,
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub subject: String,
    pub path: String,
    pub owner: Option<String>,
    pub expires: Option<String>,
}

impl BaselineEntry {
    pub fn from_diagnostic(diagnostic: &Diagnostic, owner: &str, expires: &str) -> Self {
        Self {
            fingerprint: diagnostic_fingerprint(diagnostic),
            code: diagnostic.code.clone(),
            severity: diagnostic.severity,
            subject: diagnostic
                .subject
                .as_ref()
                .map_or_else(String::new, ToString::to_string),
            path: normalize_path(&diagnostic.primary.path),
            owner: Some(owner.to_owned()),
            expires: Some(expires.to_owned()),
        }
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct DiagnosticBaseline {
    pub entries: Vec<BaselineEntry>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct BaselineResult {
    pub existing: Vec<Diagnostic>,
    pub new: Vec<Diagnostic>,
    pub worsened: Vec<Diagnostic>,
    pub expired: Vec<BaselineEntry>,
}

pub fn diagnostic_fingerprint(diagnostic: &Diagnostic) -> String {
    let payload = [
        diagnostic.code.clone(),
        diagnostic
            .subject
            .as_ref()
            .map_or_else(String::new, ToString::to_string),
        normalize_path(&diagnostic.primary.path),
        collapse_whitespace(&diagnostic.message),
    ]
    .join("\0");
    let digest = Sha256::digest(payload.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

pub fn compare_baseline(current: &[Diagnostic], baseline: &DiagnosticBaseline) -> BaselineResult {
    let mut by_fingerprint = HashMap::new();
    let mut invalid_fingerprints = HashSet::new();
    let mut result = BaselineResult::default();
    let today = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_secs()
        / 86_400;

    for entry in &baseline.entries {
        let expiry = entry.expires.as_deref().and_then(parse_date_days);
        if entry.owner.as_deref().is_none_or(str::is_empty) || expiry.is_none() {
            invalid_fingerprints.insert(entry.fingerprint.as_str());
            result.new.push(integrity_diagnostic(entry));
            continue;
        }
        if expiry.is_some_and(|day| day < today as i64) {
            result.expired.push(entry.clone());
        }
        by_fingerprint.insert(entry.fingerprint.as_str(), entry);
    }

    for diagnostic in current {
        let fingerprint = diagnostic_fingerprint(diagnostic);
        if invalid_fingerprints.contains(fingerprint.as_str()) {
            continue;
        }
        match by_fingerprint.get(fingerprint.as_str()) {
            None => result.new.push(diagnostic.clone()),
            Some(entry) if severity_rank(diagnostic.severity) > severity_rank(entry.severity) => {
                result.worsened.push(diagnostic.clone());
            }
            Some(_) => result.existing.push(diagnostic.clone()),
        }
    }
    result
}

fn integrity_diagnostic(entry: &BaselineEntry) -> Diagnostic {
    Diagnostic {
        code: "NDF-BASE-003".to_owned(),
        severity: DiagnosticSeverity::Error,
        message: format!("baseline entry {} lacks owner or expiry", entry.fingerprint),
        primary: SourceSpan::new(
            if entry.path.is_empty() {
                "ndf-baseline.json"
            } else {
                &entry.path
            },
            1,
        )
        .expect("line one is valid"),
        related: Vec::new(),
        repair: Some("Assign an owner and ISO expiry date to the baseline entry.".to_owned()),
        subject: if entry.subject.is_empty() {
            None
        } else {
            NodeId::parse(&entry.subject, None).ok()
        },
    }
}

fn severity_rank(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Information => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Error => 2,
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn collapse_whitespace(message: &str) -> String {
    message.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_date_days(value: &str) -> Option<i64> {
    if value.len() != 10 || &value[4..5] != "-" || &value[7..8] != "-" {
        return None;
    }
    let year: i64 = value[..4].parse().ok()?;
    let month: u32 = value[5..7].parse().ok()?;
    let day: u32 = value[8..].parse().ok()?;
    let month_lengths = [
        31,
        28 + u32::from(is_leap_year(year)),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&month) || day == 0 || day > month_lengths[(month - 1) as usize] {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
