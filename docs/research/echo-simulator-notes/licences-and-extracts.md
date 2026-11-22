# Echo simulator research: licence quotes and source extracts (2026-11-22)

These are working notes behind [`../echo-simulator.md`](../echo-simulator.md).

* Quotes are verbatim from the primary page unless marked **[secondary]**, meaning a mirror, a search snippet or a copy.
* No proprietary echo simulator's site, code or assets were opened.
* `epicardio/` was not read.

## Licences read today

**Usable (CC0 / CC BY / permissive):**

* **Sunnybrook Cardiac Data** (https://www.cardiacatlas.org/sunnybrook-cardiac-data/): "License and attribution of these data set, including its derivatives, follows the Public Domain (CC0 1.0 Universal)." The CAP index page adds: "This is a **public domain** dataset that does not require any request authorisation."
* **Rodero 2021, extreme and average meshes** (https://zenodo.org/records/4593739): "Creative Commons Attribution 4.0 International". The meshes are "end-diastolic CT-derived meshes", with `simulation_output.csv` provided "for validation purposes".
* **Rodero 2021, 1000-mesh cohort** (https://zenodo.org/records/4506930): "Creative Commons Attribution 4.0 International".
* **Strocchi 2020** (https://zenodo.org/records/3890034): "Creative Commons Attribution 4.0 International".
* **KCL Whole Torso Computational Models** (https://zenodo.org/records/7253863): "Creative Commons Attribution 4.0 International".
* **TotalSegmentator**:
  * Dataset v2.0.1 (https://zenodo.org/records/10047292), "Dataset with segmentations of 117 important anatomical structures in 1228 CT images": "Creative Commons Attribution 4.0 International".
  * The repository (https://github.com/wasserth/TotalSegmentator) is Apache-2.0. Its open tasks, `total` included, are "Openly available for any usage (Apache-2.0 license)".
  * The `total` classes include rib_left_1…12, rib_right_1…12, sternum, costal_cartilages, the five lung lobes, liver, heart and aorta.
  * `heartchambers_highres` is in the "Available with a license" group: "free licenses available for non-commercial usage … For a commercial license contact …".
* **Wikimedia Commons**: File:Sub-aortic-tendon-induced-ST-segment-elevation…-S1.ogv, from Ker 2009, doi:10.1186/1476-7120-7-13, PLAX. "This file is licensed under the Creative Commons Attribution 2.0 Generic license."
  * Snippets show other Cardiovascular Ultrasound supplements on Commons as CC BY 2.0 / 3.0. Those pages were not opened.
* **Code licences:**
  * OpenBCSim (COLE-type): "Copyright (c) 2015, Sigurd Storve. All rights reserved. Licensed under the BSD license."
  * PlusLib: "The Plus license below is a BSD style license, with extensions to cover contributions…"
  * UltraRay: "MIT License, Copyright (c) 2025 Felix Duelmer".
  * ingle/ultrasound-simulation: "Copyright (c) 1999-2015 James A. Zagzebski. Permission is hereby granted, free of charge…"
  * snappysonic: "BSD License Copyright (c) 2019, University College London".

**Not usable:**

* **EchoNet-Dynamic / LVH** (echonet.github.io): "for personal, non-commercial research purposes only"; "Any commercial use, sale, or other monetization is prohibited."; "YOU MAY NOT DISTRIBUTE, PUBLISH, OR REPRODUCE A COPY…"; "You must not … create derivative works from the EchoNet-Dynamic Dataset."
  * Stanford AIMI: "The annual fee is $70,000 USD per dataset".
  * The EchoNet-Pediatric terms sections were empty when fetched.
* **MITEA** (https://www.cardiacatlas.org/mitea/): "MITEA can be provided under a non-commercial license CC BY-NC-SA 4.0 for research purposes." and "No part of the MITEA dataset may be copied, or distributed or disclosed to any other person(s) without consent."
* **M&Ms** (data use agreement PDF): "Permission is granted to view and use the M&Ms Dataset without charge for research purposes only. Its sale is prohibited."
* **CETUS** (confidentiality form): the signer pledges that none of the datasets "will be given away or distributed under any circumstances".
* **CAMUS**: the page says "You must cite this paper for any use of the CAMUS database" and gives no licence. [secondary] Academic Torrents: "The CAMUS dataset is free of charge under CC BY-NC-SA 4.0 license", "strictly only non-commercial scientific research purposes."
* **TED** (derived from CAMUS): "Please refer to our paper for any use of the data or code." No licence is given.
* **ACDC**: "You must refer to this citation for any use of the ACDC database." [secondary] Hugging Face mirror: CC BY-NC-SA 4.0.
* **HMC-QU**: [secondary] CC BY-NC-SA 3.0 IGO.
* **Unity Imaging**: [secondary] CC BY-NC-ND 4.0. Its code repositories are MIT, which covers code only.
* **Radiopaedia**: the licence page returned HTTP 402. [secondary] "modified Creative Commons Attribution-Non-commercial-Share Alike 3.0 Unported License".
* **STRAUS / Alessandrini synthetic sequences**: "This database is public, so no login is required." No licence is given. The Inria STRAUS page returned 403.
* **Imperial bi-ventricular atlas**: CC BY-NC-ND 4.0.
* **4-D XCAT**: paid licence. [secondary] "$400 for the first year".
* **UK Biobank**: restricted. The access page returned 403.
* **Field II** (field-ii.dk): "It may be distributed freely unmodified. It is, however, not permitted to utilize any part of the software in commercial products without prior written consent".
* **k-Wave**: "GNU LESSER GENERAL PUBLIC LICENSE Version 3". PyMUST: "GNU LESSER GENERAL PUBLIC LICENSE Version 2.1".
* **GPL-3.0**: sofus, STORM-IRIT/scus, ImagingYeditepe/UltrasoundSimulator.
* **No licence (all rights reserved)**: Blito/burgercpp, thepochynsons/MCRay-Tracing.
* **IT'IS tissue database** legal notice: copying "only for private, scientific and noncommercial use."

## Mitchell et al. 2019 (ASE comprehensive TTE), extracts

* Source: https://www.asecho.org/wp-content/uploads/2019/01/2019_Comprehensive-TTE.pdf, read in full. The page numbers are journal pages.
* The view table in the main note (§4.1) holds the quoted phrases with their pages.
* Further phrases:
  * Transmit frequency: "typical range … 2.0 to 5.0 MHz" (p6).
  * Depth should "maximize the size of the display for the structures … of interest" (p8). For the pulmonary-vein A4C, "2 cm behind the atria" (p30).
  * The SSN transducer face points inferiorly, "almost parallel with the neck" (p5).
  * The subcostal 4C index marker points toward "the patient's left shoulder" (Table 2.24, p24).

## Physics extracts

* **Shams, Hartley & Navab 2008** (https://users.cecs.anu.edu.au/~hartley/Papers/PDF/Ramtin:MICCAI08.pdf):
  * `α_R = ((Z₂−Z₁)/(Z₂+Z₁))²`.
  * `R = α_R·I_i·|r·n|`.
  * `I_us = (I_r + α)·I_s`.
  * "bone-tissue interfaces reflect 43% and air-tissue interfaces reflect 99%".
* **Mattausch & Goksel 2016** (preprint: https://caim.research.it.uu.se/pre/Mattausch_monte-carlo_16pre.pdf):
  * Interface term: `I_r = max(cosⁿ∠(V_r, D), 0) + max(cosⁿ∠(V_i, D), 0)`.
  * PSF: `h = exp(−l²/2σ_l² − a²/2σ_a² − e²/2σ_e²)·cos(2π f_c l)`.
  * Scatterers: a 256³ Gaussian texture with a random global rotation; `c = µ + ξσ` if `ξ > ρ`; modulated by a 20³ texture.
* **Alessandrini 2015** (creatis.insa-lyon.fr/~bernard/publis/tmi_2015_alessandrini.pdf): COLE phased array at 3.3 MHz, 65 % bandwidth, transmit focus 80 mm, 76° × 76° sector, 50 MHz sampling. Echogenicity: `20·log10 F(a) = dB·a/255`.
* **Stanford RAD225 Class 3** (https://web.stanford.edu/class/rad225/subdir/Class3.pdf): the Z, c and α tables quoted in §3.2 of the main note.

## Not reached

**Paywalled or embargoed:**
* Bürger 2013, Kutter 2009, Gao 2009, Mattausch 2018 (CGF), Ng & Swanevelder 2011, Wagner 1983.

**Timed out:**
* campar.in.tum.de, which hosts Wein 2008, Reichl 2009 and Salehi 2015.

**Blocked:**
* AHA (403): Weissler 1968.
* NCBI (captcha).
* Kaggle rules pages (rendered empty).
* Open-i (empty).
* Human Heart Project dataset pages (JavaScript only).
* Fraunhofer record of Weidenbach 2005.

**Not found:**
* A tool called "FUSK" (probably Hergum 2009, k-space).
* Cronin 2020.
* A dedicated TTE-simulation systematic review.
* Normal values for leaflet kinematics.
* Durations of the E wave, diastasis and A wave.
* Electromechanical delay.
* Wall-thickening normal values.
* IVC collapsibility.
* Displacement of the heart in decubitus.
