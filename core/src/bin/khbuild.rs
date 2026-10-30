//! khbuild data.json lexicon.json out.bin: compile the interchange files into the fast-loading format.
use khcore::Model;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    // khbuild data.json lexicon.json out.bin [english.json [mapping.json]]
    if a.len() < 4 || a.len() > 6 {
        eprintln!("usage: khbuild data.json lexicon.json khdata.bin [english.json [mapping.json]]");
        std::process::exit(2);
    }
    let rd = |p: &str| {
        std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("{p}: {e}");
            std::process::exit(1)
        })
    };
    let english = a.get(4).map(|p| rd(p));
    let mapping = a.get(5).map(|p| rd(p));
    let m = Model::from_json_full(&rd(&a[1]), &rd(&a[2]), english.as_deref(), mapping.as_deref()).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let b = m.to_bytes();
    std::fs::write(&a[3], &b).unwrap_or_else(|e| {
        eprintln!("{}: {e}", a[3]);
        std::process::exit(1)
    });
    println!("wrote {}: {} words, {} sound-index entries, {} bytes", a[3], m.vocab(), m.rom_keys.len(), b.len());
}
