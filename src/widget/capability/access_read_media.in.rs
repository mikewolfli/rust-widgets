#[cfg(not(feature = "mini"))]
pub fn read_media_props(
    widget: &dyn Widget,
    property_name: &str,
) -> Result<CapabilityValue, CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::AnimatedImage => match property_name {
        "playing" => {
        if let Some(w) = widget_as::<AnimatedImage>(widget) {
        Ok(CapabilityValue::Bool(w.is_playing()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::HeroAnimation => match property_name {
        "animation_progress" => {
        if let Some(w) = widget_as::<HeroAnimation>(widget) {
        Ok(CapabilityValue::Float(w.progress() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::LottieWidget => match property_name {
        "playing" => {
        if let Some(w) = widget_as::<LottieWidget>(widget) {
        Ok(CapabilityValue::Bool(w.is_playing()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::RiveWidget => match property_name {
        "is_playing" => {
        if let Some(w) = widget_as::<RiveWidget>(widget) {
        Ok(CapabilityValue::Bool(w.is_playing()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::VideoPlayer => match property_name {
        "is_playing" => {
        if let Some(w) = widget_as::<VideoPlayer>(widget) {
        Ok(CapabilityValue::Bool(w.is_playing()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        "volume" => {
        if let Some(w) = widget_as::<VideoPlayer>(widget) {
        Ok(CapabilityValue::Float(w.volume() as f64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::AudioVisualizer => match property_name {
        "bar_count" => {
        if let Some(w) = widget_as::<AudioVisualizer>(widget) {
        Ok(CapabilityValue::UInt(w.bar_count() as u64))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        WidgetKind::CameraPreview => match property_name {
        "is_active" => {
        if let Some(w) = widget_as::<CameraPreview>(widget) {
        Ok(CapabilityValue::Bool(w.is_active()))
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        },
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
