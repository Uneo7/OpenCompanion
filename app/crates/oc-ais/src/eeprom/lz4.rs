//! LZ4 block format (lz4/doc/lz4_Block_format.md): optimal-parse hash-chain encoder and a reference decoder with
//! the same checks as boot1 (`boot1/src/lz4.c`).

const MIN_MATCH: usize = 4;
const MAX_OFFSET: usize = 0xFFFF;
/// The format asks for the last 5 bytes to be literals and the last match to start >= 12 bytes before the end.
const LAST_LITERALS: usize = 5;
const MF_LIMIT: usize = 12;
const HASH_BITS: u32 = 14;
const CHAIN: usize = 4096;

fn hash(b: &[u8]) -> usize {
    let v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
}

fn put_len(out: &mut Vec<u8>, mut n: usize) {
    while n >= 255 {
        out.push(255);
        n -= 255;
    }
    out.push(n as u8);
}

fn sequence(out: &mut Vec<u8>, lits: &[u8], m: Option<(usize, usize)>) {
    let ll = lits.len();
    let ml = m.map_or(0, |(_, l)| l - MIN_MATCH);
    let tok = ((ll.min(15) as u8) << 4) | if m.is_some() { ml.min(15) as u8 } else { 0 };
    out.push(tok);
    if ll >= 15 {
        put_len(out, ll - 15);
    }
    out.extend_from_slice(lits);
    if let Some((off, _)) = m {
        out.extend_from_slice(&(off as u16).to_le_bytes());
        if ml >= 15 {
            put_len(out, ml - 15);
        }
    }
}

/// Hash chains over `src` (positions with at least MIN_MATCH bytes left).
struct Chains {
    head: Vec<usize>,
    prev: Vec<usize>,
    next: usize,
}

impl Chains {
    fn new(n: usize) -> Self {
        Chains { head: vec![usize::MAX; 1 << HASH_BITS], prev: vec![usize::MAX; n], next: 0 }
    }
    fn insert_to(&mut self, src: &[u8], to: usize) {
        while self.next < to {
            let i = self.next;
            if i + MIN_MATCH <= src.len() {
                let h = hash(&src[i..]);
                self.prev[i] = self.head[h];
                self.head[h] = i;
            }
            self.next += 1;
        }
    }
}

/// Encoded size of a match of length `ml` (token, offset, extra length bytes).
fn match_cost(ml: usize) -> u32 {
    let e = ml - MIN_MATCH;
    3 + if e >= 15 { 1 + (e - 15) as u32 / 255 } else { 0 }
}

/// Compresses `src` into one LZ4 block (empty for an empty input: the decoder reads nothing for raw = 0).
/// Optimal parse (shortest output for the cost model "literal = 1 byte, match = token + offset + extra
/// length bytes") over hash-chain candidates; the output is a standard LZ4 block, decoded by boot1 as before.
/// A match of SUFFICIENT bytes or more is taken at once (zero fills, long repeats) to keep the time linear.
pub fn compress(src: &[u8]) -> Vec<u8> {
    const SUFFICIENT: usize = 1024;
    let n = src.len();
    let mut out = Vec::with_capacity(n / 2 + 16);
    if n == 0 {
        return out;
    }
    let limit = n.saturating_sub(MF_LIMIT);
    // price[i]: smallest cost to encode src[..i]; step[i]: (match length, offset) reaching i, length 1 = literal.
    let mut price = vec![u32::MAX; n + 1];
    let mut step = vec![(0usize, 0usize); n + 1];
    // lits[i]: length of the literal run ending at i on the chosen path (0 if a match ends there); a run of
    // 15, 270, 525... literals costs one more length byte.
    let mut lits = vec![0usize; n + 1];
    let mut ch = Chains::new(n);
    price[0] = 0;
    let mut i = 0usize;
    while i < n {
        let p = price[i];
        let run = lits[i] + 1;
        let lit = p.saturating_add(1 + u32::from(run >= 15 && (run - 15) % 255 == 0));
        if lit < price[i + 1] {
            price[i + 1] = lit;
            step[i + 1] = (1, 0);
            lits[i + 1] = run;
        }
        if i >= limit {
            i += 1;
            continue;
        }
        ch.insert_to(src, i);
        let max_len = n - LAST_LITERALS - i;
        let mut c = ch.head[hash(&src[i..])];
        let (mut best, mut best_off) = (0usize, 0usize);
        let mut depth = 0;
        while c != usize::MAX && i - c <= MAX_OFFSET && depth < CHAIN {
            depth += 1;
            let l = src[c..].iter().zip(&src[i..i + max_len]).take_while(|(a, b)| a == b).count();
            if l > best && l >= MIN_MATCH {
                // every length from the previous best + 1 up to l is reachable with this (closer) offset
                for ml in (best + 1).max(MIN_MATCH)..=l {
                    let q = p + match_cost(ml);
                    if q < price[i + ml] {
                        price[i + ml] = q;
                        step[i + ml] = (ml, i - c);
                        lits[i + ml] = 0;
                    }
                }
                best = l;
                best_off = i - c;
                if l >= SUFFICIENT || l == max_len {
                    break;
                }
            }
            c = ch.prev[c];
        }
        if best >= SUFFICIENT {
            // take it: positions inside the match are not starting points
            ch.insert_to(src, i + best);
            price[i + best] = p + match_cost(best);
            step[i + best] = (best, best_off);
            lits[i + best] = 0;
            i += best;
            continue;
        }
        i += 1;
    }
    let mut seq = Vec::new();
    let mut j = n;
    while j > 0 {
        let (l, off) = step[j];
        seq.push((j - l, l, off));
        j -= l;
    }
    seq.reverse();
    let mut anchor = 0usize;
    for (at, l, off) in seq {
        if l >= MIN_MATCH {
            sequence(&mut out, &src[anchor..at], Some((off, l)));
            anchor = at + l;
        }
    }
    sequence(&mut out, &src[anchor..], None);
    out
}

/// Decodes exactly `raw` bytes; `None` on malformed input (same rules as boot1). Returns the bytes and the
/// number of input bytes consumed.
pub fn decompress(src: &[u8], raw: usize) -> Option<(Vec<u8>, usize)> {
    let mut out = Vec::with_capacity(raw);
    let mut p = 0usize;
    let mut next = || -> Option<usize> {
        let b = *src.get(p)?;
        p += 1;
        Some(b as usize)
    };
    fn len(first: usize, next: &mut dyn FnMut() -> Option<usize>) -> Option<usize> {
        let mut l = first;
        if l == 15 {
            loop {
                let c = next()?;
                l += c;
                if c != 255 {
                    break;
                }
            }
        }
        Some(l)
    }
    while out.len() < raw {
        let tok = next()?;
        let lit = len(tok >> 4, &mut next)?;
        if lit > raw - out.len() {
            return None;
        }
        for _ in 0..lit {
            out.push(next()? as u8);
        }
        if out.len() == raw {
            break;
        }
        let off = next()? | (next()? << 8);
        let ml = len(tok & 15, &mut next)? + MIN_MATCH;
        if off == 0 || off > out.len() || ml > raw - out.len() {
            return None;
        }
        for _ in 0..ml {
            out.push(out[out.len() - off]);
        }
    }
    drop(next);
    Some((out, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(data: &[u8]) {
        let c = compress(data);
        let (d, used) = decompress(&c, data.len()).expect("decode");
        assert_eq!(d, data);
        assert_eq!(used, c.len());
    }

    #[test]
    fn round_trips() {
        round(&[]);
        round(b"a");
        round(b"abcdabcdabcdabcdabcdabcdabcdabcd0123456789");
        round(&vec![0u8; 5000]);
        let mut x = 12345u32;
        let noise: Vec<u8> = (0..20000)
            .map(|i| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                if i % 7 < 3 { (x >> 16) as u8 } else { (i % 13) as u8 }
            })
            .collect();
        round(&noise);
    }

    #[test]
    fn rejects_bad_offset() {
        // token: 1 literal, match length 4; offset 2 > 1 byte produced
        assert!(decompress(&[0x10, b'x', 2, 0], 10).is_none());
    }
}
