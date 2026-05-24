//! Source-map v3 emission for generated `.d.ts` files.

/// One generated-to-source mapping. All positions are 0-indexed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mapping {
    pub gen_line: u32,
    pub gen_col: u32,
    pub src_idx: u32,
    pub src_line: u32,
    pub src_col: u32,
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode a signed integer as a VLQ base64 string and append it to `out`.
fn encode_vlq(out: &mut String, value: i32) {
    let mut vlq: u32 = if value < 0 {
        ((-(value as i64) as u32) << 1) | 1
    } else {
        (value as u32) << 1
    };
    loop {
        let mut digit = (vlq & 0x1f) as u8;
        vlq >>= 5;
        if vlq != 0 {
            digit |= 0x20;
        }
        out.push(BASE64[digit as usize] as char);
        if vlq == 0 {
            break;
        }
    }
}

/// Encode a list of mappings into the `mappings` string.
pub fn serialize_mappings(mappings: &[Mapping]) -> String {
    let mut sorted: Vec<&Mapping> = mappings.iter().collect();
    sorted.sort_by_key(|m| (m.gen_line, m.gen_col));

    let mut out = String::new();
    let mut cur_line: u32 = 0;
    let mut prev_gen_col: i32 = 0;
    let mut prev_src_idx: i32 = 0;
    let mut prev_src_line: i32 = 0;
    let mut prev_src_col: i32 = 0;
    let mut at_line_start = true;

    for m in sorted {
        while cur_line < m.gen_line {
            out.push(';');
            cur_line += 1;
            prev_gen_col = 0;
            at_line_start = true;
        }
        if !at_line_start {
            out.push(',');
        }
        encode_vlq(&mut out, m.gen_col as i32 - prev_gen_col);
        encode_vlq(&mut out, m.src_idx as i32 - prev_src_idx);
        encode_vlq(&mut out, m.src_line as i32 - prev_src_line);
        encode_vlq(&mut out, m.src_col as i32 - prev_src_col);
        prev_gen_col = m.gen_col as i32;
        prev_src_idx = m.src_idx as i32;
        prev_src_line = m.src_line as i32;
        prev_src_col = m.src_col as i32;
        at_line_start = false;
    }
    out
}

/// Build a source-map v3 JSON document.
///
/// `file` is the name of the generated artifact (the `.d.ts` filename, no
/// directory). `sources` are paths relative to the `.d.ts.map` file.
pub fn write_source_map(file: &str, sources: &[String], mappings: &[Mapping]) -> String {
    let mappings_str = serialize_mappings(mappings);
    let mut out = String::new();
    out.push_str("{\"version\":3,\"file\":\"");
    push_json_string(&mut out, file);
    out.push_str("\",\"sources\":[");
    for (i, s) in sources.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        push_json_string(&mut out, s);
        out.push('"');
    }
    out.push_str("],\"names\":[],\"mappings\":\"");
    out.push_str(&mappings_str);
    out.push_str("\"}");
    out
}

fn push_json_string(out: &mut String, s: &str) {
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vlq_round_trip_values() {
        let cases: &[(i32, &str)] = &[
            (0, "A"),
            (1, "C"),
            (-1, "D"),
            (15, "e"),
            (16, "gB"),
            (-16, "hB"),
            (1000, "w+B"),
            (-1000, "x+B"),
        ];
        for (v, expected) in cases {
            let mut s = String::new();
            encode_vlq(&mut s, *v);
            assert_eq!(s, *expected, "encoding {v}");
        }
    }

    #[test]
    fn serialize_mappings_simple() {
        // Three mappings: line 0 col 0 -> 0,0,0 ; line 1 col 4 -> 0,1,2 ;
        // line 1 col 8 -> 0,1,6.
        let mappings = vec![
            Mapping {
                gen_line: 0,
                gen_col: 0,
                src_idx: 0,
                src_line: 0,
                src_col: 0,
            },
            Mapping {
                gen_line: 1,
                gen_col: 4,
                src_idx: 0,
                src_line: 1,
                src_col: 2,
            },
            Mapping {
                gen_line: 1,
                gen_col: 8,
                src_idx: 0,
                src_line: 1,
                src_col: 6,
            },
        ];
        let s = serialize_mappings(&mappings);
        assert_eq!(s, "AAAA;IACE,IAAI");
    }

    #[test]
    fn write_source_map_byte_equality() {
        let mappings = vec![Mapping {
            gen_line: 0,
            gen_col: 0,
            src_idx: 0,
            src_line: 0,
            src_col: 0,
        }];
        let s = write_source_map("foo.rs.d.ts", &["../src/foo.rs".to_string()], &mappings);
        assert_eq!(
            s,
            "{\"version\":3,\"file\":\"foo.rs.d.ts\",\"sources\":[\"../src/foo.rs\"],\"names\":[],\"mappings\":\"AAAA\"}"
        );
    }

    #[test]
    fn write_source_map_escapes_quotes_and_backslashes() {
        let s = write_source_map("a\"b\\c.d.ts", &["x\"y\\z.rs".to_string()], &[]);
        assert_eq!(
            s,
            "{\"version\":3,\"file\":\"a\\\"b\\\\c.d.ts\",\"sources\":[\"x\\\"y\\\\z.rs\"],\"names\":[],\"mappings\":\"\"}"
        );
    }
}
