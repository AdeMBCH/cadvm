//! Mesh-based geometric diff for STL/OBJ — **pure Rust, no Open CASCADE**.
//!
//! Triangle meshes have no B-Rep faces or solids, so the STEP pipeline does not
//! apply. Instead each triangle is classified by **distance to the other mesh**:
//! a triangle of the new version with no nearby surface in the old one is
//! *added*, a triangle of the old version with nothing nearby in the new one is
//! *removed*, and the rest is *unchanged*. The result is emitted in the very same
//! [`geom::MeshDiff`] shape the 3D viewer already consumes (unchanged / added /
//! removed triangle layers), so STL/OBJ get the same green/red/grey view as STEP
//! — without needing the geometry helper.

use std::collections::HashMap;

use crate::format::CadFormat;
use crate::geom::{BBox, Mesh, MeshDiff, MeshLayers};

type Vec3 = [f64; 3];
type Tri = [Vec3; 3];

/// Diff two meshes (raw file bytes) and return the colored layers.
pub fn diff(content_a: &[u8], content_b: &[u8], format: CadFormat) -> MeshDiff {
    let a = match parse(content_a, format) {
        Some(t) => t,
        None => return error("could not parse the old mesh"),
    };
    let b = match parse(content_b, format) {
        Some(t) => t,
        None => return error("could not parse the new mesh"),
    };

    // Combined bounding box → adaptive tolerance (2% of the diagonal).
    let mut bb = BBoxAccum::new();
    for t in a.iter().chain(b.iter()) {
        for v in t {
            bb.add(*v);
        }
    }
    let (min, max) = match bb.finish() {
        Some(mm) => mm,
        None => return ok_empty(),
    };
    let diag = dist(min, max);
    let tol = (0.02 * diag).max(1e-9);
    let cell = tol;
    let tol2 = tol * tol;

    let grid_a = TriGrid::build(&a, cell);
    let grid_b = TriGrid::build(&b, cell);

    let mut unchanged = MeshBuilder::new();
    let mut added = MeshBuilder::new();
    let mut removed = MeshBuilder::new();

    // New-version triangles: unchanged if their centroid lies on (within `tol`
    // of) the old surface, else added. Point-to-triangle distance means a shared
    // face matches even if the two meshes triangulate it differently.
    for t in &b {
        if grid_a.surface_within(centroid(t), tol2) {
            unchanged.push(t);
        } else {
            added.push(t);
        }
    }
    // Old-version triangles whose centroid is off the new surface: removed.
    for t in &a {
        if !grid_b.surface_within(centroid(t), tol2) {
            removed.push(t);
        }
    }

    MeshDiff {
        status: "ok".to_string(),
        error: None,
        bbox: Some(BBox { min, max }),
        layers: Some(MeshLayers {
            unchanged: unchanged.finish(),
            added: added.finish(),
            removed: removed.finish(),
        }),
    }
}

fn error(msg: &str) -> MeshDiff {
    MeshDiff {
        status: "error".to_string(),
        error: Some(msg.to_string()),
        bbox: None,
        layers: None,
    }
}

fn ok_empty() -> MeshDiff {
    MeshDiff {
        status: "ok".to_string(),
        error: None,
        bbox: None,
        layers: Some(MeshLayers {
            unchanged: Mesh {
                positions: vec![],
                normals: vec![],
            },
            added: Mesh {
                positions: vec![],
                normals: vec![],
            },
            removed: Mesh {
                positions: vec![],
                normals: vec![],
            },
        }),
    }
}

// ---- geometry helpers ------------------------------------------------------

fn centroid(t: &Tri) -> Vec3 {
    [
        (t[0][0] + t[1][0] + t[2][0]) / 3.0,
        (t[0][1] + t[1][1] + t[2][1]) / 3.0,
        (t[0][2] + t[1][2] + t[2][2]) / 3.0,
    ]
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn normal(t: &Tri) -> [f32; 3] {
    let u = [t[1][0] - t[0][0], t[1][1] - t[0][1], t[1][2] - t[0][2]];
    let v = [t[2][0] - t[0][0], t[2][1] - t[0][1], t[2][2] - t[0][2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len > 1e-12 {
        [
            (n[0] / len) as f32,
            (n[1] / len) as f32,
            (n[2] / len) as f32,
        ]
    } else {
        [0.0, 0.0, 1.0]
    }
}

struct BBoxAccum {
    min: Vec3,
    max: Vec3,
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
    fn add(&mut self, p: Vec3) {
        self.any = true;
        for (m, &v) in self.min.iter_mut().zip(p.iter()) {
            *m = m.min(v);
        }
        for (m, &v) in self.max.iter_mut().zip(p.iter()) {
            *m = m.max(v);
        }
    }
    fn finish(self) -> Option<(Vec3, Vec3)> {
        self.any.then_some((self.min, self.max))
    }
}

/// Flat-shaded triangle-soup builder (9 floats + per-triangle normal repeated).
struct MeshBuilder {
    positions: Vec<f32>,
    normals: Vec<f32>,
}
impl MeshBuilder {
    fn new() -> Self {
        MeshBuilder {
            positions: Vec::new(),
            normals: Vec::new(),
        }
    }
    fn push(&mut self, t: &Tri) {
        let n = normal(t);
        for v in t {
            self.positions.push(v[0] as f32);
            self.positions.push(v[1] as f32);
            self.positions.push(v[2] as f32);
            self.normals.extend_from_slice(&n);
        }
    }
    fn finish(self) -> Mesh {
        Mesh {
            positions: self.positions,
            normals: self.normals,
        }
    }
}

fn cell_of(cell: f64, x: f64) -> i64 {
    (x / cell).floor() as i64
}

/// Spatial grid of triangles for "is a point within `tol` of the surface?"
/// queries. Each triangle is bucketed into every cell its bounding box touches,
/// so a query checking the point's cell and its 26 neighbours (with
/// `cell >= tol`) finds every triangle that could be within `tol`.
struct TriGrid<'a> {
    cell: f64,
    tris: &'a [Tri],
    map: HashMap<[i64; 3], Vec<usize>>,
}
impl<'a> TriGrid<'a> {
    fn build(tris: &'a [Tri], cell: f64) -> TriGrid<'a> {
        let cell = if cell > 0.0 { cell } else { 1.0 };
        let mut map: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
        for (i, t) in tris.iter().enumerate() {
            let (mut lo, mut hi) = (t[0], t[0]);
            for v in &t[1..] {
                for d in 0..3 {
                    lo[d] = lo[d].min(v[d]);
                    hi[d] = hi[d].max(v[d]);
                }
            }
            for cx in cell_of(cell, lo[0])..=cell_of(cell, hi[0]) {
                for cy in cell_of(cell, lo[1])..=cell_of(cell, hi[1]) {
                    for cz in cell_of(cell, lo[2])..=cell_of(cell, hi[2]) {
                        map.entry([cx, cy, cz]).or_default().push(i);
                    }
                }
            }
        }
        TriGrid { cell, tris, map }
    }

    /// Is `p` within `sqrt(tol2)` of any triangle's surface?
    fn surface_within(&self, p: Vec3, tol2: f64) -> bool {
        let k = [
            cell_of(self.cell, p[0]),
            cell_of(self.cell, p[1]),
            cell_of(self.cell, p[2]),
        ];
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(ids) = self.map.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) {
                        for &i in ids {
                            let t = &self.tris[i];
                            if dist2_point_tri(p, t[0], t[1], t[2]) <= tol2 {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

// ---- vector helpers + closest point on a triangle --------------------------

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Squared distance from point `p` to triangle `abc`
/// (Ericson, *Real-Time Collision Detection*).
fn dist2_point_tri(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return dot(ap, ap);
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return dot(bp, bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        let q = [a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]];
        return dot(sub(p, q), sub(p, q));
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return dot(cp, cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        let q = [a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]];
        return dot(sub(p, q), sub(p, q));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let bc = sub(c, b);
        let q = [b[0] + w * bc[0], b[1] + w * bc[1], b[2] + w * bc[2]];
        return dot(sub(p, q), sub(p, q));
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let q = [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ];
    dot(sub(p, q), sub(p, q))
}

// ---- parsing (STL binary/ASCII, OBJ) ---------------------------------------

fn parse(content: &[u8], format: CadFormat) -> Option<Vec<Tri>> {
    match format {
        CadFormat::Stl => Some(parse_stl(content)),
        CadFormat::Obj => Some(parse_obj(content)),
        _ => None,
    }
}

fn parse_stl(content: &[u8]) -> Vec<Tri> {
    if content.len() >= 84 {
        let count =
            u32::from_le_bytes([content[80], content[81], content[82], content[83]]) as usize;
        if content.len() == 84 + count * 50 {
            return parse_stl_binary(content, count);
        }
    }
    parse_stl_ascii(content)
}

fn parse_stl_binary(content: &[u8], count: usize) -> Vec<Tri> {
    let read = |b: &[u8]| f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64;
    let mut tris = Vec::with_capacity(count);
    for t in 0..count {
        let base = 84 + t * 50 + 12; // skip the 3-float normal
        let mut tri = [[0.0; 3]; 3];
        for (v, slot) in tri.iter_mut().enumerate() {
            let o = base + v * 12;
            *slot = [
                read(&content[o..o + 4]),
                read(&content[o + 4..o + 8]),
                read(&content[o + 8..o + 12]),
            ];
        }
        tris.push(tri);
    }
    tris
}

fn parse_stl_ascii(content: &[u8]) -> Vec<Tri> {
    let text = String::from_utf8_lossy(content);
    let mut tris = Vec::new();
    let mut current: Vec<Vec3> = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("vertex") {
            if let Some(p) = parse_xyz(rest) {
                current.push(p);
            }
        } else if line.starts_with("endfacet") {
            if current.len() >= 3 {
                tris.push([current[0], current[1], current[2]]);
            }
            current.clear();
        }
    }
    tris
}

fn parse_obj(content: &[u8]) -> Vec<Tri> {
    let text = String::from_utf8_lossy(content);
    let mut verts: Vec<Vec3> = Vec::new();
    let mut tris = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("v ") {
            if let Some(p) = parse_xyz(rest) {
                verts.push(p);
            }
        } else if let Some(rest) = line.strip_prefix("f ") {
            // Each token is like `v`, `v/vt`, `v/vt/vn`, or `v//vn` (1-based,
            // negative = relative). Fan-triangulate the polygon.
            let idx: Vec<usize> = rest
                .split_whitespace()
                .filter_map(|tok| obj_index(tok.split('/').next().unwrap_or(""), verts.len()))
                .collect();
            // Fan-triangulate: (0, w, w+1) for w = 1..n-1.
            for w in 1..idx.len().saturating_sub(1) {
                tris.push([verts[idx[0]], verts[idx[w]], verts[idx[w + 1]]]);
            }
        }
    }
    tris
}

fn obj_index(tok: &str, n: usize) -> Option<usize> {
    let i: i64 = tok.parse().ok()?;
    if i > 0 {
        Some((i - 1) as usize).filter(|&x| x < n)
    } else if i < 0 {
        let x = n as i64 + i;
        (x >= 0).then_some(x as usize)
    } else {
        None
    }
}

fn parse_xyz(s: &str) -> Option<Vec3> {
    let mut it = s.split_whitespace();
    let x = it.next()?.parse().ok()?;
    let y = it.next()?.parse().ok()?;
    let z = it.next()?.parse().ok()?;
    Some([x, y, z])
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two stacked triangles forming a square in A; B keeps one and moves the
    // other far away → expect 1 unchanged, 1 added, 1 removed.
    #[test]
    fn classifies_moved_triangle() {
        let a = "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 9 9 0\nv 10 9 0\nv 9 10 0\nf 1 2 3\nf 4 5 6\n";
        // B: same first triangle, second triangle moved far in +z.
        let b = "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 9 9 50\nv 10 9 50\nv 9 10 50\nf 1 2 3\nf 4 5 6\n";
        let d = diff(a.as_bytes(), b.as_bytes(), CadFormat::Obj);
        assert!(d.is_ok());
        let l = d.layers.unwrap();
        assert_eq!(l.unchanged.triangle_count(), 1);
        assert_eq!(l.added.triangle_count(), 1);
        assert_eq!(l.removed.triangle_count(), 1);
    }

    #[test]
    fn identical_meshes_are_all_unchanged() {
        let m = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let d = diff(m.as_bytes(), m.as_bytes(), CadFormat::Obj);
        let l = d.layers.unwrap();
        assert_eq!(l.unchanged.triangle_count(), 1);
        assert_eq!(l.added.triangle_count(), 0);
        assert_eq!(l.removed.triangle_count(), 0);
    }
}
