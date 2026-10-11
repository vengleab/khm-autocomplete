//! khbuild data.json lexicon.json out.bin: compile the interchange files into the fast-loading format.
use khcore::Model;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 4 {
        eprintln!("usage: khbuild data.json lexicon.json khdata.bin");
        std::process::exit(2);
    }
    let rd = |p: &str| {
        std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("{p}: {e}");
            std::process::exit(1)
        })
    };
    let m = Model::from_json(&rd(&a[1]), &rd(&a[2])).unwrap_or_else(|e| {
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
