//! Lightweight, *textual* analysis of STEP/STP files.
//!
//! This is intentionally **not** a CAD parser: it never interprets geometry or
//! B-Rep topology. It only does cheap string scanning to surface useful summary
//! metadata (schema, entity counts, top entity types) for `log`/`diff`/`status`.
//! Deep geometric understanding is deferred to a later Open CASCADE stage.

use serde::{Deserialize, Serialize};

/// A `(entity_type, count)` pair, used for the "top entity types" summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityTypeCount {
    pub entity_type: String,
    pub count: u64,
}

/// Summary metadata extracted from a STEP file via text scanning only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StepMetadata {
    /// The schema named in `FILE_SCHEMA((...))`, if found.
    pub file_schema: Option<String>,
    /// Number of entity definitions (`#N = TYPE(...)`) in the DATA section.
    pub entity_count: Option<u64>,
    /// Line count of the HEADER section.
    pub header_line_count: Option<u64>,
    /// Line count of the DATA section.
    pub data_line_count: Option<u64>,
    /// The most frequent entity types (up to 20), descending by count.
    pub top_entity_types: Vec<EntityTypeCount>,
}

/// How many entity types to keep in the "top types" list.
const TOP_N: usize = 20;

/// Extract [`StepMetadata`] from raw STEP file bytes.
///
/// Returns `None` only if the bytes are not valid UTF-8 (STEP files are ASCII
/// text); otherwise it always returns a best-effort summary, never panics.
pub fn extract(content: &[u8]) -> Option<StepMetadata> {
    let text = std::str::from_utf8(content).ok()?;
    Some(extract_str(text))
}

/// Like [`extract`] but operates on an already-decoded string.
pub fn extract_str(text: &str) -> StepMetadata {
    let file_schema = find_file_schema(text);
    let (header_line_count, data_line_count) = section_line_counts(text);
    let data = data_section(text).unwrap_or("");
    let (entity_count, top_entity_types) = scan_entities(data);

    StepMetadata {
        file_schema,
        entity_count: Some(entity_count),
        header_line_count,
        data_line_count,
        top_entity_types,
    }
}

/// Locate a case-insensitive keyword (e.g. `HEADER;`) and return its line index.
fn find_keyword_line(text: &str, keyword: &str) -> Option<usize> {
    let kw = keyword.to_ascii_uppercase();
    text.lines()
        .position(|line| line.trim().to_ascii_uppercase().starts_with(&kw))
}

/// Approximate HEADER and DATA section line counts (between their `SEC` markers
/// and the following `ENDSEC;`).
fn section_line_counts(text: &str) -> (Option<u64>, Option<u64>) {
    let lines: Vec<&str> = text.lines().collect();
    let count_section = |start_kw: &str| -> Option<u64> {
        let start = find_keyword_line(text, start_kw)?;
        let mut count = 0u64;
        for line in lines.iter().skip(start + 1) {
            if line.trim().to_ascii_uppercase().starts_with("ENDSEC") {
                break;
            }
            count += 1;
        }
        Some(count)
    };
    (count_section("HEADER"), count_section("DATA"))
}

/// Extract the text of the DATA section (between `DATA;` and the next `ENDSEC;`).
fn data_section(text: &str) -> Option<&str> {
    let upper = text.to_ascii_uppercase();
    let data_pos = upper.find("DATA;")?;
    let after = data_pos + "DATA;".len();
    let end_rel = upper[after..].find("ENDSEC").unwrap_or(upper.len() - after);
    Some(&text[after..after + end_rel])
}

/// Find the schema string inside `FILE_SCHEMA(('...'))`.
fn find_file_schema(text: &str) -> Option<String> {
    let upper = text.to_ascii_uppercase();
    let key = "FILE_SCHEMA";
    let pos = upper.find(key)?;
    // Take the slice after the keyword and pull out the first single-quoted token.
    let rest = &text[pos + key.len()..];
    let start = rest.find('\'')?;
    let after = &rest[start + 1..];
    let end = after.find('\'')?;
    let schema = after[..end].trim();
    if schema.is_empty() {
        None
    } else {
        Some(schema.to_string())
    }
}

/// Scan a DATA section for entity definitions of the form `#N = TYPE(`.
///
/// Returns the total count and the top-N most frequent entity types. Bare `#N`
/// *references* inside other entities are ignored because they are not followed
/// by `= TYPE(`.
fn scan_entities(data: &str) -> (u64, Vec<EntityTypeCount>) {
    use std::collections::HashMap;

    let bytes = data.as_bytes();
    let mut counts: HashMap<&str, u64> = HashMap::new();
    let mut total: u64 = 0;
    let mut i = 0usize;
    let n = bytes.len();

    while i < n {
        if bytes[i] != b'#' {
            i += 1;
            continue;
        }
        // Parse the entity id digits after '#'.
        let mut j = i + 1;
        while j < n && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j == i + 1 {
            // No digits after '#'; not an id.
            i += 1;
            continue;
        }
        // Skip whitespace, then require '='.
        let mut k = j;
        while k < n && bytes[k].is_ascii_whitespace() {
            k += 1;
        }
        if k >= n || bytes[k] != b'=' {
            i = j;
            continue;
        }
        k += 1;
        // Skip whitespace before the type name.
        while k < n && bytes[k].is_ascii_whitespace() {
            k += 1;
        }
        // A complex-entity instance looks like `#N = ( ... )`; skip past '(' for
        // the type-name scan but still count it.
        if k < n && bytes[k] == b'(' {
            k += 1;
            while k < n && bytes[k].is_ascii_whitespace() {
                k += 1;
            }
        }
        // Read the identifier (uppercase letters, digits, underscores).
        let type_start = k;
        while k < n && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
            k += 1;
        }
        if k > type_start {
            let ty = &data[type_start..k];
            // A definition is an identifier directly preceding '('.
            let mut m = k;
            while m < n && bytes[m].is_ascii_whitespace() {
                m += 1;
            }
            if m < n && bytes[m] == b'(' {
                total += 1;
                *counts.entry(ty).or_insert(0) += 1;
            }
        }
        i = k.max(j);
    }

    let mut top: Vec<EntityTypeCount> = counts
        .into_iter()
        .map(|(entity_type, count)| EntityTypeCount {
            entity_type: entity_type.to_string(),
            count,
        })
        .collect();
    // Sort by count descending, then name ascending for stable output.
    top.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.entity_type.cmp(&b.entity_type))
    });
    top.truncate(TOP_N);

    (total, top)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('Example'),'2;1');
FILE_NAME('cube.step','2026-06-12T00:00:00',('cadvm'),('cadvm'),'','','');
FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));
ENDSEC;
DATA;
#1 = CARTESIAN_POINT('', (0.0, 0.0, 0.0));
#2 = DIRECTION('', (0.0, 0.0, 1.0));
#3 = CARTESIAN_POINT('', (1.0, 0.0, 0.0));
ENDSEC;
END-ISO-10303-21;
";

    #[test]
    fn extracts_schema_and_counts() {
        let md = extract_str(SAMPLE);
        assert_eq!(md.file_schema.as_deref(), Some("AUTOMOTIVE_DESIGN"));
        assert_eq!(md.entity_count, Some(3));
        let top = &md.top_entity_types;
        assert_eq!(top[0].entity_type, "CARTESIAN_POINT");
        assert_eq!(top[0].count, 2);
        assert_eq!(md.data_line_count, Some(3));
    }

    #[test]
    fn ignores_references_inside_entities() {
        let data = "#10 = ADVANCED_FACE('',(#11,#12),#13,.T.);\n#11 = PLANE('',#14);\n";
        let (total, top) = scan_entities(data);
        assert_eq!(total, 2);
        assert!(top.iter().any(|t| t.entity_type == "ADVANCED_FACE"));
        assert!(top.iter().all(|t| t.entity_type != "PLANE" || t.count == 1));
    }
}
