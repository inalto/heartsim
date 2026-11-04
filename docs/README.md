# heartsim documentation index

| document | what it is | audience |
|---|---|---|
| [book/](book/) | The mdBook user and developer guide (install, UI tour, scenarios, tutorials, quizzes, exports, FAQ, disclaimer; architecture, adding a scenario or drug, benchmarks, file formats), published to GitHub Pages | users, contributors |
| [REQUIREMENTS.md](REQUIREMENTS.md) | Product requirements: features (P0/P1/P2), rhythms/pathologies list, UX, non-functional requirements | everyone |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Technical design: crates, data flow, simulation tiers, ECG forward model, rendering, file formats, testing, packaging | implementers |
| [ROADMAP.md](ROADMAP.md) | Milestones M0–M8 with exit criteria and the dependency order of the briefs | planners |
| [anatomy-pack.md](anatomy-pack.md) | Anatomy pack format, coordinate frame, the procedural default pack, and how to bake one from the open Strocchi/Rodero meshes (download, licence, attribution) | geometry/sim implementers |
| [briefs/](briefs/) | One self-contained implementation brief per work package, with recommended model (Opus / Sonnet / Haiku) | subagents |
| [research/reference-apps.md](research/reference-apps.md) | Feature inventory and UX analysis of Epicardio EHS2, cardiacrhythmsim.com, ecgaxistrainer, Epicardio WebGL | product |
| [research/electrophysiology-and-ecg-models.md](research/electrophysiology-and-ecg-models.md) | The science: cell models, eikonal activation, conduction system, ECG forward problem, meshes, numerics | sim implementers |
| [research/rust-stack.md](research/rust-stack.md) | Crate-by-crate evaluation of rendering, GUI, compute, audio, packaging, CI | implementers |

Legal note: heartsim is an independent implementation built from the published literature. It contains no code, data or assets from any
third-party simulator.
