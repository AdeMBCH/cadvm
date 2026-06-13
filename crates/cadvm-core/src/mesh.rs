//! Lightweight metadata for triangle-mesh formats (STL, OBJ).
//!
//! Like the STEP scanner, this does **not** interpret geometry deeply — it just
//! surfaces cheap, useful figures (triangle/vertex counts and a bounding box)
//! for `show`, `log` and the metadata `diff`.

use serde::{Deserialize, Serialize};

use crate::format::CadFormat;

/// An axis-aligned bounding box of a mesh.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MeshBBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl MeshBBox {
    /// Box dimensions `[dx, dy, dz]`.
    pub fn size(&self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }
}

/// Lightweight mesh metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshMetadata {
    pub triangles: Option<u64>,
    pub vertices: Option<u64>,
    #[serde(default)]
    pub bbox: Option<MeshBBox>,
}

/// Extract mesh metadata for a tracked mesh format. Returns `None` for non-mesh
/// formats or unparseable content.
pub fn extract(content: &[u8], format: CadFormat) -> Option<MeshMetadata> {
    match format {
        CadFormat::Stl => Some(extract_stl(content)),
        CadFormat::Obj => Some(extract_obj(content)),
        _ => None,
    }
}

struct BBoxAccum {
    min: [f64; 3],
    max: [f64; 3],
    any: bool,
}

impl BBoxAccum {
    fn new() -> Self {
        BBoxAccum {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
            any: false,
        }
    }
    fn add(&mut self, p: [f64; 3]) {
        self.any = true;
        for (m, &v) in self.min.iter_mut().zip(p.iter()) {
            *m = m.min(v);
        }
        for (m, &v) in self.max.iter_mut().zip(p.iter()) {
            *m = m.max(v);
        }
    }
    fn finish(self) -> Option<MeshBBox> {
        self.any.then_some(MeshBBox {
            min: self.min,
            max: self.max,
        })
    }
}

/// STL: binary (84-byte header + 50 bytes/triangle) or ASCII (`facet`/`vertex`).
fn extract_stl(content: &[u8]) -> MeshMetadata {
    // Binary STL is detected by the exact size implied by its triangle count,
    // not by the leading "solid" word (which binary files may also contain).
    if content.len() >= 84 {
        let count =
            u32::from_le_bytes([content[80], content[81], content[82], content[83]]) as usize;
        if content.len() == 84 + count * 50 {
            return stl_binary(content, count);
        }
    }
    stl_ascii(content)
}

fn stl_binary(content: &[u8], count: usize) -> MeshMetadata {
    let mut bbox = BBoxAccum::new();
    let read_f32 = |b: &[u8]| f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64;
    for t in 0..count {
        let base = 84 + t * 50 + 12; // skip the 3-float normal
        for v in 0..3 {
            let o = base + v * 12;
            bbox.add([
                read_f32(&content[o..o + 4]),
                read_f32(&content[o + 4..o + 8]),
                read_f32(&content[o + 8..o + 12]),
            ]);
        }
    }
    MeshMetadata {
        triangles: Some(count as u64),
        vertices: Some(count as u64 * 3),
        bbox: bbox.finish(),
    }
}

fn stl_ascii(content: &[u8]) -> MeshMetadata {
    let text = String::from_utf8_lossy(content);
    let mut triangles = 0u64;
    let mut bbox = BBoxAccum::new();
    for line in text.lines() {
        let line = line.trim_start();
        if line.starts_with("facet") {
            triangles += 1;
        } else if let Some(rest) = line.strip_prefix("vertex") {
            if let Some(p) = parse_xyz(rest) {
                bbox.add(p);
            }
        }
    }
    MeshMetadata {
        triangles: Some(triangles),
        vertices: Some(triangles * 3),
        bbox: bbox.finish(),
    }
}

/// OBJ: count `v` vertices and triangulate `f` faces (n-gon → n-2 triangles).
fn extract_obj(content: &[u8]) -> MeshMetadata {
    let text = String::from_utf8_lossy(content);
    let mut vertices = 0u64;
    let mut triangles = 0u64;
    let mut bbox = BBoxAccum::new();
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("v ") {
            vertices += 1;
            if let Some(p) = parse_xyz(rest) {
                bbox.add(p);
            }
        } else if let Some(rest) = line.strip_prefix("f ") {
            let refs = rest.split_whitespace().count() as u64;
            triangles += refs.saturating_sub(2);
        }
    }
    MeshMetadata {
        triangles: Some(triangles),
        vertices: Some(vertices),
        bbox: bbox.finish(),
    }
}

/// Parse the first three whitespace-separated floats of a line.
fn parse_xyz(s: &str) -> Option<[f64; 3]> {
    let mut it = s.split_whitespace();
    let x = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    let z = it.next()?.parse().ok()?;
    Some([x, y, z])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_stl_counts() {
        let stl = "solid s\n\
            facet normal 0 0 1\n outer loop\n vertex 0 0 0\n vertex 1 0 0\n vertex 0 1 0\n endloop\n endfacet\n\
            facet normal 0 0 1\n outer loop\n vertex 0 0 0\n vertex 1 0 0\n vertex 1 1 2\n endloop\n endfacet\n\
            endsolid s\n";
        let m = extract(stl.as_bytes(), CadFormat::Stl).unwrap();
        assert_eq!(m.triangles, Some(2));
        assert_eq!(m.vertices, Some(6));
        assert_eq!(m.bbox.unwrap().max, [1.0, 1.0, 2.0]);
    }

    #[test]
    fn obj_counts_and_triangulates() {
        let obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\nf 1 2 3\n";
        let m = extract(obj.as_bytes(), CadFormat::Obj).unwrap();
        assert_eq!(m.vertices, Some(4));
        // quad (2 tris) + triangle (1 tri) = 3
        assert_eq!(m.triangles, Some(3));
        assert_eq!(m.bbox.unwrap().min, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn binary_stl_one_triangle() {
        let mut data = vec![0u8; 84 + 50];
        data[80..84].copy_from_slice(&1u32.to_le_bytes());
        // one triangle: normal (skip) + 3 verts at offset 84+12
        let verts: [f32; 9] = [0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0];
        let mut o = 84 + 12;
        for f in verts {
            data[o..o + 4].copy_from_slice(&f.to_le_bytes());
            o += 4;
        }
        let m = extract(&data, CadFormat::Stl).unwrap();
        assert_eq!(m.triangles, Some(1));
        assert_eq!(m.bbox.unwrap().max, [2.0, 3.0, 0.0]);
    }
}
