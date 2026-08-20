//! `validate-compare` — run both validators over a set of generated iXBRL
//! documents and compare their findings.
//!
//! The in-process checker ([`validate-uk`]) is fast but approximates the full
//! XBRL pass; Arelle's `validate/UK` plugin is the reference implementation
//! of JFCVC v4.0.  This tool runs both on the same documents and buckets the
//! findings by error code so divergences — both bugs in one side and the
//! deliberate differences (see below) — are visible at a glance.
//!
//! ```text
//! validate-compare [<file>...] [--dir <dir>] [--arelle <path>]
//!                  [--taxonomy-dir <dir>] [--strict]
//! ```
//!
//! Defaults: documents from `.cache/tally-full-year` (the live-company test
//! set), `arelleCmdLine` from PATH.
//!
//! Deliberate divergences between the two validators (reported as such):
//! - ct-comp **computation** documents: Arelle's 2020-era namespace regex
//!   (`http://www.govtalk.gov.uk/uk/fr/tax/uk-hmrc-ct/...`) does not match
//!   the modern `http://www.hmrc.gov.uk/schemas/ct/comp` namespace, so the
//!   plugin misclassifies them as accounts and demands the whole FRS-2022
//!   mandatory set (JFCVC.3312).  The Rust checker recognises ct-comp and
//!   skips the accounts mandatory rule.
//! - Code naming: Arelle reports schema-level value problems as
//!   `xmlSchema:valueError`; the Rust checker uses its own `schema.*` codes.
//!   The alias table below maps the ones observed in practice.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use validate_uk::Validator;
use validate_uk::checks::{detect_doc_kind, DocKind};
use validate_uk::document;

/// Map Arelle codes onto the equivalent Rust `schema.*` code so the two
/// sides bucket together (same underlying check, different naming).
/// Arelle reports every schema facet / lexical violation as a single
/// `xmlSchema:valueError`, so the message is inspected to pick the matching
/// `schema.*` code (fixed-item length-0, pattern facet, minLength facet,
/// gYear lexical, or plain value).
fn code_alias<'a>(code: &'a str, message: &str) -> &'a str {
    match code {
        "xmlSchema:valueError" => {
            if message.contains("minLength") {
                "schema.minLength"
            } else if message.contains("pattern facet") {
                "schema.patternValue"
            } else if message.contains("type gYear") {
                "schema.gYearValue"
            } else {
                "schema.fixedValue"
            }
        }
        "xmlSchema:typeError" => "schema.typeMismatch",
        "xbrl:contextPeriodType" | "xbrl.4.7.2:contextPeriodType" => {
            "schema.periodMismatch"
        }
        other => other,
    }
}

/// The Arelle invocation used for the reference pass (matches the repo's
/// `validate-accounts` flake script: UK plugin + hmrc disclosure system).
const ARELLE_ARGS: &[&str] = &[
    "--plugins",
    "validate/UK",
    "--disclosureSystem",
    "hmrc",
    "-v",
    "--logFormat",
    "%(levelname)s|%(messageCode)s|%(message)s",
];

struct Finding {
    code: String,
    message: String,
}

fn main() -> Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut dir: Option<PathBuf> = None;
    let mut arelle = String::from("arelleCmdLine");
    let mut taxonomy_dir: Option<PathBuf> = None;
    let mut strict = false;

    let mut rest = std::env::args().skip(1);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dir" => dir = Some(PathBuf::from(next_value(&mut rest, "--dir")?)),
            "--arelle" => arelle = next_value(&mut rest, "--arelle")?,
            "--taxonomy-dir" => taxonomy_dir = Some(PathBuf::from(next_value(&mut rest, "--taxonomy-dir")?)),
            "--strict" => strict = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other => files.push(PathBuf::from(other)),
        }
    }

    // Default to the live-company documents under .cache/tally-full-year.
    if files.is_empty() {
        let dir = dir.unwrap_or_else(|| PathBuf::from(".cache/tally-full-year"));
        let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
            .with_context(|| format!("cannot read {}", dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x == "html").unwrap_or(false))
            .collect();
        found.sort();
        files = found;
    }
    if files.is_empty() {
        bail!("no documents to validate (pass <file>... or --dir with *.html files)");
    }

    let validator = match &taxonomy_dir {
        Some(d) => Validator::with_taxonomy_dir(d).map_err(|e| anyhow::anyhow!(e))?,
        None => Validator::new(),
    };

    let mut totals: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // code -> (rust, arelle)
    let mut divergences: Vec<String> = Vec::new();

    for file in &files {
        let html = std::fs::read_to_string(file)
            .with_context(|| format!("cannot read {}", file.display()))?;
        let doc = document::parse(&html);
        let kind = detect_doc_kind(&doc);
        let kind_label = match kind {
            DocKind::Accounts => "Accounts",
            DocKind::Computation => "Computation",
        };

        // Rust side.
        let report = validator.validate_document(&doc);
        let mut rust: Vec<Finding> = report
            .issues
            .iter()
            .map(|i| Finding {
                code: i.code.clone(),
                message: i.message.clone(),
            })
            .collect();
        rust.sort_by(|a, b| a.code.cmp(&b.code));

        // Arelle side.
        let arelle_out = run_arelle(&arelle, file)?;
        let arelle_findings = parse_arelle_log(&arelle_out);

        // Bucket by code (aliases applied), remembering one sample message.
        let mut rust_by_code: BTreeMap<String, (usize, String)> = BTreeMap::new();
        for f in &rust {
            let e = rust_by_code
                .entry(code_alias(&f.code, &f.message).to_string())
                .or_insert((0, f.message.clone()));
            e.0 += 1;
        }
        let mut arelle_by_code: BTreeMap<String, (usize, String)> = BTreeMap::new();
        for f in &arelle_findings {
            let e = arelle_by_code
                .entry(code_alias(&f.code, &f.message).to_string())
                .or_insert((0, f.message.clone()));
            e.0 += 1;
        }

        println!("=== {} ({})", file.display(), kind_label);
        println!("  Rust   : {} issue(s)", rust.len());
        println!("  Arelle : {} issue(s)", arelle_findings.len());

        let mut any_difference = false;
        let mut codes: Vec<&String> = rust_by_code.keys().chain(arelle_by_code.keys()).collect();
        codes.sort();
        codes.dedup();
        for code in codes {
            let r = rust_by_code.get(code);
            let a = arelle_by_code.get(code);
            let rn = r.map(|(n, _)| *n).unwrap_or(0);
            let an = a.map(|(n, _)| *n).unwrap_or(0);
            let (r_old, a_old) = totals.get(code).copied().unwrap_or((0, 0));
            totals.insert(code.clone(), (r_old + rn, a_old + an));
            match (r, a) {
                (Some(_), Some(_)) => {
                    println!("    match  {code:<28} rust={rn} arelle={an}");
                }
                (Some((_, msg)), None) => {
                    any_difference = true;
                    println!("    [rust-only]   {code}: {msg}");
                }
                (None, Some((_, msg))) => {
                    any_difference = true;
                    if kind == DocKind::Computation && code.starts_with("JFCVC.33") {
                        println!(
                            "    [arelle-only] {code} (expected: Arelle misclassifies ct-comp computation as accounts and demands the FRS-2022 mandatory set — the Rust checker skips that rule for computation): {}",
                            truncate(&msg, 140)
                        );
                    } else {
                        println!("    [arelle-only] {code}: {}", truncate(&msg, 140));
                    }
                }
                _ => {}
            }
        }
        if !any_difference {
            println!("    match (no differences)");
        } else {
            divergences.push(file.display().to_string());
        }
        println!();
    }

    println!("--- per-code totals (rust vs arelle) ---");
    for (code, (r, a)) in &totals {
        let mark = if *r > 0 && *a > 0 {
            "same"
        } else if *r > 0 {
            "rust-only"
        } else {
            "arelle-only"
        };
        println!("  {code:<28} rust={r:<4} arelle={a:<4} {mark}");
    }
    println!(
        "\n{} document(s), {} with divergences",
        files.len(),
        divergences.len()
    );

    if strict && !divergences.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

fn run_arelle(bin: &str, file: &Path) -> Result<String> {
    let out = Command::new(bin)
        .args(ARELLE_ARGS)
        .arg("-f")
        .arg(file)
        .output()
        .with_context(|| format!("failed to run {bin} (is Arelle installed?)"))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parse Arelle's `--logFormat "%(levelname)s|%(messageCode)s|%(message)s"`
/// output, keeping only ERROR / WARNING lines.
fn parse_arelle_log(log: &str) -> Vec<Finding> {
    log.lines()
        .filter_map(|line| {
            let (level, rest) = line.split_once('|')?;
            if level != "ERROR" && level != "WARNING" {
                return None;
            }
            let (code, message) = rest.split_once('|')?;
            Some(Finding {
                code: code.to_string(),
                message: message.to_string(),
            })
        })
        .collect()
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(n).collect();
        out.push('…');
        out
    }
}

fn next_value(rest: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    rest.next().with_context(|| format!("missing value for {flag}"))
}

const USAGE: &str = "\
usage: validate-compare [<file>...] [--dir <dir>] [--arelle <path>] [--taxonomy-dir <dir>] [--strict]

Run both the in-process validate-uk checker and Arelle's validate/UK plugin
over the given iXBRL documents (default: every *.html under
.cache/tally-full-year) and compare their findings by error code.

  <file>...               documents to compare (default: --dir/*.html)
  --dir <dir>             directory of documents (default: .cache/tally-full-year)
  --arelle <path>         arelleCmdLine binary (default: from PATH)
  --taxonomy-dir <dir>    directory of taxonomy XSDs for the Rust checker
                          (default: embedded concept table)
  --strict                exit non-zero when any divergence is found

Known/expected divergences are annotated in the output (e.g. Arelle
misclassifying ct-comp computation documents as accounts).";
