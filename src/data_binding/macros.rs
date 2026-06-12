/// Create a [`Binding`](crate::data_binding::Binding) with an initial value.
///
/// # Examples
///
/// ```
/// # use rust_widgets::binding;
/// # use rust_widgets::data_binding::Binding;
/// let count = binding!(0);
/// assert_eq!(count.get(), 0);
/// ```
#[macro_export]
macro_rules! binding {
    ($value:expr) => {
        $crate::data_binding::Binding::new($value)
    };
}

/// Create a [`Computed`](crate::data_binding::Computed) binding.
///
/// The first argument is the compute closure, the second is the initial value.
///
/// # Examples
///
/// ```
/// # use rust_widgets::computed;
/// # use rust_widgets::data_binding::Computed;
/// let mut double = computed!(|| 2 * 21, 0);
/// assert_eq!(double.get(), 42);
/// ```
#[macro_export]
macro_rules! computed {
    ($compute:expr, $initial:expr) => {
        $crate::data_binding::Computed::new($compute, $initial)
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_binding_macro() {
        let b = binding!(42);
        assert_eq!(b.get(), 42);
    }

    #[test]
    fn test_binding_macro_with_string() {
        let b = binding!("hello".to_string());
        assert_eq!(b.get(), "hello");
    }

    #[test]
    fn test_computed_macro() {
        let mut c = computed!(|| 2 + 2, 0);
        assert_eq!(c.get(), 4);
    }

    #[test]
    fn test_computed_macro_with_closure() {
        let x = 10;
        let mut c = computed!(move || x * 3, 0);
        assert_eq!(c.get(), 30);
    }
}
