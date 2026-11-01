# Electrophysiology and ECG models for a real-time 3D heart / ECG teaching simulator

Research notes for the Rust cross-platform heart simulator (target: 60 fps on a consumer laptop
without a discrete GPU; 3D activation/repolarisation colour map driven by a live simulation; a
physiologically plausible 12-lead ECG computed from that simulation; dozens of rhythms and
pathologies; interactive parameters).

Status: research survey, 2026-10-31. Values marked "(verify)" were taken from secondary sources
or from memory of the primary paper and should be checked against the cited original before being
hard-coded. Everything else is from the cited primary source or a documentation page fetched
during this survey.

---

## 0. Executive summary

1. **Nothing in the requirement list needs a biophysical ionic model at the whole-heart level.**
   Every listed rhythm and pathology can be produced by (a) a *conduction-system graph* with
   explicit timing/refractoriness rules, (b) an *eikonal/graph-geodesic activation* solver on a
   coarse myocardial mesh, (c) a *per-node action-potential template* whose duration (APD) and
   shape are set by region/pathology, and (d) a *precomputed lead-field matrix* that turns nodal
   transmembrane potentials into 9 electrode potentials and hence 12 leads. This "Tier 1"
   pipeline costs well under 1 ms of CPU per frame at 20–50k mesh nodes and is what ECGSIM,
   Pezzuto et al. 2017, Neic et al. 2017 and the Gillette et al. 2021 digital-twin work all use
   (at much higher resolution) to produce clinically credible 12-lead ECGs.

2. **Re-entry (VT, VF, torsades, AF with wavelets) is the only thing that genuinely needs a
   reaction–diffusion (RD) solver.** Use a 2-variable phenomenological model
   (Aliev–Panfilov or Mitchell–Schaeffer; 3-variable Fenton–Karma if you want realistic APD
   restitution and spiral breakup) on a *coarse masked voxel grid* (≈2 mm, ≈50–80k active
   voxels, i.e. a 64³ box) with explicit Euler at dt ≈ 0.1–0.25 ms, 70–170 sub-steps per frame.
   That is ~4–10 M voxel-updates per frame, comfortably real-time on 2–4 CPU cores with SIMD
   (rayon), and trivially so on any integrated GPU via wgpu compute. This is "Tier 2".

3. **A template-synthesis ECG generator (McSharry/ECGSYN-style, extended to a rotating 3-D
   dipole after Sameni et al. 2007) is still worth having ("Tier 0")** for pure rhythm-strip
   drills where no 3D view is shown, for unit-testing the rhythm engine, and as the fallback
   on very weak hardware. Its beat-timing engine should be *the same* one that drives Tier 1/2,
   so the strip and the 3D view never disagree.

4. **Recommended architecture:** a single *timing engine* (conduction-system graph with
   automaticity, refractoriness, decremental AV-nodal conduction, dual pathways, accessory
   pathway, pacemaker) emits timestamped "activation events" at named breakthrough sites.
   Tier 1 consumes those events to run a graph-geodesic/eikonal solve and build the surface
   colour map and the ECG; Tier 2 is switched on only for re-entrant scenarios and feeds the
   same lead-field. Tier 0 consumes the same events to draw a rhythm strip.

---

## 1. Physiological reference numbers used throughout

| Quantity | Typical adult value | Notes / source |
|---|---|---|
| SA node intrinsic rate | 60–100 bpm | Phase-4 slope sets rate |
| AV junctional escape rate | 40–60 bpm | |
| Ventricular (Purkinje/myocardial) escape | 20–40 bpm | |
| Atrial conduction velocity | 0.8–1.0 m/s (Bachmann's bundle up to ~1.7 m/s) | Durrer et al. 1970; Lemery 2007 |
| AV node conduction velocity | 0.02–0.05 m/s | slow, decremental |
| His–Purkinje conduction velocity | 2–4 m/s | |
| Ventricular myocardium, along fibre | 0.5–0.7 m/s | Taggart 2000: ~0.67 m/s |
| Ventricular myocardium, across fibre | 0.2–0.3 m/s (ratio 1:2 to 1:3) | sheet-normal slower still |
| P wave duration | 80–110 ms | atrial activation time |
| PR interval | 120–200 ms (AV-node delay ≈ 80–120 ms of that; His–Purkinje ≈ 35–55 ms) | |
| QRS duration | 70–100 ms (<120 ms normal) | total ventricular activation (Durrer: ~62–80 ms epicardial breakthrough to last activation) |
| QT interval | 350–440 ms at 60 bpm (QTc) | |
| Ventricular APD90 | 250–330 ms, epi < M < endo | Yan & Antzelevitch 1998 |
| Atrial APD90 | 200–300 ms | |
| Atrial ERP | 200–270 ms | |
| AV node ERP | 300–400 ms (longer than atrium/ventricle; rate-dependent) | |
| Ventricular ERP | 250–300 ms (≈ APD) | |
| Accessory pathway (WPW) ERP | 250–350 ms, *non-decremental* | shorter ERP = dangerous in AF |
| Frontal-plane QRS axis | −30° to +90° normal | LAD: −30 to −90; RAD: +90 to +180; extreme: −90 to ±180 |

Sources: Durrer D, van Dam RT, Freud GE, Janse MJ, Meijler FL, Arzbaecher RC. "Total excitation of the isolated human heart." *Circulation* 1970;41(6):899–912. https://doi.org/10.1161/01.CIR.41.6.899 — Taggart P et al. "Inhomogeneous transmural conduction during early ischaemia in patients with coronary artery disease." *J Mol Cell Cardiol* 2000;32:621–630. — Yan GX, Antzelevitch C. "Cellular basis for the normal T wave and the electrocardiographic manifestations of the long-QT syndrome." *Circulation* 1998;98:1928–1936. https://doi.org/10.1161/01.CIR.98.18.1928

---

## 2. Simulation approaches (requirement A)

### 2.1 Cellular automata (CA)

The oldest teaching-simulator approach. Each cell/node has a state machine:
`Rest → Excited (for APD_up) → Absolute refractory (ARP) → Relative refractory (RRP, higher threshold / slower CV) → Rest`.
A node becomes excited when a neighbour is excited and the "time since neighbour excitation"
exceeds the edge delay `d_ij = |x_i − x_j| / CV(fibre angle)`.

- Moe GK, Rheinboldt WC, Abildskov JA. "A computer model of atrial fibrillation." *Am Heart J* 1964;67:200–220. https://doi.org/10.1016/0002-8703(64)90371-0 — the first AF multiple-wavelet CA.
- Gerhardt M, Schuster H, Tyson JJ. "A cellular automaton model of excitable media including curvature and dispersion." *Science* 1990;247:1563–1566. https://doi.org/10.1126/science.2321017 — fixes the lattice-anisotropy and lack-of-curvature problems of naive CAs.
- Lorange M, Gulrajani RM. "A computer heart model incorporating anisotropic propagation: I. Model construction and simulation of normal activation." *J Electrocardiol* 1993;26(4):245–261. https://doi.org/10.1016/0022-0736(93)90046-G — anisotropic CA on a 3-D heart with 12-lead ECG via a torso model.
- Siregar P, Sinteff JP, Julen N, Le Beux P. "An interactive 3D anisotropic cellular automata model of the heart." *Comput Biomed Res* 1998;31(5):323–347. https://doi.org/10.1006/cbmr.1998.1485 — explicitly an interactive teaching model: SA/AV node, His–Purkinje, ventricles, arrhythmias.
- Wei D, Okazaki O, Harumi K, Harasawa E, Hosaka H. "Comparative simulation of excitation and body surface electrocardiogram with isotropic and anisotropic computer heart models." *IEEE Trans Biomed Eng* 1995;42(4):343–357. https://doi.org/10.1109/10.376128 — Wei's whole-heart "ECG simulator" lineage (University of Aizu) used for teaching.

Pros: trivially cheap (one integer state per node), unconditionally stable, every parameter is
directly a clinical number (CV, ERP). Cons: lattice-direction artefacts (octagonal fronts on
grids), no wavefront-curvature effect (so re-entry cores are wrong), no restitution unless you
add it by hand, and no sub-threshold/electrotonic effects (no source–sink mismatch, no
decrement). A CA is essentially a discrete eikonal solver with a refractory rule; the
**graph-geodesic / eikonal formulation (next section) is the same idea done properly** and
should be preferred.

### 2.2 Eikonal / fast-marching / graph-geodesic activation

**Equation.** Activation time `T(x)` satisfies the anisotropic eikonal equation

```
sqrt( ∇T(x)ᵀ · M(x) · ∇T(x) ) = 1          in the myocardium Ω
T = T_0,k                                   at stimulus sites x_k (Dirichlet)

M(x) = c_f² f fᵀ + c_s² s sᵀ + c_n² n nᵀ       (velocity tensor, m²/s²)
```

where `f, s, n` are the local fibre, sheet and sheet-normal unit vectors and `c_f ≥ c_s ≥ c_n`
the conduction velocities along them (typically `c_f = 0.6`, `c_s = 0.3`, `c_n = 0.2` m/s).
Equivalently `|∇T|_{M} = 1`: the front travels distance `c(θ)` per unit time where
`c(θ)` is the velocity in direction θ. In the conductivity-tensor form used by openCARP /
Pezzuto et al., `M = D / (something)`; only the *ratio* matters for the shape and a scalar
tunes the absolute speed. The eikonal–curvature refinement
(Keener 1991) adds a diffusion-like term `−ε ∇·(M∇T)/|∇T|` that slows convex fronts, which
is what you need if you want the eikonal to initiate re-entry correctly:

- Keener JP. "An eikonal-curvature equation for action potential propagation in myocardium." *J Math Biol* 1991;29:629–651. https://doi.org/10.1007/BF00163916
- Colli Franzone P, Guerri L, Rovida S. "Wavefront propagation in an activation model of the anisotropic cardiac tissue: asymptotic analysis and numerical simulations." *J Math Biol* 1990;28:121–176. https://doi.org/10.1007/BF00163143

**Solvers.**

| Solver | Complexity | Anisotropy | Parallel | Notes |
|---|---|---|---|---|
| Dijkstra on mesh edges (graph geodesic) | O(E log V) | via edge weights `w_ij = \|e_ij\| / c(θ_ij)` | poor | Overestimates T by up to ~10 % on coarse meshes (metrication error); mitigate with 2-ring neighbourhoods. Wallman et al. 2012 found graph-based vs eikonal vs monodomain differ by only a few ms. |
| Fast Marching (Sethian) | O(N log N) | isotropic natively; anisotropic needs unfolding tricks (Sermesant 2007) | poor | exact Dijkstra-like ordering; multi-front for Purkinje breakthroughs |
| Fast Iterative Method (Jeong & Whitaker 2008; Fu et al. 2011 triangles; Fu et al. 2013 tets) | ~O(N) iterations with active list | native (local Hopf–Lax update with tensor) | excellent (GPU) | Used by Pezzuto 2017; `fim-python` (Grandits 2021) |
| Fast Sweeping (Zhao 2005) | O(N) per sweep, few sweeps | native | moderate | simple on voxel grids |

- Sethian JA. "A fast marching level set method for monotonically advancing fronts." *PNAS* 1996;93:1591–1595. https://doi.org/10.1073/pnas.93.4.1591
- Sermesant M, Konukoglu E, Delingette H, Coudière Y, Chinchapatnam P, Rhode KS, Razavi R, Ayache N. "An anisotropic multi-front fast marching method for real-time simulation of cardiac electrophysiology." *FIMH 2007*, LNCS 4466:160–169. https://doi.org/10.1007/978-3-540-72907-5_17 — explicitly "real-time", anisotropic, multi-front, with a refractory rule so it can be re-triggered.
- Konukoglu E et al. "Efficient probabilistic model personalization integrating uncertainty on data and parameters: application to eikonal-diffusion models in cardiac electrophysiology." *Prog Biophys Mol Biol* 2011;107:134–146.
- Jeong W-K, Whitaker RT. "A fast iterative method for eikonal equations." *SIAM J Sci Comput* 2008;30(5):2512–2534. https://doi.org/10.1137/060670298
- Fu Z, Jeong W-K, Pan Y, Kirby RM, Whitaker RT. "A fast iterative method for solving the eikonal equation on triangulated surfaces." *SIAM J Sci Comput* 2011;33(5):2468–2488. https://doi.org/10.1137/100788951
- Fu Z, Kirby RM, Whitaker RT. "A fast iterative method for solving the eikonal equation on tetrahedral domains." *SIAM J Sci Comput* 2013;35(5):C473–C494. https://doi.org/10.1137/120881956
- Grandits T. "A Fast Iterative Method Python package." *J Open Source Softw* 2021;6(66):3641. https://doi.org/10.21105/joss.03641 — https://github.com/thomgrand/fim-python (reference implementation to port)
- Wallman M, Smith NP, Rodriguez B. "A comparative study of graph-based, eikonal, and monodomain simulations for the estimation of cardiac activation times." *IEEE Trans Biomed Eng* 2012;59(6):1739–1748. https://doi.org/10.1109/TBME.2012.2193398
- Jacquemet V. "An eikonal-diffusion solver and its application to the interpolation and the simulation of reentrant cardiac activations." *Comput Methods Programs Biomed* 2012;108(2):548–558. https://doi.org/10.1016/j.cmpb.2011.05.003 — how to do *re-entry* with an eikonal by solving for a periodic activation map.
- Pezzuto S, Kal'avský P, Potse M, Prinzen FW, Auricchio A, Krause R. "Evaluation of a rapid anisotropic model for ECG simulation." *Front Physiol* 2017;8:265. https://doi.org/10.3389/fphys.2017.00265 — FIM on 1 mm hex meshes (192k–493k nodes) + lead-field ECG: full 12-lead in ~3 s on a low-end GPU, <1 s on a high-end GPU; activation error vs bidomain ≈ 2.6–7 ms median; QRS error generally < 0.5 mV. The T wave is not captured (uniform APD template).
- Neic A, Campos FO, Prassl AJ, Niederer SA, Bishop MJ, Vigmond EJ, Plank G. "Efficient computation of electrograms and ECGs in human whole heart simulations using a reaction-eikonal model." *J Comput Phys* 2017;346:191–211. https://doi.org/10.1016/j.jcp.2017.06.020 — "reaction-eikonal" (R-E): eikonal for timing, then an ionic model is run *locally* with the diffusion term replaced by a prescribed foot current; >1000× cheaper than bidomain with accurate ECGs. The same trick with a *template* AP instead of an ionic model is what Tier 1 below uses.
- Gillette K, Gsell MAF, Prassl AJ, Karabelas E, Reiter U, Reiter G, Grandits T, Payer C, Štern D, Urschler M, Bayer JD, Augustin CM, Neic A, Pock T, Vigmond EJ, Plank G. "A framework for the generation of digital twins of cardiac electrophysiology from clinical 12-leads ECGs." *Med Image Anal* 2021;71:102080. https://doi.org/10.1016/j.media.2021.102080 — the complete R-E + lead-field + APD-gradient recipe, validated against bidomain; the design pattern the Tier 1 model should copy.

**Cheap activation-time recipe for this product.**
1. Keep a coarse simulation mesh (20–50k nodes, see §6) with per-node `f` and per-region
   `(c_f, c_s, c_n)` and a per-node *refractory-until* timestamp.
2. Each time the timing engine emits a breakthrough event `(site, t_0)` (e.g. "RV Purkinje
   terminals at t_0", "LV anterior free wall ectopic focus at t_0"), run a multi-source
   Dijkstra/FIM from those seeds over nodes whose `refractory_until < t_candidate`. Store
   `T_act[node]`.
3. Re-running on a 50k-node tet/triangle mesh with a binary-heap Dijkstra takes ~5–15 ms
   single-threaded; FIM on the same mesh ~5–20 ms on 4 threads. This happens once per *beat*
   (every 300–1500 ms), not per frame, so it is essentially free. Spread it over frames with
   a background thread if you want zero jitter.
4. Per frame, the colour map is `V_template(t − T_act[node]; APD[node])` — a table lookup.

### 2.3 Reaction–diffusion monodomain with phenomenological cell models

**Monodomain equation.**

```
χ C_m ∂V/∂t = ∇·(σ_m ∇V) − χ I_ion(V, w)        (dimensional)
∂V/∂t      = ∇·(D ∇V) − I_ion(V, w)/C_m         (D = σ_m / (χ C_m), mm²/ms)
∂w/∂t      = g(V, w)

σ_m = σ_i σ_e / (σ_i + σ_e)   (harmonic mean of bidomain conductivities, per axis)
D   = D_f f fᵀ + D_s s sᵀ + D_n n nᵀ ;   CV ∝ sqrt(D)
```

Typical human-ventricle numbers (Niederer et al. 2011 benchmark; ten Tusscher 2006):
`σ_il = 0.17, σ_it = 0.019, σ_el = 0.62, σ_et = 0.24 S/m, χ = 140 mm⁻¹ (1400 cm⁻¹),
C_m = 0.01 µF/mm² (1 µF/cm²)` → `D_l ≈ 0.095 mm²/ms`; ten Tusscher–Panfilov 2006 used
`D = 0.154 mm²/ms` (0.00154 cm²/ms) giving CV ≈ 0.68 m/s along fibres; transverse D is
typically D_l/4 to D_l/9 (CV ratio 2–3).

- Niederer SA, Kerfoot E, Benson AP, Bernabeu MO, Bernus O, Bradley C, Cherry EM, Clayton R, Fenton FH, Garny A, Heidenreich E, Land S, Maleckar M, Pathmanathan P, Plank G, Rodríguez JF, Roy I, Sachse FB, Seemann G, Skavhaug O, Smith NP. "Verification of cardiac tissue electrophysiology simulators using an N-version benchmark." *Phil Trans R Soc A* 2011;369:4331–4351. https://doi.org/10.1098/rsta.2011.0139 — the benchmark everyone uses to check a monodomain solver (20×7×3 mm slab, TP06 epi, dx 0.1/0.2/0.5 mm, dt 0.005/0.01/0.05 ms).
- Clayton RH, Bernus O, Cherry EM, Dierckx H, Fenton FH, Mirabella L, Panfilov AV, Sachse FB, Seemann G, Zhang H. "Models of cardiac tissue electrophysiology: progress, challenges and open questions." *Prog Biophys Mol Biol* 2011;104:22–48. https://doi.org/10.1016/j.pbiomolbio.2010.05.008 — the best single review of all models below.
- Fenton FH, Cherry EM. "Models of cardiac cell." *Scholarpedia* 2008;3(8):1868. http://www.scholarpedia.org/article/Models_of_cardiac_cell

#### 2.3.1 FitzHugh–Nagumo (FHN)

```
dv/dt = v − v³/3 − w + I_stim + ∇·(D∇v)
dw/dt = ε (v + a − b w)
a = 0.7, b = 0.8, ε = 0.08      (FitzHugh 1961)
```
Variant used for cardiac demos ("Barkley-like" / Rogers–McCulloch 1994):
```
dv/dt = c1 v (v − a)(1 − v) − c2 v w + ∇·(D∇v)
dw/dt = b (v − d w)
a = 0.13, b = 0.013, c1 = 0.26, c2 = 0.1, d = 1.0      (Rogers & McCulloch)
```
- FitzHugh R. "Impulses and physiological states in theoretical models of nerve membrane." *Biophys J* 1961;1:445–466. https://doi.org/10.1016/S0006-3495(61)86902-6
- Nagumo J, Arimoto S, Yoshizawa S. "An active pulse transmission line simulating nerve axon." *Proc IRE* 1962;50:2061–2070.
- Rogers JM, McCulloch AD. "A collocation-Galerkin finite element model of cardiac action potential propagation." *IEEE Trans Biomed Eng* 1994;41:743–757. https://doi.org/10.1109/10.310090

Properties: supports spiral waves; APD shape unphysiological (hyperpolarising undershoot in
the original); no restitution to speak of; extremely robust numerically (dt up to ~0.1 in
dimensionless units). Use only as a toy/fallback.

#### 2.3.2 Aliev–Panfilov (AP, 1996)

```
∂u/dt = ∇·(D∇u) − k u (u − a)(u − 1) − u v
dv/dt = ε(u, v) · ( −v − k u (u − a − 1) )
ε(u, v) = ε_0 + μ_1 v / (u + μ_2)

Standard set:  k = 8, a = 0.15, ε_0 = 0.002, μ_1 = 0.2, μ_2 = 0.3
Dimensionless: u ∈ [0, 1]; map V_m = 100·u − 80 mV; time unit ≈ 12.9 ms
(so APD ≈ 25–30 time units ≈ 300–350 ms); a = 0.10 is often used to obtain spiral waves
more easily; threshold ≈ a.
```
- Aliev RR, Panfilov AV. "A simple two-variable model of cardiac excitation." *Chaos, Solitons & Fractals* 1996;7(3):293–301. https://doi.org/10.1016/0960-0779(95)00089-5
- Educational interactive implementation with the same parameters: https://www.ibiblio.org/e-notes/html5/ap.html ; openCARP ships it as `AlievPanfilov`; a 2025 physics-informed-operator paper used `a=0.15, μ1=0.2, μ2=0.3, k=8, ε=0.002, D=0.55` in openCARP (https://arxiv.org/pdf/2511.08418).

Properties: good-looking AP (no undershoot), tunable APD via ε_0/μ, CV ∝ sqrt(D) and
∝ roughly sqrt(k). Reproduces spiral waves and, with heterogeneity or a ≈ 0.1 + anisotropy,
spiral breakup; restitution is weak/monotonic (no alternans mechanism). Upstroke is slow
(~1 time unit ≈ 10 ms), which is *exactly why it tolerates coarse grids* (wavefront width
≈ CV × upstroke ≈ 6 mm at 0.6 m/s, so dx = 1–2 mm still resolves it).
Explicit Euler: stable at `dt = 0.01–0.05` (dimensionless, ≈ 0.13–0.65 ms) with `dx = 0.5–1.0`
grid units and `D = 1`; the diffusion CFL `dt ≤ dx²/(6D)` is then the binding limit in 3D.
Calibrate `D` so that a 1D strand gives the desired CV on the chosen dx (measure, then scale by
the square of the velocity ratio).

#### 2.3.3 Mitchell–Schaeffer (MS, 2003)

```
dv/dt = ∇·(D∇v) + J_in + J_out + J_stim
J_in  =  h v² (1 − v) / τ_in
J_out = −v / τ_out
dh/dt =  (1 − h)/τ_open       if v <  v_gate
      =  −h / τ_close         if v ≥  v_gate

Standard set: τ_in = 0.3 ms, τ_out = 6 ms (openCARP default 5 ms), τ_open = 120 ms,
              τ_close = 150 ms, v_gate = 0.13 ; v ∈ [0, 1]
```
Analytic properties (from the paper): APD restitution
`APD(DI) = τ_close · ln( h_max(DI) / h_min )`, `h_max = 1 − (1 − h_min) e^{−DI/τ_open}`,
`h_min = 4 τ_in / τ_out` → *explicit* control of restitution slope and of the threshold;
`τ_close` sets APD, `τ_open` sets recovery/ERP; CV ∝ sqrt(D/τ_in).
- Mitchell CC, Schaeffer DG. "A two-current model for the dynamics of cardiac membrane." *Bull Math Biol* 2003;65:767–793. https://doi.org/10.1016/S0092-8240(03)00041-7
- CellML: https://models.physiomeproject.org/exposure/9efc295a6f3360044b4712c28b8314e7 ; openCARP `MitchellSchaeffer` (τ_out = 5, V_gate = 0.13); PiGReads docs https://pigreads.readthedocs.io/en/latest/models/mitchell2003two.html

Properties: the best 2-variable model for *rate-dependent* behaviour (APD restitution,
alternans when slope > 1, Wenckebach-like block in tissue), spiral waves, breakup with steep
restitution. Its upstroke (set by τ_in = 0.3 ms) is fast → wavefront width ≈ 0.5–1 mm; on a
2 mm teaching grid raise τ_in to ~1–2 ms (and D accordingly) or the front will fail to
propagate. Explicit Euler needs `dt ≤ ~0.1 ms` (0.05 safer) because of τ_in. Monodomain
parameter spatial fields of this model are the standard choice in the Sermesant/INRIA
personalisation line of work and in the Gaussian-process EP work (arXiv:2311.10114).

#### 2.3.4 Fenton–Karma 3-variable (FK3V, 1998)

```
∂u/∂t = ∇·(D∇u) − (J_fi + J_so + J_si)
J_fi = −(v / τ_d) · H(u − u_c) · (1 − u) · (u − u_c)
J_so =  (u / τ_o) · (1 − H(u − u_c)) + (1/τ_r) · H(u − u_c)
J_si = −(w / (2 τ_si)) · (1 + tanh(k (u − u_c^si)))
dv/dt = (1 − H(u − u_c)) (1 − v)/τ_v⁻(u) − H(u − u_c) v/τ_v⁺
dw/dt = (1 − H(u − u_c)) (1 − w)/τ_w⁻      − H(u − u_c) w/τ_w⁺
τ_v⁻(u) = H(u − u_v) τ_v1⁻ + (1 − H(u − u_v)) τ_v2⁻
V_m = V_0 + u (V_fi − V_0),  V_0 = −85 mV, V_fi = +15 mV ;  τ_d = C_m / g_fi
```

Parameter sets (ms). MBR and MLR-I values as implemented in Nektar++ (labelled "Fenton & Karma Chaos 1998"), BR from the paper + 1998 erratum (verify against Table I of the paper, https://thevirtualheart.org/assets/media/pdf/chaos98.pdf and erratum https://thevirtualheart.org/assets/media/pdf/tablechaos1998.pdf):

| Param | BR | MBR | MLR-I | GP (erratum/paper) |
|---|---|---|---|---|
| τ_v⁺ | 3.33 | 3.33 | 10 | 3.33 (verify) |
| τ_v1⁻ | 1250 | 1000 | 18.2 | 19.6 (verify) |
| τ_v2⁻ | 19.6 | 19.2 | 18.2 | 1250 (verify) |
| τ_w⁺ | 870 | 667 | 1020 | 870 (verify) |
| τ_w⁻ | 41 | 11 | 80 | 41 (verify) |
| τ_d | 0.25 | 0.25 | 0.25 | 0.25 (verify) |
| τ_o | 12.5 | 8.3 | 12.5 | 12.5 (verify) |
| τ_r | 33.33 (erratum) | 50 | 130 | 25 (verify) |
| τ_si | 29 (erratum) | 44.84 (erratum) | 127 | 22.22 (erratum) |
| k | 10 | 10 | 10 | 10 |
| u_c^si | 0.85 | 0.85 | 0.85 | 0.85 |
| u_c | 0.13 | 0.13 | 0.13 | 0.13 |
| u_v | 0.04 | 0.04 | n/a (τ_v1⁻ = τ_v2⁻) | 0.04 |

- Fenton F, Karma A. "Vortex dynamics in three-dimensional continuous myocardium with fiber rotation: filament instability and fibrillation." *Chaos* 1998;8(1):20–47. https://doi.org/10.1063/1.166311
- Fenton FH, Cherry EM, Hastings HM, Evans SJ. "Multiple mechanisms of spiral wave breakup in a model of cardiac electrical activity." *Chaos* 2002;12(3):852–892. https://doi.org/10.1063/1.1504242 — **ten parameter sets** (Set 1–10) giving different breakup mechanisms (steep APD restitution, CV restitution, bistability, …); the paper states dx = 0.025 cm and dt = 0.1 ms (verify) with D = 0.001 cm²/ms. Use these sets for VF teaching scenarios.
- Nektar++ implementation (parameter transcription): https://doc.nektar.info/doxygen/5.5.0/_fenton_karma_8cpp_source.html

Properties: 3 ODEs, realistic APD/CV restitution (fitted to BR/LR-I), alternans, spiral
breakup → VF, explicit control of upstroke (τ_d), plateau (τ_si) and repolarisation (τ_r).
Explicit Euler `dt = 0.1–0.25 ms`, `dx = 0.25–0.5 mm` with `D = 0.1 mm²/ms` (CV ≈ 0.5 m/s);
on a 1–2 mm grid, increase τ_d to 0.5–1 ms to widen the front.

#### 2.3.5 Bueno-Orovio–Cherry–Fenton minimal model (BOCF/"4V", 2008)

Full equations (time constants in **seconds** in the paper's table; multiply by 1000 for ms):

```
∂u/∂t = ∇·(D∇u) − (J_fi + J_so + J_si)
dv/dt = (1 − H(u − θ_v)) (v_∞ − v)/τ_v⁻ − H(u − θ_v) v/τ_v⁺
dw/dt = (1 − H(u − θ_w)) (w_∞ − w)/τ_w⁻ − H(u − θ_w) w/τ_w⁺
ds/dt = ( (1 + tanh(k_s (u − u_s)))/2 − s ) / τ_s

J_fi = −v H(u − θ_v) (u − θ_v) (u_u − u) / τ_fi
J_so =  (u − u_o)(1 − H(u − θ_w))/τ_o + H(u − θ_w)/τ_so
J_si = −H(u − θ_w) w s / τ_si

τ_v⁻ = (1 − H(u − θ_v⁻)) τ_v1⁻ + H(u − θ_v⁻) τ_v2⁻
τ_w⁻ = τ_w1⁻ + (τ_w2⁻ − τ_w1⁻)(1 + tanh(k_w⁻ (u − u_w⁻)))/2
τ_so = τ_so1 + (τ_so2 − τ_so1)(1 + tanh(k_so (u − u_so)))/2
τ_s  = (1 − H(u − θ_w)) τ_s1 + H(u − θ_w) τ_s2
τ_o  = (1 − H(u − θ_o)) τ_o1 + H(u − θ_o) τ_o2
v_∞  = 1 − H(u − θ_v⁻)
w_∞  = (1 − H(u − θ_o))(1 − u/τ_w∞) + H(u − θ_o) w_∞*
V_m  = 85.7 u − 84 mV ;  D = 1.171 cm²/s = 0.1171 mm²/ms
```

| Parameter | Epi | Endo | Mid (M) |
|---|---|---|---|
| u_o | 0 | 0 | 0 |
| u_u | 1.55 | 1.56 | 1.61 |
| θ_v | 0.3 | 0.3 | 0.3 |
| θ_w | 0.13 | 0.13 | 0.13 |
| θ_v⁻ | 0.006 | 0.2 | 0.1 |
| θ_o | 0.006 | 0.006 | 0.005 |
| τ_v1⁻ (ms) | 60 | 75 | 80 |
| τ_v2⁻ (ms) | 1150 | 10 | 1.4506 |
| τ_v⁺ (ms) | 1.4506 | 1.4506 | 1.4506 |
| τ_w1⁻ (ms) | 60 | 6 | 70 |
| τ_w2⁻ (ms) | 15 | 140 | 8 |
| k_w⁻ | 65 | 200 | 200 |
| u_w⁻ | 0.03 | 0.016 | 0.016 |
| τ_w⁺ (ms) | 200 | 280 | 280 |
| τ_fi (ms) | 0.11 | 0.1 | 0.078 |
| τ_o1 (ms) | 400 | 470 | 410 |
| τ_o2 (ms) | 6 | 6 | 7 |
| τ_so1 (ms) | 30.0181 | 40 | 91 |
| τ_so2 (ms) | 0.9957 | 1.2 | 0.8 |
| k_so | 2.0458 | 2 | 2.1 |
| u_so | 0.65 | 0.65 | 0.6 |
| τ_s1 (ms) | 2.7342 | 2.7342 | 2.7342 |
| τ_s2 (ms) | 16 | 2 | 4 |
| k_s | 2.0994 | 2.0994 | 2.0994 |
| u_s | 0.9087 | 0.9087 | 0.9087 |
| τ_si (ms) | 1.8875 | 2.9013 | 3.3849 |
| τ_w∞ | 0.07 | 0.0273 | 0.01 |
| w_∞* | 0.94 | 0.78 | 0.5 |

(Transcribed from the Wikipedia table which reproduces Table 1 of the paper; the ordering of
the three columns there was partly garbled — verify against the paper. The paper also gives a
"TNNP-fit" and "PB-fit" set.)

- Bueno-Orovio A, Cherry EM, Fenton FH. "Minimal model for human ventricular action potentials in tissue." *J Theor Biol* 2008;253:544–560. https://doi.org/10.1016/j.jtbi.2008.03.029
- Model page: https://en.wikipedia.org/wiki/Bueno-Orovio%E2%80%93Cherry%E2%80%93Fenton_model ; PiGReads https://pigreads.readthedocs.io/en/stable/models/bueno2008minimal.html ; ModelDB 2014817.

Properties: 4 variables, fits human epi/endo/M APD and CV restitution, AP morphology
(including the epicardial notch via τ_so), alternans and spiral breakup; **this is the model
used in the Kaboudian–Cherry–Fenton real-time WebGL whole-heart demos.** Explicit Euler
dt = 0.1 ms, dx = 0.25–0.3 mm in the paper; it still works at 0.5 mm with D re-tuned.

- Kaboudian A, Cherry EM, Fenton FH. "Real-time interactive simulations of large-scale systems on personal computers and cell phones: Toward patient-specific heart modeling and other applications." *Sci Adv* 2019;5(3):eaav6019. https://doi.org/10.1126/sciadv.aav6019 — WebGL (Abubu.js) real-time 2D/3D BOCF/FK on consumer GPUs and phones; the proof that a GPU-fragment-shader RD solver on a masked voxel grid is real-time on integrated graphics.
- Kaboudian A, Cherry EM, Fenton FH. "Large-scale interactive numerical experiments of chaos, solitons and fractals in real time via GPU in a web browser." *Chaos, Solitons & Fractals* 2019;121:6–29. https://doi.org/10.1016/j.chaos.2019.01.005
- "Interactive cardiac dynamics" teaching demos: https://thevirtualheart.org/

#### 2.3.6 Which model can do what (summary)

| Model | Vars | Spiral (VT) | Breakup (VF) | APD restitution / alternans | Realistic AP shape | Coarse-grid tolerant (≥1 mm) | dt explicit Euler |
|---|---|---|---|---|---|---|---|
| FHN | 2 | yes | with heterogeneity | no | no | yes | ≤ 0.1 (dimless) |
| Aliev–Panfilov | 2 | yes | yes (a≈0.1, heterogeneity) | weak | plateau ok, no notch | **yes** (slow upstroke) | 0.01–0.05 (dimless) ≈ 0.1–0.6 ms |
| Mitchell–Schaeffer | 2 | yes | yes (steep restitution) | **yes, analytic** | ok | with τ_in ↑ | ≤ 0.1 ms |
| Fenton–Karma 3V | 3 | yes | **yes, 10 catalogued mechanisms** | yes | good | with τ_d ↑ | 0.1–0.25 ms |
| BOCF 4V | 4 | yes | yes | yes, human-fitted | very good, epi notch | 0.5 mm ok | 0.1 ms |
| TP06 | 19 | yes | yes | yes | biophysical | no (0.1–0.25 mm) | 0.02 ms (RL) |
| ORd 2011 | 41 | yes | yes | yes | biophysical | no | ≤ 0.01–0.02 ms adaptive |
| Courtemanche 1998 (atrial) | 21 | yes | yes (with remodelling) | yes | biophysical | no | 0.005–0.02 ms |

**T wave via APD heterogeneity:** every one of these models lets you set APD per node
(ε_0/μ_1 in AP; τ_close in MS; τ_si/τ_r in FK; τ_so/τ_si in BOCF; G_Ks/G_Kr/G_to in TP06).
Assign APD from a smooth field (apicobasal, transmural, LV/RV; §4.4) and the pseudo-ECG shows
a T wave of the right polarity.

### 2.4 Biophysical ionic models (why not for the whole heart on CPU)

| Model | State vars | Approx. cost per cell-step | Notes |
|---|---|---|---|
| ten Tusscher–Panfilov 2006 (TP06) | 19 | ~300–500 flops incl. exp/tanh, ~ 1 µs/step scalar | human ventricular epi/endo/M; dt 0.02 ms with Rush–Larsen; dx 0.1–0.25 mm; spiral breakup studied in this paper |
| O'Hara–Rudy 2011 (ORd) | 41 | ~2–3× TP06; stiff (I_Na, CaMK) | human ventricle, drug block studies; needs dt ≤ 0.01–0.02 ms or implicit |
| Courtemanche–Ramirez–Nattel 1998 | 21 | ~TP06 | human atrial; AF remodelling variants |
| Grandi 2010/2011, Tomek ToR-ORd 2019 | 40–50 | ≥ ORd | |

Whole-heart at 0.25 mm = 10–20 million nodes; ×50 sub-steps per frame × 1 µs = 500–1000 s of
CPU per frame. Even a 2 mm coarse grid (60k nodes) fails because the upstroke of these models
(dV/dt_max ≈ 300–400 V/s, upstroke ≈ 1 ms, wavefront ≈ 0.5 mm) cannot be resolved by dx = 2 mm:
propagation stalls or CV becomes grossly grid-dependent (the N-version benchmark shows CV error
already at 0.5 mm). Hence: **use ionic models only for the single-cell AP panel and for drug
effects** (block I_Kr → APD ↑ → long QT; block I_Na → upstroke ↓ → QRS widening; hyper-K⁺ →
resting potential ↑ → I_Na availability ↓ ; etc.), then *map the resulting APD / dV/dt_max /
plateau height to the phenomenological parameters of the tissue model* (τ_close, τ_si, τ_d…).
One TP06/ORd cell at 0.02 ms costs ~50 k steps/s ≈ 50 ms of CPU per second of simulated time:
trivial for a live single-cell panel.

- ten Tusscher KHWJ, Panfilov AV. "Alternans and spiral breakup in a human ventricular tissue model." *Am J Physiol Heart Circ Physiol* 2006;291:H1088–H1100. https://doi.org/10.1152/ajpheart.00109.2006 — CellML: https://models.cellml.org/e/80d
- ten Tusscher KHWJ, Noble D, Noble PJ, Panfilov AV. "A model for human ventricular tissue." *Am J Physiol Heart Circ Physiol* 2004;286:H1573–H1589. https://doi.org/10.1152/ajpheart.00794.2003
- O'Hara T, Virág L, Varró A, Rudy Y. "Simulation of the undiseased human cardiac ventricular action potential: model formulation and experimental validation." *PLoS Comput Biol* 2011;7(5):e1002061. https://doi.org/10.1371/journal.pcbi.1002061
- Courtemanche M, Ramirez RJ, Nattel S. "Ionic mechanisms underlying human atrial action potential properties: insights from a mathematical model." *Am J Physiol* 1998;275:H301–H321. https://doi.org/10.1152/ajpheart.1998.275.1.H301
- Rush S, Larsen H. "A practical algorithm for solving dynamic membrane equations." *IEEE Trans Biomed Eng* 1978;25:389–392. https://doi.org/10.1109/TBME.1978.326270 — exponential integration of Hodgkin–Huxley gates; lets TP06 run at 0.02–0.05 ms.
- Qu Z, Garfinkel A. "An advanced algorithm for solving partial differential equation in cardiac conduction." *IEEE Trans Biomed Eng* 1999;46:1166–1168. https://doi.org/10.1109/10.784149 — operator splitting + adaptive time step.

### 2.5 Hybrid: conduction-system graph feeding the myocardial mesh

This is the layer that actually produces "dozens of rhythms". Model the specialised
conduction system as a directed graph of **nodes with automaticity and refractoriness** and
**edges with delay, ERP and (optionally) decremental conduction**. Each node keeps
`last_activation`, `ERP`, `threshold`, `phase4_slope`; each edge keeps `delay(recovery_time)`,
`ERP`, `direction (antegrade / retrograde / both)`, `blocked` flag.

```
Nodes:  SAN → [CT, BB (Bachmann), IAT] → RA_wall, LA_wall → AVN_in
        AVN_in → AVN_fast → AVN_out ; AVN_in → AVN_slow → AVN_out        (dual pathways)
        AVN_out → His → { RBB → RV Purkinje terminals (k sites),
                          LBB → LAF → LV anterior/superior terminals,
                               → LPF → LV inferior/posterior terminals,
                               → septal fascicle }
        RA_wall → AP (accessory pathway) → LV/RV ventricular insertion site   (WPW)
        Ectopic foci: arbitrary nodes attached to a mesh node with their own automaticity
        Pacemaker: device node(s) with sensing/pacing timers attached to RA/RV/LV mesh nodes
```

Terminal nodes map to **breakthrough sites on the myocardial mesh** (Durrer 1970: three LV
endocardial early sites — anterior paraseptal, posterior paraseptal/base of the posterior
papillary, and mid-septal; RV: anterior free wall near the base of the anterior papillary
muscle; epicardial breakthrough at the anterior RV ≈ 20 ms after the earliest endocardial
activation). Within the mesh, fire the eikonal from all terminals with offsets matching the
Purkinje arrival times (RV terminals ≈ 5–10 ms after LV; apex before base).

- ten Tusscher KHWJ, Panfilov AV. "Modelling of the ventricular conduction system." *Prog Biophys Mol Biol* 2008;96:152–170. https://doi.org/10.1016/j.pbiomolbio.2007.07.026
- Cardone-Noott L, Bueno-Orovio A, Mincholé A, Zemzemi N, Rodriguez B. "Human ventricular activation sequence and the simulation of the electrocardiographic QRS complex and its variability in healthy and intraventricular block conditions." *Europace* 2016;18(suppl 4):iv4–iv15. https://doi.org/10.1093/europace/euw346 — root-node activation recipe reproducing Durrer; LBBB/RBBB by removing root nodes; shows which APD gradients the T wave needs.

#### 2.5.1 AV node: delay, Wenckebach, Mobitz, dual pathways

Use the **AV-nodal recovery curve** model: conduction time through the node is a function of
the recovery time `RT` = (time since last nodal/His output) and the node has a functional
refractory period below which input is blocked:

```
AVCT(RT) = AVCT_min + A · exp( −(RT − ERP_AVN) / τ_rec )        for RT ≥ ERP_AVN
blocked                                                         for RT <  ERP_AVN
Typical: AVCT_min = 80–120 ms, A = 150–400 ms, τ_rec = 50–120 ms, ERP_AVN = 250–400 ms
Fatigue (rate-dependent ERP lengthening): ERP_AVN += k_f · (number of recent conducted beats)
Concealed conduction: a blocked impulse still resets the node's RT by a fraction (0.3–0.7).
```
Iterating this map at an atrial cycle length `CL` produces 1:1 conduction with PR
lengthening (1st-degree block when AVCT > 200 ms), **Wenckebach** (progressively increasing
AVCT until RT < ERP → dropped beat, group beating, PR shortest after the pause), alternating
Wenckebach, 2:1, 3:1 … exactly as Shrier et al. demonstrated in patients.

- Shrier A, Dubarsky H, Rosengarten M, Guevara MR, Nattel S, Glass L. "Prediction of complex atrioventricular conduction rhythms in humans with use of the atrioventricular nodal recovery curve." *Circulation* 1987;76(6):1196–1205. https://doi.org/10.1161/01.CIR.76.6.1196
- Jørgensen P, Schäfer C, Guerra PG, Talajic M, Nattel S, Glass L. "A mathematical model of human atrioventricular nodal function incorporating concealed conduction." *Bull Math Biol* 2002;64:1083–1099. https://doi.org/10.1006/bulm.2002.0313
- Lian J, Müssig D, Lang V. "Computer modeling of ventricular rhythm during atrial fibrillation and ventricular pacing." *IEEE Trans Biomed Eng* 2006;53(8):1512–1520. https://doi.org/10.1109/TBME.2006.876627
- Lian J, Clifford GD, Müssig D, Lang V. "Open source model for generating RR intervals in atrial fibrillation and beyond." *BioMed Eng OnLine* 2007;6:9. https://doi.org/10.1186/1475-925X-6-9 — **open-source (PhysioNet) AV node + pacemaker model**: dual pathways, concealed conduction, retrograde conduction, ventricular pacing with sensing; produces AF RR histograms. Good blueprint for the timing engine.
- Climent AM, Guillem MS, Zhang Y, Millet J, Mazgalev TN. "Functional mathematical model of dual pathway AV nodal conduction." *Am J Physiol Heart Circ Physiol* 2011;300(4):H1393–H1401. https://doi.org/10.1152/ajpheart.01175.2010 — dual-pathway recovery curves with explicit fast/slow parameters.
- Masè M, Glass L, Disertori M, Ravelli F. "Nodal recovery, dual pathway physiology, and concealed conduction determine complex AV dynamics in human atrial tachyarrhythmias." *Am J Physiol Heart Circ Physiol* 2012;303(10):H1219–H1228. https://doi.org/10.1152/ajpheart.00228.2012
- Inada S, Hancox JC, Zhang H, Boyett MR. "One-dimensional mathematical model of the atrioventricular node including atrio-nodal, nodal, and nodal-His cells." *Biophys J* 2009;97(8):2117–2127. https://doi.org/10.1016/j.bpj.2009.06.056 — the biophysical alternative (not needed here).

**Mobitz II** is *not* an AV-nodal phenomenon; model it as an intermittent, all-or-none block
in the His/bundle edges (fixed AVCT, random or patterned dropped beats, often with a wide QRS
from coexisting BBB). **3rd-degree block:** AVN_out→His edge permanently blocked; the
downstream node with the highest automaticity (junctional 40–60 or ventricular 20–40 bpm)
takes over; the atria continue independently → AV dissociation with regular P and regular
(slower) QRS.

**AVNRT (dual pathways):** fast pathway: short delay (≈ 50 ms), long ERP (≈ 350 ms); slow
pathway: long delay (≈ 150–250 ms), short ERP (≈ 250 ms); both conduct retrogradely. A PAC
arriving at `RT < ERP_fast` but `> ERP_slow` conducts down the slow pathway (long PR), reaches
AVN_out when the fast pathway has recovered, conducts retrogradely up the fast pathway to
AVN_in, re-enters the slow pathway … → narrow-complex tachycardia at 150–250 bpm with P
buried in / just after the QRS (pseudo-r' in V1). Implement by allowing edges to be traversed
in both directions and letting the re-entrant loop continue while each edge recovers in time.

**AVRT / WPW (accessory pathway):** an extra edge from an atrial mesh node to a ventricular
mesh node, delay 20–40 ms, no decrement, ERP 250–350 ms, bidirectional. In sinus rhythm the
ventricular insertion is activated early *via slow myocardial propagation* (no Purkinje) →
short PR, slurred delta wave, QRS = fusion of pre-excited wavefront and the normal
His-Purkinje wavefront (so QRS width depends on the race between the two) — this falls out
automatically from the eikonal with two seed groups at different times. Delta-wave polarity
localises the pathway (left lateral: negative delta in I/aVL, positive V1 "type A"; right-sided
/ septal: negative V1 "type B"). Orthodromic AVRT: AVN antegrade, AP retrograde → narrow QRS
with retrograde P after the QRS (RP > 70 ms). Antidromic: AP antegrade → wide, fully
pre-excited QRS. Pre-excited AF: irregular, wide, very fast (AP ERP limits rate) — the AP ERP
parameter makes this a teachable danger scenario.

#### 2.5.2 Bundle branch blocks and fascicular blocks

Set the `blocked` flag on RBB / LBB / LAF / LPF edges, or set a long delay (incomplete block,
rate-dependent block when `delay` is a function of RT). The eikonal then activates that
ventricle from the other side through slow myocardium (0.3–0.6 m/s instead of 2–4 m/s), which
widens the QRS to 120–160 ms and produces the textbook morphologies **without any ECG-specific
hack**: LBBB → late, slow LV activation septum-from-right, broad notched R in I, aVL, V5–V6, QS
in V1, absent septal q, discordant ST–T. RBBB → terminal rightward/anterior forces: rSR' in V1,
wide S in I, V6. LAFB → LV activation proceeds inferior→superior: qR in I/aVL, rS in II/III/aVF,
axis −45° to −90°, QRS barely widened. LPFB → superior→inferior: rS in I/aVL, qR in III, axis
> +90°. Cardone-Noott 2016 validated exactly this root-node-removal approach.

#### 2.5.3 Automaticity, ectopic foci, PACs/PVCs, bigeminy, escape rhythms

Each automatic node integrates a phase-4 "clock":
```
φ(t) = (t − t_last_act) / CL_intrinsic ;  fires when φ ≥ 1 and node not refractory
Overdrive suppression: after being externally activated, CL_intrinsic is temporarily
  lengthened: CL_eff = CL_intrinsic · (1 + s·exp(−Δt/τ_ods)), s ≈ 0.1–0.3, τ_ods ≈ 2–10 s
Reset: an external activation resets φ to 0 (phase resetting; "non-compensatory pause"
  for PACs that reset the SAN; compensatory pause for PVCs that do not reach the SAN
  because of retrograde AV block or collision).
Sinus arrhythmia: CL_SAN(t) = CL_0 · (1 − a_resp · sin(2π f_resp t)), a_resp ≈ 0.05–0.15,
  f_resp ≈ 0.2–0.3 Hz (also feeds McSharry's HRV, §5).
Parasystole: a focus with entrance block (never reset) and exit block.
Bigeminy / trigeminy: a ventricular focus coupled to the previous QRS with fixed coupling
  interval (re-entrant PVC model) firing after every 1 / 2 sinus beats; or a focus with CL
  ≈ 2 × sinus CL.
```
A PVC is a breakthrough event at an arbitrary ventricular mesh node; the eikonal from that
single seed through slow myocardium gives a wide, bizarre QRS whose axis points *away* from
the focus (RVOT focus → LBBB-like morphology with inferior axis; LV apex focus → RBBB-like
with superior axis), with discordant T. **Monomorphic VT** = a ventricular focus (or a fixed
re-entrant circuit modelled as a focus with CL 300–400 ms) at 150–200 bpm; AV dissociation,
capture and fusion beats emerge if the SAN keeps running and the AV node is left
bidirectionally functional.

- Glass L, Mackey MC. *From Clocks to Chaos: The Rhythms of Life.* Princeton Univ. Press 1988. — phase-resetting and coupled-oscillator theory behind parasystole/bigeminy.
- Lerma C, Lee CF, Glass L, Goldberger AL. "The rule of bigeminy revisited: analysis in sudden cardiac death syndrome." *J Electrocardiol* 2007;40(1):78–88. https://doi.org/10.1016/j.jelectrocard.2006.04.011

#### 2.5.4 Pacemakers

Implement the standard timing cycles (NBG code; Bernstein et al. 2002):
```
Lower rate interval LRI (e.g. 1000 ms for 60 ppm), AV delay AVD (120–200 ms),
post-ventricular atrial refractory period PVARP (250–300 ms), ventricular blanking,
upper tracking rate.
VVI: if no sensed ventricular event within LRI since last paced/sensed V → pace V.
AAI: same on the atrial channel.
DDD: sense A & V; if no A within (LRI − AVD) → pace A; after A (sensed or paced) start AVD;
     if no V within AVD → pace V; after V start PVARP.
VOO/DOO (fixed/asynchronous): pace at LRI regardless of sensing (→ possible R-on-T).
Capture: a pace produces a breakthrough event at the lead-tip mesh node only if that node is
  not refractory and the output exceeds the threshold (adjustable → "loss of capture" drill).
Sensing: a chamber "senses" when the local mesh node activates (optionally with a sensitivity
  threshold vs. local EGM amplitude → "undersensing"/"oversensing" drills).
Fusion / pseudofusion fall out automatically from the eikonal when a pace and an intrinsic
  wavefront overlap.
Pacing spike: inject a 1–3 ms biphasic artefact into the ECG at the pace time
  (amplitude/polarity depends on unipolar vs bipolar lead and the lead vector; keep it a
  template, it is an artefact of the device, not of the tissue).
```
RV-apex pacing gives an LBBB-like, superior-axis QRS; RA-appendage pacing gives a normal P
after a spike; biventricular pacing narrows the QRS — all emerge from seed placement.

- Bernstein AD, Daubert J-C, Fletcher RD, Hayes DL, Lüderitz B, Reynolds DW, Schoenfeld MH, Sutton R. "The revised NASPE/BPEG generic code for antibradycardia, adaptive-rate, and multisite pacing." *PACE* 2002;25(2):260–264. https://doi.org/10.1046/j.1460-9592.2002.00260.x

#### 2.5.5 Refractoriness in the mesh

In Tier 1 each mesh node stores `t_act` and `APD`; it is refractory for `t < t_act + ERP`,
with `ERP ≈ APD + 10–30 ms` (post-repolarisation refractoriness can be a parameter). The
eikonal ignores refractory nodes as seeds and treats them as walls for the current front
(or as slow conductors in the *relative* refractory window `[APD − 50 ms, ERP]`, by
multiplying the local velocity by a recovery factor `r(t − t_act)` — this reproduces
rate-dependent aberrancy, Ashman's phenomenon and decremental conduction in a cheap way).
In Tier 2 refractoriness is intrinsic to the RD model.

---

## 3. The ECG forward problem (requirement B)

### 3.1 Sources: from transmembrane potential to the heart vector

In the bidomain/monodomain framework the extracellular current source density is
`−∇·(σ_i ∇V_m)`; in an unbounded homogeneous medium of conductivity σ_e the potential is

```
φ_e(x', t) = (1 / 4π σ_e) ∫_Ω  σ_i ∇V_m(x,t) · ∇_x (1 / |x' − x|)  dV           (Gima & Rudy 2002)
           = −(1 / 4π σ_e) ∫_Ω  ∇·(σ_i ∇V_m)  / |x' − x|  dV                     (monopole form)
```
(Gima & Rudy write it for a 1-D fibre of radius a as
`φ_e(x') = −K ∫ (∂V_m/∂x) ∂/∂x(1/r) dx`, `K = a² σ_i / (4 σ_e)`; sign conventions differ by
whether r is measured from source to field point.)

- Gima K, Rudy Y. "Ionic current basis of electrocardiographic waveforms: a model study." *Circ Res* 2002;90:889–896. https://doi.org/10.1161/hh0802.105200 (PMC1847799) — the pseudo-ECG; also the ionic explanation of T/U waves, QT, J waves.
- Plonsey R, Barr RC. *Bioelectricity: A Quantitative Approach.* 3rd ed. Springer 2007. — derivations.
- Malmivuo J, Plonsey R. *Bioelectromagnetism.* Oxford 1995. Free online: http://www.bem.fi/book/ — chapters 11 (lead vectors), 15 (12-lead), 16 (VCG, Frank), 19 (axis).

**Equivalent dipole / heart vector.** Far from the heart (|x'| ≫ heart size) the field is that of a
single dipole
```
p(t) = −∫_Ω σ_i ∇V_m(x,t) dV      (A·m)       ≈ −Σ_nodes σ_i ∇V_m,n(t) · vol_n
φ_e(x') ≈ p · x' / (4π σ_e |x'|³)
```
and with fibre anisotropy `σ_i = σ_il f fᵀ + σ_it (I − f fᵀ)` the gradient is weighted along
the fibre ("sum of transmembrane-potential gradients × fibre direction"). In a *bounded*
torso each lead measures `V_L(t) = c_L · p(t)` where `c_L` is the lead vector (Burger & van
Milaan 1946) — a fixed 3-vector per lead; this is the entire theory of the VCG/Frank
system and the Dower transform.

**Integration by parts → equivalent double layer (EDL).** If σ_i is uniform inside the closed
myocardial surface S and ∇²(1/r) = 0 away from the source,

```
∫_Ω ∇V_m · ∇(1/r) dV = ∮_S V_m ∂(1/r)/∂n dS      (for isotropic uniform σ_i)
⟹ φ_e(x') = −(σ_i / 4π σ_e) ∮_S V_m(y, t) · ( n̂(y) · (x' − y) / |x' − y|³ ) dS_y
           = −(σ_i / 4π σ_e) ∮_S V_m(y, t) dΩ(x', y)         (solid-angle form)
```
i.e. the potential equals the surface integral of V_m weighted by the solid angle each
surface patch subtends at the electrode — **a double layer of strength ∝ V_m on the closed
surface bounding the myocardium** (epicardium + endocardium of both ventricles + basal
cap). That is ECGSIM's source model (next section) *and* the cheapest exact-for-uniform-σ
way to compute an ECG from a surface-only mesh: precompute `w_{L,n}` once, then
`V_L(t) = Σ_n w_{L,n} V_m,n(t)`. Anisotropy adds a volume term ∇·(σ_i∇(1/r)) ≠ 0 which
modifies amplitudes modestly (Potse 2007); for teaching, the isotropic EDL is fine.

- Potse M, Dubé B, Vinet A. "Cardiac anisotropy in boundary-element models for the electrocardiogram." *Med Biol Eng Comput* 2009;47:719–729. https://doi.org/10.1007/s11517-009-0472-x

### 3.2 Lead field / boundary element formulation; ECGSIM

In a bounded, piecewise-homogeneous torso (torso σ ≈ 0.2 S/m, lungs 0.04–0.05, blood
0.6–0.7, muscle 0.1–0.4 anisotropic, fat 0.04; Keller et al. 2010) the Green's function is no
longer 1/r. Two equivalent ways to handle it:

1. **BEM transfer matrix (ECGSIM):** discretise the boundary surfaces (torso skin, lungs,
   cavities, ventricular surface S_v with N nodes) and solve the BEM once for the matrix
   `A (L × N)` giving electrode potentials from a unit double-layer strength at each S_v node.
   Then `Φ(L×T) = A · S(N×T)` where `S_n(t) = V_m,n(t)` (ECGSIM: each node's TMP is an
   analytic waveform with two markers — depolarisation time δ_n and repolarisation time ρ_n —
   plus amplitude). The original ECGSIM used 257 nodes on S_v and 300 thorax nodes / 64
   BSPM leads (verify; later versions ~1500 nodes), geometry from MRI of a healthy subject,
   plus data for atria in later versions. The user drags δ/ρ maps and sees the 12-lead/BSPM
   update instantly — exactly the educational interaction the product wants.
2. **Lead field / reciprocity (Potse 2018; Pezzuto 2017; Gillette 2021):** for each lead L solve
   *one* Laplace problem on the torso with unit current injected at the lead electrodes
   (reciprocity, McFee & Johnston 1953); the resulting field `Z_L(x) = ∇φ_recip` is the lead
   field; then `V_L(t) = −∫_Ω Z_L(x) · σ_i ∇V_m(x,t) dV`. Precompute `Z_L` at every mesh node
   (9 electrodes × N × 3 floats). This is the same precomputed-weights idea and lets you use
   a *volumetric* source without a BEM mesh.

- van Oosterom A, Oostendorp TF. "ECGSIM: an interactive tool for studying the genesis of QRST waveforms." *Heart* 2004;90(2):165–168. https://doi.org/10.1136/hrt.2003.014662 (PMC1768085). Software (free, Windows/macOS/Linux): https://www.ecgsim.org/
- van Oosterom A. "The equivalent double layer: source models for repolarization." In: Macfarlane PW et al. (eds) *Comprehensive Electrocardiology*, 2nd ed., Springer 2010, pp. 227–246. https://doi.org/10.1007/978-1-84882-046-3_7 — also van Oosterom A. "Genesis of the T wave as based on an equivalent surface source model." *J Electrocardiol* 2001;34(Suppl):217–227.
- van Dam PM, Oostendorp TF, Linnenbank AC, van Oosterom A. "Non-invasive imaging of cardiac activation and recovery." *Ann Biomed Eng* 2009;37(9):1739–1756. https://doi.org/10.1007/s10439-009-9747-5 (PMC2721141) — EDL applied to atria + ventricles; the "fastest route" (eikonal-like) activation initialisation.
- Potse M. "Scalable and accurate ECG simulation for reaction-diffusion models of the human heart." *Front Physiol* 2018;9:370. https://doi.org/10.3389/fphys.2018.00370 — lead fields can be downsampled ~100× with negligible loss.
- Keller DUJ, Weber FM, Seemann G, Dössel O. "Ranking the influence of tissue conductivities on forward-calculated ECGs." *IEEE Trans Biomed Eng* 2010;57(7):1568–1576. https://doi.org/10.1109/TBME.2010.2046485 — blood and lungs matter most; the rest changes amplitudes by < 10 %.
- Aras K, Good W, Tate J, Burton B, Brooks D, Coll-Font J, Doessel O, Schulze W, Potyagaylo D, Wang L, van Dam P, MacLeod R. "Experimental Data and Geometric Analysis Repository — EDGAR." *J Electrocardiol* 2015;48(6):975–981. https://doi.org/10.1016/j.jelectrocard.2015.08.008 — open torso geometries + electrode positions + BSPMs (SCI Utah). https://edgar.sci.utah.edu/

**Precomputation strategy for the product:** compute a `W (9 × N)` matrix of electrode
weights once per (geometry, electrode set) and cache it. With the infinite-medium EDL,
`W` is analytic (solid angles); with a BEM torso it is `A` from ECGSIM-style BEM. The
runtime cost is `9 × N` MACs per ECG sample. At N = 15k boundary nodes and 1 kHz that is
135 M MAC/s — negligible. Electrode positions are inputs to this precomputation, so
"electrode misplacement" exercises just swap rows or recompute `W` for the moved electrode
(solid-angle weights for one electrode over 15k nodes: ~15k × 10 flops — instantaneous).

### 3.3 The 12 leads from 9 electrode potentials

With electrode potentials `φ_RA, φ_LA, φ_LL, φ_V1…φ_V6` (RL is the driven-ground reference
and has no effect on the leads):

```
I   = φ_LA − φ_RA
II  = φ_LL − φ_RA
III = φ_LL − φ_LA               (= II − I, Einthoven's law)
WCT = (φ_RA + φ_LA + φ_LL) / 3   (Wilson central terminal)
aVR = φ_RA − (φ_LA + φ_LL)/2  = −(I + II)/2
aVL = φ_LA − (φ_RA + φ_LL)/2  =  I − II/2
aVF = φ_LL − (φ_RA + φ_LA)/2  =  II − I/2
V_k = φ_Vk − WCT ,  k = 1…6
```
Only 8 leads are independent (I, II, V1–V6); the other 4 are linear combinations.
Right-sided (V3R, V4R) and posterior (V7–V9) leads are just extra electrode positions.

**Electrode positions.** Clinical: V1 4th ICS right sternal border; V2 4th ICS left sternal
border; V3 midway V2–V4; V4 5th ICS mid-clavicular line; V5 anterior axillary line at V4
level; V6 mid-axillary line at V4 level; limb electrodes distal on limbs (or Mason–Likar
torso positions: infraclavicular fossae and left iliac fossa — note Mason–Likar shifts the
axis rightward and changes limb-lead amplitudes, a nice teaching point).

Illustrative default coordinates (cm) in a **heart-centred frame** (origin at the LV centre of
mass; x = patient's left, y = inferior (toward the feet), z = posterior — the Frank/VCG frame);
assumes a torso ellipse of half-width 17 cm and half-depth 11 cm whose centre is 3.5 cm to the
right of and 4 cm posterior to the heart centre. Replace with the real torso mesh when one is
adopted (§5.3), but these produce the right lead polarities:

| Electrode | x (left) | y (inferior) | z (posterior) |
|---|---|---|---|
| RA (right shoulder, Mason–Likar) | −17.5 | −25 | 4 |
| LA (left shoulder) | 10.5 | −25 | 4 |
| LL (left iliac) | 6.5 | 35 | 4 |
| V1 | −6.0 | −2 | −7.0 |
| V2 | −1.0 | −2 | −7.0 |
| V3 | 2.2 | −0.5 | −6.2 |
| V4 | 5.5 | 1 | −5.3 |
| V5 | 10.5 | 1 | −2.2 |
| V6 | 13.5 | 1 | 4.0 |

**Idealised frontal-plane lead vectors (Einthoven equilateral triangle, unit heart vector):**

```
I   = ( 1,      0     )      0°
II  = ( 0.5,    0.866 )    +60°
III = (−0.5,    0.866 )   +120°
aVR = (−0.866, −0.5   ) · (√3/2)    −150°
aVL = ( 0.866, −0.5   ) · (√3/2)     −30°
aVF = ( 0,      1     ) · (√3/2)     +90°
```
(Goldberger's augmented leads have magnitude √3/2 ≈ 0.866 of the bipolar leads in the ideal
triangle; hexaxial reference system = these six directions.)

### 3.4 Frank leads, the Dower transform and its inverses

Frank (1956) electrodes: **A** (left mid-axillary), **C** (45° between A and E, left anterior),
**E** (front midline), **I** (right mid-axillary), **M** (back midline), all at the 4th/5th ICS
level; **H** (neck/back of head), **F** (left leg). The resistor network gives orthogonal X
(left), Y (down), Z (back) leads with approximately equal lead-vector magnitude.

**Dower forward matrix** (12-lead from VCG, `v_8 = D · [X Y Z]ᵀ`; rows I, II, V1…V6; Dower
et al. 1980 — verify against the paper; values below are the widely reproduced ones):

```
         X        Y        Z
I     0.632   −0.235    0.059
II    0.235    1.066   −0.132
V1   −0.515    0.157   −0.917
V2    0.044    0.164   −1.387
V3    0.882    0.098   −1.277
V4    1.213    0.127   −0.601
V5    1.125    0.127   −0.086
V6    0.831    0.076    0.230
(III, aVR, aVL, aVF from I and II as in §3.3)
```
Each row *is* the lead vector `c_L` of that lead in the Frank frame — a ready-made set of
lead vectors for a Tier-0 rotating-dipole ECG (§5).

**Inverse Dower matrix** (VCG from 8 leads; Edenbrandt & Pahlm 1988 — pseudo-inverse of D;
columns I, II, V1…V6):

```
X   0.156  −0.010  −0.172  −0.074   0.122   0.231   0.239   0.194
Y  −0.227   0.887   0.057  −0.019  −0.106  −0.022   0.041   0.048
Z   0.022   0.102  −0.229  −0.310  −0.246  −0.063   0.055   0.108
```

**Kors regression matrix** (Kors et al. 1990; preferred over inverse Dower for deriving a VCG
from a measured 12-lead; same column order):

```
X   0.38  −0.07  −0.13   0.05  −0.01   0.14   0.06   0.54
Y  −0.07   0.93   0.06  −0.02  −0.05   0.06  −0.17   0.13
Z   0.11  −0.23  −0.43  −0.06  −0.14  −0.20  −0.11   0.31
```

- Frank E. "An accurate, clinically practical system for spatial vectorcardiography." *Circulation* 1956;13(5):737–749. https://doi.org/10.1161/01.CIR.13.5.737
- Dower GE, Machado HB, Osborne JA. "On deriving the electrocardiogram from vectorcardiographic leads." *Clin Cardiol* 1980;3(2):87–95. https://doi.org/10.1002/clc.4960030203
- Edenbrandt L, Pahlm O. "Vectorcardiogram synthesized from a 12-lead ECG: superiority of the inverse Dower matrix." *J Electrocardiol* 1988;21(4):361–367. https://doi.org/10.1016/0022-0736(88)90113-6
- Kors JA, van Herpen G, Sittig AC, van Bemmel JH. "Reconstruction of the Frank vectorcardiogram from standard electrocardiographic leads: diagnostic comparison of different methods." *Eur Heart J* 1990;11(12):1083–1092. https://doi.org/10.1093/oxfordjournals.eurheartj.a059647
- Comparison and tables: Jaros R, Martinek R, Danys L. "Comparison of different electrocardiography with vectorcardiography transformations." *Sensors* 2019;19(14):3072. https://doi.org/10.3390/s19143072 (open access; reproduces Dower, inverse Dower, Kors matrices)
- Einthoven W, Fahr G, de Waart A. "Über die Richtung und die manifeste Grösse der Potentialschwankungen im menschlichen Herzen…" *Pflügers Arch* 1913;150:275–315. — Wilson FN et al. "Electrocardiograms that represent the potential variations of a single electrode." *Am Heart J* 1934;9:447–458. — Goldberger E. "A simple, indifferent, electrocardiographic electrode of zero potential and a technique of obtaining augmented, unipolar, extremity leads." *Am Heart J* 1942;23:483–492. — Burger HC, van Milaan JB. "Heart-vector and leads." *Br Heart J* 1946;8:157–161.

### 3.5 Genesis of each wave, and how to get it from the simulation

**P wave.** Atrial activation starts at the SA node (high posterolateral RA near the SVC),
spreads over the RA via the crista terminalis/pectinate muscles, reaches the LA chiefly via
**Bachmann's bundle** (anterior-superior interatrial band) with contributions via the fossa
ovalis and the coronary-sinus musculature; total ≈ 80–110 ms; LA activation ends last on the
posterolateral LA. The P vector therefore points left-inferior-anterior then posterior:
upright in I, II, aVF, V4–V6, inverted in aVR, **biphasic in V1** (initial positive RA,
terminal negative LA). For the atria use a thin-walled surface mesh with
CV ≈ 0.9–1.0 m/s (Bachmann's bundle 1.5–1.7 m/s as a fast edge), APD ≈ 250 ms, AP amplitude
~ 80–100 mV. Atrial repolarisation (Ta wave) is small and opposite to P; it becomes visible
as PR-segment depression in pericarditis and as apparent ST depression at fast atrial rates
(teaching point). Pathology recipes: RA enlargement → tall peaked P (RA mass ↑ / slower RA
conduction); LA enlargement → wide notched P, deep terminal negativity in V1 (LA delay ↑);
inverted P in II (junctional/low-atrial focus: seed at the AV node region, atria activated
retrogradely, short/negative PR); ectopic atrial P morphologies from seed placement.

- Lemery R, Birnie D, Tang ASL, Green M, Gollob M, Hendry M, Lau E. "Normal atrial activation and voltage during sinus rhythm in the human heart: an endocardial and epicardial mapping study in patients with a history of atrial fibrillation." *J Cardiovasc Electrophysiol* 2007;18(4):402–408. https://doi.org/10.1111/j.1540-8167.2007.00762.x
- Loewe A, Krueger MW, Holmqvist F, Dössel O, Seemann G, Platonov PG. "Influence of the earliest right atrial activation site and its proximity to interatrial conduction pathways on the P-wave morphology." *Europace* 2016;18(suppl 4):iv35–iv43. https://doi.org/10.1093/europace/euw349 — simulated P waves vs SAN exit site and pathways.

**QRS.** Durrer's sequence: LV endocardial breakthrough at three sites (0–5 ms), septum
activated left→right (initial septal vector rightward/anterior → small q in I, V5–V6; small r
in V1), then apex and both free walls endo→epi with the massive LV free wall dominating the
mean vector (left, inferior, posterior → tall R in I, II, V5–V6, deep S in V1–V2, transition
V3–V4), finally the posterobasal LV and RVOT (terminal rightward/superior → small s in V6,
r' in aVR). Total 60–90 ms. **QRS widening in BBB** arises simply because the eikonal must
traverse myocardium at 0.3–0.6 m/s instead of Purkinje at 2–4 m/s: the delayed ventricle
contributes an unopposed late vector (LBBB: late leftward-posterior; RBBB: late
rightward-anterior). Hypertrophy (LVH/RVH): increase the wall thickness/mass term (node
volumes / number of nodes) or scale the dipole weight of that region; LVH → higher R in
V5–V6/S in V1 (Sokolow–Lyon ≥ 35 mm), left axis, "strain" T inversion from a reversed
transmural APD gradient; RVH → right axis, dominant R in V1, deep S in V6.

**T wave.** Repolarisation has no propagating front; the T vector is the volume integral of
∇V_m during phase 3, i.e. it is determined by the *gradient of repolarisation times*
`T_rep(x) = T_act(x) + APD(x)`. If APD were uniform, repolarisation would follow activation
and the T wave would be discordant (opposite to QRS). The healthy upright T in I, II, V3–V6
requires repolarisation to run roughly *opposite* to activation, i.e. **regions activated
late must have shorter APD**: (i) **transmural**: epicardial APD < M-cell < endocardial APD by
~20–40 ms (Yan & Antzelevitch 1998: the epicardium repolarises first → T peak; M cells last
→ T end; the transmural gradient sets the T width and T_peak–T_end); (ii) **apicobasal**: apex
APD shorter than base by ~20–40 ms (Ramanathan et al. 2006 ECGI; Keller et al. 2012 found a
purely apicobasal setup with shorter apical APD gave the best match; Okada et al. 2011 needed
transmural + moderate apicobasal; Cardone-Noott 2016 found transmural, apicobasal,
interventricular and anteroposterior gradients all contribute); (iii) RV APD slightly shorter
than LV. Note the experimental literature disagrees in detail (Franz 1987 found APD *longest*
where activation is earliest, i.e. an activation–APD anticorrelation; Opthof et al. 2017
dispute the in-vivo transmural gradient) — for a teaching simulator the simulation-validated
recipe below is adequate and lets the user see what each gradient does:

```
APD(x) = APD_0
       + ΔAPD_tm · (ρ(x) − 0.5)          ρ ∈ [0 endo, 1 epi]   (ΔAPD_tm ≈ −30 ms, i.e. epi shorter)
       + ΔAPD_ab · (z(x) − 0.5)          z ∈ [0 apex, 1 base]   (ΔAPD_ab ≈ +30 ms, base longer)
       + ΔAPD_lr · 1_RV(x)               (ΔAPD_lr ≈ −10…−20 ms)
       + heart-rate dependence: APD = APD_0 · (RR/1000)^0.3 … or Bazett/Fridericia-like
Typical APD_0 ≈ 280–300 ms at 60 bpm; ρ and z come from universal ventricular coordinates
(UVC, Bayer et al. 2018) or from two Laplace solves on the mesh (§5.4).
```
Sanity checks: T upright in I, II, V3–V6; inverted in aVR; T axis within ~45° of the QRS axis;
QT ≈ 400 ms at 60 bpm. Then every repolarisation pathology is a modification of the APD field
or the AP template:

| Pathology | Simulation change |
|---|---|
| Long QT (congenital / drug) | APD_0 ↑ (e.g. +80 ms); optionally APD dispersion ↑ (M-cell layer APD ↑↑) → broad/notched T; torsades from EADs needs Tier 2 with reduced repolarisation reserve |
| Hyperkalaemia | V_rest ↑ (−85 → −70 mV), APD ↓, phase-3 slope ↑ → tall narrow peaked T; P flattening (atrial CV ↓, amplitude ↓), PR ↑, QRS widening (CV ↓ 30–60 %) → sine wave; asystole |
| Hypokalaemia | APD ↑ with flattened phase 3 → low flat T, prominent U, apparent QT(U) ↑, mild ST depression |
| Hypercalcaemia / hypocalcaemia | APD ↓ (short ST, short QT) / APD ↑ (long ST segment, normal T) — change plateau duration only |
| Digoxin effect | APD ↓ with scooped ("reverse tick") ST: subtract a sagging ST template; PR ↑ (AV delay ↑); bradycardia |
| Brugada | RVOT epicardial AP with loss of dome (large I_to): set RVOT-epi template to a short, triangular AP → coved ST elevation and negative T in V1–V3 (Yan & Antzelevitch 1999) |
| Early repolarisation / J wave | epicardial notch deepened (BOCF τ_so / I_to) at inferolateral LV → J-point elevation |
| LVH strain | reverse transmural gradient in the thick LV (epi APD longer) → asymmetric T inversion V5–V6 |
| Cerebral T waves, hypothermia (Osborn) | template tweaks: deep wide T; J wave |

- Ramanathan C, Jia P, Ghanem R, Ryu K, Rudy Y. "Activation and repolarization of the normal human heart under complete physiological conditions." *PNAS* 2006;103(16):6309–6314. https://doi.org/10.1073/pnas.0601533103
- Franz MR, Bargheer K, Rafflenbeul W, Haverich A, Lichtlen PR. "Monophasic action potential mapping in human subjects with normal electrocardiograms: direct evidence for the genesis of the T wave." *Circulation* 1987;75(2):379–386. https://doi.org/10.1161/01.CIR.75.2.379
- Keller DUJ, Weiss DL, Dössel O, Seemann G. "Influence of I_Ks heterogeneities on the genesis of the T-wave: a computational evaluation." *IEEE Trans Biomed Eng* 2012;59(2):311–322. https://doi.org/10.1109/TBME.2011.2168397
- Okada J, Washio T, Maehara A, Momomura S, Sugiura S, Hisada T. "Transmural and apicobasal gradients in repolarization contribute to T-wave genesis in human surface ECG." *Am J Physiol Heart Circ Physiol* 2011;301(1):H200–H208. https://doi.org/10.1152/ajpheart.01241.2010
- Opthof T, Remme CA, Jorge E, Noriega F, Wiegerinck RF, Tasiam A, Beekman L, Alvarez-Garcia J, Cinca J, Coronel R. "Cardiac activation–repolarization patterns and ion channel expression mapping in intact isolated normal human hearts." *Heart Rhythm* 2017;14(2):265–272. https://doi.org/10.1016/j.hrthm.2016.10.010
- Yan GX, Antzelevitch C. "Cellular basis for the Brugada syndrome and other mechanisms of arrhythmogenesis associated with ST-segment elevation." *Circulation* 1999;100(15):1660–1666. https://doi.org/10.1161/01.CIR.100.15.1660

**U wave.** Small (0.1–0.3 mV), same polarity as T, 20–40 ms after T end, biggest in V2–V3,
prominent in bradycardia and hypokalaemia, inverted in ischaemia/LVH. Mechanisms proposed:
late repolarisation of the Purkinje system / M cells (Gima & Rudy reproduce it from
Purkinje APD), or mechano-electric feedback. Practical: add a small late-repolarising
population (e.g. Purkinje terminal nodes with APD ≈ T_end + 30 ms and small dipole weight)
or simply a Gaussian template at `T_end + 30 ms` with amplitude tied to the hypokalaemia /
bradycardia parameters.

- Ritsema van Eck HJ, Kors JA, van Herpen G. "The U wave in the electrocardiogram: a solution for a 100-year-old riddle." *Cardiovasc Res* 2005;67(2):256–262. https://doi.org/10.1016/j.cardiores.2005.04.010

**ST segment: ischaemia and infarction.** Acute ischaemia changes the cells in the ischaemic
zone: extracellular K⁺ ↑ → resting potential ↑ (−85 → −65…−70 mV), APD ↓ (ATP-sensitive K⁺
current), amplitude ↓, CV ↓. Both the *diastolic* difference (ischaemic cells less negative
at rest → "injury current" flowing during TQ → **TQ depression**, seen as ST elevation after
AC coupling / baseline reset) and the *systolic* difference (shorter, lower plateau → current
during ST) produce a vector pointing from the ischaemic zone toward normal tissue in
diastole and the reverse in systole; after baseline correction the net effect is **ST
elevation in leads facing a transmural (subepicardial-to-transmural) ischaemic zone** and
**ST depression (reciprocal) in opposite leads** and in **subendocardial** ischaemia (the
injury vector then points away from the overlying electrode toward the cavity). With a
template-AP Tier 1 this is purely a per-node template change inside a region mask:

```
Ischaemic/injury zone mask M(x) ∈ [0,1] (smooth border zone ~5–10 mm)
V_rest(x) = −85 + 20·M     mV
V_plateau(x) = +20 − 15·M  mV
APD(x) = APD_normal − 60·M ms
CV scale(x) = 1 − 0.5·M   (and conduction block at M≈1 for infarct → Q waves)
Transmural extent parameter: M applied from endocardium to a fraction f_tm of the wall
  (f_tm < 1 → ST depression "NSTEMI/ischaemia"; f_tm = 1 → ST elevation "STEMI")
```
Territories (coronary masks on the mesh, using AHA 17-segment labels from UVCs):
LAD → anterior/septal/apical (V1–V4, ± I, aVL; reciprocal II/III/aVF); RCA → inferior
(II, III, aVF; III > II; reciprocal I, aVL; RV infarct: V4R); LCx → lateral (I, aVL, V5–V6)
and posterior (reciprocal ST depression and tall R in V1–V3, elevation in V7–V9). Chronic
infarct: electrically silent (dipole weight 0, no propagation) region → pathologic Q waves
(the "window" effect) and persistent T inversion from the border-zone APD change.

- Loewe A, Hunter PJ, Kohl P. "Cardiac ischemia—insights from computational models." *Herzschrittmachertherapie + Elektrophysiologie* 2018;29:31–39. https://doi.org/10.1007/s00399-017-0539-6 (open access review of ischaemia ECG simulation)
- Hopenfeld B, Stinstra JG, MacLeod RS. "Mechanism for ST depression associated with contiguous subendocardial ischemia." *J Cardiovasc Electrophysiol* 2004;15(10):1200–1206. https://doi.org/10.1046/j.1540-8167.2004.04072.x
- Janse MJ, Kléber AG. "Electrophysiological changes and ventricular arrhythmias in the early phase of regional myocardial ischemia." *Circ Res* 1981;49(5):1069–1081. https://doi.org/10.1161/01.RES.49.5.1069

**Pericarditis:** diffuse epicardial injury → mild concave ST elevation in nearly all leads
except aVR/V1 (uniform thin subepicardial injury layer, M applied to the outer 10–20 % of the
whole epicardium) plus **PR depression** (atrial epicardial injury; PR elevation in aVR).

### 3.6 Frontal-plane axis, axis deviation, electrode misplacement

Axis from any two frontal leads via their lead-vector angles. With the ideal hexaxial
system and the net QRS areas (or peak amplitudes) in I and aVF:

```
θ = atan2( aVF / (√3/2),  I )          (exact in the ideal Einthoven triangle)
  ≈ atan2( aVF, I )                     (common approximation; error ≤ ~7°)
  = atan2( (2·II − I)/√3,  I )          (from I and II)
Normal: −30° ≤ θ ≤ +90°; LAD: −30°…−90°; RAD: +90°…+180°; extreme/"north-west": −90°…±180°.
```
In the simulator compute θ from the *simulated* I and aVF with the same formula the student
is taught (net area of the QRS), and also display the "true" axis of the mean QRS heart vector
`p̄ = ∫_QRS p(t) dt` projected on the frontal plane — their small discrepancy is itself
educational.

**Axis deviation scenarios:** LAFB (−45°…−90°), LPFB (>+90°), LBBB (often LAD), RBBB (normal
or RAD), LVH (LAD), RVH / PE / COPD (RAD), inferior MI (LAD from loss of inferior forces),
lateral MI (RAD), WPW (depends on pathway), ventricular ectopy (depends on focus), dextrocardia
(RAD with reversed R-progression), **tall/obese vs thin** (horizontal vs vertical heart —
rotate the heart mesh by ±20–30° about the AP axis), pregnancy. All of these are produced by
changing seeds/blocks/masks or by rotating the heart in the torso frame.

**Lead reversal / misplacement** (implement as permutations of the 9 electrode potentials
*before* forming the leads — nothing else changes):

| Swap | Result |
|---|---|
| RA ↔ LA | I inverted; II ↔ III; aVR ↔ aVL; aVF unchanged; precordials unchanged → "P inverted in I with normal V6" (vs dextrocardia where V6 is also abnormal) |
| RA ↔ LL | I = −III, II inverted, III = −I; aVR ↔ aVF; aVL unchanged |
| LA ↔ LL | I = II, II = I, III inverted; aVL ↔ aVF; aVR unchanged (often unnoticed) |
| RA ↔ RL (neutral) | lead II ≈ flat line (near-zero), I ≈ −III, III ≈ −III... (II records LL–RL ≈ 0) |
| LA ↔ RL | lead III ≈ flat line |
| V1 ↔ V2, V3 ↔ V4, etc. | broken R-wave progression |
| V1/V2 too high (2nd ICS) | rSr' / incomplete-RBBB-like pattern, negative P in V1, Brugada-mimic |
| Dextrocardia (true) | mirror the heart about the sagittal plane: I inverted, RAD, reversed R progression; right-sided leads normalise |

- Batchvarov VN, Malik M, Camm AJ. "Incorrect electrode cable connection during electrocardiographic recording." *Europace* 2007;9(11):1081–1090. https://doi.org/10.1093/europace/eum198
- Rudiger A, Hellermann JP, Mukherjee R, Follath F, Turina J. "Electrocardiographic artifacts due to electrode misplacement and their frequency in different clinical settings." *Am J Emerg Med* 2007;25(2):174–178. https://doi.org/10.1016/j.ajem.2006.06.018
- Gillette K, Gsell MAF, Bouyssier J, Prassl AJ, Neic A, Vigmond EJ, Plank G. "Influence of electrode placement on the morphology of in-silico 12 lead ECGs." *Computing in Cardiology* 2021;48. https://www.cinc.org/2021/Program/accepted/7.html — V1–V3 are most sensitive to position errors.

---

## 4. Rhythm-generation layer: template synthesis and timing engines (requirement C)

### 4.1 McSharry et al. 2003 dynamical model (ECGSYN)

A limit-cycle oscillator in the (x, y) plane whose angular position θ drives z (the ECG) through
a sum of Gaussians placed at fixed angles for P, Q, R, S, T:

```
ẋ = α x − ω y
ẏ = α y + ω x
ż = − Σ_{i ∈ {P,Q,R,S,T}} a_i Δθ_i exp( −Δθ_i² / (2 b_i²) ) − ( z − z_0(t) )

α   = 1 − sqrt(x² + y²)
θ   = atan2(y, x),   Δθ_i = (θ − θ_i) mod 2π   (wrapped to (−π, π])
ω   = 2π / RR(t)      (angular velocity; one revolution = one beat; RR from the HRV generator)
z_0(t) = A sin(2π f_resp t)    (respiratory baseline wander; A ≈ 0.15 mV in ECGSYN code, verify)
```

Table I (McSharry 2003, Table 1; repeated in Clifford/McSharry ECGSYN paper):

| Wave | time (s) | θ_i (rad) | a_i | b_i |
|---|---|---|---|---|
| P | −0.2 | −π/3 | 1.2 | 0.25 |
| Q | −0.05 | −π/12 | −5.0 | 0.1 |
| R | 0 | 0 | 30.0 | 0.1 |
| S | 0.05 | π/12 | −7.5 | 0.1 |
| T | 0.3 | π/2 | 0.75 | 0.4 |

Heart-rate dependent morphology (ECGSYN): with `α_hr = sqrt(h_mean/60)`,
`θ_P = −(π/3)·sqrt(α_hr)`, `θ_Q = −(π/12)·α_hr`, `θ_S = (π/12)·α_hr`, `θ_T = π/2` (verify:
the ECGSYN paper scales T too in later versions), `b_i ← b_i·α_hr` — i.e. the waves narrow and
move toward the R peak as the rate rises (QT shortening).

RR-interval generator (bimodal HRV spectrum):
```
S(f) = σ_1²/(sqrt(2π) c_1) · exp(−(f − f_1)²/(2 c_1²))  +  σ_2²/(sqrt(2π) c_2) · exp(−(f − f_2)²/(2 c_2²))
f_1 = 0.1 Hz (Mayer/LF), f_2 = 0.25 Hz (respiratory/HF), c_1 = c_2 = 0.01 Hz (0.1 in ECGSYN defaults),
LF/HF = σ_1²/σ_2² = 0.5, mean HR 60 bpm, SD 1 bpm (3 bpm in the paper's figure);
RR series = inverse FFT of sqrt(S(f)) with random phases, rescaled to the target mean/SD.
Integration: RK4 at f_int = 512 Hz, downsample to f_ecg = 256 Hz; add uniform noise (0.1 mV).
```
Output in z is in mV-like units after scaling so that the R peak ≈ 1 mV (ECGSYN scales z to
[−0.4, 1.2] mV). The model's great virtue for a *rhythm* trainer: every beat has an
RR-interval, and arbitrary beat types (PVC, PAC, paced) are just different `(θ_i, a_i, b_i)`
sets selected per beat by the timing engine. Its limitation: single-lead; no physiology.

- McSharry PE, Clifford GD, Tarassenko L, Smith LA. "A dynamical model for generating synthetic electrocardiogram signals." *IEEE Trans Biomed Eng* 2003;50(3):289–294. https://doi.org/10.1109/TBME.2003.808805 — PDF: https://www.lse.ac.uk/CATS/Assets/PDFs/Publications/Papers/2003/53-DynamicModelGenECG-2003-Mchsharry-etal.pdf
- Clifford GD, McSharry PE. "Generating 24-hour ECG, BP and respiratory signals with realistic linear and non-linear clinical features using a nonlinear model." *Comput Cardiol* 2004;31:709–712. — McSharry PE, Clifford GD. "Open-source software for generating electrocardiogram signals." arXiv:physics/0406017, 2004. https://arxiv.org/abs/physics/0406017 — ECGSYN code (C/Matlab/Java, GPL): https://physionet.org/content/ecgsyn/
- Sameni R, Clifford GD, Jutten C, Shamsollahi MB. "Multichannel ECG and noise modeling: application to maternal and fetal ECG signals." *EURASIP J Adv Signal Process* 2007;2007:043407. https://doi.org/10.1155/2007/43407 — **3-D extension**: a rotating dipole `d(θ) = (Σ_i a_i^x G_i(θ), Σ a_i^y G_i(θ), Σ a_i^z G_i(θ))` with separate Gaussian sets per axis, then any lead = `c_L · R · d(θ)` with Dower-style lead vectors → a cheap *12-lead* Tier 0 with correct axis behaviour (rotate R to change the axis).
- Clifford GD, Shoeb A, McSharry PE, Janz BA. "Model-based filtering, compression and classification of the ECG." *Int J Bioelectromagn* 2005;7(1):158–161. — Gaussian-sum fitting of real beats (gives you per-pathology parameter sets by fitting PTB-XL beats).
- Sayadi O, Shamsollahi MB, Clifford GD. "Synthetic ECG generation and Bayesian filtering using a Gaussian wave-based dynamical model." *Physiol Meas* 2010;31(10):1309–1329. https://doi.org/10.1088/0967-3334/31/10/002 — more Gaussians (P₁, P₂, T₁, T₂ …) for asymmetric waves.
- "ECGSim-Advanced" (open-source R Shiny teaching simulator, Gaussian sums for electrolyte/AV-block/QT patterns), *F1000Research* 2026;15:714. https://f1000research.com/articles/15-714 — an example of a template tool for these very pathologies.
- Commercial rhythm simulators (e.g. "Cardiac Rhythm Simulator", simECG https://simecg.sourceforge.net/, svtsim https://svtsim.com/) are template/timing engines of this kind; svtsim uses a node network (SAN, AVN, pathways) to teach SVT mechanisms with ladder diagrams — a good UI reference for the timing engine.

### 4.2 Timing-based beat generators

The industry pattern (also PhysioNet's `ecgsyn`, Lian 2007, simECG) is:

```
loop:
  next_event = min over automatic nodes of their next firing time
  propagate through the graph (delays, ERPs, blocks) → list of chamber activations
      (A at t_a with "morphology id", V at t_v with "morphology id")
  for each chamber activation, append a beat to the strip with the template for that morphology
      (normal P / ectopic P / retrograde P; normal QRS-T / LBBB / RBBB / PVC-from-site-k / paced)
  rate-dependent adjustments: QT(RR), PR(RR) from the AV recovery curve, T amplitude(RR)
```
Fibrillatory baselines: AF f-waves = band-limited noise (4–9 Hz, 0.05–0.2 mV) or a sum of
sinusoids with jittered phase; flutter F waves = sawtooth at 250–350/min, negative in
II/III/aVF for typical CCW; VF = chaotic 3–8 Hz signal (e.g. Rössler/Lorenz-modulated
sinusoids, or sum of 3 sinusoids with drifting frequencies) decaying from coarse to fine;
torsades = polymorphic QRS with sinusoidally rotating amplitude/polarity (period 5–20 beats)
— in the 3-D dipole formulation simply rotate the dipole axis by `Δφ` per beat.

### 4.3 Keeping the strip and the 3-D model consistent (hybrid)

Make the *timing engine the single source of truth*. It emits events
`Activation { site_id, t, origin: Sinus | Ectopic(k) | Paced | Reentry }` and
`Block { edge, t }`. Consumers:

- Tier 0 strip: maps `(site_id, origin, preceding RR)` → template id, draws the beat.
- Tier 1: seeds the eikonal from `site_id`'s mesh nodes at time `t`; the ECG is computed
  from the mesh, so the 12-lead *is* consistent with the 3-D view by construction. The Tier-0
  strip is then only a *fallback*, and the recommendation is to display Tier-1 ECG whenever
  the 3-D model is running.
- Ladder diagram (Lewis diagram) panel: drawn directly from the graph events — invaluable for
  teaching AV block, AVNRT and concealed conduction.

The one place where Tier 1 cannot follow the timing engine is sustained *functional*
re-entry (VF, polymorphic VT, true AF wavelets); there the timing engine delegates ventricular
(or atrial) activation to Tier 2 and *reads back* local activations at the sensing sites
(e.g. for pacemaker sensing, AV node retrograde input) from the RD solver.

---

## 5. Geometry, fibres, conduction system and validation data (requirement D)

### 5.1 Open heart meshes

| Dataset | Content | Resolution / size | Fibres | Conduction system | Torso + electrodes | Format | Licence |
|---|---|---|---|---|---|---|---|
| **Strocchi et al. 2020 four-chamber cohort** (KCL), Zenodo 3890034 | 24 four-chamber hearts (HF patients, CT) with element tags (LV, RV, LA, RA, valves, great vessels), ventricular fibre + sheet, UVCs, RV CRT electrode label | coarse 1.1 mm edge (~10⁶ tets), fine 0.39 mm; 22.5 GB total, 0.7–1.3 GB per heart | yes (rule-based, 80° endo → −60° epi) | no | no | EnSight .case (coarse), VTK (fine) | **CC BY 4.0** |
| **Rodero et al. 2021 healthy cohort** (KCL), Zenodo 4590294 / 4593739 ("Virtual cohort of extreme and average four-chamber heart meshes from statistical shape model") | 20 healthy four-chamber hearts + SSM-generated extremes | ~1 mm tets | yes | no | no | VTK/CARP | CC BY 4.0 (verify on record) |
| **Cardiac Atlas Project** (Auckland / Bai et al. 2015 biventricular atlas) | statistical shape models (LV, biventricular) from UK Biobank / MESA | surface meshes | no | no | no | VTK/obj | mixed: software MPL 1.1; Bai 2015 atlas CC BY-NC-ND 4.0 — **not usable commercially** without permission |
| **Karlsruhe (KIT IBT) biventricular SSM adapted for simulation**, Zenodo 4506463 | biventricular SSM adapted from Bai 2015 | surfaces + volumetric | tools provided | no | no | VTK | inherits Bai's NC terms (verify) |
| **BodyParts3D** (DBCLS, Japan) | anatomical whole-body surface models incl. heart chambers, vessels (from a 1 mm MRI male) | surface OBJ, coarse | no | no | **yes, full torso surface** (skin, lungs, bones) | OBJ | **CC BY-SA 2.1 JP** (share-alike: derived meshes must be redistributed under the same licence; fine for an app that ships the mesh if you attribute and share the mesh itself) |
| **Zygote** heart (commercial) | high-detail anatomical heart/torso for visualisation | surfaces | no | partial (visual) | yes | OBJ/FBX | commercial, per-seat/per-product licence; "Zygote body" is widely used in medical apps but cannot be redistributed as an open asset |
| **openCARP / carputils examples** | small benchmark meshes (slabs, idealised ventricle, a coarse human biventricle used in tutorials) | various | yes in some | Purkinje example | tutorial torso in ECG example | CARP (.pts/.elem/.lon) | openCARP core: *openCARP Academic Public License* (non-commercial without a licence from NumeriCor); examples/carputils: check per file (most are Apache-2.0/CC-BY) |
| **Chaste** test meshes | idealised + a rabbit/human LV mesh | | partial | no | no | triangles/gmsh | BSD-3 |
| **ECGSIM** data files | human ventricular + atrial closed surface, thorax, lungs, 64 electrodes, normal δ/ρ maps | ~1500 ventricular nodes, 300 thorax nodes (verify) | no | no | **yes, with electrodes** | ECGSIM binary + text (documented) | free for non-commercial/educational use; redistribution requires permission (verify on ecgsim.org) |
| **EDGAR** (SCI Utah) | torso tank + human torso geometries with electrode positions and BSPMs | | no | no | **yes** | MATLAB/VTK | per-dataset, mostly CC BY (verify) |
| **MedalCare-XL** (Gillette et al. 2023), Zenodo | 16,900 *simulated* 12-lead ECGs from 13 anatomies × pathologies (LBBB, RBBB, LAFB, LPFB, ischaemia, …), plus the heart/torso models | | yes | yes (His–Purkinje) | yes | WFDB + VTK | CC BY 4.0 |

- Strocchi M, Augustin CM, Gsell MAF, Karabelas E, Neic A, Gillette K, Razeghi O, Prassl AJ, Vigmond EJ, Behar JM, Gould J, Sidhu B, Rinaldi CA, Bishop MJ, Plank G, Niederer SA. "A publicly available virtual cohort of four-chamber heart meshes for cardiac electro-mechanics simulations." *PLoS ONE* 2020;15(6):e0235145. https://doi.org/10.1371/journal.pone.0235145 — data: https://zenodo.org/records/3890034 (CC BY 4.0)
- Rodero C, Strocchi M, Marciniak M, Longobardi S, Whitaker J, O'Neill MD, Gillette K, Augustin C, Plank G, Vigmond EJ, Lamata P, Niederer SA. "Linking statistical shape models and simulated function in the healthy adult human heart." *PLoS Comput Biol* 2021;17(4):e1008851. https://doi.org/10.1371/journal.pcbi.1008851 — data: https://zenodo.org/records/4590294 and https://zenodo.org/records/4593739
- Fonseca CG, Backhaus M, Bluemke DA, Britten RD, Chung JD, Cowan BR, Dinov ID, Finn JP, Hunter PJ, Kadish AH, Lee DC, Lima JAC, Medrano-Gracia P, Shivkumar K, Suinesiaputra A, Tao W, Young AA. "The Cardiac Atlas Project—an imaging database for computational modeling and statistical atlases of the heart." *Bioinformatics* 2011;27(16):2288–2295. https://doi.org/10.1093/bioinformatics/btr360 — https://www.cardiacatlas.org/
- Bai W, Shi W, de Marvao A, Dawes TJW, O'Regan DP, Cook SA, Rueckert D. "A bi-ventricular cardiac atlas built from 1000+ high resolution MR images of healthy subjects and an analysis of shape and motion." *Med Image Anal* 2015;26(1):133–145. https://doi.org/10.1016/j.media.2015.08.009
- Mitsuhashi N, Fujieda K, Tamura T, Kawamoto S, Takagi T, Okubo K. "BodyParts3D: 3D structure database for anatomical concepts." *Nucleic Acids Res* 2009;37:D782–D785. https://doi.org/10.1093/nar/gkn613 — https://dbarchive.biosciencedbc.jp/en/bodyparts3d/ (licence page lic.html)
- Plank G, Loewe A, Neic A, Augustin C, Huang Y-L, Gsell MAF, Karabelas E, Nothstein M, Prassl AJ, Sánchez J, Seemann G, Vigmond EJ. "The openCARP simulation environment for cardiac electrophysiology." *Comput Methods Programs Biomed* 2021;208:106223. https://doi.org/10.1016/j.cmpb.2021.106223 — https://opencarp.org/ ; examples https://opencarp.org/documentation/examples
- Mirams GR, Arthurs CJ, Bernabeu MO, Bordas R, Cooper J, Corrias A, Davit Y, Dunn S-J, Fletcher AG, Harvey DG, Marsh ME, Osborne JM, Pathmanathan P, Pitt-Francis J, Southern J, Zemzemi N, Gavaghan DJ. "Chaste: an open source C++ library for computational physiology and biology." *PLoS Comput Biol* 2013;9(3):e1002970. https://doi.org/10.1371/journal.pcbi.1002970 — https://chaste.github.io/
- Gillette K, Gsell MAF, Nagel C, Bender J, Winkler B, Williams SE, Bär M, Schäffter T, Dössel O, Plank G, Loewe A. "MedalCare-XL: 16,900 healthy and pathological synthetic 12 lead ECGs from electrophysiological simulations." *Sci Data* 2023;10:531. https://doi.org/10.1038/s41597-023-02416-4

**Recommendation:** base the simulation geometry on the Strocchi/Rodero four-chamber cohort
(CC BY 4.0; already has tags, fibres and UVCs). Decimate one "average" heart to (a) a ~2–3 mm
tetrahedral simulation mesh (30–60k nodes) or a 2 mm voxel mask, and (b) a ~50–150k-triangle
smoothed *render* surface (epicardium + endocardium with chamber cut-aways) with per-vertex
barycentric mapping onto the simulation mesh. Build the torso from BodyParts3D (CC BY-SA)
or a parametric ellipsoid; attach electrodes by anatomical landmarks. Keep ECGSIM for
cross-checking your 12-lead against its normal maps, not as a shipped asset.

### 5.2 Open ECG datasets for validation

| Dataset | Content | Licence |
|---|---|---|
| PTB-XL (PhysioNet) | 21,799 clinical 12-lead ECGs, 10 s, 500 Hz (and 100 Hz), 71 SCP-ECG statements (rhythm, conduction, MI by site, hypertrophy, ST-T), demographics | **CC BY 4.0** |
| PTB Diagnostic ECG Database | 549 records, 15 leads (12 + Frank XYZ), 1 kHz; MI, BBB, etc. — useful to check your Dower/Frank relationships | ODC-BY 1.0 |
| LUDB (Lobachevsky University) | 200 12-lead ECGs with P/QRS/T *onset/offset annotations* per lead → validates wave durations and morphologies | ODC-BY 1.0 |
| MIT-BIH Arrhythmia | 48 half-hour 2-lead records, beat-annotated (PVC, PAC, paced, AF, flutter, VT, …) | ODC-BY 1.0 |
| CU Ventricular Tachyarrhythmia DB, MIT-BIH Malignant Ventricular Ectopy | VT/VF/torsades episodes | ODC-BY 1.0 |
| Chapman–Shaoxing / Ningbo (PhysioNet "ecg-arrhythmia") | 45,152 12-lead ECGs, 500 Hz, rhythm labels | CC BY 4.0 |
| QT Database | 105 records with QT annotations | ODC-BY 1.0 |
| MedalCare-XL | simulated 12-lead ECGs with ground-truth heart/torso and conduction settings | CC BY 4.0 |

- Wagner P, Strodthoff N, Bousseljot R-D, Kreiseler D, Lunze FI, Samek W, Schaeffter T. "PTB-XL, a large publicly available electrocardiography dataset." *Sci Data* 2020;7:154. https://doi.org/10.1038/s41597-020-0495-6 — https://physionet.org/content/ptb-xl/1.0.3/ (DOI 10.13026/kfzx-aw45)
- Goldberger AL, Amaral LAN, Glass L, Hausdorff JM, Ivanov PCh, Mark RG, Mietus JE, Moody GB, Peng C-K, Stanley HE. "PhysioBank, PhysioToolkit, and PhysioNet." *Circulation* 2000;101(23):e215–e220. https://doi.org/10.1161/01.CIR.101.23.e215 — https://physionet.org/
- Kalyakulina AI, Yusipov II, Moskalenko VA, Nikolskiy AV, Kosonogov KA, Osipov GV, Zolotykh NY, Ivanchenko MV. "LUDB: a new open-access validation tool for electrocardiogram delineation algorithms." *IEEE Access* 2020;8:186181–186190. https://doi.org/10.1109/ACCESS.2020.3029211

Validation plan: compute median beats per lead for each simulated condition; compare
intervals (P, PR, QRS, QT), amplitudes (R/S per lead), axis and polarity tables against
PTB-XL subgroup medians (e.g. `NORM`, `LBBB`, `RBBB`, `LAFB`, `IMI`, `AMI`, `LVH`, `WPW`).
Use LUDB for wave-boundary timing.

### 5.3 Fibre orientation: Bayer rule-based LDRB method

Solve Laplace's equation `∇²φ = 0` on the ventricular mesh four times with Dirichlet data:
`φ_epi` (epi = 1, endo LV = endo RV = 0), `φ_lv` (LV endo = 1, others 0), `φ_rv` (RV endo = 1),
`φ_ab` (apex = 0, base = 1). Then at each point:

```
transmural depth  d = φ_rv/(φ_lv + φ_rv)  (LV↔RV blend) ; ρ = φ_epi (0 endo → 1 epi)
local frame: ê_ab = ∇φ_ab/|∇φ_ab| ;  ê_t = ∇φ_epi (or ∇φ_lv) orthogonalised ;  ê_c = ê_ab × ê_t
fibre angle  α(ρ) = α_endo (1 − ρ) + α_epi ρ      (helix angle, rotates about ê_t)
sheet angle  β(ρ) = β_endo (1 − ρ) + β_epi ρ      (imbrication/transverse angle, rotates about ê_c)
Q = Q_rotate(ê_ab, ê_t, ê_c ; α, β) → columns (f, s, n)
LV and RV systems blended by bislerp (quaternion slerp) using d; septum uses the LV rule.
Default angles (Bayer 2012, fitted to canine DTI): α_endo = +40°, α_epi = −50°,
                                                    β_endo = −65°, β_epi = +25°
Common human alternatives: α_endo = +60°, α_epi = −60° (Streeter); Strocchi cohort: +80° → −60°.
```
Reference implementations to port: `ldrb` (Python/FEniCS; https://pypi.org/project/ldrb),
`cardiac-fibers` (Simula, C++/MFEM, GPU-capable), openCARP `GlRuleFibers`, `fibers` in
`cardiac_geometries`. On a 50k-node tet mesh the four Laplace solves (CG with Jacobi) take
well under a second and are done offline; store `f` per element/node in the asset. For a
*surface-only* mesh a cheap stand-in is: fibre = helix at the local angle `α(ρ)` with respect
to the circumferential direction derived from the apex–base axis.

- Bayer JD, Blake RC, Plank G, Trayanova NA. "A novel rule-based algorithm for assigning myocardial fiber orientation to computational heart models." *Ann Biomed Eng* 2012;40(10):2243–2254. https://doi.org/10.1007/s10439-012-0593-5
- Doste R, Soto-Iglesias D, Bernardino G, Alcaine A, Sebastian R, Giffard-Roisin S, Sermesant M, Berruezo A, Sanchez-Quintana D, Camara O. "A rule-based method to model myocardial fiber orientation in cardiac biventricular geometries with outflow tracts." *Int J Numer Meth Biomed Eng* 2019;35(4):e3185. https://doi.org/10.1002/cnm.3185 — handles RVOT/LVOT.
- Streeter DD Jr, Spotnitz HM, Patel DP, Ross J Jr, Sonnenblick EH. "Fiber orientation in the canine left ventricle during diastole and systole." *Circ Res* 1969;24(3):339–347. https://doi.org/10.1161/01.RES.24.3.339
- Bayer J, Prassl AJ, Pashaei A, Gomez JF, Frontera A, Neic A, Plank G, Vigmond EJ. "Universal ventricular coordinates: a generic framework for describing position within the heart and transferring data." *Med Image Anal* 2018;45:83–93. https://doi.org/10.1016/j.media.2018.01.005 — the (z, ρ, φ, ν) coordinates that make APD gradients, coronary territories and AHA segments trivially definable.
- Atrial fibres (if atrial anisotropy is wanted): Wachter A, Loewe A, Krueger MW, Dössel O, Seemann G. "Mesh structure-independent modeling of patient-specific atrial fiber orientation." *Curr Dir Biomed Eng* 2015;1(1):409–412. https://doi.org/10.1515/cdbme-2015-0099 ; Roney CH et al. "Universal atrial coordinates applied to visualisation, registration and construction of patient specific meshes." *Med Image Anal* 2019;55:65–75. https://doi.org/10.1016/j.media.2019.04.004

### 5.4 Purkinje network generation

**Fractal tree (Sahli Costabal, Hurtado, Kuhl 2016).** Grow a binary tree on the endocardial
surface starting from the His bundle exit points: each branch of length L (≈ 8 mm initial,
with ±σ jitter) is a polyline of N segments; at each generation the branch splits into two
daughters at angle ±θ (≈ 0.15–0.25 rad), branches are projected back onto the surface at every
step and *repelled* by existing branches (density control, w ≈ 0.1); a stop criterion is the
generation count (≈ 15–20) or the branch length falling below L_min (≈ 1–2 mm). Leaves are
the Purkinje–myocardial junctions (PMJs). Assign CV ≈ 2–3 m/s in the tree and a PMJ delay
≈ 5–10 ms (retrograde delay larger, ≈ 15–20 ms). Code: https://github.com/fsahli/fractal-tree
(MIT). A newer package `purkinje-uv` (PyPI) grows trees in the UV-unwrapped endocardium.
The 2026 CinC abstract by the same group reports end-to-end tree generation in < 15 ms, so
on-the-fly regeneration for teaching ("remove the left anterior fascicle") is feasible.

- Sahli Costabal F, Hurtado DE, Kuhl E. "Generating Purkinje networks in the human heart." *J Biomech* 2016;49(12):2455–2465. https://doi.org/10.1016/j.jbiomech.2015.12.025 (PMC4917481)
- Ijiri T, Ashihara T, Yamaguchi T, Takayama K, Igarashi T, Shimada T, Nakazawa K. "A procedural method for modeling the Purkinje fibers of the heart." *J Physiol Sci* 2008;58(7):481–486. https://doi.org/10.2170/physiolsci.RP003208 — L-system alternative used in UT-Heart.
- Sebastian R, Zimmerman V, Romero D, Sanchez-Quintana D, Frangi AF. "Characterization and modeling of the peripheral cardiac conduction system." *IEEE Trans Med Imaging* 2013;32(1):45–55. https://doi.org/10.1109/TMI.2012.2221474
- Bordas R, Gillow K, Lou Q, Efimov IR, Gavaghan D, Kohl P, Grau V, Rodriguez B. "Rabbit-specific ventricular model of cardiac electrophysiological function including specialized conduction system." *Prog Biophys Mol Biol* 2011;107(1):90–100. https://doi.org/10.1016/j.pbiomolbio.2011.05.002

For Tier 1 you do not need a geometric tree at all: a *set of PMJ breakthrough sites with
arrival-time offsets* (Durrer-style: 3 LV + 1–2 RV sites at 0, 0, 5, 10, 15 ms, plus apical
and free-wall sites at 20–30 ms) reproduces the normal QRS; Cardone-Noott 2016 lists a
validated root-node set. Growing a visible fractal tree is nonetheless worth it for the 3-D
view (students see the His–Purkinje system light up before the myocardium), and its leaves
then *are* the seed set, so BBB/fascicular blocks are "prune this subtree".

---

## 6. Numerics for real time (requirement E)

### 6.1 Budget

60 fps → 16.7 ms wall-clock per frame, of which perhaps 4–8 ms can go to simulation on a
laptop CPU (rest: rendering with wgpu, UI, audio). Simulated time per frame = real time
(16.7 ms), or faster/slower if the user scrubs.

### 6.2 Resolution vs. cost (concrete numbers)

| Representation | Nodes/voxels | Typical dx | Per-beat eikonal (Dijkstra/FIM) | RD update cost per sub-step (AP/MS, 7-pt stencil) | Comment |
|---|---|---|---|---|---|
| Closed ventricular + atrial surface (triangles) | 10–20k | 3–4 mm edge | 1–5 ms | n/a (surface RD is possible but wrong for transmural physics) | enough for Tier 1 colour map + EDL ECG |
| Tet volume (decimated Strocchi) | 30–60k nodes, 150–300k tets | 2–3 mm | 5–20 ms | ~50–100 ns/node (unstructured gather) → 3–6 ms per sub-step | Tier 1 with transmural APD gradient; Tier 2 possible but slow on CPU |
| Masked voxel grid 64³ | ~262k cells, ~50–80k inside myocardium | 2 mm (128 mm box) | 2–5 ms (fast sweeping) | ~5–10 ns/voxel SIMD → 0.3–0.8 ms per sub-step on 4 cores | **Tier 2 CPU target** |
| Masked voxel grid 128³ | ~2.1M cells, ~400–600k inside | 1 mm | 20–40 ms | 3–6 ms per sub-step on 4 cores → too slow for 100 sub-steps/frame on CPU | GPU only (wgpu): ~0.3–0.6 ms per sub-step on an integrated GPU |

Masking: store the active voxels in a compact list with precomputed 6-neighbour indices
(structure-of-arrays, `u: Vec<f32>`, `v: Vec<f32>`, `nbr: Vec<[u32;6]>`), or keep the full
box and skip by mask — on 64³ the full box is only 262k × 2 floats = 2 MB, fitting in L2/L3,
so the dense version with branch-free masking is often *faster* than the compact one.

Sub-steps per frame for Tier 2 at 2 mm, D re-tuned to give CV_f ≈ 0.6 m/s:

| Model | dt | sub-steps / 16.7 ms frame | voxel-updates / frame (60k active) | CPU time (4 cores, SIMD, ~1 G updates/s) |
|---|---|---|---|---|
| Aliev–Panfilov | 0.25 ms (0.02 dimless) | 67 | 4.0 M | ~4 ms |
| Aliev–Panfilov | 0.13 ms (0.01) | 129 | 7.7 M | ~8 ms |
| Mitchell–Schaeffer (τ_in raised to 1 ms) | 0.1 ms | 167 | 10 M | ~10 ms (tight; use 2–3 mm or GPU) |
| Fenton–Karma (τ_d raised) | 0.2 ms | 84 | 5.0 M | ~6 ms (3 vars, more flops) |
| BOCF | 0.1 ms | 167 | 10 M | ~12–15 ms (4 vars, tanh) → GPU |

The ~1 G voxel-updates/s/4-cores figure assumes `f32`, AVX2 8-wide, a 7-point stencil and
~20–30 flops per reaction step — measured throughput of such kernels on a 2020-era 4-core
laptop is 200–400 M updates/s per core; rayon over z-slabs scales nearly linearly. Integrated
GPUs (Intel Iris Xe ~1–2 TFLOP/s, Apple M1/M2 2.6–3.6 TFLOP/s) do 64³ in < 0.1 ms per
sub-step and 128³ in ~0.5 ms, limited by memory bandwidth (~60–100 GB/s): each sub-step reads
and writes ~3 × 4 B × N.

### 6.3 Explicit Euler stability

Diffusion (forward Euler, central differences, d dimensions, 7-point stencil in 3D):
```
dt ≤ dx² / (2 d D_max)  = dx² / (6 D_max)          (isotropic, 3D)
with anisotropic D (fibre tensor) use  dt ≤ dx² / (2 Σ_i D_ii)  and the 19/27-point stencil
Examples (D = 0.1 mm²/ms): dx = 0.25 mm → dt ≤ 0.10 ms ; dx = 0.5 → 0.42 ms ; dx = 1 → 1.7 ms ; dx = 2 → 6.7 ms
```
Reaction stiffness: the fastest time constant in the model (τ_in = 0.3 ms in MS, τ_fi ≈
0.1 ms in BOCF, τ_d = 0.25 ms in FK, I_Na in TP06/ORd ~0.05 ms) requires `dt ≲ τ_fast/3` for
forward Euler. So on coarse grids (≥ 0.5 mm) **the reaction, not the diffusion, limits dt**,
which is why slow-upstroke phenomenological models are the right choice. Rush–Larsen /
exponential integration removes the gate stiffness for HH-type models; for the two-variable
models forward Euler is fine.

Accuracy: the wavefront must span ≥ 3–5 cells: `CV × t_upstroke ≥ 3 dx`. At CV = 0.6 m/s this
means t_upstroke ≥ 10 ms for dx = 2 mm (AP ✓, MS with τ_in ≈ 2 ms ✓, FK with τ_d ≈ 1 ms ✓,
TP06 ✗). Numerical CV on coarse grids is lower than the continuum value and direction-
dependent (grid anisotropy up to 10–15 % at 2 mm); calibrate D per axis on a strand and accept
the residual — invisible in a teaching colour map.

Operator splitting (Strang / Godunov): per sub-step, (1) reaction ODE update per node
(embarrassingly parallel, SIMD), (2) diffusion update (stencil). Use the same dt for both when
explicit; if you ever go implicit for diffusion (Crank–Nicolson with a CG solve) you can take
dt ≈ 0.5–1 ms with MS/AP on a 2 mm grid and cut sub-steps 4–10×, at the cost of a sparse
solve per step (~5–10 CG iterations × 60k = still ~1 ms). On CPU this is a good trade; on
GPU explicit is simpler.

- Sundnes J, Lines GT, Cai X, Nielsen BF, Mardal K-A, Tveito A. *Computing the Electrical Activity in the Heart.* Springer 2006. https://doi.org/10.1007/3-540-33437-8 — the standard text for monodomain/bidomain numerics and operator splitting.
- Spiteri RJ, Dean RC. "Stiffness analysis of cardiac electrophysiological models." *Ann Biomed Eng* 2010;38:3592–3604. https://doi.org/10.1007/s10439-010-0100-9
- cbcbeat (Simula, FEniCS-based monodomain/bidomain with splitting): Rognes ME, Farrell PE, Funke SW, Hake JE, Maleckar MMC. "cbcbeat: an adjoint-enabled framework for computational cardiac electrophysiology." *J Open Source Softw* 2017;2(13):224. https://doi.org/10.21105/joss.00224 — https://github.com/ComputationalPhysiology/cbcbeat
- finitewave (Python/numba, finite differences, AP/MS/FK/BOCF/TP06/Courtemanche on 2D/3D grids — useful for *prototyping parameter sets* before porting to Rust): https://github.com/finitewave/finitewave ; https://pypi.org/project/finitewave/ (MIT; JOSS 2026)
- cardiax (JAX Fenton–Karma, 4th-order FD, benchmarks CPU/GPU/TPU): https://github.com/epignatelli/cardiax ; JAX-EP (differentiable monodomain) https://www.researchsquare.com/article/rs-10646136
- Kaboudian et al. 2019 (above) for the GPU-fragment/compute-shader design: textures ping-pong, one texel per node, masks as a texture, electrodes as reductions.

### 6.4 CPU (rayon + SIMD) vs GPU (wgpu compute)

| | CPU rayon/SIMD | wgpu compute |
|---|---|---|
| 64³ masked grid, AP, 70–130 sub-steps/frame | 4–8 ms on 4 cores | < 1 ms |
| 128³ | not real-time | 10–60 ms on integrated GPU (borderline at 167 sub-steps); fine at 64–84 sub-steps |
| Tier 1 eikonal + template + EDL ECG | < 1 ms/frame | not needed |
| Readback of V_m for the colour map | zero-copy | avoid readback: render directly from the storage buffer / texture; compute the ECG weights reduction on GPU (9 dot products of length N per sample) and read back only 9 × (samples/frame) floats |
| Portability | everywhere, deterministic | wgpu runs on Vulkan/Metal/DX12/WebGPU; integrated GPUs are fine; `f32` only; non-deterministic reduction order |
| Recommendation | **ship CPU path as the baseline for 64³ Tier 2 and all of Tier 1** | **optional accelerator for 96³–128³ Tier 2 and for the `wasm` build** |

Design the kernels so the same `u, v` SoA buffers can live on either side; keep dt and the
mask identical so results match to float tolerance.

### 6.5 Recommended concrete target

- Tier 1 simulation mesh: 30–50k nodes (tets, ~2.5 mm) **or** a 2 mm voxel mask of the same
  anatomy; both ventricles + both atria; per-node `f`, UVC (z, ρ, φ, ν), region tag, APD,
  ERP, `t_act`. Eikonal per beat via multi-source Dijkstra with 2-ring edges (≈10 ms) or FIM.
- Tier 2 grid: 64³ at 2 mm (ventricles + atria in one box, or two 64³ boxes), Aliev–Panfilov
  default at dt = 0.25 ms (67 sub-steps/frame); Mitchell–Schaeffer (τ_in = 1.5 ms, τ_out = 6,
  τ_open = 120, τ_close set per node from the APD map ≈ APD/ln(h_max/h_min)) when APD
  restitution/alternans matter; Fenton–Karma Set-k when teaching VF mechanisms.
- ECG: EDL/lead-field weights `W (9 × N_boundary)` precomputed; ECG sample every 1 ms of
  simulated time (1 kHz), i.e. every 4 AP sub-steps; output 12 leads via §3.3.
- Render mesh: 100–150k triangles, per-vertex interpolation from the simulation nodes via
  precomputed barycentric weights; colour = V_m (or activation-time isochrones, or
  repolarisation state) through a perceptual colour map.

---

## 7. Recommended architecture (requirement F)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ Scenario / parameter layer   (rhythm presets, pathology masks, drugs, device)  │
└──────────────┬───────────────────────────────────────────────────────────────┘
               ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ TIMING ENGINE  (conduction-system graph; always on)                           │
│  nodes: SAN, atrial regions, AVN_in/fast/slow/out, His, RBB, LBB, LAF, LPF,    │
│         PMJ groups, accessory pathway, ectopic foci, pacemaker channels       │
│  rules: automaticity (phase 4, overdrive suppression, resetting),             │
│         ERP per node/edge, AV recovery curve (Wenckebach), concealed          │
│         conduction, dual pathways, bidirectional edges (AVNRT/AVRT),          │
│         pacing timing cycles, sensing from mesh                                │
│  emits: Activation{site, t, origin}, Block{edge, t}, Ladder-diagram events     │
└──────┬───────────────────────────────┬───────────────────────┬───────────────┘
       ▼                               ▼                       ▼
┌──────────────┐  ┌─────────────────────────────────┐  ┌──────────────────────┐
│ TIER 0       │  │ TIER 1  (default 3-D engine)     │  │ TIER 2 (on demand)   │
│ template     │  │ eikonal/graph activation per beat │  │ reaction–diffusion   │
│ strip:       │  │ + per-node AP template(APD map,   │  │ AP / MS / FK on      │
│ McSharry/    │  │   pathology templates)            │  │ 64³ masked voxels    │
│ Sameni 3-D   │  │ + refractoriness, aberrancy       │  │ (CPU rayon / wgpu)   │
│ dipole,      │  │ → V_m(node, t) every 1 ms         │  │ → V_m(voxel, t)      │
│ Dower leads  │  │                                   │  │ feeds back local     │
│              │  │                                   │  │ activations to graph │
└──────┬───────┘  └──────────────┬────────────────────┘  └──────────┬───────────┘
       │                         ▼                                  ▼
       │          ┌───────────────────────────────────────────────────────────┐
       │          │ FORWARD ECG: V_L(t) = Σ_n W[L,n]·V_m,n(t)  (EDL / lead field) │
       │          │ → 9 electrode potentials → 12 leads (+V3R/V4R/V7–V9)        │
       │          │ electrode positions / swaps → recompute or permute W rows  │
       │          └───────────────────────────────────────────────────────────┘
       ▼                                   ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ VIEWS: 3-D heart colour map (V_m / isochrones), 12-lead + rhythm strip,        │
│        ladder diagram, single-cell AP panel (TP06/ORd for drug effects),       │
│        axis wheel (I/aVF), electrode placement editor on the torso             │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Tier 0 — template synthesis.** McSharry/ECGSYN extended to a 3-D dipole (Sameni 2007) and
projected with the Dower lead vectors. Use for rhythm-only drills, for the `wasm`/mobile
fallback, and as the regression oracle for the timing engine. Per-beat template ids come from
the timing engine. Cost: negligible.

**Tier 1 — graph + eikonal + APD map + EDL ECG (the default).** Justification: it is the
*minimum model that is causal*: every ECG feature is produced by anatomy, conduction and
repolarisation parameters that the student can see and change (CV per region, block
locations, foci, APD gradients, ischaemic masks, electrode positions), and it reproduces
every listed non-re-entrant pathology — sinus rhythms, AV blocks, BBB/hemiblocks, WPW, PAC/PVC,
escape and paced rhythms, monomorphic VT (as a focus), STEMI/ischaemia, LVH/RVH, electrolyte
and QT changes, Brugada, pericarditis, axis/electrode exercises — with sub-millisecond
per-frame cost and *no numerical fragility* (no dt/dx tuning, deterministic, identical on
every device). It is the approach of ECGSIM (validated against measured BSPMs for 20 years of
teaching) and of the KCL/Graz digital-twin pipeline (validated against bidomain and clinical
ECGs). Its single known weakness — it cannot *create* functional re-entry — is handled by
Tier 2.

**Tier 2 — reaction–diffusion for re-entry.** Switched on for VT (re-entrant, scar-based),
polymorphic VT / torsades, VF, AF with wavelets, flutter around an obstacle, and for
"induce re-entry with a premature stimulus" exercises. Aliev–Panfilov by default (coarse-grid
tolerant, 2 variables), Mitchell–Schaeffer when restitution matters, Fenton–Karma for
breakup mechanisms. Initialise from the Tier-1 state (set `u, v` from the template phase of
each node) so switching is seamless; return to Tier 1 after defibrillation/termination. The
ECG uses the same lead-field weights (sampled on the voxel boundary).

**Why not RD everywhere?** On a 2 mm grid an RD model still costs 4–10 ms/frame of CPU and
is parameter-sensitive (block artefacts, source–sink failures at coarse resolution, grid
anisotropy), and it does *not* make the non-re-entrant scenarios more accurate — the eikonal
with templates matches bidomain activation to a few ms and QRS to < 0.5 mV (Pezzuto 2017;
Neic 2017). Keeping Tier 1 as the default also keeps the ECG strictly reproducible for
assessment items.

**Why not a canned animation?** Because every parameter in the requirement (CVs, ERPs, foci,
blocks, pacing, drugs, electrodes) changes both the colour map and the ECG *consistently
only if they are computed from one state* — which the tiered pipeline guarantees while
remaining far cheaper than the frame budget.

### 7.1 Implementation milestones (suggested)

1. Timing engine + ladder diagram + Tier-0 strip (unit-test rhythms against textbook interval
   rules and MIT-BIH beat statistics).
2. Geometry pipeline: Strocchi heart → decimated sim mesh + render mesh + UVCs + LDRB fibres +
   coronary/AHA masks; torso + electrodes; precompute `W`.
3. Tier 1: Dijkstra/FIM, AP templates, APD map, EDL ECG; tune to PTB-XL `NORM` medians
   (intervals, axes, R/S amplitudes per lead); then BBB/hemiblock/MI/LVH presets.
4. Tier 2: 64³ AP solver (CPU), re-entry induction, VF/VT presets; optional wgpu path.
5. Single-cell panel with TP06/ORd for drug/electrolyte effects, mapped onto Tier-1/2
   parameters.

---

## 8. References (consolidated, alphabetical)

- Aliev RR, Panfilov AV. A simple two-variable model of cardiac excitation. *Chaos Solitons Fractals* 1996;7:293–301. https://doi.org/10.1016/0960-0779(95)00089-5
- Aras K et al. Experimental Data and Geometric Analysis Repository — EDGAR. *J Electrocardiol* 2015;48:975–981. https://doi.org/10.1016/j.jelectrocard.2015.08.008
- Bai W et al. A bi-ventricular cardiac atlas built from 1000+ high resolution MR images. *Med Image Anal* 2015;26:133–145. https://doi.org/10.1016/j.media.2015.08.009
- Batchvarov VN, Malik M, Camm AJ. Incorrect electrode cable connection during electrocardiographic recording. *Europace* 2007;9:1081–1090. https://doi.org/10.1093/europace/eum198
- Bayer JD, Blake RC, Plank G, Trayanova NA. A novel rule-based algorithm for assigning myocardial fiber orientation to computational heart models. *Ann Biomed Eng* 2012;40:2243–2254. https://doi.org/10.1007/s10439-012-0593-5
- Bayer J et al. Universal ventricular coordinates. *Med Image Anal* 2018;45:83–93. https://doi.org/10.1016/j.media.2018.01.005
- Bernstein AD et al. The revised NASPE/BPEG generic code. *PACE* 2002;25:260–264. https://doi.org/10.1046/j.1460-9592.2002.00260.x
- Bordas R et al. Rabbit-specific ventricular model… including specialized conduction system. *Prog Biophys Mol Biol* 2011;107:90–100. https://doi.org/10.1016/j.pbiomolbio.2011.05.002
- Bueno-Orovio A, Cherry EM, Fenton FH. Minimal model for human ventricular action potentials in tissue. *J Theor Biol* 2008;253:544–560. https://doi.org/10.1016/j.jtbi.2008.03.029
- Burger HC, van Milaan JB. Heart-vector and leads. *Br Heart J* 1946;8:157–161.
- Cardone-Noott L et al. Human ventricular activation sequence and the simulation of the electrocardiographic QRS complex… *Europace* 2016;18(suppl 4):iv4–iv15. https://doi.org/10.1093/europace/euw346
- Clayton RH et al. Models of cardiac tissue electrophysiology: progress, challenges and open questions. *Prog Biophys Mol Biol* 2011;104:22–48. https://doi.org/10.1016/j.pbiomolbio.2010.05.008
- Climent AM et al. Functional mathematical model of dual pathway AV nodal conduction. *Am J Physiol Heart Circ Physiol* 2011;300:H1393–H1401. https://doi.org/10.1152/ajpheart.01175.2010
- Clifford GD et al. Model-based filtering, compression and classification of the ECG. *Int J Bioelectromagn* 2005;7:158–161.
- Colli Franzone P, Guerri L, Rovida S. Wavefront propagation in an activation model of the anisotropic cardiac tissue. *J Math Biol* 1990;28:121–176. https://doi.org/10.1007/BF00163143
- Courtemanche M, Ramirez RJ, Nattel S. Ionic mechanisms underlying human atrial action potential properties. *Am J Physiol* 1998;275:H301–H321. https://doi.org/10.1152/ajpheart.1998.275.1.H301
- Doste R et al. A rule-based method to model myocardial fiber orientation in cardiac biventricular geometries with outflow tracts. *Int J Numer Meth Biomed Eng* 2019;35:e3185. https://doi.org/10.1002/cnm.3185
- Dower GE, Machado HB, Osborne JA. On deriving the electrocardiogram from vectorcardiographic leads. *Clin Cardiol* 1980;3:87–95. https://doi.org/10.1002/clc.4960030203
- Durrer D et al. Total excitation of the isolated human heart. *Circulation* 1970;41:899–912. https://doi.org/10.1161/01.CIR.41.6.899
- Edenbrandt L, Pahlm O. Vectorcardiogram synthesized from a 12-lead ECG: superiority of the inverse Dower matrix. *J Electrocardiol* 1988;21:361–367. https://doi.org/10.1016/0022-0736(88)90113-6
- Fenton F, Karma A. Vortex dynamics in three-dimensional continuous myocardium with fiber rotation. *Chaos* 1998;8:20–47. https://doi.org/10.1063/1.166311
- Fenton FH, Cherry EM, Hastings HM, Evans SJ. Multiple mechanisms of spiral wave breakup in a model of cardiac electrical activity. *Chaos* 2002;12:852–892. https://doi.org/10.1063/1.1504242
- Fenton FH, Cherry EM. Models of cardiac cell. *Scholarpedia* 2008;3(8):1868. http://www.scholarpedia.org/article/Models_of_cardiac_cell
- FitzHugh R. Impulses and physiological states in theoretical models of nerve membrane. *Biophys J* 1961;1:445–466. https://doi.org/10.1016/S0006-3495(61)86902-6
- Fonseca CG et al. The Cardiac Atlas Project. *Bioinformatics* 2011;27:2288–2295. https://doi.org/10.1093/bioinformatics/btr360
- Frank E. An accurate, clinically practical system for spatial vectorcardiography. *Circulation* 1956;13:737–749. https://doi.org/10.1161/01.CIR.13.5.737
- Franz MR et al. Monophasic action potential mapping in human subjects with normal electrocardiograms: direct evidence for the genesis of the T wave. *Circulation* 1987;75:379–386. https://doi.org/10.1161/01.CIR.75.2.379
- Fu Z, Jeong W-K, Pan Y, Kirby RM, Whitaker RT. A fast iterative method for solving the eikonal equation on triangulated surfaces. *SIAM J Sci Comput* 2011;33:2468–2488. https://doi.org/10.1137/100788951
- Fu Z, Kirby RM, Whitaker RT. A fast iterative method for solving the eikonal equation on tetrahedral domains. *SIAM J Sci Comput* 2013;35:C473–C494. https://doi.org/10.1137/120881956
- Gerhardt M, Schuster H, Tyson JJ. A cellular automaton model of excitable media including curvature and dispersion. *Science* 1990;247:1563–1566. https://doi.org/10.1126/science.2321017
- Gillette K et al. A framework for the generation of digital twins of cardiac electrophysiology from clinical 12-leads ECGs. *Med Image Anal* 2021;71:102080. https://doi.org/10.1016/j.media.2021.102080
- Gillette K et al. Influence of electrode placement on the morphology of in-silico 12 lead ECGs. *Comput Cardiol* 2021;48. https://www.cinc.org/2021/Program/accepted/7.html
- Gillette K et al. MedalCare-XL: 16,900 healthy and pathological synthetic 12 lead ECGs. *Sci Data* 2023;10:531. https://doi.org/10.1038/s41597-023-02416-4
- Gima K, Rudy Y. Ionic current basis of electrocardiographic waveforms: a model study. *Circ Res* 2002;90:889–896. https://doi.org/10.1161/hh0802.105200
- Glass L, Mackey MC. *From Clocks to Chaos.* Princeton 1988.
- Goldberger AL et al. PhysioBank, PhysioToolkit, and PhysioNet. *Circulation* 2000;101:e215–e220. https://doi.org/10.1161/01.CIR.101.23.e215
- Grandits T. A Fast Iterative Method Python package. *JOSS* 2021;6(66):3641. https://doi.org/10.21105/joss.03641
- Hopenfeld B, Stinstra JG, MacLeod RS. Mechanism for ST depression associated with contiguous subendocardial ischemia. *J Cardiovasc Electrophysiol* 2004;15:1200–1206. https://doi.org/10.1046/j.1540-8167.2004.04072.x
- Ijiri T et al. A procedural method for modeling the Purkinje fibers of the heart. *J Physiol Sci* 2008;58:481–486. https://doi.org/10.2170/physiolsci.RP003208
- Inada S, Hancox JC, Zhang H, Boyett MR. One-dimensional mathematical model of the atrioventricular node. *Biophys J* 2009;97:2117–2127. https://doi.org/10.1016/j.bpj.2009.06.056
- Jacquemet V. An eikonal-diffusion solver and its application to the interpolation and the simulation of reentrant cardiac activations. *Comput Methods Programs Biomed* 2012;108:548–558. https://doi.org/10.1016/j.cmpb.2011.05.003
- Janse MJ, Kléber AG. Electrophysiological changes and ventricular arrhythmias in the early phase of regional myocardial ischemia. *Circ Res* 1981;49:1069–1081. https://doi.org/10.1161/01.RES.49.5.1069
- Jaros R, Martinek R, Danys L. Comparison of different electrocardiography with vectorcardiography transformations. *Sensors* 2019;19:3072. https://doi.org/10.3390/s19143072
- Jeong W-K, Whitaker RT. A fast iterative method for eikonal equations. *SIAM J Sci Comput* 2008;30:2512–2534. https://doi.org/10.1137/060670298
- Jørgensen P et al. A mathematical model of human atrioventricular nodal function incorporating concealed conduction. *Bull Math Biol* 2002;64:1083–1099. https://doi.org/10.1006/bulm.2002.0313
- Kaboudian A, Cherry EM, Fenton FH. Real-time interactive simulations of large-scale systems on personal computers and cell phones. *Sci Adv* 2019;5:eaav6019. https://doi.org/10.1126/sciadv.aav6019
- Kaboudian A, Cherry EM, Fenton FH. Large-scale interactive numerical experiments of chaos, solitons and fractals in real time via GPU in a web browser. *Chaos Solitons Fractals* 2019;121:6–29. https://doi.org/10.1016/j.chaos.2019.01.005
- Kalyakulina AI et al. LUDB: a new open-access validation tool for electrocardiogram delineation algorithms. *IEEE Access* 2020;8:186181–186190. https://doi.org/10.1109/ACCESS.2020.3029211
- Keener JP. An eikonal-curvature equation for action potential propagation in myocardium. *J Math Biol* 1991;29:629–651. https://doi.org/10.1007/BF00163916
- Keller DUJ, Weber FM, Seemann G, Dössel O. Ranking the influence of tissue conductivities on forward-calculated ECGs. *IEEE Trans Biomed Eng* 2010;57:1568–1576. https://doi.org/10.1109/TBME.2010.2046485
- Keller DUJ, Weiss DL, Dössel O, Seemann G. Influence of I_Ks heterogeneities on the genesis of the T-wave. *IEEE Trans Biomed Eng* 2012;59:311–322. https://doi.org/10.1109/TBME.2011.2168397
- Kors JA, van Herpen G, Sittig AC, van Bemmel JH. Reconstruction of the Frank vectorcardiogram from standard electrocardiographic leads. *Eur Heart J* 1990;11:1083–1092. https://doi.org/10.1093/oxfordjournals.eurheartj.a059647
- Lemery R et al. Normal atrial activation and voltage during sinus rhythm in the human heart. *J Cardiovasc Electrophysiol* 2007;18:402–408. https://doi.org/10.1111/j.1540-8167.2007.00762.x
- Lerma C, Lee CF, Glass L, Goldberger AL. The rule of bigeminy revisited. *J Electrocardiol* 2007;40:78–88. https://doi.org/10.1016/j.jelectrocard.2006.04.011
- Lian J, Müssig D, Lang V. Computer modeling of ventricular rhythm during atrial fibrillation and ventricular pacing. *IEEE Trans Biomed Eng* 2006;53:1512–1520. https://doi.org/10.1109/TBME.2006.876627
- Lian J, Clifford GD, Müssig D, Lang V. Open source model for generating RR intervals in atrial fibrillation and beyond. *BioMed Eng OnLine* 2007;6:9. https://doi.org/10.1186/1475-925X-6-9
- Loewe A et al. Influence of the earliest right atrial activation site and its proximity to interatrial conduction pathways on the P-wave morphology. *Europace* 2016;18(suppl 4):iv35–iv43. https://doi.org/10.1093/europace/euw349
- Loewe A, Hunter PJ, Kohl P. Cardiac ischemia—insights from computational models. *Herzschrittmacherther Elektrophysiol* 2018;29:31–39. https://doi.org/10.1007/s00399-017-0539-6
- Lorange M, Gulrajani RM. A computer heart model incorporating anisotropic propagation. *J Electrocardiol* 1993;26:245–261. https://doi.org/10.1016/0022-0736(93)90046-G
- Malmivuo J, Plonsey R. *Bioelectromagnetism.* Oxford 1995. http://www.bem.fi/book/
- Masè M, Glass L, Disertori M, Ravelli F. Nodal recovery, dual pathway physiology, and concealed conduction determine complex AV dynamics in human atrial tachyarrhythmias. *Am J Physiol Heart Circ Physiol* 2012;303:H1219–H1228. https://doi.org/10.1152/ajpheart.00228.2012
- McSharry PE, Clifford GD, Tarassenko L, Smith LA. A dynamical model for generating synthetic electrocardiogram signals. *IEEE Trans Biomed Eng* 2003;50:289–294. https://doi.org/10.1109/TBME.2003.808805
- McSharry PE, Clifford GD. Open-source software for generating electrocardiogram signals. arXiv:physics/0406017 (2004). https://arxiv.org/abs/physics/0406017 ; https://physionet.org/content/ecgsyn/
- Mirams GR et al. Chaste: an open source C++ library for computational physiology and biology. *PLoS Comput Biol* 2013;9:e1002970. https://doi.org/10.1371/journal.pcbi.1002970
- Mitchell CC, Schaeffer DG. A two-current model for the dynamics of cardiac membrane. *Bull Math Biol* 2003;65:767–793. https://doi.org/10.1016/S0092-8240(03)00041-7
- Mitsuhashi N et al. BodyParts3D: 3D structure database for anatomical concepts. *Nucleic Acids Res* 2009;37:D782–D785. https://doi.org/10.1093/nar/gkn613
- Moe GK, Rheinboldt WC, Abildskov JA. A computer model of atrial fibrillation. *Am Heart J* 1964;67:200–220. https://doi.org/10.1016/0002-8703(64)90371-0
- Neic A et al. Efficient computation of electrograms and ECGs in human whole heart simulations using a reaction-eikonal model. *J Comput Phys* 2017;346:191–211. https://doi.org/10.1016/j.jcp.2017.06.020
- Niederer SA et al. Verification of cardiac tissue electrophysiology simulators using an N-version benchmark. *Phil Trans R Soc A* 2011;369:4331–4351. https://doi.org/10.1098/rsta.2011.0139
- O'Hara T, Virág L, Varró A, Rudy Y. Simulation of the undiseased human cardiac ventricular action potential. *PLoS Comput Biol* 2011;7:e1002061. https://doi.org/10.1371/journal.pcbi.1002061
- Okada J et al. Transmural and apicobasal gradients in repolarization contribute to T-wave genesis in human surface ECG. *Am J Physiol Heart Circ Physiol* 2011;301:H200–H208. https://doi.org/10.1152/ajpheart.01241.2010
- Opthof T et al. Cardiac activation–repolarization patterns and ion channel expression mapping in intact isolated normal human hearts. *Heart Rhythm* 2017;14:265–272. https://doi.org/10.1016/j.hrthm.2016.10.010
- Pezzuto S et al. Evaluation of a rapid anisotropic model for ECG simulation. *Front Physiol* 2017;8:265. https://doi.org/10.3389/fphys.2017.00265
- Plank G et al. The openCARP simulation environment for cardiac electrophysiology. *Comput Methods Programs Biomed* 2021;208:106223. https://doi.org/10.1016/j.cmpb.2021.106223
- Plonsey R, Barr RC. *Bioelectricity: A Quantitative Approach.* Springer 2007.
- Potse M, Dubé B, Vinet A. Cardiac anisotropy in boundary-element models for the electrocardiogram. *Med Biol Eng Comput* 2009;47:719–729. https://doi.org/10.1007/s11517-009-0472-x
- Potse M. Scalable and accurate ECG simulation for reaction-diffusion models of the human heart. *Front Physiol* 2018;9:370. https://doi.org/10.3389/fphys.2018.00370
- Qu Z, Garfinkel A. An advanced algorithm for solving partial differential equation in cardiac conduction. *IEEE Trans Biomed Eng* 1999;46:1166–1168. https://doi.org/10.1109/10.784149
- Ramanathan C, Jia P, Ghanem R, Ryu K, Rudy Y. Activation and repolarization of the normal human heart under complete physiological conditions. *PNAS* 2006;103:6309–6314. https://doi.org/10.1073/pnas.0601533103
- Ritsema van Eck HJ, Kors JA, van Herpen G. The U wave in the electrocardiogram: a solution for a 100-year-old riddle. *Cardiovasc Res* 2005;67:256–262. https://doi.org/10.1016/j.cardiores.2005.04.010
- Rodero C et al. Linking statistical shape models and simulated function in the healthy adult human heart. *PLoS Comput Biol* 2021;17:e1008851. https://doi.org/10.1371/journal.pcbi.1008851
- Rogers JM, McCulloch AD. A collocation-Galerkin finite element model of cardiac action potential propagation. *IEEE Trans Biomed Eng* 1994;41:743–757. https://doi.org/10.1109/10.310090
- Rognes ME et al. cbcbeat: an adjoint-enabled framework for computational cardiac electrophysiology. *JOSS* 2017;2(13):224. https://doi.org/10.21105/joss.00224
- Roney CH et al. Universal atrial coordinates. *Med Image Anal* 2019;55:65–75. https://doi.org/10.1016/j.media.2019.04.004
- Rudiger A et al. Electrocardiographic artifacts due to electrode misplacement. *Am J Emerg Med* 2007;25:174–178. https://doi.org/10.1016/j.ajem.2006.06.018
- Rush S, Larsen H. A practical algorithm for solving dynamic membrane equations. *IEEE Trans Biomed Eng* 1978;25:389–392. https://doi.org/10.1109/TBME.1978.326270
- Sahli Costabal F, Hurtado DE, Kuhl E. Generating Purkinje networks in the human heart. *J Biomech* 2016;49:2455–2465. https://doi.org/10.1016/j.jbiomech.2015.12.025 ; https://github.com/fsahli/fractal-tree
- Sameni R, Clifford GD, Jutten C, Shamsollahi MB. Multichannel ECG and noise modeling. *EURASIP J Adv Signal Process* 2007;2007:043407. https://doi.org/10.1155/2007/43407
- Sayadi O, Shamsollahi MB, Clifford GD. Synthetic ECG generation and Bayesian filtering using a Gaussian wave-based dynamical model. *Physiol Meas* 2010;31:1309–1329. https://doi.org/10.1088/0967-3334/31/10/002
- Sebastian R et al. Characterization and modeling of the peripheral cardiac conduction system. *IEEE Trans Med Imaging* 2013;32:45–55. https://doi.org/10.1109/TMI.2012.2221474
- Sermesant M et al. An anisotropic multi-front fast marching method for real-time simulation of cardiac electrophysiology. *FIMH 2007*, LNCS 4466:160–169. https://doi.org/10.1007/978-3-540-72907-5_17
- Sethian JA. A fast marching level set method for monotonically advancing fronts. *PNAS* 1996;93:1591–1595. https://doi.org/10.1073/pnas.93.4.1591
- Shrier A et al. Prediction of complex atrioventricular conduction rhythms in humans with use of the atrioventricular nodal recovery curve. *Circulation* 1987;76:1196–1205. https://doi.org/10.1161/01.CIR.76.6.1196
- Siregar P, Sinteff JP, Julen N, Le Beux P. An interactive 3D anisotropic cellular automata model of the heart. *Comput Biomed Res* 1998;31:323–347. https://doi.org/10.1006/cbmr.1998.1485
- Spiteri RJ, Dean RC. Stiffness analysis of cardiac electrophysiological models. *Ann Biomed Eng* 2010;38:3592–3604. https://doi.org/10.1007/s10439-010-0100-9
- Streeter DD Jr et al. Fiber orientation in the canine left ventricle during diastole and systole. *Circ Res* 1969;24:339–347. https://doi.org/10.1161/01.RES.24.3.339
- Strocchi M et al. A publicly available virtual cohort of four-chamber heart meshes for cardiac electro-mechanics simulations. *PLoS ONE* 2020;15:e0235145. https://doi.org/10.1371/journal.pone.0235145 ; https://zenodo.org/records/3890034
- Sundnes J et al. *Computing the Electrical Activity in the Heart.* Springer 2006. https://doi.org/10.1007/3-540-33437-8
- ten Tusscher KHWJ, Noble D, Noble PJ, Panfilov AV. A model for human ventricular tissue. *Am J Physiol Heart Circ Physiol* 2004;286:H1573–H1589. https://doi.org/10.1152/ajpheart.00794.2003
- ten Tusscher KHWJ, Panfilov AV. Alternans and spiral breakup in a human ventricular tissue model. *Am J Physiol Heart Circ Physiol* 2006;291:H1088–H1100. https://doi.org/10.1152/ajpheart.00109.2006
- ten Tusscher KHWJ, Panfilov AV. Modelling of the ventricular conduction system. *Prog Biophys Mol Biol* 2008;96:152–170. https://doi.org/10.1016/j.pbiomolbio.2007.07.026
- van Dam PM, Oostendorp TF, Linnenbank AC, van Oosterom A. Non-invasive imaging of cardiac activation and recovery. *Ann Biomed Eng* 2009;37:1739–1756. https://doi.org/10.1007/s10439-009-9747-5
- van Oosterom A, Oostendorp TF. ECGSIM: an interactive tool for studying the genesis of QRST waveforms. *Heart* 2004;90:165–168. https://doi.org/10.1136/hrt.2003.014662 ; https://www.ecgsim.org/
- van Oosterom A. The equivalent double layer: source models for repolarization. In *Comprehensive Electrocardiology*, Springer 2010. https://doi.org/10.1007/978-1-84882-046-3_7
- Wachter A et al. Mesh structure-independent modeling of patient-specific atrial fiber orientation. *Curr Dir Biomed Eng* 2015;1:409–412. https://doi.org/10.1515/cdbme-2015-0099
- Wagner P et al. PTB-XL, a large publicly available electrocardiography dataset. *Sci Data* 2020;7:154. https://doi.org/10.1038/s41597-020-0495-6 ; https://physionet.org/content/ptb-xl/
- Wallman M, Smith NP, Rodriguez B. A comparative study of graph-based, eikonal, and monodomain simulations for the estimation of cardiac activation times. *IEEE Trans Biomed Eng* 2012;59:1739–1748. https://doi.org/10.1109/TBME.2012.2193398
- Wei D et al. Comparative simulation of excitation and body surface electrocardiogram with isotropic and anisotropic computer heart models. *IEEE Trans Biomed Eng* 1995;42:343–357. https://doi.org/10.1109/10.376128
- Yan GX, Antzelevitch C. Cellular basis for the normal T wave and the electrocardiographic manifestations of the long-QT syndrome. *Circulation* 1998;98:1928–1936. https://doi.org/10.1161/01.CIR.98.18.1928
- Yan GX, Antzelevitch C. Cellular basis for the Brugada syndrome and other mechanisms of arrhythmogenesis associated with ST-segment elevation. *Circulation* 1999;100:1660–1666. https://doi.org/10.1161/01.CIR.100.15.1660

Software / data pages consulted: openCARP https://opencarp.org/ ; Chaste https://chaste.github.io/ ; cbcbeat https://github.com/ComputationalPhysiology/cbcbeat ; finitewave https://github.com/finitewave/finitewave ; cardiax https://github.com/epignatelli/cardiax ; fim-python https://github.com/thomgrand/fim-python ; ldrb https://pypi.org/project/ldrb ; fractal-tree https://github.com/fsahli/fractal-tree ; purkinje-uv https://pypi.org/project/purkinje-uv/ ; PiGReads model pages https://pigreads.readthedocs.io/ ; CellML https://models.cellml.org/ ; The Virtual Heart https://thevirtualheart.org/ ; ECGSIM https://www.ecgsim.org/ ; EDGAR https://edgar.sci.utah.edu/ ; PhysioNet https://physionet.org/ ; BodyParts3D https://dbarchive.biosciencedbc.jp/en/bodyparts3d/ ; Zenodo records 3890034, 4590294, 4593739, 4506463.
