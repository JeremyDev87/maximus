use std::cell::OnceCell;
use std::cmp::Ordering;

use icu_collator::CollatorBorrowed;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnvTemplateSortMode {
    #[default]
    Plain,
    Prefix,
}

pub(crate) fn locale_compare_like(left: &str, right: &str) -> Ordering {
    thread_local! {
        static COLLATOR: OnceCell<Option<CollatorBorrowed<'static>>> = const { OnceCell::new() };
    }

    COLLATOR.with(|cell| {
        cell.get_or_init(|| CollatorBorrowed::try_new(Default::default(), Default::default()).ok())
            .as_ref()
            .map(|collator| collator.compare(left, right))
            .unwrap_or_else(|| left.cmp(right))
    })
}

pub(crate) fn compare_env_template_keys(
    left: &str,
    right: &str,
    sort_mode: EnvTemplateSortMode,
) -> Ordering {
    match sort_mode {
        EnvTemplateSortMode::Plain => locale_compare_like(left, right),
        EnvTemplateSortMode::Prefix => {
            let left_prefix = env_key_prefix(left);
            let right_prefix = env_key_prefix(right);

            locale_compare_like(left_prefix, right_prefix)
                .then_with(|| locale_compare_like(left, right))
        }
    }
}

pub(crate) fn env_key_prefix(key: &str) -> &str {
    key.split_once('_').map(|(prefix, _)| prefix).unwrap_or(key)
}
