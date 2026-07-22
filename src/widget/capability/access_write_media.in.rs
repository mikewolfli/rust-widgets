#[cfg(not(feature = "mini"))]
pub fn write_media_props(
    widget: &mut dyn Widget,
    property_name: &str,
    value: CapabilityValue,
) -> Result<(), CapabilityAccessError> {
    match widget.kind() {
        WidgetKind::AnimatedImage => {
        if let Some(w) = widget_as_mut::<AnimatedImage>(widget) {
        match property_name {
        "playing" => {
        if expect_bool(value)? {
        w.play();
        } else {
        w.pause();
        }
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::HeroAnimation => {
        if let Some(w) = widget_as_mut::<HeroAnimation>(widget) {
        match property_name {
        "animation_progress" => {
        w.set_progress(expect_f32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::LottieWidget => {
        if let Some(w) = widget_as_mut::<LottieWidget>(widget) {
        match property_name {
        "playing" => {
        if expect_bool(value)? {
        w.play();
        } else {
        w.pause();
        }
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::RiveWidget => {
        if let Some(w) = widget_as_mut::<RiveWidget>(widget) {
        match property_name {
        "is_playing" => {
        if expect_bool(value)? {
        w.play();
        } else {
        w.pause();
        }
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::VideoPlayer => {
        if let Some(w) = widget_as_mut::<VideoPlayer>(widget) {
        match property_name {
        "is_playing" => {
        if expect_bool(value)? {
        w.play();
        } else {
        w.pause();
        }
        Ok(())
        }
        "volume" => {
        w.set_volume(expect_f32(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::AudioVisualizer => {
        if let Some(w) = widget_as_mut::<AudioVisualizer>(widget) {
        match property_name {
        "bar_count" => {
        w.set_bar_count(expect_usize(value)?);
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        WidgetKind::CameraPreview => {
        if let Some(w) = widget_as_mut::<CameraPreview>(widget) {
        match property_name {
        "is_active" => {
        if expect_bool(value)? {
        w.start_preview();
        } else {
        w.stop_preview();
        }
        Ok(())
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
        }
        } else {
        Err(CapabilityAccessError::UnsupportedOnWidget)
        }
        }
        _ => Err(CapabilityAccessError::UnsupportedOnWidget),
    }
}
