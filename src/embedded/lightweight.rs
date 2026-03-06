use crate::widget::Widget;

/// Lightweight widget configuration for embedded systems
#[derive(Debug, Clone)]
pub struct LightweightConfig {
    pub disable_shadows: bool,
    pub disable_animations: bool,
    pub disable_gradients: bool,
    pub simple_borders: bool,
    pub reduced_padding: bool,
    pub minimal_signals: bool,
}

impl LightweightConfig {
    pub fn new() -> Self {
        Self {
            disable_shadows: false,
            disable_animations: false,
            disable_gradients: false,
            simple_borders: false,
            reduced_padding: false,
            minimal_signals: false,
        }
    }

    pub fn minimal() -> Self {
        Self {
            disable_shadows: true,
            disable_animations: true,
            disable_gradients: true,
            simple_borders: true,
            reduced_padding: true,
            minimal_signals: true,
        }
    }

    pub fn with_shadows_disabled(mut self) -> Self {
        self.disable_shadows = true;
        self
    }

    pub fn with_animations_disabled(mut self) -> Self {
        self.disable_animations = true;
        self
    }

    pub fn with_gradients_disabled(mut self) -> Self {
        self.disable_gradients = true;
        self
    }
}

impl Default for LightweightConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Lightweight widget wrapper that reduces memory footprint
pub struct LightweightWidget<W: Widget> {
    inner: W,
    config: LightweightConfig,
}

impl<W: Widget> LightweightWidget<W> {
    pub fn new(widget: W) -> Self {
        Self {
            inner: widget,
            config: LightweightConfig::new(),
        }
    }

    pub fn with_config(mut self, config: LightweightConfig) -> Self {
        self.config = config;
        self
    }

    pub fn inner(&self) -> &W {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

/// Factory for creating lightweight widgets
pub struct LightweightWidgetFactory {
    config: LightweightConfig,
    widget_count: usize,
    max_widgets: usize,
}

impl LightweightWidgetFactory {
    pub fn new() -> Self {
        Self {
            config: LightweightConfig::minimal(),
            widget_count: 0,
            max_widgets: 100,
        }
    }

    pub fn with_config(mut self, config: LightweightConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_max_widgets(mut self, max: usize) -> Self {
        self.max_widgets = max;
        self
    }

    pub fn can_create(&self) -> bool {
        self.widget_count < self.max_widgets
    }

    pub fn create<W, F>(&mut self, factory: F) -> Option<LightweightWidget<W>>
    where
        F: FnOnce() -> W,
        W: Widget,
    {
        if self.can_create() {
            self.widget_count += 1;
            Some(LightweightWidget::new(factory()).with_config(self.config.clone()))
        } else {
            None
        }
    }

    pub fn release(&mut self) {
        self.widget_count = self.widget_count.saturating_sub(1);
    }

    pub fn widget_count(&self) -> usize {
        self.widget_count
    }
}

impl Default for LightweightWidgetFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Optimized widget style for embedded systems
#[derive(Debug, Clone)]
pub struct LightweightStyle {
    pub background_color: Option<u32>,
    pub text_color: Option<u32>,
    pub border_color: Option<u32>,
    pub border_width: u8,
    pub padding: u8,
    pub font_size: u8,
}

impl LightweightStyle {
    pub fn new() -> Self {
        Self {
            background_color: None,
            text_color: None,
            border_color: None,
            border_width: 0,
            padding: 4,
            font_size: 12,
        }
    }

    pub fn compact() -> Self {
        Self {
            background_color: None,
            text_color: Some(0x000000),
            border_color: Some(0x808080),
            border_width: 1,
            padding: 2,
            font_size: 10,
        }
    }

}

impl Default for LightweightStyle {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory-efficient widget pool
pub struct WidgetPool<T> {
    available: Vec<T>,
    in_use: Vec<bool>,
    max_size: usize,
}

impl<T> WidgetPool<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            available: Vec::with_capacity(max_size),
            in_use: Vec::with_capacity(max_size),
            max_size,
        }
    }

    pub fn acquire<F>(&mut self, factory: F) -> Option<PoolHandle<T>>
    where
        F: FnOnce() -> T,
    {
        if let Some(index) = self.in_use.iter().position(|&used| !used) {
            self.in_use[index] = true;
            Some(PoolHandle {
                index,
                pool: self as *mut Self,
            })
        } else if self.available.len() < self.max_size {
            let index = self.available.len();
            self.available.push(factory());
            self.in_use.push(true);
            Some(PoolHandle {
                index,
                pool: self as *mut Self,
            })
        } else {
            None
        }
    }

    pub fn release(&mut self, index: usize) {
        if index < self.in_use.len() {
            self.in_use[index] = false;
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.available.len() && self.in_use[index] {
            Some(&self.available[index])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.available.len() && self.in_use[index] {
            Some(&mut self.available[index])
        } else {
            None
        }
    }

    pub fn used_count(&self) -> usize {
        self.in_use.iter().filter(|&&used| used).count()
    }

    pub fn available_count(&self) -> usize {
        self.available.len() - self.used_count()
    }
}

pub struct PoolHandle<T> {
    index: usize,
    pool: *mut WidgetPool<T>,
}

impl<T> PoolHandle<T> {
    pub fn index(&self) -> usize {
        self.index
    }
}

impl<T> Drop for PoolHandle<T> {
    fn drop(&mut self) {
        unsafe {
            if !self.pool.is_null() {
                (*self.pool).release(self.index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lightweight_config() {
        let config = LightweightConfig::minimal();
        assert!(config.disable_shadows);
        assert!(config.disable_animations);
        assert!(config.disable_gradients);
    }

    #[test]
    fn test_widget_factory() {
        use crate::core::Rect;
        
        let mut factory = LightweightWidgetFactory::new()
            .with_max_widgets(2);

        assert!(factory.can_create());

        let widget1 = factory.create(|| crate::widget::Label::new("Test 1".to_string(), Rect::new(0, 0, 100, 30)));
        assert!(widget1.is_some());

        let widget2 = factory.create(|| crate::widget::Label::new("Test 2".to_string(), Rect::new(0, 0, 100, 30)));
        assert!(widget2.is_some());

        let widget3 = factory.create(|| crate::widget::Label::new("Test 3".to_string(), Rect::new(0, 0, 100, 30)));
        assert!(widget3.is_none());

        assert_eq!(factory.widget_count(), 2);
    }

    #[test]
    fn test_lightweight_style() {
        let style = LightweightStyle::compact();
        assert_eq!(style.padding, 2);
        assert_eq!(style.font_size, 10);
    }

    #[test]
    fn test_widget_pool() {
        let mut pool: WidgetPool<i32> = WidgetPool::new(3);

        let handle1 = pool.acquire(|| 1);
        assert!(handle1.is_some());

        let handle2 = pool.acquire(|| 2);
        assert!(handle2.is_some());

        assert_eq!(pool.used_count(), 2);
        assert_eq!(pool.available_count(), 0);

        drop(handle1);
        assert_eq!(pool.used_count(), 1);
    }
}
