//! The update screen (CONTRACT, «The update screen»; Dub Studio `update_screen.rs`): what this release changed in what
//! the game downloads. The plan compares the disk with the release's pins and its retired list: what is updated (old →
//! new, size), what is new, what is no longer needed and is removed (space freed), each with its reason from the
//! release's table of changes below, as a code the window puts into its language. One button downloads it all and
//! removes the former files only after the downloads that take their place; the screen comes once per release whose
//! plan is not empty, and later from «What to download».

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::setup::{self, Component, Requirement};

/// The release whose plan the window has shown (the last line counts).
pub const SEEN_FILE: &str = "update-seen.txt";

/// One change of this release: a component (by id) or a leftover of a former release (by its path in the data
/// folder), the code of the reason the window shows, and for a component the files of a former release it takes the
/// place of.
struct Change {
    what: &'static str,
    reason: &'static str,
    replaces: &'static [&'static str],
}

/// The table of changes of this release against 0.13.4. A release that changes what the game downloads writes its own
/// rows here.
const CHANGES: &[Change] = &[
    Change { what: "narrator-engine", reason: "llama_newer", replaces: &[] },
    Change { what: "narrator-engine-cuda12", reason: "narrator_old_cards", replaces: &[] },
    Change { what: "voice-engine", reason: "voice_server", replaces: &["models/tts-engine/audiocpp_engine.dll"] },
    Change { what: "voice-model-q8_0", reason: "voice_quant", replaces: &[] },
    Change { what: "voice-model-q6_k", reason: "voice_quant", replaces: &[] },
    Change { what: "asr-crisperwhisper", reason: "crisperwhisper", replaces: &[] },
    Change { what: "models/asr", reason: "parakeet_onnx_gone", replaces: &[] },
    Change { what: "models/asr-ultra", reason: "parakeet_onnx_gone", replaces: &[] },
    Change { what: "models/runtime/onnxruntime-win-x64-1.28.2", reason: "ort_gone", replaces: &[] },
    Change { what: "tools/whisper", reason: "faster_whisper_gone", replaces: &[] },
    Change { what: "models/whisper", reason: "faster_whisper_gone", replaces: &[] },
    Change { what: "models/tts-engine/audiocpp_engine.dll", reason: "higgs_library_gone", replaces: &[] },
    Change { what: "references", reason: "references_moved", replaces: &[] },
];

fn change(what: &str) -> Option<&'static Change> {
    CHANGES.iter().find(|c| c.what == what)
}

/// A component to download: updated (`from` names the release on disk when the disk tells it) or new.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: String,
    pub title: String,
    pub from: Option<String>,
    pub to: String,
    /// Bytes still to download.
    pub download: u64,
    pub reason: String,
}

/// A leftover of a former release that the button removes.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gone {
    pub path: String,
    /// Bytes its removal frees.
    pub size: u64,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub version: String,
    pub updated: Vec<Row>,
    pub added: Vec<Row>,
    pub retired: Vec<Gone>,
    pub download: u64,
    pub freed: u64,
    /// The window shows the screen at start: the plan is not empty, the game was set up by a former release and this
    /// release's plan has not been shown yet.
    pub show: bool,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.updated.is_empty() && self.added.is_empty() && self.retired.is_empty()
    }

    /// The components the button downloads.
    pub fn ids(&self) -> Vec<String> {
        self.updated.iter().chain(&self.added).map(|r| r.id.clone()).collect()
    }
}

/// The release a pinned URL names: the GitHub release tag, the Hugging Face revision, or else the file name.
fn release_of(url: &str) -> String {
    let parts: Vec<&str> = url.split('/').collect();
    let after = |word: &str| parts.iter().position(|p| *p == word).and_then(|i| parts.get(i + 1)).copied();
    if let (Some(_), Some(tag)) = (parts.iter().position(|p| *p == "releases"), after("download")) {
        return tag.to_string();
    }
    if let Some(revision) = after("resolve") {
        return revision.chars().take(8).collect();
    }
    parts.last().copied().unwrap_or_default().to_string()
}

/// A former release of the component on disk, and its name when the disk tells it: a file it replaces, an unpacked
/// archive of another build (its program there, its install mark not this pin's), or a file of another size.
fn former(root: &Path, component: &Component) -> Option<Option<String>> {
    let replaced = change(component.id).into_iter().flat_map(|c| c.replaces.iter()).find(|rel| root.join(rel).is_file());
    if let Some(rel) = replaced {
        return Some(rel.rsplit('/').next().map(str::to_string));
    }
    let other = component.files.iter().any(|file| {
        if file.is_archive() {
            let marker = root.join(file.marker);
            marker.extension().is_some() && marker.is_file()
        } else {
            std::fs::metadata(root.join(file.path)).is_ok_and(|meta| meta.is_file() && meta.len() != file.bytes)
        }
    });
    other.then_some(None)
}

/// Bytes of a file or a folder tree.
fn tree_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    std::fs::read_dir(path).map(|entries| entries.flatten().map(|entry| tree_size(&entry.path())).sum()).unwrap_or(0)
}

/// The leftovers of former releases on disk: the retired engines and models, and the voice references former versions
/// cut beside the clips.
pub fn leftovers(root: &Path) -> Vec<(&'static str, Vec<PathBuf>)> {
    let mut out: Vec<(&'static str, Vec<PathBuf>)> =
        setup::RETIRED_ASR.iter().map(|rel| (*rel, root.join(rel))).filter(|(_, path)| path.exists()).map(|(rel, path)| (rel, vec![path])).collect();
    let references = setup::retired_references(root);
    if !references.is_empty() {
        out.push(("references", references));
    }
    out
}

/// The plan of this release for the game in `root`.
pub fn plan(root: &Path, version: &str) -> Plan {
    let status = setup::status(root);
    let (mut updated, mut added) = (Vec::new(), Vec::new());
    for st in status.iter().filter(|s| !s.present && s.component.requirement != Requirement::Optional) {
        let component = &st.component;
        let row = |from: Option<String>, reason: String| Row {
            id: component.id.to_string(),
            title: component.title.to_string(),
            from,
            to: component.files.first().map(|f| release_of(f.url)).unwrap_or_default(),
            download: component.bytes().saturating_sub(st.have_bytes),
            reason,
        };
        let reason = change(component.id).map(|c| c.reason.to_string());
        match former(root, component) {
            Some(from) => updated.push(row(from, reason.unwrap_or_else(|| "newer".into()))),
            None => {
                if let Some(reason) = reason {
                    added.push(row(None, reason));
                }
            }
        }
    }
    let retired: Vec<Gone> = leftovers(root)
        .into_iter()
        .map(|(what, paths)| Gone {
            path: if what == "references" { "models/voices/*.ref*s.wav".to_string() } else { what.to_string() },
            size: paths.iter().map(|p| tree_size(p)).sum(),
            reason: change(what).map_or_else(|| "unused".to_string(), |c| c.reason.to_string()),
        })
        .collect();
    let download = updated.iter().chain(&added).map(|r| r.download).sum();
    let freed = retired.iter().map(|g| g.size).sum();
    let set_up_before = !updated.is_empty() || !retired.is_empty() || status.iter().any(|s| s.component.id == "narrator" && s.present);
    let mut plan = Plan { version: version.to_string(), updated, added, retired, download, freed, show: false };
    plan.show = set_up_before && !plan.is_empty() && !seen(root, version);
    plan
}

/// Removes the leftovers of former releases (the button, once what takes their place is downloaded).
pub fn remove_leftovers(root: &Path) {
    setup::remove_retired_asr(root);
    setup::remove_retired_references(root);
}

/// Whether the window has shown this release's plan.
fn seen(root: &Path, version: &str) -> bool {
    match std::fs::read_to_string(root.join(SEEN_FILE)) {
        Ok(text) => text.lines().last().is_some_and(|line| line.trim() == version),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => {
            tracing::error!("[ERROR] {SEEN_FILE}: {e}");
            false
        }
    }
}

/// The window has shown this release's plan (the player pressed the button or «Later»).
pub fn mark_seen(root: &Path, version: &str) -> Result<(), String> {
    std::fs::write(root.join(SEEN_FILE), format!("{version}\n")).map_err(|e| format!("{SEEN_FILE}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_is_named_by_its_tag_or_revision() {
        assert_eq!(release_of("https://github.com/ggml-org/llama.cpp/releases/download/b11900/llama-b11900-bin-win-cuda-13.4-x64.zip"), "b11900");
        assert_eq!(release_of("https://huggingface.co/drbaph/Higgs-Audio-v3-Studio/resolve/c6e9db5a2062c15accc1b9bfa54d927bbdb124dc/models/higgs-q8_0/q8_0.gguf"), "c6e9db5a");
        assert_eq!(release_of("https://example.invalid/e.dll"), "e.dll");
    }

    #[test]
    fn every_change_names_a_component_or_a_leftover() {
        let ids: Vec<&str> = crate::setup::all_component_ids();
        for c in CHANGES {
            assert!(ids.contains(&c.what) || c.what == "references" || crate::setup::RETIRED_ASR.contains(&c.what), "{} is neither a component nor a leftover", c.what);
        }
    }

    #[test]
    fn a_former_release_shows_once_and_its_leftovers_wait_for_the_button() {
        let root = tempfile::tempdir().unwrap();
        let fresh = plan(root.path(), "9.9.9");
        assert!(!fresh.show, "a first start is the setup screen's, not the update screen's");
        assert!(fresh.retired.is_empty() && fresh.updated.is_empty());

        let library = root.path().join("models/tts-engine/audiocpp_engine.dll");
        std::fs::create_dir_all(library.parent().unwrap()).unwrap();
        std::fs::write(&library, vec![0u8; 1000]).unwrap();
        let whisper = root.path().join("models/whisper/large-v3");
        std::fs::create_dir_all(&whisper).unwrap();
        std::fs::write(whisper.join("model.bin"), vec![0u8; 500]).unwrap();
        let voices = root.path().join("models/voices");
        std::fs::create_dir_all(&voices).unwrap();
        std::fs::write(voices.join("A.ref12s.wav"), vec![0u8; 20]).unwrap();

        let p = plan(root.path(), "9.9.9");
        assert!(p.show);
        if let Some(server) = p.updated.iter().find(|r| r.id == "voice-engine") {
            assert_eq!(server.from.as_deref(), Some("audiocpp_engine.dll"), "the server takes the place of the library");
        }
        let gone: Vec<(&str, u64)> = p.retired.iter().map(|g| (g.path.as_str(), g.size)).collect();
        assert!(gone.contains(&("models/whisper", 500)) && gone.contains(&("models/tts-engine/audiocpp_engine.dll", 1000)), "{gone:?}");
        assert!(gone.contains(&("models/voices/*.ref*s.wav", 20)), "{gone:?}");
        assert!(library.is_file() && whisper.is_dir(), "the plan itself removes nothing");

        mark_seen(root.path(), "9.9.9").unwrap();
        assert!(!plan(root.path(), "9.9.9").show, "shown once per release");
        assert!(plan(root.path(), "10.0.0").show, "the next release shows its own");

        remove_leftovers(root.path());
        assert!(!library.exists() && !root.path().join("models/whisper").exists() && !voices.join("A.ref12s.wav").exists());
        assert!(plan(root.path(), "10.0.0").retired.is_empty());
    }
}
