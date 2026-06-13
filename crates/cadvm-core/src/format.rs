//! Supported CAD file formats.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// CAD formats tracked by cadvm.
///
/// STEP/STP are B-Rep (boundary representation) formats and support the full
/// geometric diff. STL/OBJ are triangle-mesh formats: they are versioned with
/// lightweight metadata, and get a mesh-based (not B-Rep) geometric diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CadFormat {
    Step,
    Stp,
    Stl,
    Obj,
}

impl CadFormat {
    /// Detect the format from a path's extension, if it is a tracked format.
    pub fn from_path(path: &Path) -> Option<CadFormat> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "step" => Some(CadFormat::Step),
            "stp" => Some(CadFormat::Stp),
            "stl" => Some(CadFormat::Stl),
            "obj" => Some(CadFormat::Obj),
            _ => None,
        }
    }

    /// Lowercase extension (without the dot) for this format.
    pub fn extension(self) -> &'static str {
        match self {
            CadFormat::Step => "step",
            CadFormat::Stp => "stp",
            CadFormat::Stl => "stl",
            CadFormat::Obj => "obj",
        }
    }

    /// Whether this is a B-Rep format (STEP/STP) — full geometric diff applies.
    pub fn is_brep(self) -> bool {
        matches!(self, CadFormat::Step | CadFormat::Stp)
    }

    /// Whether this is a triangle-mesh format (STL/OBJ).
    pub fn is_mesh(self) -> bool {
        matches!(self, CadFormat::Stl | CadFormat::Obj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_tracked_formats_case_insensitively() {
        assert_eq!(
            CadFormat::from_path(&PathBuf::from("a.step")),
            Some(CadFormat::Step)
        );
        assert_eq!(
            CadFormat::from_path(&PathBuf::from("a.STP")),
            Some(CadFormat::Stp)
        );
        assert_eq!(
            CadFormat::from_path(&PathBuf::from("m.STL")),
            Some(CadFormat::Stl)
        );
        assert_eq!(
            CadFormat::from_path(&PathBuf::from("m.obj")),
            Some(CadFormat::Obj)
        );
        assert_eq!(CadFormat::from_path(&PathBuf::from("a.iges")), None);
        assert_eq!(CadFormat::from_path(&PathBuf::from("noext")), None);
    }

    #[test]
    fn brep_vs_mesh() {
        assert!(CadFormat::Step.is_brep() && !CadFormat::Step.is_mesh());
        assert!(CadFormat::Stl.is_mesh() && !CadFormat::Stl.is_brep());
    }
}
