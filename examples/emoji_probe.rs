use rust_widgets::render::text::{paint_active, Cell, ColorBitmapFace};

fn main() {
    let bytes = rust_widgets::render::text::font_assets::EMOJI;
    let cbf = ColorBitmapFace::parse(bytes).expect("colour tables parse");
    let face = ttf_parser::Face::parse(bytes, 0).expect("ttf-parser face");
    let mut cmap = std::collections::BTreeMap::new();
    for cp in 0u32..=0x10FFFF {
        if let Some(ch) = char::from_u32(cp) {
            if let Some(gid) = face.glyph_index(ch) {
                cmap.entry(gid.0).or_insert(cp);
            }
        }
    }
    println!("cmap entries: {}", cmap.len());
    for (gid, cp) in cmap.iter().take(40) {
        let img = cbf.image(*gid);
        println!("  gid={gid} U+{cp:04X} image={:?}", img.map(|i| (i.offset, i.length)));
    }

    for ch in ['\u{1F1E6}', '\u{1F1E8}', '\u{1F1FA}', 'A'] {
        let cell = Cell::new(32, 32);
        let mut out = vec![0u8; cell.area() * 4];
        match paint_active(ch, cell, &mut out) {
            Some(p) => {
                let opaque = out.chunks_exact(4).filter(|px| px[3] != 0).count();
                let colours: std::collections::HashSet<(u8, u8, u8)> = out
                    .chunks_exact(4)
                    .filter(|px| px[3] != 0)
                    .map(|px| (px[0], px[1], px[2]))
                    .collect();
                println!(
                    "U+{:04X} -> source={} ink={:?} colour={} opaque={opaque} distinct={}",
                    ch as u32,
                    p.source,
                    p.ink,
                    p.is_color(),
                    colours.len()
                );
            }
            None => println!("U+{:04X} -> not painted", ch as u32),
        }
    }
}
