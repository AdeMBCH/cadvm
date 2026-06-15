//! Geometric verification: assert that a diff matches expectations.
//!
//! Turns the structured geometric diff into a flat map of named metrics, then
//! evaluates simple assertions (`metric op value`) against them. This is the
//! "judge" an AI agent or CI gate needs: declare what the edit *should* do, get
//! a pass/fail.
//!
//! Metric keys come from the diff:
//! - **STEP/STP**: `volume_a`, `volume_b`, `volume_delta`, `added_volume`,
//!   `removed_volume`, `common_volume`, `area_a`, `area_b`, `faces_a`, `faces_b`,
//!   `faces_added`, `faces_removed`, `faces_common`.
//! - **STL/OBJ**: `added_tris`, `removed_tris`, `unchanged_tris`,
//!   `bbox_dx`, `bbox_dy`, `bbox_dz`.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::geom::{GeomDiff, MeshDiff};

/// Comparison operator in an assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl Op {
    pub fn as_str(self) -> &'static str {
        match self {
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Eq => "==",
            Op::Ne => "!=",
        }
    }
    fn apply(self, lhs: f64, rhs: f64) -> bool {
        match self {
            Op::Lt => lhs < rhs,
            Op::Le => lhs <= rhs,
            Op::Gt => lhs > rhs,
            Op::Ge => lhs >= rhs,
            Op::Eq => lhs == rhs,
            Op::Ne => lhs != rhs,
        }
    }
}

/// A single parsed assertion, e.g. `added_volume > 100`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub metric: String,
    pub op: Op,
    pub value: f64,
}

/// Parse an assertion of the form `<metric><op><value>` (whitespace optional),
/// e.g. `added_volume>100`, `faces_added == 3`, `removed_volume <= 0.5`.
pub fn parse_check(s: &str) -> Result<Check, String> {
    // Two-char operators must be tried before single-char ones.
    for (token, op) in [
        ("<=", Op::Le),
        (">=", Op::Ge),
        ("==", Op::Eq),
        ("!=", Op::Ne),
        ("<", Op::Lt),
        (">", Op::Gt),
    ] {
        if let Some(pos) = s.find(token) {
            let metric = s[..pos].trim().to_string();
            let value_str = s[pos + token.len()..].trim();
            if metric.is_empty() {
                return Err(format!("missing metric in `{s}`"));
            }
            let value: f64 = value_str
                .parse()
                .map_err(|_| format!("invalid number `{value_str}` in `{s}`"))?;
            return Ok(Check { metric, op, value });
        }
    }
    Err(format!(
        "no comparison operator in `{s}` (use < <= > >= == !=)"
    ))
}

/// The result of one check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckResult {
    pub metric: String,
    pub op: Op,
    pub expected: f64,
    /// `None` if the metric is not available for this file/format.
    pub actual: Option<f64>,
    pub pass: bool,
}

/// The full verification outcome.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerifyReport {
    pub pass: bool,
    pub metrics: BTreeMap<String, f64>,
    pub checks: Vec<CheckResult>,
}

/// Evaluate `checks` against `metrics`. With no checks, `pass` is `true` (a
/// pure "describe the metrics" call).
pub fn evaluate(metrics: BTreeMap<String, f64>, checks: &[Check]) -> VerifyReport {
    let mut results = Vec::with_capacity(checks.len());
    let mut all = true;
    for c in checks {
        let actual = metrics.get(&c.metric).copied();
        let pass = actual.map(|a| c.op.apply(a, c.value)).unwrap_or(false);
        all &= pass;
        results.push(CheckResult {
            metric: c.metric.clone(),
            op: c.op,
            expected: c.value,
            actual,
            pass,
        });
    }
    VerifyReport {
        pass: all,
        metrics,
        checks: results,
    }
}

/// Flatten a B-Rep (STEP) geometric diff into named metrics.
pub fn metrics_from_geom(g: &GeomDiff) -> BTreeMap<String, f64> {
    let mut m = BTreeMap::new();
    if let Some(a) = &g.a {
        m.insert("volume_a".into(), a.volume);
        m.insert("area_a".into(), a.area);
        m.insert("faces_a".into(), a.faces as f64);
    }
    if let Some(b) = &g.b {
        m.insert("volume_b".into(), b.volume);
        m.insert("area_b".into(), b.area);
        m.insert("faces_b".into(), b.faces as f64);
    }
    if let (Some(a), Some(b)) = (&g.a, &g.b) {
        m.insert("volume_delta".into(), b.volume - a.volume);
    }
    if let Some(p) = &g.added {
        m.insert("added_volume".into(), p.volume);
    }
    if let Some(p) = &g.removed {
        m.insert("removed_volume".into(), p.volume);
    }
    if let Some(p) = &g.common {
        m.insert("common_volume".into(), p.volume);
    }
    if let Some(ft) = &g.faces_topo {
        m.insert("faces_added".into(), ft.added as f64);
        m.insert("faces_removed".into(), ft.removed as f64);
        m.insert("faces_common".into(), ft.common as f64);
    }
    m
}

/// Flatten a mesh (STL/OBJ) geometric diff into named metrics.
pub fn metrics_from_mesh(d: &MeshDiff) -> BTreeMap<String, f64> {
    let mut m = BTreeMap::new();
    if let Some(l) = &d.layers {
        m.insert("unchanged_tris".into(), l.unchanged.triangle_count() as f64);
        m.insert("added_tris".into(), l.added.triangle_count() as f64);
        m.insert("removed_tris".into(), l.removed.triangle_count() as f64);
    }
    if let Some(b) = &d.bbox {
        let s = b.size();
        m.insert("bbox_dx".into(), s[0]);
        m.insert("bbox_dy".into(), s[1]);
        m.insert("bbox_dz".into(), s[2]);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_operators() {
        assert_eq!(
            parse_check("added_volume>100").unwrap(),
            Check {
                metric: "added_volume".into(),
                op: Op::Gt,
                value: 100.0
            }
        );
        assert_eq!(parse_check(" faces_added == 3 ").unwrap().op, Op::Eq);
        assert_eq!(parse_check("removed_volume<=0.5").unwrap().op, Op::Le);
        assert!(parse_check("nonsense").is_err());
        assert!(parse_check(">5").is_err());
    }

    #[test]
    fn evaluates_pass_and_fail() {
        let mut metrics = BTreeMap::new();
        metrics.insert("added_volume".to_string(), 173.0);
        metrics.insert("removed_volume".to_string(), 110.0);
        let checks = vec![
            parse_check("added_volume>100").unwrap(),
            parse_check("removed_volume<1").unwrap(),
        ];
        let r = evaluate(metrics, &checks);
        assert!(!r.pass);
        assert!(r.checks[0].pass);
        assert!(!r.checks[1].pass);
    }

    #[test]
    fn missing_metric_fails_check() {
        let r = evaluate(BTreeMap::new(), &[parse_check("added_volume>1").unwrap()]);
        assert!(!r.pass);
        assert_eq!(r.checks[0].actual, None);
    }

    #[test]
    fn no_checks_passes() {
        let r = evaluate(BTreeMap::new(), &[]);
        assert!(r.pass);
    }
}
