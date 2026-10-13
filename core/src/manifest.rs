//! Model manifest: what a neural model on disk is (see models/README.md). Parsed and validated without any
//! ONNX dependency, so the format is settled before the runtime exists.
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelManifest {
    pub name: String,
    pub version: String,
    pub kind: String,
    pub file: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub sha256: Option<String>,
    pub tokenizer: String,
    pub context_tokens: u32,
    #[serde(default = "default_top_k")]
    pub rerank_top_k: u32,
    #[serde(default = "default_alpha")]
    pub blend_alpha: f64,
    #[serde(default)]
    pub license: String,
}

fn default_top_k() -> u32 {
    20
}
fn default_alpha() -> f64 {
    0.5
}

impl ModelManifest {
    pub fn from_json(text: &str) -> Result<ModelManifest, String> {
        let m: ModelManifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
        m.check_fields()?;
        Ok(m)
    }

    fn check_fields(&self) -> Result<(), String> {
        let semver = self.version.split('.').count() == 3
            && self.version.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
        if self.name.is_empty() {
            return Err("name is empty".into());
        }
        if !semver {
            return Err(format!("version {:?} is not x.y.z", self.version));
        }
        if self.kind != "onnx-causal-lm" {
            return Err(format!("unknown model kind {:?}", self.kind));
        }
        for (what, p) in [("file", &self.file), ("tokenizer", &self.tokenizer)] {
            // paths are relative to the manifest and must stay inside its folder
            if p.is_empty() || Path::new(p).is_absolute() || p.split(['/', '\\']).any(|c| c == "..") {
                return Err(format!("{what} must be a relative path inside the model folder, got {p:?}"));
            }
        }
        if self.size_bytes == 0 {
            return Err("size_bytes must be positive".into());
        }
        if let Some(h) = &self.sha256 {
            if h.len() != 64 || !h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err("sha256 must be 64 lowercase hex digits".into());
            }
        }
        if !(1..=512).contains(&self.context_tokens) {
            return Err("context_tokens must be 1..=512".into());
        }
        if !(1..=50).contains(&self.rerank_top_k) {
            return Err("rerank_top_k must be 1..=50".into());
        }
        if !(0.0..=1.0).contains(&self.blend_alpha) {
            return Err("blend_alpha must be between 0 and 1".into());
        }
        Ok(())
    }

    /// Check the files next to the manifest: both exist and the model has the declared size.
    pub fn validate(&self, dir: &Path) -> Result<(), String> {
        let model = dir.join(&self.file);
        let meta = std::fs::metadata(&model).map_err(|e| format!("{}: {e}", model.display()))?;
        if meta.len() != self.size_bytes {
            return Err(format!("{} is {} bytes, the manifest says {}", self.file, meta.len(), self.size_bytes));
        }
        let tok = dir.join(&self.tokenizer);
        std::fs::metadata(&tok).map_err(|e| format!("{}: {e}", tok.display()))?;
        Ok(())
    }

    /// Read `manifest.json` from a model folder and validate the files.
    pub fn load(dir: &Path) -> Result<ModelManifest, String> {
        let p = dir.join("manifest.json");
        let m = ModelManifest::from_json(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?)?;
        m.validate(dir)?;
        Ok(m)
    }
}
