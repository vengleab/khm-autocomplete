//! Prints the built-in consonant / vowel charts as a mapping file (the format of data/mapping.json).
//!
//!   cargo run -q -p khcore --example dump_mapping > /tmp/mapping.json
fn main() {
    println!("{}", khcore::tables::Tables::builtin().to_json());
}
