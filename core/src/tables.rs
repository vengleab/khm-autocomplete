//! The two transliteration charts, as data. Both directions (`romanize`, `compose`) read these.
//! Change the tables, not the functions, when a reading is wrong.
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Series {
    A,
    O,
}

/// (letter, series, Latin onset). First series = A, second series = O.
pub const CONS: [(char, Series, &str); 33] = [
    ('ក', Series::A, "k"),
    ('ខ', Series::A, "kh"),
    ('គ', Series::O, "k"),
    ('ឃ', Series::O, "kh"),
    ('ង', Series::O, "ng"),
    ('ច', Series::A, "ch"),
    ('ឆ', Series::A, "chh"),
    ('ជ', Series::O, "ch"),
    ('ឈ', Series::O, "chh"),
    ('ញ', Series::O, "nh"),
    ('ដ', Series::A, "d"),
    ('ឋ', Series::A, "th"),
    ('ឌ', Series::O, "d"),
    ('ឍ', Series::O, "th"),
    ('ណ', Series::O, "n"),
    ('ត', Series::A, "t"),
    ('ថ', Series::A, "th"),
    ('ទ', Series::O, "t"),
    ('ធ', Series::O, "th"),
    ('ន', Series::O, "n"),
    ('ប', Series::A, "b"),
    ('ផ', Series::A, "ph"),
    ('ព', Series::O, "p"),
    ('ភ', Series::O, "ph"),
    ('ម', Series::O, "m"),
    ('យ', Series::O, "y"),
    ('រ', Series::O, "r"),
    ('ល', Series::O, "l"),
    ('វ', Series::O, "v"),
    ('ស', Series::A, "s"),
    ('ហ', Series::A, "h"),
    ('ឡ', Series::A, "l"),
    ('អ', Series::A, ""),
];

/// One row of the vowel chart. `s` is the sign ("" = inherent vowel); `a`/`o` are the readings after a
/// first/second-series consonant. The first reading is what Khmer-to-Latin writes.
pub struct VowRow {
    pub s: &'static str,
    pub a: &'static [&'static str],
    pub o: &'static [&'static str],
}

impl VowRow {
    pub fn readings(&self, ser: Series) -> &'static [&'static str] {
        match ser {
            Series::A => self.a,
            Series::O => self.o,
        }
    }
}

pub const VOW: [VowRow; 21] = [
    VowRow { s: "", a: &["a"], o: &["o"] },
    VowRow { s: "ា", a: &["a", "aa"], o: &["ea", "ia"] },
    VowRow { s: "ិ", a: &["e"], o: &["i"] },
    VowRow { s: "ី", a: &["ei", "ey"], o: &["i", "ii", "ee"] },
    VowRow { s: "ឹ", a: &["oe"], o: &["oe", "u"] },
    VowRow { s: "ឺ", a: &["oeu", "eu"], o: &["eu", "oeu"] },
    VowRow { s: "ុ", a: &["o"], o: &["u"] },
    VowRow { s: "ូ", a: &["ou"], o: &["ou", "u", "oo"] },
    VowRow { s: "ួ", a: &["uo", "ua"], o: &["uo", "ua"] },
    VowRow { s: "ើ", a: &["aeu", "ae"], o: &["eu", "oe"] },
    VowRow { s: "ឿ", a: &["oea"], o: &["eua", "oea"] },
    VowRow { s: "ៀ", a: &["ie", "ia"], o: &["ie", "ia"] },
    VowRow { s: "េ", a: &["e"], o: &["e"] },
    VowRow { s: "ែ", a: &["ae"], o: &["ae"] },
    VowRow { s: "ៃ", a: &["ai", "ay"], o: &["ey", "ay", "ei"] },
    VowRow { s: "ោ", a: &["ao"], o: &["o", "ou"] },
    VowRow { s: "ៅ", a: &["au", "aw"], o: &["ov", "aw", "ow"] },
    VowRow { s: "ុំ", a: &["om"], o: &["um"] },
    VowRow { s: "ំ", a: &["am"], o: &["um"] },
    VowRow { s: "ាំ", a: &["am"], o: &["oam", "eam"] },
    VowRow { s: "ះ", a: &["ah"], o: &["eah", "ah"] },
];

/// Vowel signs that sit on a consonant (U+17B6..=U+17C5).
pub const VOWSIGNS: &str = "ាិីឹឺុូួើឿៀេែៃោៅ";

pub fn sign_row(s: &str) -> Option<&'static VowRow> {
    VOW.iter().find(|r| r.s == s)
}
pub fn series_of(c: char) -> Option<Series> {
    CONS.iter().find(|x| x.0 == c).map(|x| x.1)
}
pub fn onset(c: char) -> Option<&'static str> {
    CONS.iter().find(|x| x.0 == c).map(|x| x.2)
}

/// Final-consonant sounds.
pub fn final_of(c: char) -> Option<&'static str> {
    Some(match c {
        'ក' | 'ខ' | 'គ' | 'ឃ' => "k",
        'ង' => "ng",
        'ច' | 'ឆ' | 'ជ' | 'ឈ' => "ch",
        'ញ' => "nh",
        'ដ' | 'ឋ' | 'ឌ' | 'ឍ' | 'ត' | 'ថ' | 'ទ' | 'ធ' => "t",
        'ណ' | 'ន' => "n",
        'ប' | 'ផ' | 'ព' | 'ភ' => "p",
        'ម' => "m",
        'យ' => "y",
        'រ' => "",
        'ល' | 'ឡ' => "l",
        'វ' => "v",
        'ស' => "s",
        'ហ' => "h",
        'អ' => "",
        _ => return None,
    })
}

/// Independent vowel letters and the sound they write.
pub fn indep(c: char) -> Option<&'static str> {
    Some(match c {
        'ឥ' => "e",
        'ឦ' => "ei",
        'ឧ' => "o",
        'ឩ' => "ou",
        'ឪ' => "au",
        'ឫ' | 'ឬ' | 'ឭ' | 'ឮ' => "rue",
        'ឯ' => "ae",
        'ឰ' => "ai",
        'ឱ' | 'ឲ' => "ao",
        'ឳ' => "au",
        _ => return None,
    })
}

/// Latin onset -> Khmer letters, in first-seen order (that order is part of the reference behavior).
pub fn onset_lat() -> &'static Vec<(&'static str, Vec<char>)> {
    static T: OnceLock<Vec<(&'static str, Vec<char>)>> = OnceLock::new();
    T.get_or_init(|| {
        let mut v: Vec<(&'static str, Vec<char>)> = Vec::new();
        for (l, _, lat) in CONS.iter() {
            if lat.is_empty() {
                continue;
            }
            match v.iter_mut().find(|e| e.0 == *lat) {
                Some(e) => e.1.push(*l),
                None => v.push((lat, vec![*l])),
            }
        }
        v.push(("g", vec!['គ']));
        v.push(("j", vec!['ច', 'ជ']));
        v.push(("c", vec!['ច', 'ជ']));
        v.push(("ny", vec!['ញ']));
        v.push(("w", vec!['វ']));
        v.push(("q", vec!['ក']));
        v
    })
}

/// Latin final -> Khmer letters.
pub const FINAL_LAT: [(&str, &[char]); 13] = [
    ("k", &['ក']),
    ("ng", &['ង']),
    ("ch", &['ច']),
    ("nh", &['ញ']),
    ("t", &['ត', 'ដ', 'ទ']),
    ("n", &['ន', 'ណ']),
    ("p", &['ប', 'ព']),
    ("m", &['ម']),
    ("y", &['យ']),
    ("l", &['ល']),
    ("v", &['វ']),
    ("s", &['ស']),
    ("h", &['ហ']),
];

/// Letters that can take a series-switching mark.
pub fn to_a(c: char) -> bool {
    "ងញនមយរលវណ".contains(c)
}
pub fn to_o(c: char) -> bool {
    "បសហអឡ".contains(c)
}

// ------------------------------------------------------------------------------------------------ editable tables
// The consts above are the built-in charts. `Tables` is the same data as an owned value that can be loaded from a
// mapping file (`data/mapping.json`, edited in the admin web pages), so the charts can change without a rebuild.

/// One row of the vowel chart, owned (see `VowRow`).
#[derive(Clone, Debug, PartialEq)]
pub struct VowEntry {
    pub s: String,
    pub a: Vec<String>,
    pub o: Vec<String>,
}

impl VowEntry {
    pub fn readings(&self, ser: Series) -> &[String] {
        match ser {
            Series::A => &self.a,
            Series::O => &self.o,
        }
    }
}

/// The charts both directions read. `Tables::builtin()` equals the consts above.
#[derive(Clone, Debug, PartialEq)]
pub struct Tables {
    /// (letter, series, Latin onset), in chart order
    pub cons: Vec<(char, Series, String)>,
    /// final-consonant sound of each consonant (Khmer -> Latin); "" for a letter that is silent as a final
    pub finals: Vec<(char, String)>,
    pub vow: Vec<VowEntry>,
    /// independent vowel letters and the sound they write
    pub indep: Vec<(char, String)>,
    /// extra Latin onsets (g, j, c...) on top of the ones the consonant rows give; a key that already exists is replaced
    pub onset_extra: Vec<(String, Vec<char>)>,
    /// Latin final -> Khmer letters (Latin -> Khmer)
    pub final_lat: Vec<(String, Vec<char>)>,
    pub to_a: Vec<char>,
    pub to_o: Vec<char>,
    /// derived: Latin onset -> letters, first-seen order, extras applied (that order is part of the reference behavior)
    onset_lat: Vec<(String, Vec<char>)>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct MapJson {
    #[serde(default)]
    version: u32,
    consonants: Vec<ConsJson>,
    #[serde(default, rename = "onsetExtra")]
    onset_extra: Vec<LatLetters>,
    vowels: Vec<VowJson>,
    #[serde(default)]
    finals: Vec<LatLetters>,
    #[serde(default)]
    independent: Vec<IndepJson>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct ConsJson {
    letter: String,
    series: String,
    onset: String,
    #[serde(default, rename = "final")]
    fin: String,
    #[serde(default, rename = "toA")]
    to_a: bool,
    #[serde(default, rename = "toO")]
    to_o: bool,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct LatLetters {
    latin: String,
    letters: String,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct VowJson {
    sign: String,
    #[serde(rename = "A")]
    a: Vec<String>,
    #[serde(rename = "O")]
    o: Vec<String>,
}
#[derive(serde::Serialize, serde::Deserialize)]
struct IndepJson {
    letter: String,
    latin: String,
}

fn one_char(s: &str, what: &str) -> Result<char, String> {
    let mut it = s.chars();
    match (it.next(), it.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(format!("{what} must be exactly one character, got {s:?}")),
    }
}

impl Tables {
    fn derive(mut self) -> Tables {
        let mut v: Vec<(String, Vec<char>)> = Vec::new();
        for (l, _, lat) in &self.cons {
            if lat.is_empty() {
                continue;
            }
            match v.iter_mut().find(|e| e.0 == *lat) {
                Some(e) => e.1.push(*l),
                None => v.push((lat.clone(), vec![*l])),
            }
        }
        // like `Object.assign` in the reference: an existing key keeps its place and gets the new letters
        for (lat, ls) in &self.onset_extra {
            match v.iter_mut().find(|e| e.0 == *lat) {
                Some(e) => e.1 = ls.clone(),
                None => v.push((lat.clone(), ls.clone())),
            }
        }
        self.onset_lat = v;
        self
    }

    /// The built-in charts (the consts of this module).
    pub fn builtin() -> &'static Tables {
        static T: OnceLock<Tables> = OnceLock::new();
        T.get_or_init(|| {
            let cons: Vec<(char, Series, String)> = CONS.iter().map(|(l, s, o)| (*l, *s, o.to_string())).collect();
            let finals = CONS.iter().map(|c| (c.0, final_of(c.0).unwrap_or("").to_string())).collect();
            let vow = VOW
                .iter()
                .map(|r| VowEntry {
                    s: r.s.to_string(),
                    a: r.a.iter().map(|x| x.to_string()).collect(),
                    o: r.o.iter().map(|x| x.to_string()).collect(),
                })
                .collect();
            let indep = "ឥឦឧឩឪឫឬឭឮឯឰឱឲឳ".chars().filter_map(|c| indep(c).map(|s| (c, s.to_string()))).collect();
            let onset_extra = vec![
                ("g".to_string(), vec!['គ']),
                ("j".to_string(), vec!['ច', 'ជ']),
                ("c".to_string(), vec!['ច', 'ជ']),
                ("ny".to_string(), vec!['ញ']),
                ("w".to_string(), vec!['វ']),
                ("q".to_string(), vec!['ក']),
            ];
            let final_lat = FINAL_LAT.iter().map(|(l, cs)| (l.to_string(), cs.to_vec())).collect();
            let to_a = CONS.iter().map(|c| c.0).filter(|c| self::to_a(*c)).collect();
            let to_o = CONS.iter().map(|c| c.0).filter(|c| self::to_o(*c)).collect();
            Tables { cons, finals, vow, indep, onset_extra, final_lat, to_a, to_o, onset_lat: Vec::new() }.derive()
        })
    }

    /// Parse and check a mapping file. Errors say what is wrong; a mapping that loads can never make the engine panic.
    pub fn from_json(text: &str) -> Result<Tables, String> {
        let m: MapJson = serde_json::from_str(text).map_err(|e| format!("mapping: {e}"))?;
        let mut cons = Vec::new();
        let mut finals = Vec::new();
        let (mut to_a, mut to_o) = (Vec::new(), Vec::new());
        for c in &m.consonants {
            let l = one_char(&c.letter, "consonant letter")?;
            if cons.iter().any(|x: &(char, Series, String)| x.0 == l) {
                return Err(format!("consonant {l} listed twice"));
            }
            let ser = match c.series.as_str() {
                "A" => Series::A,
                "O" => Series::O,
                s => return Err(format!("series of {l} must be A or O, got {s:?}")),
            };
            cons.push((l, ser, c.onset.clone()));
            finals.push((l, c.fin.clone()));
            if c.to_a {
                to_a.push(l);
            }
            if c.to_o {
                to_o.push(l);
            }
        }
        let mut vow: Vec<VowEntry> = Vec::new();
        for v in &m.vowels {
            if vow.iter().any(|x| x.s == v.sign) {
                return Err(format!("vowel sign {:?} listed twice", v.sign));
            }
            for (name, list) in [("A", &v.a), ("O", &v.o)] {
                if list.iter().any(|x| x.is_empty()) || list.iter().any(|x| !x.chars().all(|c| c.is_ascii_lowercase()))
                {
                    return Err(format!("vowel {:?} series {name}: spellings must be lowercase a-z", v.sign));
                }
            }
            vow.push(VowEntry { s: v.sign.clone(), a: v.a.clone(), o: v.o.clone() });
        }
        // every sign the Khmer -> Latin direction can meet needs a row with at least one reading in each series
        let need = std::iter::once("").chain(VOWSIGNS_LIST.iter().copied()).chain(["ំ", "ះ"]);
        for s in need {
            match vow.iter().find(|r| r.s == s) {
                Some(r) if !r.a.is_empty() && !r.o.is_empty() => {}
                Some(_) => return Err(format!("vowel sign {s:?} needs at least one spelling in each series")),
                None => return Err(format!("vowel sign {s:?} is missing")),
            }
        }
        let mut onset_extra = Vec::new();
        for e in &m.onset_extra {
            let letters: Vec<char> = e.letters.chars().collect();
            if e.latin.is_empty() || !e.latin.chars().all(|c| c.is_ascii_lowercase()) || letters.is_empty() {
                return Err(format!("onset {:?}: needs lowercase a-z and at least one letter", e.latin));
            }
            if letters.iter().any(|l| !cons.iter().any(|c| c.0 == *l)) {
                return Err(format!(
                    "onset {:?}: {:?} has a letter that is not a listed consonant",
                    e.latin, e.letters
                ));
            }
            onset_extra.push((e.latin.clone(), letters));
        }
        let mut final_lat = Vec::new();
        for e in &m.finals {
            let letters: Vec<char> = e.letters.chars().collect();
            if e.latin.is_empty() || !e.latin.chars().all(|c| c.is_ascii_lowercase()) || letters.is_empty() {
                return Err(format!("final {:?}: needs lowercase a-z and at least one letter", e.latin));
            }
            if letters.iter().any(|l| !cons.iter().any(|c| c.0 == *l)) {
                return Err(format!(
                    "final {:?}: {:?} has a letter that is not a listed consonant",
                    e.latin, e.letters
                ));
            }
            final_lat.push((e.latin.clone(), letters));
        }
        for c in &cons {
            let ok = |s: &str| s.chars().all(|c| c.is_ascii_lowercase());
            if !ok(&c.2) {
                return Err(format!("onset of {}: lowercase a-z only", c.0));
            }
        }
        for (l, f) in &finals {
            if !f.chars().all(|c| c.is_ascii_lowercase()) {
                return Err(format!("final of {l}: lowercase a-z only"));
            }
        }
        let mut indep = Vec::new();
        for i in &m.independent {
            if !i.latin.chars().all(|c| c.is_ascii_lowercase()) {
                return Err(format!("independent vowel {}: lowercase a-z only", i.letter));
            }
            indep.push((one_char(&i.letter, "independent vowel letter")?, i.latin.clone()));
        }
        Ok(Tables { cons, finals, vow, indep, onset_extra, final_lat, to_a, to_o, onset_lat: Vec::new() }.derive())
    }

    /// The mapping file for these tables (`from_json(to_json())` gives the same tables back).
    pub fn to_json(&self) -> String {
        let m = MapJson {
            version: 1,
            consonants: self
                .cons
                .iter()
                .zip(&self.finals)
                .map(|((l, s, o), (_, f))| ConsJson {
                    letter: l.to_string(),
                    series: if *s == Series::A { "A" } else { "O" }.to_string(),
                    onset: o.clone(),
                    fin: f.clone(),
                    to_a: self.to_a.contains(l),
                    to_o: self.to_o.contains(l),
                })
                .collect(),
            onset_extra: self
                .onset_extra
                .iter()
                .map(|(l, cs)| LatLetters { latin: l.clone(), letters: cs.iter().collect() })
                .collect(),
            vowels: self.vow.iter().map(|v| VowJson { sign: v.s.clone(), a: v.a.clone(), o: v.o.clone() }).collect(),
            finals: self
                .final_lat
                .iter()
                .map(|(l, cs)| LatLetters { latin: l.clone(), letters: cs.iter().collect() })
                .collect(),
            independent: self
                .indep
                .iter()
                .map(|(c, s)| IndepJson { letter: c.to_string(), latin: s.clone() })
                .collect(),
        };
        serde_json::to_string(&m).expect("mapping serializes")
    }

    pub fn sign_row(&self, s: &str) -> Option<&VowEntry> {
        self.vow.iter().find(|r| r.s == s)
    }
    pub fn series_of(&self, c: char) -> Option<Series> {
        self.cons.iter().find(|x| x.0 == c).map(|x| x.1)
    }
    pub fn onset(&self, c: char) -> Option<&str> {
        self.cons.iter().find(|x| x.0 == c).map(|x| x.2.as_str())
    }
    pub fn final_of(&self, c: char) -> Option<&str> {
        self.finals.iter().find(|x| x.0 == c).map(|x| x.1.as_str())
    }
    pub fn indep(&self, c: char) -> Option<&str> {
        self.indep.iter().find(|x| x.0 == c).map(|x| x.1.as_str())
    }
    pub fn onset_lat(&self) -> &[(String, Vec<char>)] {
        &self.onset_lat
    }
    pub fn to_a(&self, c: char) -> bool {
        self.to_a.contains(&c)
    }
    pub fn to_o(&self, c: char) -> bool {
        self.to_o.contains(&c)
    }
}

const VOWSIGNS_LIST: [&str; 16] = ["ា", "ិ", "ី", "ឹ", "ឺ", "ុ", "ូ", "ួ", "ើ", "ឿ", "ៀ", "េ", "ែ", "ៃ", "ោ", "ៅ"];
