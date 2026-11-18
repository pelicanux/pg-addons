use super::*;
#[derive(Deserialize)]
struct Field {
    key: String,
    label: String,
    group: String,
    section: String,
    values: Vec<String>,
}
pub(super) fn categories(
    t: &Text,
    game: Option<&optiscaler::State>,
    directory: Option<&Path>,
) -> Vec<Category> {
    let fields: Vec<Field> = serde_json::from_str(include_str!("../../../config/fields.json"))
        .expect("native OptiScaler fields");
    [
        ("main", "settings"),
        ("upscaling", "mango-graphics"),
        ("framegeneration", "bolt"),
        ("latency", "sliders"),
    ]
    .into_iter()
    .map(|(id, icon)| {
        let ready = game.is_some_and(|g| {
            g.installed
                && (g.nr_route.is_none() || g.nr_route == Some(optiscaler::NrRoute::Optiscaler))
        });
        let enabled =
            ready && (id != "framegeneration" || game.is_some_and(|g| g.framegen_available));
        let mut sections: Vec<Section> = Vec::new();
        if id == "main" {
            let install_controls = super::management::game_controls(t, game, directory);
            if !install_controls.is_empty() {
                sections.push(Section {
                    columns: false,
                    title: String::new(),
                    controls: install_controls,
                });
            }
        }
        if let Some(game) = game {
            for field in fields.iter().filter(|f| f.group == id) {
                if field.key.starts_with("FSR.Fsr4") && !game.settings.contains_key(&field.key) {
                    continue;
                }
                if field.key == "FrameGen.FGNvngxReplacement" && !game.framegen_modern {
                    continue;
                }
                let label = t.get(&field.label);
                if sections
                    .last()
                    .is_none_or(|s| s.title != t.get(&field.section))
                {
                    sections.push(Section {
                        columns: true,
                        title: t.get(&field.section),
                        controls: Vec::new(),
                    });
                }
                let current = game
                    .settings
                    .get(&field.key)
                    .cloned()
                    .unwrap_or_else(|| "auto".into());
                let control = if field.key == "Menu.ShortcutKey" {
                    Control::Shortcut {
                        id: format!("setting:{}", field.key),
                        label,
                        value: current,
                        enabled,
                    }
                } else if let Some((on, off)) = toggle(&field.key) {
                    Control::Switch {
                        appearance: ui::SwitchStyle::Round,
                        id: format!("setting:{}", field.key),
                        label,
                        hint: t.get(if current == on {
                            "on"
                        } else if current == off {
                            "off"
                        } else {
                            "auto"
                        }),
                        checked: current == on,
                        enabled,
                        on: on.into(),
                        off: off.into(),
                    }
                } else {
                    let mut choices: Vec<_> = field
                        .values
                        .iter()
                        .map(|value| ui::Choice {
                            value: if game.settings.contains_key("FSR.Fsr4Update")
                                && !game.settings.contains_key("FSR.Fsr4GpuSupport")
                                && field.key.starts_with("Upscalers.")
                            {
                                match value.as_str() {
                                    "ffx" => "fsr31".into(),
                                    "ffx_12" => "fsr31_12".into(),
                                    _ => value.clone(),
                                }
                            } else {
                                value.clone()
                            },
                            label: value_label(&field.key, value, t),
                        })
                        .collect();
                    if !game.framegen_modern
                        && (field.key == "FrameGen.FGInput" || field.key == "FrameGen.FGOutput")
                    {
                        choices.retain(|c| {
                            !["nvngxfg", "dlssg", "xefg"].contains(&c.value.as_str())
                                || c.value == "dlssg" && field.key == "FrameGen.FGInput"
                                || c.value == "xefg" && field.key == "FrameGen.FGOutput"
                        });
                        choices.push(ui::Choice {
                            value: "nukems".into(),
                            label: "Nukem · FSR 3 FG".into(),
                        });
                    }
                    let display = if current == "auto" && field.key.starts_with("FrameGen.") {
                        if field.key.ends_with("Enabled") {
                            "false"
                        } else if field.key.ends_with("FGNvngxReplacement") {
                            "None"
                        } else {
                            "nofg"
                        }
                    } else {
                        &current
                    };
                    if !choices.iter().any(|choice| choice.value == display) {
                        choices.push(ui::Choice {
                            value: display.into(),
                            label: display.into(),
                        });
                    }
                    Control::Select {
                        id: format!("setting:{}", field.key),
                        label,
                        value: display.into(),
                        choices,
                        enabled,
                    }
                };
                sections.last_mut().unwrap().controls.push(control);
            }
            if id == "main" && game.installed {
                sections.push(Section {
                    columns: true,
                    title: "OptiPatcher".into(),
                    controls: vec![Control::Switch {
                        appearance: ui::SwitchStyle::Round,
                        id: "patcher".into(),
                        label: "OptiPatcher".into(),
                        hint: t.get("patcher_hint"),
                        enabled: ready,
                        checked: game.patcher.enabled,
                        on: "true".into(),
                        off: "false".into(),
                    }],
                });
            }
            if id == "upscaling" {
                sections.push(Section {
                    columns: true,
                    title: String::new(),
                    controls: vec![text(String::new(), t.get("fsr_library_hint"))],
                });
            }
        }
        Category {
            id: id.into(),
            label: t.get(id),
            icon: icon.into(),
            hint: t.get(if id == "framegeneration" && !enabled {
                "fg_unavailable"
            } else {
                match id {
                    "main" => "main_hint",
                    "upscaling" => "upscaling_hint",
                    "framegeneration" => "framegeneration_hint",
                    _ => "latency_hint",
                }
            }),
            enabled: id == "main" || ready,
            sections,
        }
    })
    .collect()
}
fn toggle(key: &str) -> Option<(&'static str, &'static str)> {
    match key {
        "FrameGen.Enabled"
        | "FSR.Fsr4Update"
        | "FSR.Fsr4ForceEnableInt8"
        | "Inputs.EnableDlssInputs"
        | "Spoofing.Dxgi"
        | "fakenvapi.ForceLatencyFlex" => Some(("true", "false")),
        "fakenvapi.ForceReflex" => Some(("2", "1")),
        _ => None,
    }
}
fn value_label(key: &str, value: &str, t: &Text) -> String {
    if value == "auto" {
        return t.get("auto");
    }
    if value == "true" {
        return t.get("on");
    }
    if value == "false" || value == "nofg" || value == "None" {
        return t.get("off");
    }
    if key == "Menu.Scale" {
        return format!("{}%", (value.parse::<f32>().unwrap_or(1.) * 100.).round());
    }
    if key == "FSR.Fsr4GpuSupport" || key == "FSR.Fsr4Model" {
        let label = if key == "FSR.Fsr4Model" {
            match value {
                "0" => "fsr4_fp8",
                "1" => "fsr4_int8_model",
                "2" => "fsr4_rdna2",
                _ => "fsr4_lite",
            }
        } else {
            match value {
                "0" => "fsr4_no_override",
                "1" => "fsr4_fp8",
                "2" => "fsr4_int8_model",
                "3" => "fsr4_rdna2",
                _ => "fsr4_lite",
            }
        };
        return t.get(label);
    }
    if key == "FSR.UpscalerIndex" {
        return match value {
            "0" => "FSR 4",
            "1" => "FSR 3.1",
            _ => "FSR 2.3",
        }
        .into();
    }
    if key == "fakenvapi.LatencyFlexMode" {
        return t.get(match value {
            "0" => "conservative",
            "1" => "aggressive",
            _ => "reflex_ids",
        });
    }
    match value {
        "ffx" | "fsr31" => "FSR",
        "ffx_12" | "fsr31_12" => "FSR (DX12 bridge)",
        "xess" => "XeSS",
        "xess_12" => "XeSS (DX12 bridge)",
        "fsr21" => "FSR 2.1",
        "fsr22" => "FSR 2.2",
        "dlss" => "DLSS",
        "dlssg" => "DLSSG",
        "nvngxfg" => "DLSSG via NGX",
        "fsrfg" | "FFX" => "FSR FG",
        "fsrfg30" => "FSR 3.0 FG",
        "upscaler" => "OptiFG",
        "xefg" => "XeFG",
        "Nukems" | "nukems" => "Nukem · FSR 3 FG",
        other => other,
    }
    .into()
}
