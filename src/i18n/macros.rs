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

#[cfg(test)]
mod tests {
    /// Reset the global i18n state so tests run with no loaded translations.
    fn reset_global() {
        *crate::i18n::global::GLOBAL_I18N.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    #[test]
    fn tr_macro_basic_key() {
        reset_global();
        // With global not initialized, should return the key itself
        assert_eq!(crate::tr!("test_key"), "test_key");
    }

    #[test]
    fn tr_macro_with_count() {
        assert_eq!(crate::tr!("test_key", 5), "test_key");
    }

    #[test]
    fn tr_macro_with_context_and_count() {
        reset_global();
        assert_eq!(crate::tr!("test_key", "context", 1), "test_key");
    }

    #[test]
    fn tr_macro_empty_key() {
        reset_global();
        assert_eq!(crate::tr!(""), "");
    }

    #[test]
    fn tr_macro_with_special_chars() {
        // Keys with dots, underscores, slashes
        assert_eq!(crate::tr!("common.button.ok"), "common.button.ok");
        assert_eq!(crate::tr!("dialog.file_dialog.open_file"), "dialog.file_dialog.open_file");
        assert_eq!(crate::tr!("a.b.c.d.e"), "a.b.c.d.e");
    }

    #[test]
    fn tr_macro_with_unicode_key() {
        reset_global();
        assert_eq!(crate::tr!("你好"), "你好");
        assert_eq!(crate::tr!("こんにちは"), "こんにちは");
    }

    #[test]
    fn tr_macro_long_key() {
        let long_key = "a".repeat(255);
        assert_eq!(crate::tr!(&long_key), long_key);
    }

    #[test]
    fn tr_macro_with_zero_count() {
        reset_global();
        // Zero is a valid count for plural forms
        assert_eq!(crate::tr!("key", 0), "key");
    }

    #[test]
    fn tr_macro_with_large_count() {
        reset_global();
        assert_eq!(crate::tr!("key", 999999), "key");
    }

    #[test]
    fn tr_macro_context_empty_string() {
        reset_global();
        assert_eq!(crate::tr!("key", "", 1), "key");
    }

    #[test]
    fn tr_macro_multiple_contexts() {
        // Verify different context values pass through correctly
        assert_eq!(crate::tr!("greeting", "formal", 1), "greeting");
        assert_eq!(crate::tr!("greeting", "casual", 1), "greeting");
    }

    #[test]
    fn tr_macro_numeric_count() {
        reset_global();
        // Count as various numeric types that coerce to u32
        assert_eq!(crate::tr!("items", 1u32), "items");
        assert_eq!(crate::tr!("items", 2), "items");
        assert_eq!(crate::tr!("items", 0u32), "items");
    }
}
