//! Supported CAD file formats for V1.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// CAD formats tracked by cadvm V1. Only textual STEP/STP files are supported;
/// binary/mesh formats (STL, OBJ, native CAD) are explicitly out of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CadFormat {
    Step,
    Stp,
}

impl CadFormat {
    /// Detect the format from a path's extension, if it is a tracked format.
    pub fn from_path(path: &Path) -> Option<CadFormat> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "step" => Some(CadFormat::Step),
            "stp" => Some(CadFormat::Stp),
            _ => None,
        }
    }

    /// Lowercase extension (without the dot) for this format.
    pub fn extension(self) -> &'static str {
        match self {
            CadFormat::Step => "step",
            CadFormat::Stp => "stp",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detects_step_and_stp_case_insensitively() {
        assert_eq!(
            CadFormat::from_path(&PathBuf::from("a.step")),
            Some(CadFormat::Step)
        );
        assert_eq!(
            CadFormat::from_path(&PathBuf::from("a.STP")),
            Some(CadFormat::Stp)
        );
        assert_eq!(CadFormat::from_path(&PathBuf::from("a.stl")), None);
        assert_eq!(CadFormat::from_path(&PathBuf::from("noext")), None);
    }
}
