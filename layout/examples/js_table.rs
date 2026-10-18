//! Prints the `NIDA` constant for web/template.html (ASCII key -> what it types on the Khmer layout) as one JS line.
//!
//!   cargo run -q -p khlayout --example js_table
//!
//! core/tests/layout.rs fails if the table in the template drifts from `khlayout::from_ascii`.
fn main() {
    let mut parts = Vec::new();
    for c in '!'..='~' {
        if let Some(k) = khlayout::from_ascii(c) {
            let key = match c {
                '"' => "\\\"".to_string(),
                '\\' => "\\\\".to_string(),
                _ => c.to_string(),
            };
            let val: String = k
                .chars()
                .map(|ch| match ch {
                    '"' => "\\\"".to_string(),
                    '\\' => "\\\\".to_string(),
                    _ if (ch as u32) < 0x2000 && !ch.is_control() => ch.to_string(),
                    _ => format!("\\u{:04x}", ch as u32),
                })
                .collect();
            parts.push(format!("\"{key}\":\"{val}\""));
        }
    }
    println!("const NIDA = {{{}}};", parts.join(","));
}
