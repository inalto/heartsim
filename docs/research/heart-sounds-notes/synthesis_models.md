# Synthesis models for heart sounds (PCG) — parametric, murmur, transfer, learned

Reading guide. "Cited Findings" are statements taken from a fetched source. Anything I could not confirm in a fetched
full text is in "Inferences" (clearly my reasoning) or "Gaps" (including formulas I recall from the literature but could
not re-verify — these are labelled UNVERIFIED and must be checked against the paper before use). Several primary PDFs
(PMC, HAL, IEEE, Springer) were behind bot walls or paywalls during this session; this is noted per item.

## 1. Parametric component models for S1 / S2 (damped sinusoids, Gabor atoms, chirps, dynamical models)

### Takeaway
Three families are implementable from the literature in well under 300 lines each: (a) Gaussian/split-Gaussian-windowed
sinusoid atoms (Almasi 2011; Nonaka & Seita 2026 "SynPCG"), (b) attack + exponentially damped sinusoids (Zhou et al.
2026 fetal simulator), and (c) the Xu–Durand–Pibarot nonlinear decaying-frequency chirp for A2 and P2. Only SynPCG
publishes a complete numeric parameter table in an openly readable text, and its values are tuned for classifier
pre-training, not for listening; the chirp model is the only one with physiologically anchored frequency trajectories
(A2 250 -> 53 Hz, P2 200 -> 50 Hz over 60 ms).

### Cited Findings

**Xu, Durand, Pibarot — nonlinear transient chirp model of A2 / P2 (IEEE TBME 2000, 47(10):1328–1335; TBME 2001, 48(3):277–283, DOI 10.1109/10.914790)**
- A2 and P2 are each modelled as a narrow-band, short-duration, single-component nonlinear chirp whose instantaneous frequency decreases with time; analysis–synthesis on 32 isolated A2 and 32 isolated P2 from four pigs with pulmonary hypertension gave NRMSE 2.1 ± 0.3 % and correlation 0.97 ± 0.02 for A2, NRMSE 2.52 ± 0.5 % and correlation 0.96 ± 0.02 for P2. — [abstract record](https://manuscript.isc.ac/Inventory/49/1388033.htm)
- The 2001 follow-up validates separation on a *simulated* S2 with A2 and P2 overlapping by 5–30 ms; reconstruction error 1–6 %, growing with overlap. — [PubMed 11327495](https://pubmed.ncbi.nlm.nih.gov/11327495/)
- Model form as restated by Djebbari & Bereksi-Reguig: valvular sound v(t) = a(t)·sin(phi(t)); S2(t) = a_A2(t)·sin(phi_A2(t)) + a_P2(t − d_s)·sin(phi_P2(t − d_s)), d_s = split between A2 and P2 onsets. — [Djebbari & Bereksi-Reguig 2013, BioMed Eng OnLine 12:37, DOI 10.1186/1475-925X-12-37](https://pmc.ncbi.nlm.nih.gov/articles/PMC3706289/)
- Numeric trajectory used for simulation: "The A2 chirp begin from 250 Hz and falls to 53 Hz at 60 ms whilst the P2 chirp goes from 200 to 50 Hz"; simulated component duration is set to 60 ms; A2 and P2 usually last 30–60 ms, less than 80 ms. — [Djebbari 2013](https://pmc.ncbi.nlm.nih.gov/articles/PMC3706289/)
- Split timing: A2–P2 split rises during inspiration to 30–80 ms and decreases under 15 ms during expiration (as cited there); the split duration is limited to < 100 ms. — [Djebbari 2013](https://pmc.ncbi.nlm.nih.gov/articles/PMC3706289/)
- The frequency decrease is attributed to the decaying aortic and pulmonary pressures after end-systole; Xu et al. compared against the exponentially damped sinusoid model, matching pursuit, and the linear chirp model and judged the nonlinear chirp the suitable model for analysis–synthesis of valvular sounds. The model is stated to be valid for the valvular components of both S1 and S2 (different highest/lowest frequencies). — [Djebbari 2013](https://pmc.ncbi.nlm.nih.gov/articles/PMC3706289/)
- Clinical link: the A2–P2 onset interval is used to estimate pulmonary artery pressure (so a simulator can map PAP -> split). — [Djebbari 2013](https://pmc.ncbi.nlm.nih.gov/articles/PMC3706289/)

**Tran, Jones, Fothergill — "Heart sound simulator" (Med Biol Eng Comput 1995, 33:357–359)**
- A heart sound simulator combining a set of equations to model several PCG behaviours; "formed by a linear chirp with an amplitude adjusted according to clinically recorded S1 and S2 heart sounds". Xu et al. extended it to the nonlinear chirp. — [Djebbari 2013](https://pmc.ncbi.nlm.nih.gov/articles/PMC3706289/); bibliographic record [Springer BF02510515](https://link.springer.com/article/10.1007/BF02510515)
- Note: the third author is Fothergill (not Fazel-Rezai). — [Springer BF02510515](https://link.springer.com/article/10.1007/BF02510515)

**Almasi, Shamsollahi, Senhadji — "A dynamical model for generating synthetic phonocardiogram signals" (EMBC 2011, DOI 10.1109/IEMBS.2011.6091376)**
- PCG dynamical model inspired by McSharry's ECG model; each heart sound (S1, S2) is represented with two Gabor kernels, i.e. four kernels per beat; the model is a set of three ODEs giving a 3-D trajectory; parameters are treated as random variables, the ECG R-peak is the external phase reference, and parameters are obtained by fitting to recorded beats. — [PMC3390312](https://pmc.ncbi.nlm.nih.gov/articles/PMC3390312) (search-snippet level; full text blocked by reCAPTCHA in this session), DOI confirmed via [Semantic Scholar record](https://api.semanticscholar.org/graph/v1/paper/DOI:10.1109/IEMBS.2011.6091376?fields=title,openAccessPdf,externalIds)
- The same model is reused as the state model of an EKF/EKS denoiser: "Bayesian denoising framework of phonocardiogram based on a new dynamical model" (IRBM 2013). — [HAL hal-00879037](https://hal-univ-rennes1.archives-ouvertes.fr/hal-00879037)

**Nonaka & Seita — "Boosting Phonocardiogram Classification Performance with Function Generated Data" (PMLR v297, 2026) — "SynPCG"**
- Signal model: for S in {S1, S2}, tau = t − mu_S: x_S(t) = A_S · g_asym(tau; sigma_rise, sigma_decay) · sin(2·pi·f_S·t + phi_S), with peak-normalised split-Gaussian envelope g_asym = exp(−½(tau/sigma_rise)²) for tau < 0, exp(−½(tau/sigma_decay)²) for tau >= 0; sigma_rise < sigma_decay "captures the faster attack and slower decay". Envelope is C1 at the peak. "Additional weak carriers can be superposed to emulate subtle multi-band structure". — [paper PDF](https://raw.githubusercontent.com/mlresearch/v297/main/assets/nonaka26a/nonaka26a.pdf)
- Sampling: 8 kHz, 10-s records (80,000 samples) to match BMD-HS; one S1 and one S2 burst per cycle, cycles concatenated; then baseline fluctuation and white noise added. Heart-rate range stated as "typically 50 to 80 beats per minute". — [paper PDF](https://raw.githubusercontent.com/mlresearch/v297/main/assets/nonaka26a/nonaka26a.pdf)
- Table 3 parameters (base value; per-sample perturbation; per-beat perturbation). Units are not printed in the table. — [paper PDF](https://raw.githubusercontent.com/mlresearch/v297/main/assets/nonaka26a/nonaka26a.pdf)

  | Sound | Parameter | Base | Per-sample | Per-beat |
  |---|---|---|---|---|
  | S1 | frequency | 24.00 | N(0, 2.5) | N(0, 1.0) |
  | S1 | duration | 0.75 | U(−0.25, 0.25) | N(0, 0.05) |
  | S1 | height | 2.50 | U(−0.50, 0.50) | N(0, 0.025) |
  | S1 | neg ratio | 0.25 | N(0, 0.05) | N(0, 0.005) |
  | S1 | shift | 0.20 | N(0, 0.05) | N(0, 0.005) |
  | S2 | frequency | 60.00 | N(0, 5.0) | N(0, 1.0) |
  | S2 | duration | 0.25 | U(−0.10, 0.10) | N(0, 0.025) |
  | S2 | height | 3.00 | U(−0.50, 0.50) | N(0, 0.05) |
  | S2 | neg ratio | 0.25 | N(0, 0.05) | N(0, 0.005) |
  | S2 | shift | 0.55 | N(0, 0.075) | N(0, 0.005) |

- "neg ratio" defines the negative-side (rise) standard deviation as a scaled version of the positive-side one (Algorithm 4); parameters were set "through comprehensive feedback from medical experts"; inter-patient variation deliberately larger than beat-to-beat variation. — [paper PDF](https://raw.githubusercontent.com/mlresearch/v297/main/assets/nonaka26a/nonaka26a.pdf)
- Evaluation is by classifier only (no listening test): synthetic pre-training then real fine-tuning raised mean AUROC on BMD-HS by ~15.3 % (AS), 17.0 % (AR), 17.1 % (MR); on a private 8,564-recording cohort by ~7.1 / 8.8 / 6.1 %; "synthetic-only training generalizes poorly". — [PMLR page](https://proceedings.mlr.press/v297/nonaka26a.html)

**Zhou et al. — "A Dynamic Parametric Simulator for Fetal Heart Sounds" (arXiv 2601.16792, 2026)**
- Kernel: phi(t) = A·sin(2·pi·f0·t)·a(t); a(t) rises linearly over attack time T_a then decays as exp(−t/tau); zero for t < 0. S1 kernel at beat onset t_k, S2 kernel at t_k + DeltaT(k). Default T_a = 8 ms. Per-cycle adjustable set (A_S1, A_S2, tau_S1, tau_S2, DeltaT), drawn from uniform or truncated-Gaussian priors within bounds. — [arXiv 2601.16792](https://arxiv.org/html/2601.16792v1)
- Rhythm: RR_k = mean RR + alpha·d_k (smoothed drift) + eta_k (jitter). Noise: AR(1) n_t = rho·n_{t−1} + sqrt(1−rho²)·eps_t, scaled to a target SNR (default 10 dB, range 5–20 dB). fs default 1000 Hz (500–2000 Hz). — [arXiv 2601.16792](https://arxiv.org/html/2601.16792v1)
- Tissue propagation as convolution of two one-sided exponentials h1 = A1·exp(−beta1·t), h2 = A2·exp(−beta2·t) (defaults beta1 = 100, beta2 = 300, A1 = 1.0, A2 = 0.8), normalised to unit integral. — [arXiv 2601.16792](https://arxiv.org/html/2601.16792v1)
- Validation is qualitative only (envelope, ACF, PSD match against one PhysioNet fetal PCG record sampled at 333 Hz); numeric values of f0, tau, A, DeltaT are not printed (only a corner plot). — [arXiv 2601.16792](https://arxiv.org/html/2601.16792v1)

**Frequency content of the components (for choosing carriers)**
- S1 10–140 Hz with most energy 25–45 Hz; S2 10–200 Hz with most energy 55–75 Hz; S3/S4 about 20–70 Hz and much less audible. — [Generative DL and signal processing for augmentation of cardiac auscultation signals, arXiv 2410.10125](https://arxiv.org/pdf/2410.10125)
- Conflicting bands in other reviews: S1/S2 50–500 Hz and S3/S4 20–200 Hz — [MEMS/ECM sensor review, arXiv 2406.12432](https://arxiv.org/pdf/2406.12432); S3/S4 15–65 Hz — [heart-sound deep-learning survey, arXiv 2301.09362](https://arxiv.org/pdf/2301.09362). Treat band edges as source-dependent.

### Inferences
- SynPCG Table 3 units are almost certainly Hz for "frequency" (24 Hz S1, 60 Hz S2 sit inside the 25–45 / 55–75 Hz energy bands above) and fraction of the cardiac cycle (or seconds at ~60–75 bpm) for "shift" (S1 at 0.20, S2 at 0.55 -> S1–S2 interval ~0.35) — consistent with a ~300–350 ms systole. "duration" 0.75 / 0.25 cannot be seconds of audible sound; it is most likely a generation-window length or sigma scale. This must be read from the code (see section 5; the repo currently has no licence and the synthesis code was not visible in the README).
- 24 Hz and 60 Hz single carriers are below what small laptop/phone speakers reproduce; for an *audible* teaching simulator each sound needs energy at 60–200 Hz. The Xu chirp (250 -> ~50 Hz) gives that naturally and also gives the characteristic "snap" of S2; the same form with lower start frequencies can serve for the mitral/tricuspid components of S1 (the source says the model is valid for S1 valvular components but gives no S1 numbers).
- Recommended minimal design for heartsim (my synthesis, not from one paper): each event = sum of 1–3 atoms; atom = envelope(t)·sin(phase(t)) where envelope is either split-Gaussian (SynPCG) or (1 − e^{−t/tr})·e^{−t/td} and phase is either linear (Gabor) or the Xu square-root law. S1 = M1 + T1 (two atoms 10–30 ms apart), S2 = A2 + P2 (split from engine state: respiration phase, RBBB/LBBB, PAP), S3/S4 = one low-frequency (20–70 Hz) atom each, clicks/snaps = one short high-frequency atom. All closed-form, so evaluation is pure f32/f64 arithmetic with no state other than event onset times — trivially deterministic and sample-rate independent.
- Event onsets should come from `TimingEngine` events (QRS onset -> S1 after the electromechanical delay; end of ejection -> A2), never from an independent PCG clock; the Almasi idea of using the R-peak as phase reference is the same principle.
- Determinism: any per-beat jitter (SynPCG's N(0, sigma) perturbations) should be drawn from a counter-based or seeded PRNG keyed by (scenario seed, beat index, parameter id) so that seeking/replay yields identical audio.

### Gaps
- **Xu 2001 simulation constants — UNVERIFIED (recalled, not re-read; IEEE full text paywalled).** My recollection of the simulated signals is, with t in ms on 0–60 ms: a_A(t) = 1.0·(1 − e^{−t/8})·e^{−t/16}·sin(pi·t/60), phi_A(t) = 2·pi·(24.3·t + 451.4·sqrt(t))/1000; a_P(t) = 0.5·(1 − e^{−t/8})·e^{−t/16}·sin(pi·t/60), phi_P(t) = 2·pi·(21.8·t + 356.3·sqrt(t))/1000. Consistency check against the fetched source: the implied IF_A(t) = 24.3 + 225.7/sqrt(t) Hz gives 250 Hz at 1 ms and 53.4 Hz at 60 ms, matching "250 Hz ... falls to 53 Hz at 60 ms" exactly; IF_P(t) = 21.8 + 178.2/sqrt(t) gives 200 Hz at 1 ms and 44.8 Hz at 60 ms versus the cited "200 to 50 Hz" (approximate match only). A targeted web search for these constants returned nothing, so they must be confirmed in TBME 48(3):277–283 before being written into a parameter file with a citation. The TBME 2000 DOI (believed to be 10.1109/10.871405) is also unverified.
- **Almasi 2011 equations and parameter table not retrieved** (PMC reCAPTCHA, HAL Anubis wall, Europe PMC XML error). From the McSharry lineage the expected form is theta' = omega; z' = −sum_i (alpha_i·...)·exp(−(theta − mu_i)²/(2·sigma_i²))·[cos/sin(2·pi·f_i·theta − phi_i)] i.e. z(theta) = sum_i alpha_i·exp(−(theta−mu_i)²/2sigma_i²)·cos(2·pi·f_i·theta − phi_i) — UNVERIFIED; fitted alpha/sigma/mu/f values, sampling rate and SNR results are not in these notes. Open copies to try by hand: https://pmc.ncbi.nlm.nih.gov/articles/PMC3390312 and https://inserm.hal.science/inserm-00661935.
- Not found / not searched to completion: Chen & Durand S1 models; Zhang, Durand et al. 1998 matching-pursuit atom tables (only referenced indirectly via Djebbari's citations of "matching pursuit method"); Jabloun et al. 2013; Sæderup et al.; Kazemnejad/Sameni EPHNOGRAM-based ECG+PCG models; Joseph et al. No numeric parameters for S3, S4, ejection clicks or opening snaps were found in any synthesis paper — only the frequency bands above.
- No listener-realism study was found for any parametric S1/S2 model; all evaluations are waveform-fit (NRMSE/correlation) or downstream-classifier metrics.

## 2. Murmur synthesis (band-limited / amplitude-modulated noise; severity; musical murmurs; physical models)

### Takeaway
The published synthetic approach is "white noise x cycle-anchored envelope x low-pass/band-limit" with the envelope
shape encoding the lesion (crescendo–decrescendo for AS, decrescendo after S2 for AR, plateau S1->S2 for MR). Published
numeric band limits per lesion are sparse and inconsistent; general murmur energy is ~25–400 Hz with components above
600 Hz, and one source ties AS severity to a higher spectral peak frequency (> 300 Hz).

### Cited Findings
- SynPCG murmur model: "Pathologic murmurs were modeled as band-limited noise segments whose amplitude is shaped by task-specific envelopes and temporally anchored to the cardiac cycle (relative to S1/S2)." Rationale: "Regurgitant or stenotic jets are turbulent and therefore noise-like"; control over timing (window placement), intensity (envelope peak) and shape (crescendo/decrescendo/plateau). — [Nonaka & Seita 2026 PDF](https://raw.githubusercontent.com/mlresearch/v297/main/assets/nonaka26a/nonaka26a.pdf)
- AS (Algorithm 6): white noise over the beat, scaled by (max_wn_amp − min_wn_amp), multiplied by (asymmetric bell + min_wn_amp), then smoothed with a moving-average kernel of width `as_smoothing_size` (Algorithm 7: Ones(w)/w convolution — this is the band-limiting step), then time-shifted into systole; described as "an ejection murmur that peaks in early to mid-systole between S1 and S2". — [Nonaka & Seita 2026 PDF](https://raw.githubusercontent.com/mlresearch/v297/main/assets/nonaka26a/nonaka26a.pdf)
- AR (Algorithms 8–9): same construction placed between S2 and the next S1 ("high-frequency diastolic decrescendo murmur following S2"), asymmetric bell used for the decrescendo. MR (Algorithms 10–11): white noise of length `mr_duration`, uniform amplitude (holosystolic, "uniform intensity from S1"), moving-average smoothed, start shifted to S1 onset. — [Nonaka & Seita 2026 PDF](https://raw.githubusercontent.com/mlresearch/v297/main/assets/nonaka26a/nonaka26a.pdf)
- The paper's Table 3 lists only S1/S2 parameters; murmur amplitudes, smoothing widths and durations are not tabulated in the text. — [Nonaka & Seita 2026 PDF](https://raw.githubusercontent.com/mlresearch/v297/main/assets/nonaka26a/nonaka26a.pdf)
- General murmur band: roughly 25–400 Hz, with some energy above 600 Hz at lower amplitude. — [Scaling to multimodal and multichannel heart sound classification, arXiv 2509.11606](https://arxiv.org/pdf/2509.11606)
- Holosystolic mitral murmur: random spectral components peaking from about 300 to 900 Hz (single spectral-phonocardiograph example). — [US patent 4,967,760 / 5,012,815 "Dynamic spectral phonocardiograph"](https://patents.google.com/patent/US5012815)
- Aortic stenosis: peak frequency exceeds 300 Hz and severity correlates positively with peak frequency (patent statement). — [US patent publication 12,690,834](https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/12690834)
- Timing classes: AS and MR systolic; MS diastolic. — [heart-sound deep-learning survey, arXiv 2301.09362](https://arxiv.org/pdf/2301.09362)
- An older patent (US 8,364,249) builds the anomalous signal separately and adds it to a non-anomalous base signal — the same additive architecture. — [US8364249](https://patents.google.com/patent/US8364249)

### Inferences
- A moving average of width w at fs has its first spectral null at fs/w; SynPCG's smoothing is therefore a crude low-pass, not a true band-pass. For audio quality use a seeded white-noise source through a 2nd-order band-pass (biquad) or a pair of one-pole filters, then multiply by the envelope. A biquad is deterministic and costs a few flops per sample — fine for WASM real time.
- Envelope library needed (all closed-form): split-Gaussian (diamond; peak position early -> mild AS, late-peaking -> severe AS per standard teaching), exponential decrescendo (AR, PR), plateau with short ramps (MR, VSD, TR), low-frequency diastolic rumble with presystolic accentuation tied to the atrial kick event (MS), continuous with late-systolic peak (PDA). The lesion-to-envelope mapping for MS, VSD, PDA is standard clinical teaching, not from a synthesis paper found here; the sibling research notes on clinical acoustics should supply citations.
- "Musical" murmurs (Still's, seagull AR/MR) are tonal: synthesise as a narrow-band resonator (high-Q band-pass on noise) or a slightly frequency-jittered harmonic oscillator under the same envelope rather than broadband noise. No published parameter set was found.
- Severity can be driven from engine haemodynamics (my design suggestion): amplitude and centre frequency scaled with jet velocity / gradient; AS peak time moved later with severity; AR/MR duration from the pressure-gradient time course.
- Noise determinism: generate the noise from a PRNG seeded by (scenario seed, beat index, murmur id); never from wall-clock or audio-callback block boundaries, so that different buffer sizes give bit-identical output.

### Gaps
- No peer-reviewed table of centre frequency/bandwidth per lesion (AS, MR, AR, MS, VSD, PDA) was found; figures above come from patents, a single-patient example, and generic reviews, and they conflict. Classic phonocardiography texts (e.g. Leatham; Luisada; Tavel) and Durand/Pibarot's review "Digital signal processing of the phonocardiogram" (Crit Rev Biomed Eng 1995) would be the place to look — not retrieved here.
- No simple physically based (jet-turbulence / vortex-shedding Strouhal) real-time murmur model with published parameters was located in this session; SynPCG's reference list mentions "parametric modelling and numerical ... of murmurs" work that I did not open.
- No quantitative peak-timing-versus-severity relation from a synthesis paper.

## 3. Chest-wall / stethoscope transfer and rendering at the four auscultation areas

### Takeaway
I found little quantitative, citable material: stethoscope studies confirm bell and diaphragm differ mainly at low
frequencies and that the diagnostically critical band is ~70–120 Hz, but no published per-site attenuation matrix for
each sound component. Site rendering will have to be a designed gain table backed by clinical-teaching sources rather
than by an acoustic transfer measurement.

### Cited Findings
- Heart sounds fall between 20 and 650 Hz and are better detected by the bell (Rennert et al., as cited); the band most useful for the most critical heart sounds is 70–120 Hz (Noland, as cited); a transfer-function study measured 37.5–1000 Hz, where nearly all heart and lung sounds lie, and found bell and diaphragm of a given stethoscope "usually have different transmission characteristics, particularly at low frequencies". — as summarised from [Frequency Responses of Conventional and Amplified Stethoscopes for Measuring Heart Sounds, Saudi J Med Med Sci 2020, DOI 10.4103/sjmms.sjmms_118_19](https://pmc.ncbi.nlm.nih.gov/articles/PMC7305673) (search-level summary; full text only partly verified)
- That study compared diaphragm-mode responses at 85 (largest response), 250, 400, 550 and 1050 Hz for simulated normal, aortic-stenosis and pulmonic-stenosis sounds on Littmann Classic S.E. II, Cardionics E-Scope II and Starkey SLI-ST3. — [same paper, abstract verified via Europe PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC7305673)
- A stethoscope-diaphragm patent describes increased sensitivity in bands centred near 90 Hz and 300 Hz and reduced sensitivity near 200 Hz and above ~500 Hz (design-specific). — [US6026170A, Electronic stethoscope with idealized bell and idealized diaphragm modes](https://patents.google.com/patent/US6026170A/en)
- Preprocessing bands used by PCG researchers, usable as "what a recording chain keeps": 20–150 Hz band-pass (fetal simulator pipeline) — [arXiv 2601.16792](https://arxiv.org/html/2601.16792v1).
- Tissue path as a cascade of two one-pole low-passes (convolution of two exponentials, beta = 100 and 300 s^-1) — [arXiv 2601.16792](https://arxiv.org/html/2601.16792v1) (fetal/abdominal context, not adult chest).

### Inferences
- Implementable rendering chain: per-component dry sources (M1, T1, A2, P2, S3, S4, clicks, each murmur) -> per-site gain vector (4 sites x components, a RON table) -> chest-piece filter: "bell" = low-pass/band-pass roughly 20–150 Hz; "diaphragm" = high-pass shelving roughly 100–500 Hz. The 20–150 vs 100–500 Hz split in the task statement is consistent with the cited sources' direction (bell favours low frequencies; critical band 70–120 Hz) but the exact corner frequencies are a design choice, not a cited measurement.
- Keeping components separate until the final mix is what makes one source renderable at aortic/pulmonic/tricuspid/mitral areas, and also lets the UI solo/label components for teaching.
- All filters should be fixed-coefficient biquads computed from constants (no runtime FFT), processed sample-by-sample in a fixed order for bit-reproducibility across native and wasm32 (avoid `mul_add`/fast-math differences; use explicit f32 ops).

### Gaps
- No measured site-by-component attenuation table (e.g. A2 vs P2 level at 2nd right ICS vs apex) found; no adult chest-wall transfer function with numeric poles; no authoritative bell/diaphragm magnitude curves in dB. The PMC stethoscope article's body text could not be fully read (bot wall), so its numbers are from the abstract and a search summary.

## 4. Learned generators (GAN / diffusion / WaveNet, 2019–2026) versus a small parametric model

### Takeaway
Recent learned PCG generators exist (DiffWave/WaveGrad/WaveNet/DoppelGANger/latent diffusion), are trained on PhysioNet
2016/2022 data for classifier augmentation, need GPU-scale sampling, and in the one expert-listening study found they
sounded heart-like but carried weak abnormality cues. None is a fit for a deterministic, < 25 MiB WASM teaching app; a
parametric model is the better choice.

### Cited Findings
- Bao et al. 2026, class-conditional diffusion on 128x128 log-mel spectrograms (compact 2-D U-Net, 64->128->256 channels; DDIM 100 steps, guidance 1.2; PhysioNet/CinC 2016 at 2000 Hz, ~4 s clips): ResNet-50 reached 92.24 % on real test data and 82.8 % on class-balanced synthetic batches; pilot listening study, 60 clips, two clinicians: real clips 38/40 judged plausible (95 %), synthetic 17/20 (85 %) for clinician A; "abnormality sensitivity is low for both real and synthetic 4 s excerpts"; synthetic clips show "reduced envelope periodicity and increased transient burstiness". — [arXiv 2606.02448](https://arxiv.org/pdf/2606.02448)
- Jamshidi et al. 2024 compare WaveNet, DoppelGANger and DiffWave on PhysioNet/CinC 2022 (CirCor) PCG, evaluated by MAE and maximum mean discrepancy. — [arXiv 2412.16207](https://arxiv.org/abs/2412.16207v1)
- ECG-conditioned PCG generation with WaveGrad and DiffWave (trained 24 h on an RTX 4090; mel conditioning at 4 kHz sample rate) for augmentation of abnormal-heart-sound classifiers. — [arXiv 2410.10125](https://arxiv.org/pdf/2410.10125)
- Hierarchical latent diffusion (H-LDM) for PCG also exists. — [alphaXiv 2511.14312](https://www.alphaxiv.org/abs/2511.14312.md)
- Function-generated (parametric) data is competitive with out-of-domain real-data pre-training for classifiers, but "synthetic-only training generalizes poorly". — [Nonaka & Seita 2026](https://proceedings.mlr.press/v297/nonaka26a.html)

### Inferences
- Learned models conflict with heartsim's constraints on four counts: (1) weights + inference runtime threaten the 25 MiB budget; (2) cross-platform bit-determinism of float NN inference (native SIMD vs wasm32) is hard to guarantee; (3) they cannot be driven sample-accurately by `TimingEngine` events or by arbitrary scenario physiology (split width, murmur severity); (4) they are trained on PhysioNet recordings whose licences/derivative status would need separate legal review. A parametric model has none of these issues and its parameters are explainable to students.
- The listening result (heart-like but poor abnormality salience) suggests learned generators are not yet better teachers than explicit lesion-specific synthesis.

### Gaps
- Code/weight licences for the learned generators were not established: no repository or licence statement was found for Bao 2026, Jamshidi 2024, the WaveGrad/DiffWave ECG->PCG paper, or H-LDM in this session.
- No head-to-head listening comparison between a parametric synthesiser and a learned generator was found.

## 5. Open-source implementations and licences

### Takeaway
No heart-sound synthesiser with a reusable permissive licence was found; the two relevant repositories carry no licence
at all, so the equations must be re-implemented from the papers (which is straightforward).

### Cited Findings
- `seitalab/SynPCG` (Python; classification + "synthetic data generation using function-based approaches"): README says "[License information to be added]"; GitHub API reports `license: null`. — [GitHub seitalab/SynPCG](https://github.com/seitalab/SynPCG)
- Fetal heart sound simulator: released "as both a Python package and an interactive web front end" at https://ytzhou123.github.io/FH-Simulator/; the GitHub repo `ytzhou123/FH-Simulator` (TypeScript) reports `license: null`; the paper states no licence. — [arXiv 2601.16792](https://arxiv.org/html/2601.16792v1), [repo](https://github.com/ytzhou123/FH-Simulator)
- `pyPCG` is a Python toolbox for PCG *analysis* (not synthesis). — [arXiv 2408.12966](https://arxiv.org/html/2408.12966v1)
- `imec-int/heartbeat` appeared in results as CC0 "with some exceptions" but is a Java port unrelated to PCG synthesis (search-snippet level, not inspected). — [GitHub imec-int/heartbeat](https://github.com/imec-int/heartbeat)

### Inferences
- Unlicensed repositories are "all rights reserved" by default: do not copy code from SynPCG or FH-Simulator. The mathematical models (split-Gaussian x sinusoid; attack/decay x sinusoid; chirp) are not copyrightable and can be re-implemented from the papers, with parameter values cited to the paper tables (SynPCG paper is in PMLR; check its paper licence before reproducing the table verbatim in shipped docs — citing values is fine).
- Re-check both repositories for a licence later; SynPCG's README says one will be added.

### Gaps
- A direct GitHub search (topics "phonocardiogram", "heart sound simulator") was not run exhaustively; smaller MIT-licensed hobby synthesisers may exist. No Rust crate for heart-sound synthesis was looked for or found.

## 6. How existing simulators / manikins / teaching apps generate sound

### Takeaway
Where documented, commercial auscultation trainers use recordings of real patients (sometimes cleaned up), not
synthesis; for Harvey the method is not publicly stated in the sources found. Synthesis from a physiological model
would therefore be a differentiator, with the caveat that there is no published realism benchmark to lean on.

### Cited Findings
- Kyoto Kagaku auscultation simulators: sounds "recorded from actual people and reproduced using a high quality sound system"; the Cardiology Patient Simulator "K" has 88 cases (manufacturer listing). — [MedicalExpo listing, Kyoto Kagaku MW10](https://www.medicalexpo.com/prod/kyoto-kagaku/product-69057-830784.html)
- Cardionics SAM 4: sounds "based on actual patient sounds that have been recorded, cleaned up a little bit" (vendor material). — [Anatomy Warehouse, Hands-on Lab Ep. 4: SAM4](https://anatomywarehouse.com/news/hands-on-lab-episode-4-sam4-auscultation/)
- Harvey (Laerdal / Gordon Center) simulates many cardiac conditions by varying blood pressure, pulses, heart sounds, murmurs and breath sounds; 30 cardiac conditions per a university listing; the sources do not say whether sounds are recorded or synthesised. — [Laerdal Harvey](https://laerdal.com/us/harvey/), [MDC Harvey simulator sheet](https://www.mdc.edu/medical/documents/harvey_simulator.pdf)
- An early hardware "Phonocardiogram simulator" patent exists (US 3,508,347), showing electronic synthesis of heart sounds for teaching predates recordings-based manikins. — [US3508347](https://patents.google.com/patent/US3508347)
- The stethoscope frequency-response study used "digitized heart sound simulations" (normal, aortic and pulmonic stenosis) as its test signals. — [Saudi J Med Med Sci 2020](https://pmc.ncbi.nlm.nih.gov/articles/PMC7305673)

### Inferences
- Because competitors ship recordings, their sounds cannot follow arbitrary rhythm/rate changes beat by beat; a synthesiser driven by `TimingEngine` can (e.g. variable S1 intensity in AV dissociation, paradoxical split in LBBB, post-PVC murmur accentuation in AS) — a genuine teaching advantage that recordings cannot give.
- Vendor statements are marketing copy; treat "recorded from real patients" as a claim, not an audited fact.

### Gaps
- No technical documentation found for teaching apps (e.g. Littmann learning tools, Easy Auscultation, 3D-heart apps) on synthesis versus recordings. Epicardio's own method was deliberately not examined (proprietary; see project rules).
- "Ichiro" (Kyoto Kagaku) specifics were not found.
