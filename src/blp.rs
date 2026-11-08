//! BLP2, the game's texture container: the first mip level as straight RGBA8.

const HEADER: usize = 148;
const PALETTE: usize = 256 * 4;

/// The first mip of a BLP2 file as `(width, height, rgba)`. Reads palettes with 0, 1, 4 or 8
/// bits of alpha, DXT1, DXT3 and DXT5 blocks, and raw BGRA.
pub fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.len() < HEADER || &bytes[..4] != b"BLP2" {
        return Err("not a BLP2 file".into());
    }
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"));
    let (encoding, alpha_depth, preferred) = (bytes[8], bytes[9], bytes[10]);
    let (w, h) = (u32_at(12), u32_at(16));
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return Err(format!("unlikely size {w}x{h}"));
    }
    let (offset, size) = (u32_at(20) as usize, u32_at(84) as usize);
    let data = bytes.get(offset..offset.saturating_add(size)).ok_or("mip 0 runs past the end of the file")?;
    let (w, h) = (w as usize, h as usize);
    let rgba = match encoding {
        1 => palette(data, &bytes[HEADER..HEADER + PALETTE], w, h, alpha_depth)?,
        2 => {
            let kind = match (alpha_depth, preferred) {
                (0 | 1, _) => Dxt::One { alpha: alpha_depth != 0 },
                (_, 7) => Dxt::Five,
                _ => Dxt::Three,
            };
            dxt(data, w, h, kind)?
        }
        3 => {
            let raw = data.get(..w * h * 4).ok_or("raw pixels run short")?;
            raw.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()
        }
        e => return Err(format!("unknown encoding {e}")),
    };
    Ok((w as u32, h as u32, rgba))
}

fn palette(data: &[u8], pal: &[u8], w: usize, h: usize, alpha_depth: u8) -> Result<Vec<u8>, String> {
    let n = w * h;
    let alpha_len = (n * alpha_depth as usize).div_ceil(8);
    if data.len() < n + alpha_len {
        return Err("palette indices run short".into());
    }
    let alphas = &data[n..];
    Ok((0..n)
        .flat_map(|i| {
            let c = &pal[data[i] as usize * 4..][..4];
            let a = match alpha_depth {
                8 => alphas[i],
                4 => ((alphas[i / 2] >> ((i & 1) * 4)) & 15) * 17,
                1 => (alphas[i / 8] >> (i & 7) & 1) * 255,
                _ => 255,
            };
            [c[2], c[1], c[0], a]
        })
        .collect())
}

#[derive(Clone, Copy)]
enum Dxt {
    /// `alpha` says whether the punch-through colour is transparent.
    One { alpha: bool },
    Three,
    Five,
}

fn dxt(data: &[u8], w: usize, h: usize, kind: Dxt) -> Result<Vec<u8>, String> {
    let block = if matches!(kind, Dxt::One { .. }) { 8 } else { 16 };
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    if data.len() < bw * bh * block {
        return Err("DXT blocks run short".into());
    }
    let mut out = vec![0; w * h * 4];
    for (b, src) in data.chunks_exact(block).take(bw * bh).enumerate() {
        let (alphas, colors) = match kind {
            Dxt::One { alpha } => (None, colors(src, true, alpha)),
            Dxt::Three => (Some(std::array::from_fn(|i| ((src[i / 2] >> ((i & 1) * 4)) & 15) * 17)), colors(&src[8..], false, false)),
            Dxt::Five => (Some(alpha_block(src)), colors(&src[8..], false, false)),
        };
        for i in 0..16 {
            let (x, y) = ((b % bw) * 4 + (i & 3), (b / bw) * 4 + (i >> 2));
            if x < w && y < h {
                let mut px = colors[i];
                if let Some(a) = alphas {
                    px[3] = a[i];
                }
                out[(y * w + x) * 4..][..4].copy_from_slice(&px);
            }
        }
    }
    Ok(out)
}

/// A block's sixteen colours from its two RGB565 endpoints and two-bit indices. DXT1 with
/// `c0 <= c1` mixes only halfway and makes index 3 black, transparent when `punch` is set.
fn colors(src: &[u8], dxt1: bool, punch: bool) -> [[u8; 4]; 16] {
    let c0 = u16::from_le_bytes([src[0], src[1]]);
    let c1 = u16::from_le_bytes([src[2], src[3]]);
    let rgb = |c: u16| [f32::from((c >> 11) & 31) * 255.0 / 31.0, f32::from((c >> 5) & 63) * 255.0 / 63.0, f32::from(c & 31) * 255.0 / 31.0];
    let (a, b) = (rgb(c0), rgb(c1));
    let mix = |wa: f32, wb: f32| std::array::from_fn::<f32, 3, _>(|i| (wa * a[i] + wb * b[i]) / (wa + wb));
    let opaque = |c: [f32; 3]| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8, 255];
    let table = if c0 > c1 || !dxt1 {
        [opaque(a), opaque(b), opaque(mix(2.0, 1.0)), opaque(mix(1.0, 2.0))]
    } else {
        [opaque(a), opaque(b), opaque(mix(1.0, 1.0)), [0, 0, 0, if punch { 0 } else { 255 }]]
    };
    let bits = u32::from_le_bytes([src[4], src[5], src[6], src[7]]);
    std::array::from_fn(|i| table[(bits >> (2 * i) & 3) as usize])
}

/// DXT5's sixteen alphas: two endpoints, then three-bit indices into eight steps between them.
fn alpha_block(src: &[u8]) -> [u8; 16] {
    let (a0, a1) = (f32::from(src[0]), f32::from(src[1]));
    let mut steps = [a0, a1, 0.0, 0.0, 0.0, 0.0, 0.0, 255.0];
    if a0 > a1 {
        for (i, s) in steps.iter_mut().enumerate().skip(2) {
            let t = (i - 1) as f32;
            *s = ((7.0 - t) * a0 + t * a1) / 7.0;
        }
    } else {
        for (i, s) in steps.iter_mut().enumerate().take(6).skip(2) {
            let t = (i - 1) as f32;
            *s = ((5.0 - t) * a0 + t * a1) / 5.0;
        }
    }
    let bits = src[2..8].iter().rev().fold(0u64, |acc, &b| acc << 8 | u64::from(b));
    std::array::from_fn(|i| steps[(bits >> (3 * i) & 7) as usize].round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BLP2 header for a `w`×`h` image whose mip 0 is `data`, right after the palette.
    fn blp(encoding: u8, alpha_depth: u8, preferred: u8, w: u32, h: u32, palette: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = vec![0; HEADER + PALETTE];
        out[..4].copy_from_slice(b"BLP2");
        out[4] = 1;
        out[8..11].copy_from_slice(&[encoding, alpha_depth, preferred]);
        out[12..16].copy_from_slice(&w.to_le_bytes());
        out[16..20].copy_from_slice(&h.to_le_bytes());
        out[20..24].copy_from_slice(&((HEADER + PALETTE) as u32).to_le_bytes());
        out[84..88].copy_from_slice(&(data.len() as u32).to_le_bytes());
        out[HEADER..HEADER + palette.len()].copy_from_slice(palette);
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn swaps_raw_bgra_to_rgba() {
        let file = blp(3, 8, 8, 2, 1, &[], &[10, 20, 30, 40, 0, 0, 255, 128]);
        assert_eq!(decode(&file), Ok((2, 1, vec![30, 20, 10, 40, 255, 0, 0, 128])));
    }

    #[test]
    fn looks_palette_indices_up_with_their_alpha() {
        let file = blp(1, 8, 0, 2, 1, &[1, 2, 3, 0, 200, 100, 50, 0], &[1, 0, 77, 255]);
        assert_eq!(decode(&file), Ok((2, 1, vec![50, 100, 200, 77, 3, 2, 1, 255])));
    }

    #[test]
    fn decodes_a_dxt5_block_with_interpolated_alpha() {
        // Alphas 255 and 0, the first row on alpha steps 0, 1, 2 and 7; colours red (0xF800) and
        // blue (0x001F), the first row on colour steps 0, 1, 2 and 3.
        let alpha = [255, 0, 0b10_001_000, 0b1110, 0, 0, 0, 0];
        let color = [0x00, 0xF8, 0x1F, 0x00, 0b11_10_01_00, 0, 0, 0];
        let file = blp(2, 8, 7, 4, 4, &[], &[alpha, color].concat());
        let (w, h, rgba) = decode(&file).unwrap();
        assert_eq!((w, h), (4, 4));
        assert_eq!(&rgba[..16], &[255, 0, 0, 255, 0, 0, 255, 0, 170, 0, 85, 219, 85, 0, 170, 36]);
        assert_eq!(&rgba[16..20], &[255, 0, 0, 255]);
    }

    #[test]
    fn makes_dxt1_punch_through_transparent() {
        // c0 <= c1 selects the three-colour table; index 3 is transparent black.
        let file = blp(2, 1, 0, 4, 4, &[], &[0x1F, 0x00, 0x00, 0xF8, 0b11_10_01_00, 0, 0, 0]);
        let (_, _, rgba) = decode(&file).unwrap();
        assert_eq!(&rgba[..16], &[0, 0, 255, 255, 255, 0, 0, 255, 128, 0, 128, 255, 0, 0, 0, 0]);
    }

    #[test]
    fn rejects_what_it_cannot_read() {
        assert_eq!(decode(b"PNG"), Err("not a BLP2 file".into()));
        assert_eq!(decode(&blp(3, 8, 8, 2, 2, &[], &[0; 8])), Err("raw pixels run short".into()));
        assert_eq!(decode(&blp(9, 8, 8, 1, 1, &[], &[0; 4])), Err("unknown encoding 9".into()));
    }
}
