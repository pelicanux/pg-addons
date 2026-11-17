//! Apex Neural Rendering (ANR FG) + OptiScaler menu + nr-record service.
pub mod native;

use peligames_addon_kit::{
    jobs,
    protocol::{RpcError, ServiceCall, TransferProgress},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Cursor, Read},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

pub(crate) fn error(message: impl Into<String>) -> RpcError {
    RpcError {
        code: -32000,
        message: message.into(),
    }
}

fn params<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value).map_err(|_| RpcError {
        code: -32602,
        message: "Invalid request parameters".into(),
    })
}

#[derive(Deserialize)]
struct Job {
    id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallMode {
    None,
    Full,
    Addon,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub anr_ready: bool,
    pub anr_version: Option<String>,
    pub record_ready: bool,
    pub record_version: Option<String>,
    pub directory: PathBuf,
    pub recordings_dir: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameState {
    pub mode: InstallMode,
    pub proxy: Option<String>,
    pub has_optiscaler_ini: bool,
    pub record_installed: bool,
    pub nr_fg: BTreeMap<String, String>,
    pub optiscaler: BTreeMap<String, String>,
    pub nr_record: BTreeMap<String, String>,
}

pub struct Service {
    pub runtime: tokio::runtime::Runtime,
    pub jobs: jobs::Jobs,
}

impl Service {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
            jobs: jobs::Jobs::default(),
        })
    }

    pub fn handle(&mut self, method: &str, value: Value) -> Result<Value, RpcError> {
        let ServiceCall { host, request }: ServiceCall<Value> = params(value)?;
        if !host.component_directory.is_absolute() {
            return Err(error("Invalid managed component directory"));
        }
        match method {
            "native.view" => native::view(self, &host.component_directory, params(request)?),
            "native.action" => native::action(self, host, params(request)?),
            "job.inspect" => {
                let job: Job = params(request)?;
                let status = self.jobs.status(job.id).map_err(|e| error(e.message))?;
                serde_json::to_value(status).map_err(|_| error("Cannot encode job status"))
            }
            _ => Err(RpcError {
                code: -32601,
                message: "Method not found".into(),
            }),
        }
    }
}

pub fn runtime_info(root: &Path) -> RuntimeInfo {
    let full_ready = root.join("anr-fg/full/game/dxgi.dll").is_file();
    let addon_ready = root
        .join("anr-fg/addon/Apex Neural Rendering FG/dlssg/nvngx_dlssg.dll")
        .is_file();
    let anr_version = fs::read_to_string(root.join("anr-fg/version.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let record_ready = find_nr_record_root(&root.join("nr-record")).is_some();
    let record_version = fs::read_to_string(root.join("nr-record/version.txt"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let recordings_dir = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| root.to_path_buf())
        .join("nr-record");
    let _ = fs::create_dir_all(&recordings_dir);
    RuntimeInfo {
        anr_ready: full_ready && addon_ready,
        anr_version,
        record_ready,
        record_version,
        directory: root.to_path_buf(),
        recordings_dir,
    }
}

pub fn find_nr_record_root(base: &Path) -> Option<PathBuf> {
    if base
        .join("capture/OptiScaler/custom/nr-record/dlssg/nr_record.ini")
        .is_file()
    {
        return Some(base.to_path_buf());
    }
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir()
                && p.join("capture/OptiScaler/custom/nr-record/dlssg/nr_record.ini")
                    .is_file()
            {
                return Some(p);
            }
        }
    }
    None
}

pub fn game_state(game_dir: &Path) -> GameState {
    let full_record = game_dir.join("OptiScaler/nr_fg_install.json");
    let addon_record_1 = game_dir.join("OptiScaler/custom/Apex Neural Rendering FG/nr_fg_install.json");
    let addon_record_2 = game_dir.join("custom/Apex Neural Rendering FG/nr_fg_install.json");
    let anr_dll_1 = game_dir.join("OptiScaler/custom/Apex Neural Rendering FG/dlssg/nvngx_dlssg.dll");
    let anr_dll_2 = game_dir.join("custom/Apex Neural Rendering FG/dlssg/nvngx_dlssg.dll");

    let mut proxy = None;
    let mode = if full_record.is_file() {
        if let Ok(text) = fs::read_to_string(&full_record)
            && let Ok(val) = serde_json::from_str::<Value>(&text)
        {
            proxy = val.get("proxy").and_then(Value::as_str).map(String::from);
        }
        InstallMode::Full
    } else if addon_record_1.is_file() || addon_record_2.is_file() || anr_dll_1.is_file() || anr_dll_2.is_file() {
        InstallMode::Addon
    } else {
        InstallMode::None
    };

    let opti_ini_path = game_dir.join("OptiScaler.ini");
    let has_optiscaler_ini = opti_ini_path.is_file();

    let nr_fg_ini_path = if game_dir
        .join("OptiScaler/custom/Apex Neural Rendering FG/dlssg/nr_fg.ini")
        .is_file()
    {
        game_dir.join("OptiScaler/custom/Apex Neural Rendering FG/dlssg/nr_fg.ini")
    } else {
        game_dir.join("custom/Apex Neural Rendering FG/dlssg/nr_fg.ini")
    };

    let record_ini_path = game_dir.join("OptiScaler/custom/nr-record/dlssg/nr_record.ini");
    let record_installed = record_ini_path.is_file();

    let nr_fg = fs::read_to_string(&nr_fg_ini_path)
        .map(|s| parse_flat_ini(&s))
        .unwrap_or_default();
    let optiscaler = fs::read_to_string(&opti_ini_path)
        .map(|s| parse_sectioned_ini(&s))
        .unwrap_or_default();
    let nr_record = fs::read_to_string(&record_ini_path)
        .map(|s| parse_flat_ini(&s))
        .unwrap_or_default();

    GameState {
        mode,
        proxy,
        has_optiscaler_ini,
        record_installed,
        nr_fg,
        optiscaler,
        nr_record,
    }
}

pub fn parse_flat_ini(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') || trimmed.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

pub fn patch_flat_ini(text: &str, key: &str, value: &str) -> Result<String, String> {
    if key.is_empty() || key.contains(['\n', '\r', '=']) || value.contains(['\n', '\r']) {
        return Err("Invalid INI key or value".into());
    }
    let mut lines: Vec<String> = Vec::new();
    let mut updated = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(';')
            && !trimmed.starts_with('#')
            && let Some((k, _)) = trimmed.split_once('=')
            && k.trim() == key
        {
            lines.push(format!("{key} = {value}"));
            updated = true;
            continue;
        }
        lines.push(line.to_string());
    }
    if !updated {
        lines.push(format!("{key} = {value}"));
    }
    Ok(lines.join("\n") + "\n")
}

pub fn parse_sectioned_ini(text: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut section = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            section = trimmed[1..trimmed.len() - 1].trim().to_string();
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let full_key = if section.is_empty() {
                k.trim().to_string()
            } else {
                format!("{section}.{}", k.trim())
            };
            map.insert(full_key, v.trim().to_string());
        }
    }
    map
}

pub fn patch_sectioned_ini(text: &str, full_key: &str, value: &str) -> Result<String, String> {
    let (target_sec, target_key) = full_key
        .split_once('.')
        .ok_or("Expected Section.Key format")?;
    if target_sec.is_empty()
        || target_key.is_empty()
        || value.contains(['\n', '\r'])
        || target_key.contains(['\n', '\r', '='])
    {
        return Err("Invalid INI key or value".into());
    }
    let mut out = Vec::new();
    let mut in_sec = false;
    let mut seen_sec = false;
    let mut done = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_sec && !done {
                out.push(format!("{target_key}={value}"));
                done = true;
            }
            let sec_name = trimmed[1..trimmed.len() - 1].trim();
            in_sec = sec_name.eq_ignore_ascii_case(target_sec);
            if in_sec {
                seen_sec = true;
            }
            out.push(line.to_string());
            continue;
        }
        if in_sec
            && !trimmed.starts_with(';')
            && !trimmed.starts_with('#')
            && let Some((k, _)) = trimmed.split_once('=')
            && k.trim().eq_ignore_ascii_case(target_key)
        {
            out.push(format!("{target_key}={value}"));
            done = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !done {
        if !seen_sec {
            out.push(format!("[{target_sec}]"));
        }
        out.push(format!("{target_key}={value}"));
    }
    Ok(out.join("\n") + "\n")
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.map_err(|e| e.to_string())?;
        let rel = entry
            .path()
            .strip_prefix(src)
            .map_err(|e| e.to_string())?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn install_full(root: &Path, game_dir: &Path, proxy: &str) -> Result<String, String> {
    let proxy = if proxy.is_empty() { "dxgi.dll" } else { proxy };
    if !matches!(proxy, "dxgi.dll" | "winmm.dll" | "version.dll") {
        return Err("Proxy DLL must be dxgi.dll, winmm.dll or version.dll".into());
    }
    let full_game = root.join("anr-fg/full/game");
    if !full_game.join("dxgi.dll").is_file() {
        return Err("Baixe o pacote ANR FG primeiro na aba Gerenciar.".into());
    }
    let _lock = peligames_mod_config::DirectoryLock::acquire(game_dir)?;
    let opti = game_dir.join("OptiScaler");
    let record_path = opti.join("nr_fg_install.json");
    let backup = opti.join("nr_fg_backup");
    fs::create_dir_all(&backup).map_err(|e| e.to_string())?;

    if !record_path.is_file() {
        let mut originals = serde_json::Map::new();
        for name in [proxy, "OptiScaler.ini"] {
            let existing = game_dir.join(name);
            if existing.is_file() {
                let bpath = backup.join(name);
                fs::copy(&existing, &bpath).map_err(|e| e.to_string())?;
                originals.insert(name.to_string(), Value::String(sha256_file(&existing)?));
            }
        }
        let record = json!({
            "originals": originals,
            "proxy": proxy,
        });
        fs::write(
            &record_path,
            serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }

    let cfg_path = opti.join("custom/Apex Neural Rendering FG/dlssg/nr_fg.ini");
    let preserved_cfg = fs::read_to_string(&cfg_path).ok();

    fs::copy(full_game.join("dxgi.dll"), game_dir.join(proxy)).map_err(|e| e.to_string())?;
    if !game_dir.join("OptiScaler.ini").is_file() {
        fs::copy(
            full_game.join("OptiScaler.ini"),
            game_dir.join("OptiScaler.ini"),
        )
        .map_err(|e| e.to_string())?;
    } else if let Ok(existing_ini) = fs::read_to_string(game_dir.join("OptiScaler.ini")) {
        let mut patched = patch_sectioned_ini(&existing_ini, "FrameGen.FGInput", "nvngxfg")?;
        patched = patch_sectioned_ini(
            &patched,
            "FrameGen.FGNvngxReplacement",
            "Apex Neural Rendering FG",
        )?;
        patched = patch_sectioned_ini(&patched, "FrameGen.DisableHudless", "false")?;
        peligames_mod_config::write_atomic(&game_dir.join("OptiScaler.ini"), &patched)?;
    }
    copy_dir_recursive(&full_game.join("OptiScaler"), &opti)?;
    if let Some(cfg) = preserved_cfg {
        let _ = peligames_mod_config::write_atomic(&cfg_path, &cfg);
    }

    let dll_base = proxy.trim_end_matches(".dll");
    Ok(format!("WINEDLLOVERRIDES=\"{dll_base}=n,b\" %command%"))
}

pub fn install_addon(root: &Path, game_dir: &Path) -> Result<(), String> {
    let addon_src = root.join("anr-fg/addon/Apex Neural Rendering FG");
    if !addon_src.join("dlssg/nvngx_dlssg.dll").is_file() {
        return Err("Baixe o pacote ANR FG primeiro na aba Gerenciar.".into());
    }
    let ini_path = game_dir.join("OptiScaler.ini");
    if !ini_path.is_file() {
        return Err("OptiScaler.ini não encontrado no jogo. Use 'Instalar ANR FG Completo'.".into());
    }
    let _lock = peligames_mod_config::DirectoryLock::acquire(game_dir)?;
    let main_dir = if game_dir.join("OptiScaler").is_dir() {
        game_dir.join("OptiScaler")
    } else {
        game_dir.to_path_buf()
    };
    let dest = main_dir.join("custom/Apex Neural Rendering FG");
    let record_path = dest.join("nr_fg_install.json");
    let ini_text = fs::read_to_string(&ini_path).map_err(|e| e.to_string())?;
    let parsed = parse_sectioned_ini(&ini_text);

    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    if !record_path.is_file() {
        let record = json!({
            "optiscaler_ini": ini_path,
            "before": {
                "FGInput": parsed.get("FrameGen.FGInput").cloned().unwrap_or_default(),
                "FGNvngxReplacement": parsed.get("FrameGen.FGNvngxReplacement").cloned().unwrap_or_default(),
                "DisableHudless": parsed.get("FrameGen.DisableHudless").cloned().unwrap_or_default(),
            }
        });
        fs::write(
            &record_path,
            serde_json::to_string_pretty(&record).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }

    let cfg_path = dest.join("dlssg/nr_fg.ini");
    let preserved_cfg = fs::read_to_string(&cfg_path).ok();
    copy_dir_recursive(&addon_src, &dest)?;
    if let Some(cfg) = preserved_cfg {
        let _ = peligames_mod_config::write_atomic(&cfg_path, &cfg);
    }

    let mut patched = patch_sectioned_ini(&ini_text, "FrameGen.FGInput", "nvngxfg")?;
    patched = patch_sectioned_ini(
        &patched,
        "FrameGen.FGNvngxReplacement",
        "Apex Neural Rendering FG",
    )?;
    patched = patch_sectioned_ini(&patched, "FrameGen.DisableHudless", "false")?;
    peligames_mod_config::write_atomic(&ini_path, &patched)?;
    Ok(())
}

pub fn remove_game(game_dir: &Path) -> Result<Option<String>, String> {
    let _lock = peligames_mod_config::DirectoryLock::acquire(game_dir)?;
    let opti = game_dir.join("OptiScaler");
    let full_record = opti.join("nr_fg_install.json");
    if full_record.is_file() {
        let text = fs::read_to_string(&full_record).map_err(|e| e.to_string())?;
        let val: Value = serde_json::from_str(&text).unwrap_or_default();
        let proxy = val
            .get("proxy")
            .and_then(Value::as_str)
            .unwrap_or("dxgi.dll");
        let originals = val.get("originals").and_then(Value::as_object);
        let backup = opti.join("nr_fg_backup");
        for name in [proxy, "OptiScaler.ini"] {
            if let Some(orig_hash) = originals
                .and_then(|o| o.get(name))
                .and_then(Value::as_str)
            {
                let bfile = backup.join(name);
                if bfile.is_file() && sha256_file(&bfile).ok().as_deref() == Some(orig_hash) {
                    fs::copy(&bfile, game_dir.join(name)).map_err(|e| e.to_string())?;
                }
            } else {
                let _ = fs::remove_file(game_dir.join(name));
            }
        }
        let _ = fs::remove_file(game_dir.join("OptiScaler.log"));
        let _ = fs::remove_dir_all(&opti);
        return Ok(None);
    }

    for candidate in [
        game_dir.join("OptiScaler/custom/Apex Neural Rendering FG"),
        game_dir.join("custom/Apex Neural Rendering FG"),
    ] {
        let record_path = candidate.join("nr_fg_install.json");
        if candidate.is_dir() {
            if let Ok(text) = fs::read_to_string(&record_path)
                && let Ok(val) = serde_json::from_str::<Value>(&text)
                && let Some(before) = val.get("before").and_then(Value::as_object)
            {
                let ini_path = game_dir.join("OptiScaler.ini");
                if let Ok(mut ini_text) = fs::read_to_string(&ini_path) {
                    for (k, v) in before {
                        if let Some(v_str) = v.as_str().filter(|s| !s.is_empty()) {
                            if let Ok(next) =
                                patch_sectioned_ini(&ini_text, &format!("FrameGen.{k}"), v_str)
                            {
                                ini_text = next;
                            }
                        }
                    }
                    let _ = peligames_mod_config::write_atomic(&ini_path, &ini_text);
                }
            }
            let _ = fs::remove_dir_all(&candidate);
        }
    }
    Ok(None)
}

pub fn install_record(root: &Path, game_dir: &Path) -> Result<(), String> {
    let rec_root = find_nr_record_root(&root.join("nr-record"))
        .ok_or("Baixe o pacote nr-record primeiro na aba Gerenciar.")?;
    let src = rec_root.join("capture/OptiScaler/custom/nr-record");
    let ini_path = game_dir.join("OptiScaler.ini");
    if !ini_path.is_file() {
        return Err("Instale o ANR FG (ou OptiScaler) no jogo antes de ativar o nr-record.".into());
    }
    let _lock = peligames_mod_config::DirectoryLock::acquire(game_dir)?;
    let opti = game_dir.join("OptiScaler");
    let dest = opti.join("custom/nr-record");
    let backup_dir = opti.join("nr_record_backup");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let backup_ini = backup_dir.join("OptiScaler.ini");
    if !backup_ini.is_file() {
        fs::copy(&ini_path, &backup_ini).map_err(|e| e.to_string())?;
    }
    let preserved_dlssg = fs::read_to_string(dest.join("dlssg/nr_record.ini")).ok();
    copy_dir_recursive(&src, &dest)?;
    let dlssg_ini_path = dest.join("dlssg/nr_record.ini");
    if let Some(prev) = preserved_dlssg {
        peligames_mod_config::write_atomic(&dlssg_ini_path, &prev)?;
    } else if let Ok(text) = fs::read_to_string(&dlssg_ini_path) {
        let forward = if opti
            .join("custom/Apex Neural Rendering FG/dlssg/nvngx_dlssg.dll")
            .is_file()
        {
            "..\\..\\Apex Neural Rendering FG\\dlssg\\nvngx_dlssg.dll"
        } else {
            "nvidia"
        };
        let patched = patch_flat_ini(&text, "forward", forward)?;
        peligames_mod_config::write_atomic(&dlssg_ini_path, &patched)?;
    }
    let ini_text = fs::read_to_string(&ini_path).map_err(|e| e.to_string())?;
    let patched = patch_sectioned_ini(&ini_text, "FrameGen.FGNvngxReplacement", "nr-record")?;
    peligames_mod_config::write_atomic(&ini_path, &patched)?;
    Ok(())
}

pub fn remove_record(game_dir: &Path) -> Result<(), String> {
    let _lock = peligames_mod_config::DirectoryLock::acquire(game_dir)?;
    let opti = game_dir.join("OptiScaler");
    let dest = opti.join("custom/nr-record");
    let backup_dir = opti.join("nr_record_backup");
    let ini_path = game_dir.join("OptiScaler.ini");
    if ini_path.is_file() {
        let fallback_provider = if opti
            .join("custom/Apex Neural Rendering FG/dlssg/nvngx_dlssg.dll")
            .is_file()
        {
            "Apex Neural Rendering FG"
        } else {
            "Nukems"
        };
        if let Ok(text) = fs::read_to_string(&ini_path)
            && let Ok(patched) =
                patch_sectioned_ini(&text, "FrameGen.FGNvngxReplacement", fallback_provider)
        {
            let _ = peligames_mod_config::write_atomic(&ini_path, &patched);
        }
    }
    let _ = fs::remove_dir_all(&dest);
    let _ = fs::remove_dir_all(&backup_dir);
    Ok(())
}

pub fn update_setting(game_dir: &Path, scope_key: &str, value: &str) -> Result<(), String> {
    let _lock = peligames_mod_config::DirectoryLock::acquire(game_dir)?;
    if let Some(key) = scope_key.strip_prefix("anr:") {
        let path = if game_dir
            .join("OptiScaler/custom/Apex Neural Rendering FG/dlssg/nr_fg.ini")
            .is_file()
        {
            game_dir.join("OptiScaler/custom/Apex Neural Rendering FG/dlssg/nr_fg.ini")
        } else {
            game_dir.join("custom/Apex Neural Rendering FG/dlssg/nr_fg.ini")
        };
        let current = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let patched = patch_flat_ini(&current, key, value)?;
        return peligames_mod_config::write_atomic(&path, &patched);
    }
    if let Some(key) = scope_key.strip_prefix("opti:") {
        let path = game_dir.join("OptiScaler.ini");
        let current = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let patched = patch_sectioned_ini(&current, key, value)?;
        return peligames_mod_config::write_atomic(&path, &patched);
    }
    if let Some(key) = scope_key.strip_prefix("rec:") {
        let dlssg_path = game_dir.join("OptiScaler/custom/nr-record/dlssg/nr_record.ini");
        let current = fs::read_to_string(&dlssg_path).map_err(|e| e.to_string())?;
        let patched = patch_flat_ini(&current, key, value)?;
        peligames_mod_config::write_atomic(&dlssg_path, &patched)?;
        let dlss_path = game_dir.join("OptiScaler/custom/nr-record/dlss/nr_record.ini");
        if matches!(
            key,
            "record_key" | "capture_key" | "capture_frames" | "record_max_seconds" | "record_free_gb"
        ) && let Ok(sr_text) = fs::read_to_string(&dlss_path)
            && let Ok(sr_patched) = patch_flat_ini(&sr_text, key, value)
        {
            let _ = peligames_mod_config::write_atomic(&dlss_path, &sr_patched);
        }
        return Ok(());
    }
    Err("Unknown setting prefix".into())
}

pub fn launch_viewer(root: &Path) -> Result<(), String> {
    let rec_root = find_nr_record_root(&root.join("nr-record"))
        .ok_or("Baixe o pacote nr-record primeiro na aba Gerenciar.")?;
    let viewer = rec_root.join("viewer/renderer");
    if !viewer.is_file() {
        return Err("Executável viewer/renderer não encontrado no pacote nr-record.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(&viewer) {
            let mut perms = meta.permissions();
            perms.set_mode(perms.mode() | 0o755);
            let _ = fs::set_permissions(&viewer, perms);
        }
    }
    std::process::Command::new(&viewer)
        .arg("--captures")
        .current_dir(&rec_root)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

type ProgressFn = Arc<dyn Fn(TransferProgress) + Send + Sync>;

fn notify(progress: &ProgressFn, phase: &'static str, downloaded: u64, total: Option<u64>) {
    progress(TransferProgress {
        phase,
        downloaded,
        total,
    });
}

async fn download_bytes(
    client: &reqwest::Client,
    url: &str,
    limit: u64,
    progress: &ProgressFn,
) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let total = response.content_length();
    if total.is_some_and(|n| n > limit) {
        return Err("Component download exceeds size limit".into());
    }
    let mut bytes = Vec::new();
    let mut last = Instant::now();
    notify(progress, "downloading", 0, total);
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() as u64 + chunk.len() as u64 > limit {
            return Err("Component download exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
        if last.elapsed() >= Duration::from_millis(200) {
            notify(progress, "downloading", bytes.len() as u64, total);
            last = Instant::now();
        }
    }
    notify(progress, "downloading", bytes.len() as u64, total);
    Ok(bytes)
}

fn unpack_zip(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            return Err("Unsafe path in zip archive".into());
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            fs::write(&out, &buf).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GhRelease {
    #[serde(default)]
    draft: bool,
    tag_name: String,
    assets: Vec<GhAsset>,
}

pub async fn download_anr_runtime(root: PathBuf, progress: ProgressFn) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("PeliGames")
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    notify(&progress, "connecting", 0, None);
    let releases_bytes = download_bytes(
        &client,
        "https://api.github.com/repos/apex-compute/ANR/releases",
        4 * 1024 * 1024,
        &progress,
    )
    .await?;
    let releases: Vec<GhRelease> =
        serde_json::from_slice(&releases_bytes).map_err(|e| e.to_string())?;
    let rel = releases
        .into_iter()
        .find(|r| {
            !r.draft
                && r.assets.iter().any(|a| a.name.ends_with("-full.zip"))
                && r.assets.iter().any(|a| a.name.ends_with("-addon.zip"))
        })
        .ok_or("Nenhuma release oficial do ANR FG encontrada.")?;
    let full_asset = rel
        .assets
        .iter()
        .find(|a| a.name.ends_with("-full.zip"))
        .unwrap();
    let addon_asset = rel
        .assets
        .iter()
        .find(|a| a.name.ends_with("-addon.zip"))
        .unwrap();
    let full_bytes =
        download_bytes(&client, &full_asset.browser_download_url, 64 * 1024 * 1024, &progress)
            .await?;
    let addon_bytes =
        download_bytes(&client, &addon_asset.browser_download_url, 32 * 1024 * 1024, &progress)
            .await?;
    notify(&progress, "extracting", 0, None);
    let stage = root.join("anr-fg.staging");
    let _ = fs::remove_dir_all(&stage);
    unpack_zip(&full_bytes, &stage.join("full"))?;
    unpack_zip(&addon_bytes, &stage.join("addon"))?;
    fs::write(stage.join("version.txt"), &rel.tag_name).map_err(|e| e.to_string())?;
    let target = root.join("anr-fg");
    let _ = fs::remove_dir_all(&target);
    fs::rename(&stage, &target).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn download_record_runtime(root: PathBuf, progress: ProgressFn) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("PeliGames")
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    notify(&progress, "connecting", 0, None);
    let releases_bytes = download_bytes(
        &client,
        "https://api.github.com/repos/apex-compute/ANR/releases",
        4 * 1024 * 1024,
        &progress,
    )
    .await?;
    let releases: Vec<GhRelease> =
        serde_json::from_slice(&releases_bytes).map_err(|e| e.to_string())?;
    let (tag, asset_url, asset_name) = releases
        .into_iter()
        .filter(|r| !r.draft)
        .find_map(|r| {
            r.assets
                .into_iter()
                .find(|a| a.name.starts_with("nr-record-") && a.name.ends_with("-linux.tar.gz"))
                .map(|a| (r.tag_name, a.browser_download_url, a.name))
        })
        .ok_or("Nenhum pacote oficial nr-record-linux.tar.gz encontrado.")?;
    let bytes = download_bytes(&client, &asset_url, 64 * 1024 * 1024, &progress).await?;
    notify(&progress, "extracting", 0, None);
    let stage = root.join("nr-record.staging");
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    peligames_archive::unpack_tar_gz(&bytes, &stage)?;
    let ver_label = asset_name
        .strip_prefix("nr-record-")
        .and_then(|s| s.strip_suffix("-linux.tar.gz"))
        .unwrap_or(&tag);
    fs::write(stage.join("version.txt"), ver_label).map_err(|e| e.to_string())?;
    let target = root.join("nr-record");
    let _ = fs::remove_dir_all(&target);
    fs::rename(&stage, &target).map_err(|e| e.to_string())?;
    Ok(())
}
