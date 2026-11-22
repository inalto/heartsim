//! Guideline algorithms as decision trees (brief B29 part B).
//!
//! A tree is static data: its [`Node`]s (each a question with the figure or table and page it is
//! read from), its [`Leaf`]s (each a result with its rule and locator) and one pure `walk` function
//! that, given the answer to every question, goes from the root to a leaf. The answers are
//! [`Fact`]s computed from the typed measurements: a category (criterion met or not, or one of a
//! few ranges) or *unknown* when the measurement was left empty.
//!
//! [`decide`] handles the unknowns without guessing: it walks the tree once for every value the
//! unknown facts could take. When every walk ends at the same leaf, that leaf is the result (the
//! empty fields could not change it); when they end at different leaves, the result is
//! *indeterminate* and names the results still possible. The path shown is the part common to every
//! walk, so it never claims an answer that was not measured.

use crate::fmt;
use crate::grade::{ge, gt, le, lt};
use crate::model::{Cite, Finding, Message, Severity, Text, Unit};
use crate::tx;

/// A comparison against a published cut-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    /// `<`
    Lt,
    /// `≤`
    Le,
    /// `>`
    Gt,
    /// `≥`
    Ge,
}

impl Cmp {
    /// Whether `v cmp limit` holds (a value within rounding of the limit counts as equal to it).
    #[must_use]
    pub fn holds(self, v: f64, limit: f64) -> bool {
        match self {
            Cmp::Lt => lt(v, limit),
            Cmp::Le => le(v, limit),
            Cmp::Gt => gt(v, limit),
            Cmp::Ge => ge(v, limit),
        }
    }

    /// The symbol.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Cmp::Lt => "<",
            Cmp::Le => "≤",
            Cmp::Gt => ">",
            Cmp::Ge => "≥",
        }
    }

    /// The comparison that holds when this one does not.
    #[must_use]
    pub fn negated(self) -> Cmp {
        match self {
            Cmp::Lt => Cmp::Ge,
            Cmp::Le => Cmp::Gt,
            Cmp::Gt => Cmp::Le,
            Cmp::Ge => Cmp::Lt,
        }
    }
}

/// One published cut-off: "septal e' ≤ 6 cm/s".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Test {
    /// The quantity's symbol as the source writes it.
    pub symbol: &'static str,
    /// The comparison that meets the criterion.
    pub cmp: Cmp,
    /// The cut-off.
    pub limit: f64,
    /// Unit of the value and the cut-off.
    pub unit: Unit,
    /// Decimals of the value shown.
    pub decimals: u8,
}

/// Shorthand for a [`Test`].
#[must_use]
pub const fn test(symbol: &'static str, cmp: Cmp, limit: f64, unit: Unit, decimals: u8) -> Test {
    Test {
        symbol,
        cmp,
        limit,
        unit,
        decimals,
    }
}

fn unit_suffix(unit: Unit) -> String {
    match unit.symbol() {
        "" => String::new(),
        "%" => "%".to_owned(),
        s => format!(" {s}"),
    }
}

impl Test {
    /// Whether `v` meets the criterion; `None` when the value is missing.
    #[must_use]
    pub fn eval(&self, v: Option<f64>) -> Option<bool> {
        v.map(|v| self.cmp.holds(v, self.limit))
    }

    /// The value against the cut-off, with the relation that holds ("septal e' = 5.0 ≤ 6 cm/s"), or
    /// "septal e' = ?" when the value is missing.
    #[must_use]
    pub fn show(&self, v: Option<f64>) -> String {
        match v {
            None => format!("{} = ?", self.symbol),
            Some(x) => {
                let rel = if self.cmp.holds(x, self.limit) {
                    self.cmp
                } else {
                    self.cmp.negated()
                };
                format!(
                    "{} = {} {} {}{}",
                    self.symbol,
                    fmt::num(x, self.decimals),
                    rel.symbol(),
                    fmt::trim(self.limit),
                    unit_suffix(self.unit)
                )
            }
        }
    }
}

/// One answer the tree needs: a category out of `arity` (0 and 1 for a criterion not met / met),
/// or unknown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fact {
    /// The category, if the measurements give it.
    pub value: Option<u8>,
    /// How many categories there are.
    pub arity: u8,
    /// The measured values behind it, with their cut-offs (shown as the step's criterion).
    pub shown: String,
}

impl Fact {
    /// A criterion met (`Some(true)`), not met or not measured.
    #[must_use]
    pub fn flag(value: Option<bool>, shown: String) -> Self {
        Self {
            value: value.map(u8::from),
            arity: 2,
            shown,
        }
    }

    /// A category out of `arity`.
    #[must_use]
    pub fn class(value: Option<u8>, arity: u8, shown: String) -> Self {
        Self { value, arity, shown }
    }

    /// One test on one value.
    #[must_use]
    pub fn of(t: &Test, v: Option<f64>) -> Self {
        Self::flag(t.eval(v), t.show(v))
    }

    /// Any of several alternative tests that were measured: met if one is met, not met if every
    /// measured one is not, unknown only if none was measured.
    #[must_use]
    pub fn any_measured(tests: &[(Test, Option<f64>)]) -> Self {
        let measured: Vec<bool> = tests.iter().filter_map(|(t, v)| t.eval(*v)).collect();
        let value = if measured.is_empty() {
            None
        } else {
            Some(measured.iter().any(|b| *b))
        };
        Self::flag(value, show_all(tests))
    }
}

/// Every test with its value, separated by semicolons.
#[must_use]
pub fn show_all(tests: &[(Test, Option<f64>)]) -> String {
    tests
        .iter()
        .map(|(t, v)| t.show(*v))
        .collect::<Vec<_>>()
        .join("; ")
}

/// A question of the algorithm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    /// Stable id within the tree.
    pub id: &'static str,
    /// The question, written as the start of a sentence the answer completes.
    pub question: Text,
    /// The figure or table and page it is read from.
    pub cite: Cite,
    /// The facts (by index) it looks at; their values are shown as the step's criterion.
    pub uses: &'static [usize],
}

/// What a leaf concludes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafKind {
    /// A result the source gives.
    Result,
    /// The source's own "indeterminate" (or an explicit "cannot be assessed").
    Indeterminate,
    /// The measurements put the case outside the algorithm (it does not apply).
    OutOfScope,
}

/// A result of the algorithm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leaf {
    /// Stable id within the tree.
    pub id: &'static str,
    /// What kind of end.
    pub kind: LeafKind,
    /// How it is drawn.
    pub severity: Severity,
    /// The result, worded as a criterion met ("meets the source's criteria for …").
    pub text: Text,
    /// The rule that leads here, in symbols.
    pub rule: &'static str,
    /// The figure or table and page.
    pub cite: Cite,
}

/// The steps one walk took and where it ended.
#[derive(Debug, Clone, PartialEq)]
pub struct Walk {
    /// Each question asked and the answer taken.
    pub steps: Vec<(&'static Node, Text)>,
    /// The end.
    pub leaf: &'static Leaf,
}

impl Walk {
    /// An empty walk ending at `leaf` (the steps are pushed as the walk goes).
    #[must_use]
    pub fn new(leaf: &'static Leaf) -> Self {
        Self {
            steps: Vec::new(),
            leaf,
        }
    }

    /// Records a step.
    pub fn ask(&mut self, node: &'static Node, answer: Text) {
        self.steps.push((node, answer));
    }

    /// Ends the walk at `leaf`.
    #[must_use]
    pub fn end(mut self, leaf: &'static Leaf) -> Self {
        self.leaf = leaf;
        self
    }
}

/// A guideline algorithm.
#[derive(Debug, Clone, Copy)]
pub struct Tree {
    /// The id of the calculator that runs it.
    pub calculator: &'static str,
    /// Every question.
    pub nodes: &'static [&'static Node],
    /// Every result.
    pub leaves: &'static [&'static Leaf],
    /// The end used when empty fields leave more than one result possible.
    pub open: &'static Leaf,
    /// The number of categories of each fact `walk` reads, in order.
    pub arities: &'static [u8],
    /// From a complete set of answers (one category per fact) to a leaf.
    pub walk: fn(&[u8]) -> Walk,
}

/// The result of [`decide`].
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    /// The path as findings: one per question asked, then the result.
    pub findings: Vec<Finding>,
    /// The leaf reached ([`Tree::open`] when the empty fields leave the result open).
    pub leaf: &'static Leaf,
}

/// Common answers.
pub mod answers {
    use crate::model::Text;
    use crate::tx;
    /// Yes.
    pub const YES: Text = tx!("yes", "sì");
    /// No.
    pub const NO: Text = tx!("no", "no");
}

/// Walks `tree` for the facts measured; see the module documentation for the unknown ones.
///
/// # Panics
/// If `facts` does not match [`Tree::arities`] (a programming error the tests catch).
#[must_use]
pub fn decide(tree: &Tree, facts: &[Fact]) -> Decision {
    assert_eq!(facts.len(), tree.arities.len(), "{}: fact count", tree.calculator);
    for (k, f) in facts.iter().enumerate() {
        assert_eq!(f.arity, tree.arities[k], "{}: arity of fact {k}", tree.calculator);
        assert!(
            f.value.is_none_or(|v| v < f.arity),
            "{}: fact {k}",
            tree.calculator
        );
    }
    let unknown: Vec<usize> = (0..facts.len()).filter(|&k| facts[k].value.is_none()).collect();
    let mut answer: Vec<u8> = facts.iter().map(|f| f.value.unwrap_or(0)).collect();
    let mut walks = Vec::new();
    loop {
        walks.push((tree.walk)(&answer));
        // next combination of the unknown facts (mixed radix)
        let mut carried = true;
        for &k in &unknown {
            answer[k] += 1;
            if answer[k] < facts[k].arity {
                carried = false;
                break;
            }
            answer[k] = 0;
        }
        if carried {
            break;
        }
    }
    let first = &walks[0];
    let common = (0..first.steps.len())
        .take_while(|&s| walks.iter().all(|w| w.steps.get(s) == first.steps.get(s)))
        .count();
    let mut findings: Vec<Finding> = first.steps[..common]
        .iter()
        .map(|(node, ans)| step_finding(node, *ans, facts))
        .collect();
    let mut ends: Vec<&'static Leaf> = Vec::new();
    for w in &walks {
        if !ends.iter().any(|l| std::ptr::eq(*l, w.leaf)) {
            ends.push(w.leaf);
        }
    }
    let empty: Vec<&str> = unknown.iter().map(|&k| facts[k].shown.as_str()).collect();
    if let [leaf] = ends[..] {
        if common < first.steps.len() || walks.iter().any(|w| w.steps.len() != common) {
            findings.push(Finding {
                severity: Severity::Info,
                text: tx!(
                    "Fields left empty: whatever their values, the algorithm reaches the same result",
                    "Campi lasciati vuoti: qualunque ne sia il valore, l'algoritmo arriva allo stesso risultato"
                )
                .into(),
                criterion: empty.join("; "),
                source: leaf.cite,
            });
        }
        findings.push(leaf_finding(leaf));
        Decision { findings, leaf }
    } else {
        let list = |lang: &str| {
            ends.iter()
                .map(|l| l.text.get(lang))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let open = tree.open;
        // the measurements the first undecided question looks at, measured or not
        let criterion = first
            .steps
            .get(common)
            .map(|(node, _)| {
                node.uses
                    .iter()
                    .map(|&k| facts[k].shown.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| empty.join("; "));
        findings.push(Finding {
            severity: Severity::Indeterminate,
            text: Message {
                en: format!("{}. Still possible: {}", open.text.en, list("en")),
                it: format!("{}. Ancora possibili: {}", open.text.it, list("it")),
            },
            criterion,
            source: open.cite,
        });
        Decision { findings, leaf: open }
    }
}

fn step_finding(node: &Node, answer: Text, facts: &[Fact]) -> Finding {
    let criterion = node
        .uses
        .iter()
        .map(|&k| facts[k].shown.as_str())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ");
    Finding {
        severity: Severity::Info,
        text: Message {
            en: format!("{}: {}", node.question.en, answer.en),
            it: format!("{}: {}", node.question.it, answer.it),
        },
        criterion: if criterion.is_empty() {
            node.id.to_owned()
        } else {
            criterion
        },
        source: node.cite,
    }
}

fn leaf_finding(leaf: &Leaf) -> Finding {
    let (en, it) = match leaf.kind {
        LeafKind::Result => ("Result", "Risultato"),
        LeafKind::Indeterminate => ("Indeterminate", "Indeterminato"),
        LeafKind::OutOfScope => ("Not applicable", "Non applicabile"),
    };
    Finding {
        severity: leaf.severity,
        text: Message {
            en: format!("{en}: {}", leaf.text.en),
            it: format!("{it}: {}", leaf.text.it),
        },
        criterion: leaf.rule.to_owned(),
        source: leaf.cite,
    }
}

/// The leaf a finding reports, if it is the last finding of a [`decide`] on `tree` (a leaf of the
/// tree, or its open end).
#[must_use]
pub fn reached(tree: &Tree, finding: &Finding) -> Option<&'static Leaf> {
    if let Some(l) = tree.leaves.iter().copied().find(|l| leaf_finding(l) == *finding) {
        return Some(l);
    }
    let open = tree.open;
    (finding.severity == Severity::Indeterminate
        && finding.source == open.cite
        && finding.text.en.starts_with(&format!("{}. ", open.text.en)))
    .then_some(open)
}

/// Every tree, for the tests and for the page (which titles the findings of a tree "path").
#[must_use]
pub fn all() -> &'static [&'static Tree] {
    crate::calcs::TREES
}

/// The tree a calculator runs, if it is a decision tool.
#[must_use]
pub fn for_calculator(id: &str) -> Option<&'static Tree> {
    all().iter().copied().find(|t| t.calculator == id)
}

/// Every combination of known values and "unknown" over `facts` (arity per fact), for the
/// totality tests: each fact takes each of its categories and also `None`.
#[must_use]
pub fn every_combination(arities: &[u8]) -> Vec<Vec<Option<u8>>> {
    let mut out = vec![Vec::new()];
    for &a in arities {
        let mut next = Vec::with_capacity(out.len() * (usize::from(a) + 1));
        for prefix in &out {
            for v in (0..a).map(Some).chain(std::iter::once(None)) {
                let mut p = prefix.clone();
                p.push(v);
                next.push(p);
            }
        }
        out = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Reference;

    static R: Reference = Reference {
        authors: "A",
        title: "T",
        journal: "J",
        year: 2000,
        doi: None,
        url: None,
    };
    const C: Cite = Cite {
        reference: &R,
        locator: "Figure 1, p. 1",
    };
    static Q: Node = Node {
        id: "q",
        question: tx!("a or b", "a o b"),
        cite: C,
        uses: &[0, 1],
    };
    static YES: Leaf = Leaf {
        id: "yes",
        kind: LeafKind::Result,
        severity: Severity::Abnormal,
        text: tx!("met", "soddisfatto"),
        rule: "a or b",
        cite: C,
    };
    static NO: Leaf = Leaf {
        id: "no",
        kind: LeafKind::Result,
        severity: Severity::Normal,
        text: tx!("not met", "non soddisfatto"),
        rule: "neither",
        cite: C,
    };
    static OPEN: Leaf = Leaf {
        id: "open",
        kind: LeafKind::Indeterminate,
        severity: Severity::Indeterminate,
        text: tx!("open", "aperto"),
        rule: "-",
        cite: C,
    };
    fn walk(f: &[u8]) -> Walk {
        let mut w = Walk::new(&NO);
        let met = f[0] == 1 || f[1] == 1;
        w.ask(&Q, if met { answers::YES } else { answers::NO });
        w.end(if met { &YES } else { &NO })
    }
    static T: Tree = Tree {
        calculator: "x",
        nodes: &[&Q],
        leaves: &[&YES, &NO],
        open: &OPEN,
        arities: &[2, 2],
        walk,
    };

    fn f(v: Option<u8>) -> Fact {
        Fact::class(v, 2, format!("{v:?}"))
    }

    #[test]
    fn an_unknown_that_cannot_change_the_result_is_resolved() {
        // a met, b unknown: a or b holds whatever b is
        let d = decide(&T, &[f(Some(1)), f(None)]);
        assert_eq!(d.leaf.id, "yes");
        assert_eq!(d.findings.len(), 2);
        assert_eq!(d.findings[0].text.en, "a or b: yes");
        assert_eq!(d.findings[1].severity, Severity::Abnormal);
    }

    #[test]
    fn an_unknown_that_could_change_it_leaves_the_result_open() {
        let d = decide(&T, &[f(Some(0)), f(None)]);
        assert_eq!(d.leaf.id, "open");
        // no step is claimed: the first question already depends on b
        assert_eq!(d.findings.len(), 1);
        assert_eq!(d.findings[0].severity, Severity::Indeterminate);
        assert!(
            d.findings[0].text.en.contains("not met | met"),
            "{:?}",
            d.findings[0].text
        );
        assert_eq!(d.findings[0].criterion, "Some(0); None");
    }

    #[test]
    fn tests_show_the_relation_that_holds() {
        let t = test("septal e'", Cmp::Le, 6.0, Unit::CmS, 1);
        assert_eq!(t.show(Some(5.0)), "septal e' = 5.0 ≤ 6 cm/s");
        assert_eq!(t.show(Some(6.0)), "septal e' = 6.0 ≤ 6 cm/s");
        assert_eq!(t.show(Some(6.1)), "septal e' = 6.1 > 6 cm/s");
        assert_eq!(t.show(None), "septal e' = ?");
        assert_eq!(t.eval(Some(6.0)), Some(true));
        let any = Fact::any_measured(&[(t, Some(8.0)), (t, None)]);
        assert_eq!(any.value, Some(0));
        let none = Fact::any_measured(&[(t, None)]);
        assert_eq!(none.value, None);
    }

    #[test]
    fn every_combination_covers_unknown_too() {
        let c = every_combination(&[2, 3]);
        assert_eq!(c.len(), 3 * 4);
        assert!(c.contains(&vec![None, Some(2)]));
    }
}
