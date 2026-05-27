//! i18n macros - translation macros
/// tr! macro for translation
#[macro_export]
macro_rules! tr {
    ($key:expr) => {
        $crate::i18n::translate($key)
    };
    ($key:expr, $count:expr) => {
        $crate::i18n::translate_with_context($key, None, $count)
    };
    ($key:expr, $context:expr, $count:expr) => {
        $crate::i18n::translate_with_context($key, Some($context), $count)
    };
}
