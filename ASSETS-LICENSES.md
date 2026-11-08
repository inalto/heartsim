# Asset licences

Every non-code asset shipped with heartsim is listed here with its origin and licence. Only CC BY, CC BY-SA (noted), CC0, OFL and
public-domain material may be added. **Nothing from the Epicardio Simulation program (`epicardio/`, `epicardio-extracted/`) is
used; those folders are proprietary, git-ignored, and exist only for study.**

| asset | path | origin | licence | notes |
|---|---|---|---|---|
| Procedural icosphere | generated at runtime | heartsim | MIT OR Apache-2.0 | viewport placeholder (M0) |
| Default anatomy pack: heart | `assets/anatomy/default/` (`mesh_lv/rv/ra/la/aorta/pulmonary_artery/svc/ivc/pulmonary_veins/valve_rings.glb`, `cells.bin`, `neighbours.bin`, `segments.bin`, `hoods/`, `landmarks.ron`) | Rodero C, Strocchi M, Marciniak M, Longobardi S, Whitaker J, O'Neill MD, Gillette K, Augustin C, Plank G, Vigmond EJ, Lamata P, Niederer SA. *Virtual cohort of extreme and average four-chamber heart meshes from statistical shape model* (v1.0.0), Zenodo 2021, doi:10.5281/zenodo.4593739, file `average.tar.gz` (paper: *PLoS Comput Biol* 2021;17:e1008851, doi:10.1371/journal.pcbi.1008851) | CC BY 4.0 | re-posed, re-centred, surfaces extracted/smoothed/decimated, myocardium resampled to cells; vessel tubes, appendage lobe, coronary sinus, conduction system, atrial sub-regions, AHA segments and landmarks added by heartsim (MIT OR Apache-2.0); baked by `cargo xtask bake-anatomy`, attribution in the pack's `LICENSE`; see `docs/anatomy-pack.md` and `docs/research/anatomy-meshes.md` |
| Default anatomy pack: torso and electrodes | `assets/anatomy/default/torso.glb`, `electrodes.ron` | Qian S, Monaci S, Mendonca-Costa C, Campos F, Gemmell P, Zaidi S, Rajani R, Whitaker J, Rinaldi CA, Bishop MJ. *Whole Torso Computational Models* (v1.0), King's College London, Zenodo 2022, doi:10.5281/zenodo.7253863, file `KCL_torso1.zip` | CC BY 4.0 | outer skin surface of torso 1 extracted, closed at the CT's cut planes, smoothed, decimated, posed in the heart frame; electrodes placed by heartsim's rule; all other tags discarded |
| Procedural anatomy pack (`cargo xtask bake-anatomy --procedural`, test fixture `AnatomyPack::procedural`) | generated on demand | heartsim, `heartsim_geometry::procedural` from published population-average dimensions (Lang 2015, Kligfield 2007, Streeter 1969 / Bayer 2012, Cerqueira 2002, Engblom 2005, Kayyali 2025, Narumi 2022, Durrer 1970) | MIT OR Apache-2.0 | no third-party mesh or data; the default pack until 0.6 |
| Scenario files | `assets/scenarios/*.ron` | heartsim authors, parameters from cited literature | MIT OR Apache-2.0 | |
| Tutorials | `assets/tutorials/*/tutorial.ron` | heartsim authors, original text | MIT OR Apache-2.0 | |
| Application icon | `assets/icon.svg` (+ generated PNG/ICO/ICNS in `assets/generated/icons/`) | heartsim authors, original artwork | MIT OR Apache-2.0 | |
| Default fonts | eframe `default_fonts` (Ubuntu-Light, NotoEmoji, Hack) | egui project | UFL / OFL / MIT | bundled by eframe |

Planned (see briefs B18, B43): PTB-XL ECG subset (PhysioNet, CC BY 4.0). Rejected or reserve mesh sources (BodyParts3D, Strocchi
2020, the Buttmann 2026 CT torsos, …) are listed with their licences in `docs/research/anatomy-meshes.md`. Each shipped asset is
added to this table with the exact record DOI and attribution text before shipping.
