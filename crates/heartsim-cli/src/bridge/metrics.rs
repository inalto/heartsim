//! Comparison statistics. Pairs where either side is not finite (a cell that never activated, a sample
//! outside the mesh) are left out and counted.

use serde::Serialize;

/// Agreement of a paired sample, `test` minus `reference`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Agreement {
    /// Pairs compared.
    pub n: usize,
    /// Pairs dropped because a value was missing.
    pub missing: usize,
    pub rms: f64,
    /// Mean of `test − reference`.
    pub bias: f64,
    pub max_abs: f64,
    /// Pearson r; `None` when either side is constant or there are too few pairs.
    pub r: Option<f64>,
}

/// Compare `test` with `reference`.
#[must_use]
pub fn agreement(reference: &[f64], test: &[f64]) -> Agreement {
    let pairs: Vec<(f64, f64)> = reference
        .iter()
        .zip(test)
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .map(|(a, b)| (*a, *b))
        .collect();
    let n = pairs.len();
    let missing = reference.len().max(test.len()) - n;
    if n == 0 {
        return Agreement {
            n,
            missing,
            rms: f64::NAN,
            bias: f64::NAN,
            max_abs: f64::NAN,
            r: None,
        };
    }
    let count = n as f64;
    let sq = pairs.iter().map(|(a, b)| (b - a).powi(2)).sum::<f64>();
    Agreement {
        n,
        missing,
        rms: (sq / count).sqrt(),
        bias: pairs.iter().map(|(a, b)| b - a).sum::<f64>() / count,
        max_abs: pairs.iter().map(|(a, b)| (b - a).abs()).fold(0.0, f64::max),
        r: pearson(&pairs),
    }
}

fn pearson(pairs: &[(f64, f64)]) -> Option<f64> {
    if pairs.len() < 3 {
        return None;
    }
    let n = pairs.len() as f64;
    let (ma, mb) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (a, b) in pairs {
        ab += (a - ma) * (b - mb);
        aa += (a - ma).powi(2);
        bb += (b - mb).powi(2);
    }
    (aa > 1e-18 && bb > 1e-18).then(|| ab / (aa * bb).sqrt())
}

/// Pearson r of two equally long series (morphology correlation).
#[must_use]
pub fn correlation(a: &[f64], b: &[f64]) -> Option<f64> {
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(b)
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| (*x, *y))
        .collect();
    pearson(&pairs)
}

/// Format a number for a report table, with `-` for a missing value.
#[must_use]
pub fn cell(v: Option<f64>, decimals: usize) -> String {
    match v {
        Some(x) if x.is_finite() => format!("{x:.decimals$}"),
        _ => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constant_delay_is_all_bias() {
        let a = agreement(&[1.0, 2.0, 3.0, 4.0], &[6.0, 7.0, 8.0, 9.0]);
        assert_eq!((a.n, a.missing), (4, 0));
        assert!((a.rms - 5.0).abs() < 1e-12);
        assert!((a.bias - 5.0).abs() < 1e-12);
        assert!((a.r.unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn missing_values_are_counted_not_averaged() {
        let a = agreement(&[1.0, f64::NAN, 3.0], &[1.0, 2.0, f64::NAN]);
        assert_eq!((a.n, a.missing), (1, 2));
        assert_eq!(a.rms, 0.0);
        assert_eq!(a.r, None);
        assert!(agreement(&[f64::NAN], &[1.0]).rms.is_nan());
    }

    #[test]
    fn correlation_has_no_value_for_a_flat_signal() {
        assert_eq!(correlation(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]), None);
        let r = correlation(&[1.0, 2.0, 3.0], &[3.0, 2.0, 1.0]).unwrap();
        assert!((r + 1.0).abs() < 1e-12);
    }
}
