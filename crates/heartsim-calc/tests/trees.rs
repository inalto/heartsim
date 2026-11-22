//! Acceptance tests of brief B29 part B for the decision trees: every tree is total (each
//! combination of criteria met, not met and not measured reaches a leaf or the tree's open end),
//! every leaf is reachable, every node and leaf carries a locator, and every decision tool of the
//! registry has its tree.

use heartsim_calc::tree::{self, Fact, LeafKind};
use heartsim_calc::{ALL, Review, Severity, by_id};
use std::collections::HashSet;

#[test]
fn every_tree_is_total_and_every_leaf_is_reached() {
    for t in tree::all() {
        let mut reached = HashSet::new();
        for combo in tree::every_combination(t.arities) {
            let facts: Vec<Fact> = combo
                .iter()
                .zip(t.arities)
                .map(|(v, a)| Fact::class(*v, *a, format!("{v:?}")))
                .collect();
            let d = tree::decide(t, &facts);
            let last = d.findings.last().expect("a result");
            let leaf = tree::reached(t, last).unwrap_or_else(|| panic!("{}: {combo:?}", t.calculator));
            assert!(std::ptr::eq(leaf, d.leaf), "{}", t.calculator);
            assert!(
                t.leaves.iter().any(|l| std::ptr::eq(*l, leaf)) || std::ptr::eq(leaf, t.open),
                "{}: {combo:?}",
                t.calculator
            );
            // a fully measured combination never ends open
            if combo.iter().all(Option::is_some) {
                assert!(!std::ptr::eq(leaf, t.open), "{}: {combo:?}", t.calculator);
            }
            // the path before the result is made of the tree's own nodes
            for f in &d.findings[..d.findings.len() - 1] {
                assert!(
                    t.nodes.iter().any(|n| n.cite == f.source) || f.severity == Severity::Info,
                    "{}",
                    t.calculator
                );
            }
            reached.insert(leaf.id);
        }
        for l in t.leaves {
            assert!(
                reached.contains(l.id),
                "{}: leaf {} is never reached",
                t.calculator,
                l.id
            );
        }
    }
}

#[test]
fn every_node_and_leaf_carries_a_locator() {
    for t in tree::all() {
        let calc = by_id(t.calculator).unwrap_or_else(|| panic!("{} is registered", t.calculator));
        assert_eq!(calc.review, Review::Verified);
        let mut ids = HashSet::new();
        for n in t.nodes {
            assert!(
                !n.cite.locator.is_empty() && n.cite.locator.contains("p."),
                "{}.{}",
                t.calculator,
                n.id
            );
            assert!(!n.question.en.is_empty() && !n.question.it.is_empty());
            assert!(!n.uses.is_empty(), "{}.{} looks at nothing", t.calculator, n.id);
            assert!(n.uses.iter().all(|k| *k < t.arities.len()));
            assert!(ids.insert(n.id), "{}: node {} twice", t.calculator, n.id);
        }
        let mut ids = HashSet::new();
        for l in t.leaves.iter().chain(std::iter::once(&t.open)) {
            assert!(
                !l.cite.locator.is_empty() && l.cite.locator.contains("p."),
                "{}.{}",
                t.calculator,
                l.id
            );
            assert!(!l.text.en.is_empty() && !l.text.it.is_empty() && !l.rule.is_empty());
            assert!(ids.insert(l.id), "{}: leaf {} twice", t.calculator, l.id);
            // worded as a criterion, never as a diagnosis
            assert!(!l.text.en.contains("the patient has"));
        }
        assert_eq!(t.open.kind, LeafKind::Indeterminate);
        assert_eq!(t.open.severity, Severity::Indeterminate);
    }
}

#[test]
fn the_registry_and_the_trees_agree() {
    let trees: Vec<&str> = tree::all().iter().map(|t| t.calculator).collect();
    let unique: HashSet<&&str> = trees.iter().collect();
    assert_eq!(unique.len(), trees.len());
    // in registry order
    let order: Vec<&str> = ALL
        .iter()
        .map(|c| c.id)
        .filter(|id| tree::for_calculator(id).is_some())
        .collect();
    assert_eq!(order, trees);
    assert_eq!(trees.len(), 16);
    // the worked example of each decision tool ends at a leaf of its tree
    for t in tree::all() {
        let c = by_id(t.calculator).unwrap();
        let out = c.evaluate(&c.example_inputs()).unwrap();
        assert!(
            tree::reached(t, out.findings.last().unwrap()).is_some(),
            "{}",
            c.id
        );
    }
}
