use std::fs;
use std::path::Path;

use maximus_core::baseline::{
    contains_match, detail_hash, find_matching_record, load_baseline_records,
    parse_baseline_records, record_matches, BaselineMatchKey, BaselineRecord,
};
use tempfile::TempDir;

fn sample_record() -> BaselineRecord {
    BaselineRecord {
        id: "env-example".to_string(),
        file: Path::new("packages/app/.env.example").to_path_buf(),
        detail_hash: detail_hash("Create .env.example"),
    }
}

#[test]
fn load_baseline_records_parses_wrapped_json_successfully() {
    let fixture = TempDir::new().unwrap();
    let path = fixture.path().join("baseline.json");
    let record = sample_record();

    fs::write(
        &path,
        format!(
            r#"{{
  "baseline": [
    {{
      "id": "{id}",
      "file": "{file}",
      "detailHash": "{detail_hash}"
    }}
  ]
}}"#,
            id = record.id,
            file = record.file.display(),
            detail_hash = record.detail_hash
        ),
    )
    .unwrap();

    let parsed = load_baseline_records(&path).unwrap();

    assert_eq!(parsed, vec![record]);
}

#[test]
fn load_baseline_records_surfaces_parse_errors() {
    let fixture = TempDir::new().unwrap();
    let path = fixture.path().join("baseline.json");

    fs::write(&path, "{ not valid json").unwrap();

    let error = load_baseline_records(&path).unwrap_err();

    assert!(error.to_string().contains("baseline.json"));
}

#[test]
fn exact_match_requires_matching_id_file_and_detail_hash() {
    let record = sample_record();
    let key = BaselineMatchKey {
        id: &record.id,
        file: &record.file,
        detail_hash: &record.detail_hash,
    };

    assert!(record_matches(&record, &key));
    assert_eq!(find_matching_record(std::slice::from_ref(&record), &key), Some(&record));
    assert!(contains_match(std::slice::from_ref(&record), &key));
}

#[test]
fn non_match_rejects_mismatched_detail_hash() {
    let record = sample_record();
    let key = BaselineMatchKey {
        id: &record.id,
        file: &record.file,
        detail_hash: "0000000000000000",
    };

    assert!(!record_matches(&record, &key));
    assert!(find_matching_record(std::slice::from_ref(&record), &key).is_none());
    assert!(!contains_match(std::slice::from_ref(&record), &key));
}

#[test]
fn parse_baseline_records_supports_bare_arrays() {
    let record = sample_record();
    let parsed = parse_baseline_records(
        &format!(
            r#"[{{"id":"{id}","file":"{file}","detailHash":"{detail_hash}"}}]"#,
            id = record.id,
            file = record.file.display(),
            detail_hash = record.detail_hash
        ),
        "baseline.json",
    )
    .unwrap();

    assert_eq!(parsed, vec![record]);
}
