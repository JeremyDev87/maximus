use std::cmp::Ordering;

use icu_collator::Collator;

pub(crate) fn locale_compare_like(left: &str, right: &str) -> Ordering {
    Collator::try_new(Default::default(), Default::default())
        .map(|collator| collator.compare(left, right))
        .unwrap_or_else(|_| left.cmp(right))
}
