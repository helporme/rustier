//! Encodes TypeScript declaration source map mappings.

use std::collections::BTreeMap;

use crate::compiler::{db::Db, ir::rust::Span};

/// A single source map mapping before source and name indexes are assigned.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Mapping {
    pub generated_line: usize,
    pub generated_column: usize,
    pub source: String,
    pub original_line: usize,
    pub original_column: usize,
    pub name: String,
}

pub(super) struct CompiledMappings {
    pub sources: Vec<String>,
    pub names: Vec<String>,
    pub encoded: String,
}

/// A zero-based source location resolved from an IR span.
pub(super) struct OriginalLocation {
    pub source: String,
    pub line: usize,
    pub column: usize,
}

/// Resolves the beginning of an IR span into a source-map location.
pub(super) fn original_location(db: &dyn Db, span: &Span) -> OriginalLocation {
    let source = span.src(db);
    let prefix = &source[..span.offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len(), |(_, line)| line.len());

    OriginalLocation {
        source: span
            .src_path
            .as_path()
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/"),
        line,
        column,
    }
}

/// Finds the next generated declaration whose name has exact token boundaries.
pub(super) fn find_name_line(
    source: &str,
    start_line: usize,
    prefix: &str,
    name: &str,
    suffix: &str,
) -> Option<usize> {
    source
        .lines()
        .enumerate()
        .skip(start_line)
        .find(|(_, line)| {
            line.strip_prefix(prefix)
                .and_then(|line| line.strip_prefix(name))
                .is_some_and(|line| line.starts_with(suffix))
        })
        .map(|(line, _)| line)
}

struct IndexedMapping {
    generated_line: usize,
    generated_column: usize,
    source: usize,
    original_line: usize,
    original_column: usize,
    name: usize,
}

pub(super) fn compile_mappings(mappings: &[Mapping]) -> CompiledMappings {
    let mut sources = Vec::new();
    let mut names = Vec::new();
    let mut source_indices = BTreeMap::new();
    let mut name_indices = BTreeMap::new();
    let mut indexed_mappings = Vec::with_capacity(mappings.len());

    for mapping in mappings {
        let source = match source_indices.get(&mapping.source) {
            Some(index) => *index,
            None => {
                let index = sources.len();
                sources.push(mapping.source.clone());
                source_indices.insert(mapping.source.clone(), index);
                index
            }
        };
        let name = match name_indices.get(&mapping.name) {
            Some(index) => *index,
            None => {
                let index = names.len();
                names.push(mapping.name.clone());
                name_indices.insert(mapping.name.clone(), index);
                index
            }
        };
        indexed_mappings.push(IndexedMapping {
            generated_line: mapping.generated_line,
            generated_column: mapping.generated_column,
            source,
            original_line: mapping.original_line,
            original_column: mapping.original_column,
            name,
        });
    }

    CompiledMappings {
        sources,
        names,
        encoded: encode_mappings(&indexed_mappings),
    }
}

fn encode_mappings(mappings: &[IndexedMapping]) -> String {
    let Some(last_mapping) = mappings.last() else {
        return String::new();
    };

    let mut out = String::new();
    let mut previous_source = 0;
    let mut previous_original_line = 0;
    let mut previous_original_column = 0;
    let mut previous_name = 0;
    let mut mapping_index = 0;

    for line in 0..=last_mapping.generated_line {
        if line != 0 {
            out.push(';');
        }

        let mut previous_generated_column = 0;
        let mut line_mapping_count = 0;
        while mapping_index < mappings.len() && mappings[mapping_index].generated_line == line {
            let mapping = &mappings[mapping_index];
            if line_mapping_count > 0 {
                out.push(',');
            }

            encode_vlq(
                mapping.generated_column as i64 - previous_generated_column as i64,
                &mut out,
            );
            encode_vlq(mapping.source as i64 - previous_source as i64, &mut out);
            encode_vlq(
                mapping.original_line as i64 - previous_original_line as i64,
                &mut out,
            );
            encode_vlq(
                mapping.original_column as i64 - previous_original_column as i64,
                &mut out,
            );
            encode_vlq(mapping.name as i64 - previous_name as i64, &mut out);

            previous_generated_column = mapping.generated_column;
            previous_source = mapping.source;
            previous_original_line = mapping.original_line;
            previous_original_column = mapping.original_column;
            previous_name = mapping.name;
            mapping_index += 1;
            line_mapping_count += 1;
        }
    }

    out
}

fn encode_vlq(value: i64, out: &mut String) {
    const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const VLQ_BASE_SHIFT: u64 = 5;
    const VLQ_BASE: u64 = 1 << VLQ_BASE_SHIFT;
    const VLQ_BASE_MASK: u64 = VLQ_BASE - 1;
    const VLQ_CONTINUATION_BIT: u64 = VLQ_BASE;

    let mut value = if value < 0 {
        ((-value as u64) << 1) + 1
    } else {
        (value as u64) << 1
    };

    loop {
        let mut digit = value & VLQ_BASE_MASK;
        value >>= VLQ_BASE_SHIFT;
        if value > 0 {
            digit |= VLQ_CONTINUATION_BIT;
        }

        out.push(BASE64[digit as usize] as char);

        if value == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_empty_mappings() {
        let mappings = compile_mappings(&[]);

        assert_eq!(mappings.encoded, "");
        assert!(mappings.sources.is_empty());
        assert!(mappings.names.is_empty());
    }

    #[test]
    fn encodes_relative_segment_deltas() {
        let mappings = [
            Mapping {
                generated_line: 0,
                generated_column: 0,
                source: "a.rs".to_string(),
                original_line: 0,
                original_column: 0,
                name: "first".to_string(),
            },
            Mapping {
                generated_line: 0,
                generated_column: 10,
                source: "a.rs".to_string(),
                original_line: 0,
                original_column: 5,
                name: "second".to_string(),
            },
            Mapping {
                generated_line: 1,
                generated_column: 0,
                source: "b.rs".to_string(),
                original_line: 2,
                original_column: 0,
                name: "second".to_string(),
            },
        ];

        assert_eq!(compile_mappings(&mappings).encoded, "AAAAA,UAAKC;ACELA");
    }

    #[test]
    fn compiles_sources_and_names_in_first_seen_order() {
        let mappings = [
            Mapping {
                generated_line: 0,
                generated_column: 0,
                source: "b.rs".to_string(),
                original_line: 0,
                original_column: 0,
                name: "second".to_string(),
            },
            Mapping {
                generated_line: 1,
                generated_column: 0,
                source: "a.rs".to_string(),
                original_line: 1,
                original_column: 0,
                name: "first".to_string(),
            },
            Mapping {
                generated_line: 2,
                generated_column: 0,
                source: "b.rs".to_string(),
                original_line: 2,
                original_column: 0,
                name: "second".to_string(),
            },
        ];
        let compiled = compile_mappings(&mappings);

        assert_eq!(compiled.sources, ["b.rs", "a.rs"]);
        assert_eq!(compiled.names, ["second", "first"]);
    }
}
