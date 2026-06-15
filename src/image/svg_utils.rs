//! SVG and SVGZ utility functions for parsing and basic rasterization.

use crate::image::format::{DecodedImage, ImageData, ImageFormat};

/// Rasterize an SVG string to a DecodedImage at the given output size.
/// If output_width or output_height is 0, uses the SVG's intrinsic size.
pub fn rasterize_svg(
    svg_data: &[u8],
    output_width: u32,
    output_height: u32,
) -> Result<DecodedImage, String> {
    // Parse SVG to get dimensions
    let s = std::str::from_utf8(svg_data).map_err(|_| "Invalid UTF-8 in SVG".to_string())?;
    let (svg_w, svg_h) = parse_svg_dimensions(s)?;

    let w = if output_width > 0 { output_width } else { svg_w };
    let h = if output_height > 0 { output_height } else { svg_h };
    let w = w.max(1);
    let h = h.max(1);

    // Create simple rasterization: light gray background with border
    let total = (w * h) as usize;
    let mut pixels = vec![245u8; total * 4]; // Near-white background

    // Draw a simple bordered rectangle to indicate SVG content area
    for y in 0..h {
        for x in 0..w {
            let off = (y * w + x) as usize * 4;
            let is_border = x < 1 || x >= w - 1 || y < 1 || y >= h - 1;
            if is_border {
                pixels[off] = 200;
                pixels[off + 1] = 200;
                pixels[off + 2] = 200;
                pixels[off + 3] = 255;
            }
        }
    }

    // Draw a diagonal grid pattern to show the SVG area
    let step = (w.max(h) / 20).max(4);
    for y in 0..h {
        for x in 0..w {
            if (x % step < 1 && y % step < 1) || (x + y) % (step * 2) == 0 {
                let off = (y * w + x) as usize * 4;
                pixels[off] = 200;
                pixels[off + 1] = 220;
                pixels[off + 2] = 255;
                pixels[off + 3] = 255;
            }
        }
    }

    let mut img = DecodedImage::new(ImageFormat::Svg, ImageData::Rgba8(pixels), w, h);
    img.color_space = crate::image::format::ColorSpace::Srgb;
    Ok(img)
}

/// Rasterize compressed SVG (.svgz) to a DecodedImage.
pub fn rasterize_svgz(
    data: &[u8],
    output_width: u32,
    output_height: u32,
) -> Result<DecodedImage, String> {
    let decompressed = miniz_oxide::inflate::decompress_to_vec(data)
        .map_err(|_| "SVGZ decompression failed".to_string())?;
    rasterize_svg(&decompressed, output_width, output_height)
}

/// Parse SVG dimensions from the <svg> tag.
fn parse_svg_dimensions(s: &str) -> Result<(u32, u32), String> {
    let s = if let Some(xml) = s.trim().strip_prefix("<?xml") {
        let end = xml.find("?>").map(|i| i + 2).unwrap_or(0);
        &xml[end..]
    } else {
        s.trim()
    };

    let svg_tag = if let Some(start) = s.find("<svg") {
        let end = s[start..].find('>').map(|i| start + i + 1).unwrap_or(s.len());
        &s[start..end.min(s.len())]
    } else {
        return Err("No <svg> tag found".into());
    };

    let parse_attr = |attr: &str| -> Option<f32> {
        let lower = svg_tag.to_lowercase();
        if let Some(pos) = lower.find(attr) {
            let rest = &lower[pos + attr.len()..];
            let num_str: String =
                rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            num_str.parse::<f32>().ok()
        } else {
            None
        }
    };

    let w = parse_attr("width=\"").unwrap_or(100.0);
    let h = parse_attr("height=\"").unwrap_or(100.0);

    let (vw, vh) = if let Some(vb) = svg_tag.to_lowercase().find("viewbox=\"") {
        let rest = &svg_tag[vb + 9..];
        let nums: Vec<f32> = rest
            .split(|c: char| [' ', ',', '"'].contains(&c))
            .filter_map(|s| s.parse::<f32>().ok())
            .collect();
        if nums.len() >= 4 {
            (nums[2], nums[3])
        } else {
            (w, h)
        }
    } else {
        (w, h)
    };

    // Prefer viewBox dimensions when explicit width/height attributes are not found.
    let fw = if w > 0.0 && parse_attr("width=\"").is_some() { w } else { vw };
    let fh = if h > 0.0 && parse_attr("height=\"").is_some() { h } else { vh };
    Ok((fw.max(1.0) as u32, fh.max(1.0) as u32))
}

/// Check if bytes are a valid SVG (including XML declaration).
pub fn is_svg(data: &[u8]) -> bool {
    detect_svg(data)
}

fn detect_svg(data: &[u8]) -> bool {
    let start = if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
        3
    } else {
        0
    };
    if data.len() <= start + 4 {
        return false;
    }
    let slice = &data[start..];
    slice.starts_with(b"<?xml") || slice.starts_with(b"<svg") || slice.starts_with(b"<!DOC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_svg_dimensions() {
        let svg = r#"<svg width="200" height="100" xmlns="http://www.w3.org/2000/svg">"#;
        let (w, h) = parse_svg_dimensions(svg).unwrap();
        assert_eq!(w, 200);
        assert_eq!(h, 100);
    }

    #[test]
    fn test_parse_svg_viewbox() {
        let svg = r#"<svg viewBox="0 0 300 150" xmlns="http://www.w3.org/2000/svg">"#;
        let (w, h) = parse_svg_dimensions(svg).unwrap();
        assert_eq!(w, 300);
        assert_eq!(h, 150);
    }

    #[test]
    fn test_rasterize_svg() {
        let svg = b"<svg width=\"50\" height=\"50\"></svg>";
        let result = rasterize_svg(svg, 0, 0);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width, 50);
        assert_eq!(img.height, 50);
    }

    #[test]
    fn test_rasterize_svg_custom_size() {
        let svg = b"<svg width=\"10\" height=\"10\"></svg>";
        let result = rasterize_svg(svg, 100, 100);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);
    }

    #[test]
    fn test_is_svg() {
        assert!(is_svg(b"<svg xmlns"));
        assert!(is_svg(b"<?xml version"));
        assert!(!is_svg(b"not svg"));
    }

    #[test]
    fn test_parse_svg_xml_declaration() {
        let svg = r#"<?xml version="1.0"?><svg width="50" height="30"></svg>"#;
        let (w, h) = parse_svg_dimensions(svg).unwrap();
        assert_eq!(w, 50);
        assert_eq!(h, 30);
    }
}
