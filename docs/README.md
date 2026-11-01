# heartsim documentation index

| document | what it is | audience |
|---|---|---|
| [REQUIREMENTS.md](REQUIREMENTS.md) | Product requirements: features (P0/P1/P2), rhythms/pathologies list, UX, non-functional requirements | everyone |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Technical design: crates, data flow, simulation tiers, ECG forward model, rendering, file formats, testing, packaging | implementers |
| [ROADMAP.md](ROADMAP.md) | Milestones M0–M8 with exit criteria and the dependency order of the briefs | planners |
| [briefs/](briefs/) | One self-contained implementation brief per work package, with recommended model (Opus / Sonnet / Haiku) | subagents |
| [research/reference-apps.md](research/reference-apps.md) | Feature inventory and UX analysis of Epicardio EHS2, cardiacrhythmsim.com, ecgaxistrainer, Epicardio WebGL | product |
| [research/electrophysiology-and-ecg-models.md](research/electrophysiology-and-ecg-models.md) | The science: cell models, eikonal activation, conduction system, ECG forward problem, meshes, numerics | sim implementers |
| [research/rust-stack.md](research/rust-stack.md) | Crate-by-crate evaluation of rendering, GUI, compute, audio, packaging, CI | implementers |
| [reverse-engineering/EPICARDIO-ANALYSIS.md](reverse-engineering/EPICARDIO-ANALYSIS.md) | How the Epicardio Unity app works internally (engine, data model, ECG, features) and what we keep/improve | architects |
| reverse-engineering/*.txt, pathology-recipes.md | Raw dumps: C# types, scene hierarchy, asset inventory, per-pathology parameter diffs | reference |

Legal reminder: `epicardio/` and `epicardio-extracted/` are proprietary, analysed for understanding only, git-ignored, never to be
copied into the product.
