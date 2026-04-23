use std::collections::{BTreeMap, BTreeSet};
use std::iter::FromIterator;

use maximus_core::{parse_env, parse_jsonc};
use proptest::prelude::*;
use serde_json::{Map, Value};

fn valid_env_key_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Za-z_][A-Za-z0-9_.-]{0,12}").expect("valid env key regex")
}

fn env_value_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[A-Za-z0-9_./+=-]{0,15}").expect("valid env value regex")
}

fn valid_env_line_strategy() -> impl Strategy<Value = (String, String, bool)> {
    (
        valid_env_key_strategy(),
        env_value_strategy(),
        any::<bool>(),
    )
}

fn invalid_env_line_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::string::string_regex("[A-Za-z0-9:?][A-Za-z0-9 :?]{0,11}")
            .expect("valid invalid-line regex"),
        Just("export ONLY".to_string()),
    ]
}

fn json_value_strategy() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|value| Value::Number(value.into())),
        prop::string::string_regex("[A-Za-z0-9 _.-]{0,12}")
            .expect("valid string regex")
            .prop_map(Value::String),
        Just(Value::Null),
    ];

    leaf.prop_recursive(3, 32, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            prop::collection::btree_map(
                prop::string::string_regex("[A-Za-z_][A-Za-z0-9_]{0,6}")
                    .expect("valid object key regex"),
                inner,
                0..5,
            )
            .prop_map(|entries| Value::Object(Map::from_iter(entries.into_iter()))),
        ]
    })
}

proptest! {
    #[test]
    fn parse_env_tracks_first_seen_order_and_last_value(
        lines in prop::collection::vec(valid_env_line_strategy(), 0..40),
    ) {
        let mut text = String::new();
        let mut expected_order = Vec::new();
        let mut expected_values = BTreeMap::new();
        let mut seen_keys = BTreeSet::new();

        for (key, value, use_export_prefix) in &lines {
            if *use_export_prefix {
                text.push_str("export ");
            }
            text.push_str(key);
            text.push('=');
            text.push_str(value);
            text.push('\n');

            if seen_keys.insert(key.clone()) {
                expected_order.push(key.clone());
            }

            expected_values.insert(key.clone(), value.clone());
        }

        let parsed = parse_env(&text, Some(".env.test"));

        prop_assert_eq!(parsed.invalid_lines.len(), 0);
        prop_assert_eq!(parsed.entries.len(), lines.len());
        prop_assert_eq!(parsed.order, expected_order);

        let actual_values = parsed
            .values
            .iter()
            .map(|(key, entry)| (key.clone(), entry.value.clone()))
            .collect::<BTreeMap<_, _>>();
        prop_assert_eq!(actual_values, expected_values.clone());

        for (key, value) in &expected_values {
            prop_assert_eq!(&parsed.values[key].raw_value, value);
            prop_assert_eq!(&parsed.values[key].value, value.trim());
        }
    }

    #[test]
    fn parse_env_reports_invalid_lines_without_disturbing_valid_entries(
        valid_lines in prop::collection::vec(valid_env_line_strategy(), 0..20),
        invalid_lines in prop::collection::vec(invalid_env_line_strategy(), 0..20),
    ) {
        let mut lines = Vec::new();
        let mut expected_valid_entries = Vec::new();

        for (key, value, use_export_prefix) in valid_lines {
            let mut line = String::new();
            if use_export_prefix {
                line.push_str("export ");
            }
            line.push_str(&key);
            line.push('=');
            line.push_str(&value);
            expected_valid_entries.push((key, value));
            lines.push(line);
        }

        let invalid_line_numbers = invalid_lines
            .iter()
            .enumerate()
            .map(|(index, line)| (index + lines.len() + 1, line.clone()))
            .collect::<Vec<_>>();

        lines.extend(invalid_lines);

        let text = lines.join("\n") + "\n";
        let parsed = parse_env(&text, Some("fixture.env"));

        prop_assert_eq!(parsed.entries.len(), expected_valid_entries.len());
        prop_assert_eq!(parsed.invalid_lines.len(), invalid_line_numbers.len());

        for ((line_number, content), actual) in invalid_line_numbers.iter().zip(&parsed.invalid_lines) {
            prop_assert_eq!(&actual.label, "fixture.env");
            prop_assert_eq!(actual.line, *line_number);
            prop_assert_eq!(&actual.content, content);
        }
    }

    #[test]
    fn parse_jsonc_roundtrips_valid_json_values(value in json_value_strategy()) {
        let text = serde_json::to_string(&value).expect("serializing generated JSON value");
        let parsed: Value = parse_jsonc(&text, "fixture.jsonc").expect("generated JSON should parse");

        prop_assert_eq!(parsed, value);
    }
}
