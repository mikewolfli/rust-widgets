use rust_widgets::render::text::{paint_active, Cell};

fn dump(ch: char, w: u32, h: u32) {
    let cell = Cell::new(w, h);
    let mut out = vec![0u8; cell.area()];
    let p = paint_active(ch, cell, &mut out).expect("covered");
    println!("--- {ch:?} cell {w}x{h} source={} ink={:?} max={}", p.source, p.ink,
             out.iter().copied().max().unwrap_or(0));
    let ramp = " .:-=+*#%@";
    for y in 0..h {
        let mut line = String::new();
        for x in 0..w {
            let v = out[(y * w + x) as usize];
            let idx = (v as usize * (ramp.len() - 1)) / 255;
            line.push(ramp.chars().nth(idx).unwrap());
        }
        println!("|{line}|");
    }
}

fn main() {
    dump('A', 24, 24);
    dump('o', 24, 24);
}
