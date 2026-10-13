//! The engine side of the Windows IPC: turns protocol requests into `khcore` calls.
use khcore::{api_json, Engine, Key, Mode};
use khipc::Handler;

pub struct EngineHandler(pub Engine);

fn mode_of(m: u8) -> Result<Mode, String> {
    match m {
        0 => Ok(Mode::Khmer),
        1 => Ok(Mode::Latin),
        2 => Ok(Mode::English),
        _ => Err(format!("unknown mode {m}")),
    }
}

impl Handler for EngineHandler {
    fn vocab(&self) -> u64 {
        self.0.model().vocab() as u64
    }

    fn suggest(&self, text: &str, mode: u8, just_accepted: Option<&str>) -> Result<String, String> {
        Ok(api_json::candidates(&self.0.suggest(text, mode_of(mode)?, just_accepted)))
    }

    fn key_action(&self, text: &str, mode: u8, key: u8, just_accepted: Option<&str>) -> Result<String, String> {
        let key = match key {
            0 => Key::Space,
            1 => Key::Enter,
            2 => Key::Tab,
            k => return Err(format!("unknown key {k}")),
        };
        Ok(api_json::key_action(&self.0.key_action(text, mode_of(mode)?, key, just_accepted)))
    }
}
