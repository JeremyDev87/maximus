use std::cell::OnceCell;
use std::cmp::Ordering;

use icu_collator::CollatorBorrowed;

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
