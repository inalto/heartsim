//! The calculator model: what a calculator asks for, what it returns, where its numbers come from
//! and how far they were checked.
//!
//! A [`Calculator`] is static data plus one pure function. [`Calculator::evaluate`] first checks the
//! inputs against the description (present, in range, a known choice), then calls the function,
//! which may add consistency errors (an end-systolic volume larger than the end-diastolic one) and
//! returns an [`Outcome`]: named values with units, the formula written out with the numbers put in,
//! and the [`Finding`]s a guideline table gives for the result. Every finding names its criterion
//! and the page it was read on, so a later decision tree (brief B29 part B) can be a calculator
//! whose findings are the nodes it passed through.

use std::fmt;

/// A short text in the two languages the calculators are written in. Other interface languages
/// fall back to English.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Text {
    /// English.
    pub en: &'static str,
    /// Italian.
    pub it: &'static str,
}

impl Text {
    /// The text in `lang` (an ISO 639-1 code); English for any language other than Italian.
    #[must_use]
    pub fn get(&self, lang: &str) -> &'static str {
        if lang == "it" { self.it } else { self.en }
    }
}

/// Builds a [`Text`]: `tx!("English", "Italiano")`.
#[macro_export]
macro_rules! tx {
    ($en:expr, $it:expr $(,)?) => {
        $crate::Text { en: $en, it: $it }
    };
}

/// An owned text in English and Italian, for findings and messages that carry numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// English.
    pub en: String,
    /// Italian.
    pub it: String,
}

impl Message {
    /// The text in `lang`; English for any language other than Italian.
    #[must_use]
    pub fn get(&self, lang: &str) -> &str {
        if lang == "it" { &self.it } else { &self.en }
    }
}

impl From<Text> for Message {
    fn from(t: Text) -> Self {
        Self {
            en: t.en.to_owned(),
            it: t.it.to_owned(),
        }
    }
}

/// The group a calculator is listed under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    /// Valve areas and gradients of narrowed native valves.
    Stenosis,
    /// Orifice area, volume and fraction of leaking native valves.
    Regurgitation,
    /// Left-sided filling and systolic pressures from Doppler signals.
    FillingPressures,
    /// Left-ventricular size, mass and systolic function; outflow and aortic size.
    VentricularFunction,
    /// Right-heart size and function, right atrial and pulmonary pressures, body surface area.
    RightHeart,
    /// Effusion size and the respiratory signs of tamponade and constriction.
    Pericardium,
    /// Shunt ratios and pressures from shunt and obstruction jets.
    Shunts,
    /// Effective orifice area and velocity indices of valve prostheses.
    Prosthetic,
    /// The physics of pulse-echo ultrasound and Doppler.
    Physics,
}

impl Category {
    /// Every category, in the order the page lists them.
    pub const ALL: [Category; 9] = [
        Category::Stenosis,
        Category::Regurgitation,
        Category::FillingPressures,
        Category::VentricularFunction,
        Category::RightHeart,
        Category::Pericardium,
        Category::Shunts,
        Category::Prosthetic,
        Category::Physics,
    ];

    /// The category's name.
    #[must_use]
    pub fn name(self) -> Text {
        match self {
            Category::Stenosis => tx!("Stenosis", "Stenosi"),
            Category::Regurgitation => tx!("Regurgitation", "Insufficienza"),
            Category::FillingPressures => tx!("Filling pressures", "Pressioni di riempimento"),
            Category::VentricularFunction => tx!("Left ventricle and aorta", "Ventricolo sinistro e aorta"),
            Category::RightHeart => tx!(
                "Right heart and pulmonary circulation",
                "Cuore destro e circolo polmonare"
            ),
            Category::Pericardium => tx!("Pericardium", "Pericardio"),
            Category::Shunts => tx!("Shunts and congenital", "Shunt e cardiopatie congenite"),
            Category::Prosthetic => tx!("Prosthetic valves", "Protesi valvolari"),
            Category::Physics => tx!("Ultrasound physics", "Fisica degli ultrasuoni"),
        }
    }
}

/// The unit of an input or a result. Symbols are the same in every language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Unit {
    None,
    Cm,
    Mm,
    Cm2,
    CmS,
    MS,
    Ms,
    Us,
    MmHg,
    Ml,
    MlM2,
    M2,
    Bpm,
    LMin,
    LMinM2,
    Percent,
    G,
    GM2,
    CmM2,
    Cm2M2,
    Kg,
    Mhz,
    Khz,
    WoodUnits,
    MmPerMmHg,
    KgM3,
    MRayl,
    W,
    WCm2,
    MPaS,
    MlS,
    MmHgSPerMl,
    Deg,
    Cycles,
    Score,
    KgM2,
    PerS,
}

impl Unit {
    /// The symbol as printed after a value (empty for a pure number).
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::None | Unit::Score => "",
            Unit::Cm => "cm",
            Unit::Mm => "mm",
            Unit::Cm2 => "cm²",
            Unit::CmS => "cm/s",
            Unit::MS => "m/s",
            Unit::Ms => "ms",
            Unit::Us => "µs",
            Unit::MmHg => "mmHg",
            Unit::Ml => "mL",
            Unit::MlM2 => "mL/m²",
            Unit::M2 => "m²",
            Unit::Bpm => "bpm",
            Unit::LMin => "L/min",
            Unit::LMinM2 => "L/min/m²",
            Unit::Percent => "%",
            Unit::G => "g",
            Unit::GM2 => "g/m²",
            Unit::CmM2 => "cm/m²",
            Unit::Cm2M2 => "cm²/m²",
            Unit::Kg => "kg",
            Unit::Mhz => "MHz",
            Unit::Khz => "kHz",
            Unit::WoodUnits => "WU",
            Unit::MmPerMmHg => "mm/mmHg",
            Unit::KgM3 => "kg/m³",
            Unit::MRayl => "MRayl",
            Unit::W => "W",
            Unit::WCm2 => "W/cm²",
            Unit::MPaS => "mPa·s",
            Unit::MlS => "mL/s",
            Unit::MmHgSPerMl => "mmHg·s/mL",
            Unit::Deg => "°",
            Unit::Cycles => "cycles",
            Unit::KgM2 => "kg/m²",
            Unit::PerS => "1/s",
        }
    }
}

/// One option of a choice input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    /// Stable id (used on the command line and in saved examples).
    pub id: &'static str,
    /// What the page shows.
    pub label: Text,
}

/// What kind of value an input takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputKind {
    /// A measured number with its unit and the range a physiological measurement can have; values
    /// outside it are refused (most likely a typing or unit error).
    Number {
        /// Unit the value is typed in.
        unit: Unit,
        /// Smallest accepted value (inclusive).
        min: f64,
        /// Largest accepted value (inclusive).
        max: f64,
        /// Decimals shown in the range hint.
        decimals: u8,
    },
    /// One of a fixed list.
    Choice(&'static [Choice]),
}

/// One input of a calculator, in the order it is acquired.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputSpec {
    /// Stable id, unique within the calculator, with the unit in its name (`lvot_d_cm`).
    pub id: &'static str,
    /// Field label.
    pub label: Text,
    /// Number or choice.
    pub kind: InputKind,
    /// May be left empty.
    pub optional: bool,
    /// Shown (and checked) only while the choice input `.0` holds one of `.1`.
    pub when: Option<(&'static str, &'static [&'static str])>,
}

/// A typed value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A number in the input's unit.
    Number(f64),
    /// The id of one [`Choice`].
    Choice(String),
}

/// The values typed into a calculator, by input id. Absent ids are empty fields.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inputs {
    values: Vec<(String, Value)>,
}

impl Inputs {
    /// No values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets (or replaces) a value.
    pub fn set(&mut self, id: &str, value: Value) {
        if let Some(slot) = self.values.iter_mut().find(|(k, _)| k == id) {
            slot.1 = value;
        } else {
            self.values.push((id.to_owned(), value));
        }
    }

    /// Builder form of [`Self::set`] for a number.
    #[must_use]
    pub fn with(mut self, id: &str, v: f64) -> Self {
        self.set(id, Value::Number(v));
        self
    }

    /// Builder form of [`Self::set`] for a choice.
    #[must_use]
    pub fn with_choice(mut self, id: &str, choice: &str) -> Self {
        self.set(id, Value::Choice(choice.to_owned()));
        self
    }

    /// Clears a field.
    pub fn remove(&mut self, id: &str) {
        self.values.retain(|(k, _)| k != id);
    }

    /// The value of a field, if filled.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.values.iter().find(|(k, _)| k == id).map(|(_, v)| v)
    }

    /// The number in a field, if filled with a number.
    #[must_use]
    pub fn num(&self, id: &str) -> Option<f64> {
        match self.get(id) {
            Some(Value::Number(v)) => Some(*v),
            _ => None,
        }
    }

    /// A required number. [`Calculator::evaluate`] has checked that it is present before the
    /// calculator's function runs; NaN otherwise, which the result check then refuses.
    #[must_use]
    pub fn n(&self, id: &str) -> f64 {
        self.num(id).unwrap_or(f64::NAN)
    }

    /// The choice id in a field, if filled.
    #[must_use]
    pub fn choice(&self, id: &str) -> Option<&str> {
        match self.get(id) {
            Some(Value::Choice(c)) => Some(c.as_str()),
            _ => None,
        }
    }

    /// Every filled field.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }
}

/// Why an input, or the set of inputs, was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    /// A required field is empty.
    Missing,
    /// The number is outside the input's plausible range.
    OutOfRange {
        /// Lower limit, inclusive.
        min: f64,
        /// Upper limit, inclusive.
        max: f64,
    },
    /// The value is not one of the input's choices, or a number was given for a choice.
    NotAChoice,
    /// A choice was given for a number field, or the text is not a number.
    NotANumber,
    /// The values contradict each other (the text says how).
    Inconsistent(Message),
    /// No input of that id (command line only).
    Unknown,
}

/// A refused input, with the field it belongs to (`None`: the inputs together).
#[derive(Debug, Clone, PartialEq)]
pub struct InputError {
    /// The field to show the message beside.
    pub input: Option<&'static str>,
    /// What is wrong.
    pub kind: ErrorKind,
}

impl InputError {
    /// A consistency error on `input`.
    #[must_use]
    pub fn inconsistent(input: &'static str, message: Text) -> Self {
        Self {
            input: Some(input),
            kind: ErrorKind::Inconsistent(message.into()),
        }
    }

    /// The message in `lang`, with the limits written out.
    #[must_use]
    pub fn message(&self, lang: &str) -> String {
        let it = lang == "it";
        match &self.kind {
            ErrorKind::Missing => {
                if it {
                    "Campo obbligatorio".into()
                } else {
                    "Required".into()
                }
            }
            ErrorKind::OutOfRange { min, max } => {
                let (a, b) = (crate::fmt::trim(*min), crate::fmt::trim(*max));
                if it {
                    format!("Fuori dall'intervallo plausibile {a}–{b}")
                } else {
                    format!("Outside the plausible range {a}–{b}")
                }
            }
            ErrorKind::NotAChoice => {
                if it {
                    "Scelta non valida".into()
                } else {
                    "Not one of the choices".into()
                }
            }
            ErrorKind::NotANumber => {
                if it {
                    "Non è un numero".into()
                } else {
                    "Not a number".into()
                }
            }
            ErrorKind::Inconsistent(m) => m.get(lang).to_owned(),
            ErrorKind::Unknown => {
                if it {
                    "Campo sconosciuto".into()
                } else {
                    "Unknown field".into()
                }
            }
        }
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.input {
            Some(id) => write!(f, "{id}: {}", self.message("en")),
            None => write!(f, "{}", self.message("en")),
        }
    }
}

/// A computed value.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    /// Stable id with the unit in its name (`ava_cm2`).
    pub id: &'static str,
    /// Name shown beside the value.
    pub label: Text,
    /// The value, always finite.
    pub value: f64,
    /// Its unit.
    pub unit: Unit,
    /// Decimals shown.
    pub decimals: u8,
}

impl Quantity {
    /// The value rounded for display, with its unit ("0.79 cm²").
    #[must_use]
    pub fn display(&self) -> String {
        crate::fmt::with_unit(self.value, self.decimals, self.unit)
    }
}

/// One line of the worked formula: the symbols, the same line with the numbers put in, the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The formula in symbols ("AVA = CSA × VTI(LVOT) ÷ VTI(AV)").
    pub formula: String,
    /// The same with the numbers ("= 3.80 cm² × 21.0 cm ÷ 90.0 cm").
    pub substituted: String,
    /// The result with its unit ("= 0.89 cm²").
    pub result: String,
}

/// How a finding is drawn (and how strong a deviation it reports).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Within the range the source calls normal.
    Normal,
    /// Mild (or the source's first abnormal step).
    Mild,
    /// Moderate.
    Moderate,
    /// Severe, or the source's strongest category.
    Severe,
    /// Not graded: a criterion met or not met, or a note.
    Info,
    /// Outside the normal range or meeting the source's criterion for an abnormal result, where the
    /// source does not grade it (a decision tool's "elevated filling pressure").
    Abnormal,
    /// A decision tool that cannot conclude: the source's own "indeterminate", or measurements left
    /// empty that could change the result.
    Indeterminate,
}

/// What a guideline says about a result: which of its criteria the value meets.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// Strength.
    pub severity: Severity,
    /// What the value meets, worded as a criterion and never as a diagnosis.
    pub text: Message,
    /// The criterion itself ("AVA < 1.0 cm²").
    pub criterion: String,
    /// Where the criterion was read.
    pub source: Cite,
}

/// The result of a calculator.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    /// The computed values, main result first.
    pub values: Vec<Quantity>,
    /// The formula with the numbers substituted, one step per value.
    pub steps: Vec<Step>,
    /// Interpretation (empty for [`Review::FormulaOnly`] calculators).
    pub findings: Vec<Finding>,
}

impl Outcome {
    /// The value with id `id`.
    #[must_use]
    pub fn value(&self, id: &str) -> Option<f64> {
        self.values.iter().find(|q| q.id == id).map(|q| q.value)
    }

    /// Adds a value and its formula step.
    pub fn push(&mut self, q: Quantity, formula: &str, substituted: String) {
        self.steps.push(Step {
            formula: formula.to_owned(),
            substituted: {
                let mut s = substituted;
                s.insert_str(0, "= ");
                s
            },
            result: format!("= {}", q.display()),
        });
        self.values.push(q);
    }
}

/// A publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    /// First author et al., or the authors.
    pub authors: &'static str,
    /// Title.
    pub title: &'static str,
    /// Journal, volume and pages, or the publisher.
    pub journal: &'static str,
    /// Year.
    pub year: u16,
    /// DOI without the resolver prefix, if any.
    pub doi: Option<&'static str>,
    /// A link to an openly readable copy, if any.
    pub url: Option<&'static str>,
}

/// A reference with the place in it that a formula or criterion comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cite {
    /// The publication.
    pub reference: &'static Reference,
    /// Table, equation, figure or section, and page.
    pub locator: &'static str,
}

/// How far a calculator was checked against its primary source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Review {
    /// The formula and every threshold it shows were read on the primary source (locators given).
    Verified,
    /// The arithmetic is standard and was checked, but no grading is shown: the criteria were not
    /// read on a primary source.
    FormulaOnly,
}

impl Review {
    /// The badge text.
    #[must_use]
    pub fn label(self) -> Text {
        match self {
            Review::Verified => tx!("Verified against the source", "Verificato sulla fonte"),
            Review::FormulaOnly => tx!(
                "Formula only (no grading)",
                "Solo formula (nessuna classificazione)"
            ),
        }
    }
}

/// An input the running simulation really has, so the page may offer to copy it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimLink {
    /// The heart rate of the running rhythm goes into this input (bpm).
    HeartRate(&'static str),
}

/// A value of a worked example.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExampleValue {
    /// A number.
    Number(f64),
    /// A choice id.
    Choice(&'static str),
}

/// One calculator.
#[derive(Debug, Clone, Copy)]
pub struct Calculator {
    /// Stable, unique id (kebab case); the command line and the saved favourites use it.
    pub id: &'static str,
    /// Where it is listed.
    pub category: Category,
    /// Name.
    pub name: Text,
    /// Other names and the quantities it gives, comma-separated, for the search.
    pub synonyms: Text,
    /// What it computes and the assumptions it rests on, in a sentence or two.
    pub summary: Text,
    /// Inputs in the order of acquisition.
    pub inputs: &'static [InputSpec],
    /// The pure computation; runs only on inputs that passed the range checks.
    pub compute: fn(&Inputs) -> Result<Outcome, Vec<InputError>>,
    /// Sources: the formula's first, then those of the grading.
    pub references: &'static [Cite],
    /// Review status.
    pub review: Review,
    /// Our own worked example (physiologically plausible, not taken from any source).
    pub example: &'static [(&'static str, ExampleValue)],
    /// Input the simulator can fill, if any.
    pub sim_link: Option<SimLink>,
}

impl Calculator {
    /// The input with id `id`.
    #[must_use]
    pub fn input(&self, id: &str) -> Option<&'static InputSpec> {
        self.inputs.iter().find(|i| i.id == id)
    }

    /// Whether `spec` is shown for these inputs (its `when` condition holds).
    #[must_use]
    pub fn visible(spec: &InputSpec, inputs: &Inputs) -> bool {
        match spec.when {
            None => true,
            Some((choice_id, ids)) => inputs.choice(choice_id).is_some_and(|c| ids.contains(&c)),
        }
    }

    /// The worked example as inputs.
    #[must_use]
    pub fn example_inputs(&self) -> Inputs {
        let mut out = Inputs::new();
        for (id, v) in self.example {
            match v {
                ExampleValue::Number(x) => out.set(id, Value::Number(*x)),
                ExampleValue::Choice(c) => out.set(id, Value::Choice((*c).to_owned())),
            }
        }
        out
    }

    /// Checks every visible input against its description, then computes. Errors are listed per
    /// field; an `Ok` outcome holds only finite values.
    pub fn evaluate(&self, inputs: &Inputs) -> Result<Outcome, Vec<InputError>> {
        let mut errors = Vec::new();
        for spec in self.inputs {
            if !Self::visible(spec, inputs) {
                continue;
            }
            let err = |kind| InputError {
                input: Some(spec.id),
                kind,
            };
            match (spec.kind, inputs.get(spec.id)) {
                (_, None) => {
                    if !spec.optional {
                        errors.push(err(ErrorKind::Missing));
                    }
                }
                (InputKind::Number { min, max, .. }, Some(Value::Number(v))) => {
                    if !v.is_finite() || *v < min || *v > max {
                        errors.push(err(ErrorKind::OutOfRange { min, max }));
                    }
                }
                (InputKind::Number { .. }, Some(Value::Choice(_))) => errors.push(err(ErrorKind::NotANumber)),
                (InputKind::Choice(choices), Some(Value::Choice(c))) => {
                    if !choices.iter().any(|x| x.id == c) {
                        errors.push(err(ErrorKind::NotAChoice));
                    }
                }
                (InputKind::Choice(_), Some(Value::Number(_))) => errors.push(err(ErrorKind::NotAChoice)),
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let outcome = (self.compute)(inputs)?;
        if outcome.values.iter().any(|q| !q.value.is_finite()) {
            return Err(vec![InputError {
                input: None,
                kind: ErrorKind::Inconsistent(
                    tx!(
                        "These values give no finite result",
                        "Questi valori non danno un risultato finito"
                    )
                    .into(),
                ),
            }]);
        }
        Ok(outcome)
    }

    /// Whether every word of `query` appears (case-insensitively) in the name, the other names or
    /// an input label, in `lang`.
    #[must_use]
    pub fn matches(&self, query: &str, lang: &str) -> bool {
        let mut hay = format!("{} {} {}", self.name.get(lang), self.synonyms.get(lang), self.id);
        for i in self.inputs {
            hay.push(' ');
            hay.push_str(i.label.get(lang));
        }
        let hay = fold(&hay);
        fold(query).split_whitespace().all(|w| hay.contains(w))
    }
}

/// Lower case with the accents of Italian taken off and primes and curly apostrophes made straight,
/// so that "e′" typed with a prime finds "e'".
fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' => 'a',
            'è' | 'é' => 'e',
            'ì' | 'í' => 'i',
            'ò' | 'ó' => 'o',
            'ù' | 'ú' => 'u',
            '\u{2032}' | '\u{2019}' => '\'',
            _ => c,
        })
        .collect()
}
