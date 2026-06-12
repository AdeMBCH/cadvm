//! Bridge to the `cadvm-geom` C++/OCCT helper (Step 2: geometric diff).
//!
//! The Rust core stays pure Rust: it shells out to a standalone C++ executable
//! (`cadvm-geom`, built from `cpp/`) and parses its JSON output. This keeps the
//! heavy Open CASCADE dependency isolated in a single subprocess with a narrow,
//! stable contract.
//!
//! The binary is located via the `CADVM_GEOM_BIN` environment variable, falling
//! back to `cadvm-geom` on `PATH`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// Environment variable pointing at the `cadvm-geom` executable.
pub const ENV_GEOM_BIN: &str = "CADVM_GEOM_BIN";

/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl BBox {
    /// Box dimensions `[dx, dy, dz]`.
    pub fn size(&self) -> [f64; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }
}

/// Full metrics for one input shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeMetrics {
    pub volume: f64,
    pub area: f64,
    pub solids: u64,
    pub shells: u64,
    pub faces: u64,
    #[serde(default)]
    pub bbox: Option<BBox>,
}

/// Metrics for a boolean-result piece (added/removed/common).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PieceMetrics {
    pub volume: f64,
    pub faces: u64,
}

/// Topological (face-to-face) classification counts.
///
/// Faces of A and B are matched by a coarse geometric signature; `common` faces
/// appear in both, `added` only in B, `removed` only in A.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaceTopo {
    pub common: u64,
    pub added: u64,
    pub removed: u64,
}

/// The geometric comparison of two STEP shapes, as reported by `cadvm-geom`.
///
/// `added`/`removed`/`common` are the boolean decomposition of the two inputs
/// (B−A, A−B, A∩B). All metric fields are optional because the error path omits
/// them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeomDiff {
    /// `"ok"` or `"error"`.
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    pub file_a: String,
    pub file_b: String,
    #[serde(default)]
    pub a: Option<ShapeMetrics>,
    #[serde(default)]
    pub b: Option<ShapeMetrics>,
    #[serde(default)]
    pub common: Option<PieceMetrics>,
    #[serde(default)]
    pub added: Option<PieceMetrics>,
    #[serde(default)]
    pub removed: Option<PieceMetrics>,
    /// Topological face-to-face classification.
    #[serde(default)]
    pub faces_topo: Option<FaceTopo>,
}

impl GeomDiff {
    /// Whether the helper reported a successful comparison.
    pub fn is_ok(&self) -> bool {
        self.status == "ok"
    }
}

/// A flat-shaded triangle soup: every 9 floats in `positions` is one triangle,
/// with a matching per-vertex normal in `normals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
}

impl Mesh {
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 9
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }
}

/// The mesh layers of a geometric diff: the full input shapes (context) and the
/// three colored boolean pieces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshLayers {
    #[serde(default)]
    pub shape_a: Option<Mesh>,
    #[serde(default)]
    pub shape_b: Option<Mesh>,
    pub common: Mesh,
    pub added: Mesh,
    pub removed: Mesh,
}

/// Tessellated geometric diff produced by `cadvm-geom mesh`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshDiff {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub bbox: Option<BBox>,
    #[serde(default)]
    pub layers: Option<MeshLayers>,
}

impl MeshDiff {
    pub fn is_ok(&self) -> bool {
        self.status == "ok"
    }

    /// Total triangle count across all layers.
    pub fn total_triangles(&self) -> usize {
        self.layers.as_ref().map_or(0, |l| {
            l.common.triangle_count() + l.added.triangle_count() + l.removed.triangle_count()
        })
    }
}

/// Resolve the path to the `cadvm-geom` binary.
pub fn binary_path() -> PathBuf {
    match std::env::var_os(ENV_GEOM_BIN) {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from("cadvm-geom"),
    }
}

/// Run the geometric diff between two STEP files using the configured binary.
pub fn diff_files(a: &Path, b: &Path) -> Result<GeomDiff> {
    diff_files_with(&binary_path(), a, b)
}

/// Run the geometric diff using an explicit binary path (used by tests).
pub fn diff_files_with(bin: &Path, a: &Path, b: &Path) -> Result<GeomDiff> {
    let output = Command::new(bin).arg("diff").arg(a).arg(b).output();
    let output = match output {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CoreError::GeomBinaryNotFound(bin.to_path_buf()));
        }
        Err(e) => return Err(CoreError::io(bin, e)),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(CoreError::GeomFailed(if stderr.is_empty() {
            format!("exit status {}", output.status)
        } else {
            stderr
        }));
    }

    let diff: GeomDiff = serde_json::from_slice(&output.stdout)
        .map_err(|e| CoreError::GeomFailed(format!("invalid JSON from helper: {e}")))?;
    Ok(diff)
}

/// Tessellate the geometric diff of two STEP files, writing the mesh JSON to
/// `out_json` and returning the parsed result.
pub fn mesh_files(a: &Path, b: &Path, out_json: &Path) -> Result<MeshDiff> {
    mesh_files_with(&binary_path(), a, b, out_json)
}

/// Like [`mesh_files`] but with an explicit binary path (used by tests).
pub fn mesh_files_with(bin: &Path, a: &Path, b: &Path, out_json: &Path) -> Result<MeshDiff> {
    let output = Command::new(bin)
        .arg("mesh")
        .arg(a)
        .arg(b)
        .arg(out_json)
        .output();
    let output = match output {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CoreError::GeomBinaryNotFound(bin.to_path_buf()));
        }
        Err(e) => return Err(CoreError::io(bin, e)),
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(CoreError::GeomFailed(if stderr.is_empty() {
            format!("exit status {}", output.status)
        } else {
            stderr
        }));
    }

    // The helper prints a short ack to stdout; a geometry error appears there
    // and means no usable output file was written.
    if let Ok(ack) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
        if ack.get("status").and_then(|s| s.as_str()) == Some("error") {
            let msg = ack
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown geometry error");
            return Err(CoreError::GeomFailed(msg.to_string()));
        }
    }

    let bytes = std::fs::read(out_json).map_err(|e| CoreError::io(out_json, e))?;
    let diff: MeshDiff = serde_json::from_slice(&bytes)
        .map_err(|e| CoreError::GeomFailed(format!("invalid mesh JSON from helper: {e}")))?;
    Ok(diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ok_payload() {
        let json = r#"{"status":"ok","file_a":"a.step","file_b":"b.step",
            "a":{"volume":1000.0,"area":600.0,"solids":1,"shells":1,"faces":6,
                 "bbox":{"min":[0,0,0],"max":[10,10,10]}},
            "b":{"volume":1200.0,"area":640.0,"solids":1,"shells":1,"faces":6,"bbox":null},
            "common":{"volume":950.0,"faces":6},
            "added":{"volume":250.0,"faces":3},
            "removed":{"volume":50.0,"faces":2},
            "faces_topo":{"common":5,"added":1,"removed":1}}"#;
        let d: GeomDiff = serde_json::from_str(json).unwrap();
        assert!(d.is_ok());
        assert_eq!(d.added.unwrap().volume, 250.0);
        assert_eq!(d.b.as_ref().unwrap().shells, 1);
        assert_eq!(d.faces_topo.unwrap().added, 1);
        let bbox = d.a.unwrap().bbox.unwrap();
        assert_eq!(bbox.size(), [10.0, 10.0, 10.0]);
    }

    #[test]
    fn parses_error_payload() {
        let json = r#"{"status":"error","error":"boolean Cut failed",
            "file_a":"a.step","file_b":"b.step"}"#;
        let d: GeomDiff = serde_json::from_str(json).unwrap();
        assert!(!d.is_ok());
        assert_eq!(d.error.as_deref(), Some("boolean Cut failed"));
        assert!(d.a.is_none());
    }

    #[test]
    fn missing_binary_is_reported() {
        let err = diff_files_with(
            Path::new("/nonexistent/cadvm-geom-xyz"),
            Path::new("a.step"),
            Path::new("b.step"),
        )
        .unwrap_err();
        assert!(matches!(err, CoreError::GeomBinaryNotFound(_)));
    }
}
