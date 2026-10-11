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
