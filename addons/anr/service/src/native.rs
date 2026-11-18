//! Native GPUI presentation for Apex Neural Rendering (ANR FG), OptiScaler menu and nr-record.
use super::*;
use peligames_addon_kit::native_ui::{self as ui, Category, Choice, Control, Section, StatusItem, View};
use std::collections::BTreeMap;

struct Text(BTreeMap<String, String>);

impl Text {
    fn new(language: &str) -> Self {
        Self(
            serde_json::from_str(if language.starts_with("pt") {
                include_str!("../../ui/locales/pt-BR.json")
            } else {
                include_str!("../../ui/locales/en.json")
            })
            .expect("bundled ANR translations"),
        )
    }

    fn get(&self, key: &str) -> String {
        self.0.get(key).cloned().unwrap_or_else(|| key.into())
    }
}

fn text_ctrl(label: String, text: String) -> Control {
    Control::Text { label, text }
}

fn btn_ctrl(
    id: &str,
    label: String,
    enabled: bool,
    danger: bool,
    confirmation: Option<String>,
) -> Control {
    Control::Button {
        id: id.into(),
        label,
        enabled,
        danger,
        confirmation,
    }
}

pub fn view(_service: &Service, root: &Path, request: ui::Request) -> Result<Value, RpcError> {
    let t = Text::new(&request.context.language);
    let rt = runtime_info(root);
    let game = if request.context.native_game {
        None
    } else {
        request
            .context
            .directory
            .as_deref()
            .map(game_state)
    };
    let game_scope = request.context.directory.is_some();

    let mut categories = Vec::new();
    if game_scope {
        categories.push(anr_fg_category(
            &t,
            &rt,
            game.as_ref(),
            request.context.directory.as_deref(),
        ));
        categories.push(optiscaler_menu_category(&t, game.as_ref()));
        categories.push(record_category(&t, &rt, game.as_ref()));
        categories.push(management_category(&t, &rt));
        categories.push(usage_category(&t, game.as_ref()));
    } else {
        categories.push(management_category(&t, &rt));
        categories.push(record_category(&t, &rt, None));
    }

    let result = View {
        title: "Apex Neural Rendering (ANR)".into(),
        categories,
        notice: request
            .context
            .native_game
            .then(|| "ANR FG requer um jogo Windows executado via Proton.".into()),
    };
    ui::validate(&result).map_err(error)?;
    serde_json::to_value(result).map_err(|_| error("Cannot encode native view"))
}

fn game_install_controls(
    t: &Text,
    rt: &RuntimeInfo,
    game: &GameState,
    directory: &Path,
) -> Vec<Control> {
    let mut controls = Vec::new();
    let status_label = match game.mode {
        InstallMode::Full => format!(
            "{} · {}",
            t.get("installed_full"),
            game.proxy.as_deref().unwrap_or("dxgi.dll")
        ),
        InstallMode::Addon => t.get("installed_addon"),
        InstallMode::None => t.get("not_installed"),
    };
    controls.push(text_ctrl("ANR FG".into(), status_label));
    if game.mode == InstallMode::None {
        controls.push(Control::Select {
            id: "proxy".into(),
            label: t.get("proxy"),
            value: "dxgi.dll".into(),
            choices: ["dxgi.dll", "winmm.dll", "version.dll"]
                .into_iter()
                .map(|v| Choice {
                    label: v.into(),
                    value: v.into(),
                })
                .collect(),
            enabled: true,
        });
        controls.push(btn_ctrl(
            "install",
            t.get("install_full"),
            rt.anr_ready,
            false,
            None,
        ));
        if game.has_optiscaler_ini {
            controls.push(btn_ctrl(
                "install-addon",
                t.get("install_addon"),
                rt.anr_ready,
                false,
                None,
            ));
        }
        controls.push(text_ctrl(String::new(), t.get("configuration_locked")));
    } else {
        controls.push(btn_ctrl(
            "upgrade-game",
            t.get("upgrade_game"),
            rt.anr_ready,
            false,
            None,
        ));
        controls.push(btn_ctrl(
            "remove-game",
            t.get("remove_game"),
            true,
            true,
            Some(t.get("remove_game_confirm")),
        ));
    }
    controls.push(Control::Path {
        label: t.get("destination"),
        path: directory.into(),
    });
    controls.push(Control::Inventory {
        label: t.get("inventory_game"),
        hint: t.get("inventory_hint"),
        items: vec![
            StatusItem {
                label: "ANR FG (nvngx_dlssg.dll)".into(),
                value: if game.mode != InstallMode::None {
                    t.get("present")
                } else {
                    t.get("absent")
                },
                present: game.mode != InstallMode::None,
            },
            StatusItem {
                label: "OptiScaler.ini".into(),
                value: if game.has_optiscaler_ini {
                    t.get("present")
                } else {
                    t.get("absent")
                },
                present: game.has_optiscaler_ini,
            },
            StatusItem {
                label: "nr-record (Captura)".into(),
                value: if game.record_installed {
                    t.get("present")
                } else {
                    t.get("absent")
                },
                present: game.record_installed,
            },
        ],
    });
    controls
}

fn management_category(t: &Text, rt: &RuntimeInfo) -> Category {
    let mut controls = Vec::new();

    let mut anr_btns = vec![btn_ctrl(
        "download-anr",
        t.get(if rt.anr_ready {
            "update_anr"
        } else {
            "download_anr"
        }),
        true,
        false,
        None,
    )];
    if rt.anr_ready {
        anr_btns.push(btn_ctrl(
            "remove-anr-runtime",
            t.get("remove_anr_runtime"),
            true,
            true,
            Some(t.get("remove_anr_runtime_confirm")),
        ));
    }
    controls.push(Control::Row {
        label: format!(
            "{} · {}",
            t.get("runtime_anr_title"),
            rt.anr_version
                .clone()
                .unwrap_or_else(|| t.get("not_installed"))
        ),
        text: t.get("runtime_anr_hint"),
        controls: anr_btns,
    });

    let mut rec_btns = vec![btn_ctrl(
        "download-record",
        t.get(if rt.record_ready {
            "update_record"
        } else {
            "download_record"
        }),
        true,
        false,
        None,
    )];
    if rt.record_ready {
        rec_btns.push(btn_ctrl(
            "remove-record-runtime",
            t.get("remove_record_runtime"),
            true,
            true,
            Some(t.get("remove_record_runtime_confirm")),
        ));
    }
    controls.push(Control::Row {
        label: format!(
            "{} · {}",
            t.get("runtime_record_title"),
            rt.record_version
                .clone()
                .unwrap_or_else(|| t.get("not_installed"))
        ),
        text: t.get("runtime_record_hint"),
        controls: rec_btns,
    });

    controls.push(Control::Path {
        label: t.get("open_package"),
        path: rt.directory.clone(),
    });
    controls.push(Control::Inventory {
        label: t.get("inventory_runtime"),
        hint: t.get("inventory_hint"),
        items: vec![
            StatusItem {
                label: "ANR FG (Completo + Add-on)".into(),
                value: rt
                    .anr_version
                    .clone()
                    .unwrap_or_else(|| t.get("absent")),
                present: rt.anr_ready,
            },
            StatusItem {
                label: "nr-record (Captura + Visualizador)".into(),
                value: rt
                    .record_version
                    .clone()
                    .unwrap_or_else(|| t.get("absent")),
                present: rt.record_ready,
            },
        ],
    });
    controls.push(Control::Row {
        label: t.get("projects_title"),
        text: String::new(),
        controls: vec![
            Control::Link {
                label: "Apex Compute ANR (GitHub)".into(),
                url: "https://github.com/apex-compute/ANR".into(),
            },
            Control::Link {
                label: "Apex Neural Rendering".into(),
                url: "https://apexcompute.com/neural-rendering".into(),
            },
        ],
    });

    Category {
        id: "management".into(),
        label: t.get("management"),
        icon: "hub-download".into(),
        hint: t.get("runtime_hint"),
        enabled: true,
        sections: vec![Section {
            columns: false,
            title: String::new(),
            controls,
        }],
    }
}

fn anr_fg_category(
    t: &Text,
    rt: &RuntimeInfo,
    game: Option<&GameState>,
    directory: Option<&Path>,
) -> Category {
    let enabled = game.is_some_and(|g| g.mode != InstallMode::None);
    let val = |key: &str, default: &str| -> String {
        game.and_then(|g| g.nr_fg.get(key))
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let path_val = val("path", "auto");
    let max_gen = val("max_generated", "5");
    let scene_cut = val("scene_cut", "1");
    let nits = val("nits_per_unit", "80");

    let mut sections = Vec::new();
    if let (Some(directory), Some(game)) = (directory, game) {
        sections.push(Section {
            columns: false,
            title: String::new(),
            controls: game_install_controls(t, rt, game, directory),
        });
    }
    sections.extend([
        Section {
            columns: true,
            title: t.get("section_anr_core"),
            controls: vec![
                Control::Select {
                    id: "setting:anr:path".into(),
                    label: t.get("anr_path"),
                    value: path_val,
                    choices: vec![
                        Choice {
                            value: "auto".into(),
                            label: t.get("anr_path_auto"),
                        },
                        Choice {
                            value: "coopmat".into(),
                            label: t.get("anr_path_coopmat"),
                        },
                        Choice {
                            value: "packed".into(),
                            label: t.get("anr_path_packed"),
                        },
                        Choice {
                            value: "dp4a".into(),
                            label: t.get("anr_path_dp4a"),
                        },
                        Choice {
                            value: "portable".into(),
                            label: t.get("anr_path_portable"),
                        },
                        Choice {
                            value: "portable-linalg".into(),
                            label: t.get("anr_path_linalg"),
                        },
                        Choice {
                            value: "portable-xmx".into(),
                            label: t.get("anr_path_xmx"),
                        },
                    ],
                    enabled,
                },
                Control::Select {
                    id: "setting:anr:max_generated".into(),
                    label: t.get("anr_max_generated"),
                    value: max_gen,
                    choices: vec![
                        Choice {
                            value: "1".into(),
                            label: t.get("anr_gen_1"),
                        },
                        Choice {
                            value: "2".into(),
                            label: t.get("anr_gen_2"),
                        },
                        Choice {
                            value: "3".into(),
                            label: t.get("anr_gen_3"),
                        },
                        Choice {
                            value: "4".into(),
                            label: t.get("anr_gen_4"),
                        },
                        Choice {
                            value: "5".into(),
                            label: t.get("anr_gen_5"),
                        },
                    ],
                    enabled,
                },
            ],
        },
        Section {
            columns: true,
            title: t.get("section_anr_hdr"),
            controls: vec![
                Control::Switch {
                    appearance: ui::SwitchStyle::Round,
                    id: "setting:anr:scene_cut".into(),
                    label: t.get("anr_scene_cut"),
                    hint: t.get(if scene_cut == "1" { "on" } else { "off" }),
                    checked: scene_cut == "1",
                    enabled,
                    on: "1".into(),
                    off: "0".into(),
                },
                Control::Select {
                    id: "setting:anr:nits_per_unit".into(),
                    label: t.get("anr_nits"),
                    value: nits,
                    choices: ["80", "100", "150", "200", "300", "400"]
                        .into_iter()
                        .map(|n| Choice {
                            value: n.into(),
                            label: format!("{n} nits"),
                        })
                        .collect(),
                    enabled,
                },
            ],
        },
    ]);

    Category {
        id: "anr-fg".into(),
        label: t.get("anr_fg"),
        icon: "bolt".into(),
        hint: t.get("anr_fg_hint"),
        enabled: true,
        sections,
    }
}

fn optiscaler_menu_category(t: &Text, game: Option<&GameState>) -> Category {
    let enabled = game.is_some_and(|g| g.has_optiscaler_ini);
    let val = |key: &str, default: &str| -> String {
        game.and_then(|g| g.optiscaler.get(key))
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let shortcut = val("Menu.ShortcutKey", "0x2D");
    let scale = val("Menu.Scale", "1.0");
    let overlay = val("Menu.OverlayMenu", "true");
    let show_fps = val("Menu.ShowFps", "false");
    let fg_input = val("FrameGen.FGInput", "nvngxfg");
    let fg_provider = val("FrameGen.FGNvngxReplacement", "Apex Neural Rendering FG");
    let hudless = val("FrameGen.DisableHudless", "false");
    let dx12_up = val("Upscalers.Dx12Upscaler", "auto");

    let sections = vec![
        Section {
            columns: true,
            title: t.get("section_opti_menu"),
            controls: vec![
                Control::Shortcut {
                    id: "setting:opti:Menu.ShortcutKey".into(),
                    label: t.get("opti_shortcut"),
                    value: shortcut,
                    enabled,
                },
                Control::Select {
                    id: "setting:opti:Menu.Scale".into(),
                    label: t.get("opti_scale"),
                    value: scale,
                    choices: [
                        ("1.0", "100%"),
                        ("1.25", "125%"),
                        ("1.5", "150%"),
                        ("1.75", "175%"),
                        ("2.0", "200%"),
                    ]
                    .into_iter()
                    .map(|(v, l)| Choice {
                        value: v.into(),
                        label: l.into(),
                    })
                    .collect(),
                    enabled,
                },
                Control::Switch {
                    appearance: ui::SwitchStyle::Round,
                    id: "setting:opti:Menu.OverlayMenu".into(),
                    label: t.get("opti_overlay"),
                    hint: t.get(if overlay == "true" { "on" } else { "off" }),
                    checked: overlay == "true",
                    enabled,
                    on: "true".into(),
                    off: "false".into(),
                },
                Control::Switch {
                    appearance: ui::SwitchStyle::Round,
                    id: "setting:opti:Menu.ShowFps".into(),
                    label: t.get("opti_fps"),
                    hint: t.get(if show_fps == "true" { "on" } else { "off" }),
                    checked: show_fps == "true",
                    enabled,
                    on: "true".into(),
                    off: "false".into(),
                },
            ],
        },
        Section {
            columns: true,
            title: t.get("section_opti_fg"),
            controls: vec![
                Control::Select {
                    id: "setting:opti:FrameGen.FGInput".into(),
                    label: t.get("opti_fg_input"),
                    value: fg_input,
                    choices: vec![
                        Choice {
                            value: "nvngxfg".into(),
                            label: "DLSS-G via NGX (nvngxfg)".into(),
                        },
                        Choice {
                            value: "nofg".into(),
                            label: t.get("off"),
                        },
                    ],
                    enabled,
                },
                Control::Select {
                    id: "setting:opti:FrameGen.FGNvngxReplacement".into(),
                    label: t.get("opti_fg_provider"),
                    value: fg_provider,
                    choices: vec![
                        Choice {
                            value: "Apex Neural Rendering FG".into(),
                            label: "Apex Neural Rendering FG".into(),
                        },
                        Choice {
                            value: "nr-record".into(),
                            label: "nr-record (Gravador)".into(),
                        },
                        Choice {
                            value: "Nukems".into(),
                            label: "Nukems (FSR 3 FG)".into(),
                        },
                    ],
                    enabled,
                },
                Control::Switch {
                    appearance: ui::SwitchStyle::Round,
                    id: "setting:opti:FrameGen.DisableHudless".into(),
                    label: t.get("opti_hudless"),
                    hint: t.get(if hudless == "true" { "on" } else { "off" }),
                    checked: hudless == "true",
                    enabled,
                    on: "true".into(),
                    off: "false".into(),
                },
                Control::Select {
                    id: "setting:opti:Upscalers.Dx12Upscaler".into(),
                    label: t.get("opti_dx12_upscaler"),
                    value: dx12_up,
                    choices: vec![
                        Choice {
                            value: "auto".into(),
                            label: t.get("auto"),
                        },
                        Choice {
                            value: "dlss".into(),
                            label: "DLSS".into(),
                        },
                        Choice {
                            value: "fsr31".into(),
                            label: "FSR 3.1".into(),
                        },
                        Choice {
                            value: "xess".into(),
                            label: "XeSS".into(),
                        },
                        Choice {
                            value: "nr-record".into(),
                            label: "nr-record (Capturar Upscaler)".into(),
                        },
                    ],
                    enabled,
                },
            ],
        },
    ];

    Category {
        id: "optiscaler-menu".into(),
        label: t.get("optiscaler_menu"),
        icon: "settings".into(),
        hint: t.get("optiscaler_menu_hint"),
        enabled,
        sections,
    }
}

fn record_category(t: &Text, rt: &RuntimeInfo, game: Option<&GameState>) -> Category {
    let mut sections = Vec::new();
    let rec_installed = game.is_some_and(|g| g.record_installed);

    if let Some(game) = game {
        let mut status_controls = vec![text_ctrl(
            t.get("record_status"),
            t.get(if game.record_installed {
                "record_installed"
            } else {
                "record_not_installed"
            }),
        )];
        if !game.record_installed {
            status_controls.push(btn_ctrl(
                "install-record",
                t.get("install_record"),
                rt.record_ready && game.has_optiscaler_ini,
                false,
                None,
            ));
        } else {
            status_controls.push(btn_ctrl(
                "remove-record",
                t.get("remove_record"),
                true,
                true,
                Some(t.get("remove_record_confirm")),
            ));
        }
        sections.push(Section {
            columns: false,
            title: String::new(),
            controls: status_controls,
        });

        let val = |key: &str, default: &str| -> String {
            game.nr_record
                .get(key)
                .cloned()
                .unwrap_or_else(|| default.into())
        };
        let fkeys: Vec<Choice> = ["F6", "F7", "F8", "F9", "F10", "F11", "F12"]
            .into_iter()
            .map(|k| Choice {
                value: k.into(),
                label: k.into(),
            })
            .collect();

        sections.push(Section {
            columns: true,
            title: t.get("section_record_keys"),
            controls: vec![
                Control::Select {
                    id: "setting:rec:record_key".into(),
                    label: t.get("rec_record_key"),
                    value: val("record_key", "F6"),
                    choices: fkeys.clone(),
                    enabled: rec_installed,
                },
                Control::Select {
                    id: "setting:rec:capture_key".into(),
                    label: t.get("rec_capture_key"),
                    value: val("capture_key", "F9"),
                    choices: fkeys,
                    enabled: rec_installed,
                },
                Control::Select {
                    id: "setting:rec:capture_frames".into(),
                    label: t.get("rec_capture_frames"),
                    value: val("capture_frames", "8"),
                    choices: ["4", "8", "16", "32"]
                        .into_iter()
                        .map(|n| Choice {
                            value: n.into(),
                            label: format!("{n} frames"),
                        })
                        .collect(),
                    enabled: rec_installed,
                },
                Control::Select {
                    id: "setting:rec:record_mode".into(),
                    label: t.get("rec_mode"),
                    value: val("record_mode", "interleaved"),
                    choices: vec![
                        Choice {
                            value: "interleaved".into(),
                            label: t.get("rec_mode_interleaved"),
                        },
                        Choice {
                            value: "generated".into(),
                            label: t.get("rec_mode_generated"),
                        },
                    ],
                    enabled: rec_installed,
                },
                Control::Select {
                    id: "setting:rec:record_present".into(),
                    label: t.get("rec_present"),
                    value: val("record_present", "bypass"),
                    choices: vec![
                        Choice {
                            value: "bypass".into(),
                            label: t.get("rec_present_bypass"),
                        },
                        Choice {
                            value: "keep".into(),
                            label: t.get("rec_present_keep"),
                        },
                    ],
                    enabled: rec_installed,
                },
            ],
        });

        let forward_val = val(
            "forward",
            "..\\..\\Apex Neural Rendering FG\\dlssg\\nvngx_dlssg.dll",
        );
        sections.push(Section {
            columns: true,
            title: t.get("section_record_limits"),
            controls: vec![
                Control::Select {
                    id: "setting:rec:forward".into(),
                    label: t.get("rec_forward"),
                    value: forward_val,
                    choices: vec![
                        Choice {
                            value: "..\\..\\Apex Neural Rendering FG\\dlssg\\nvngx_dlssg.dll"
                                .into(),
                            label: t.get("rec_forward_anr"),
                        },
                        Choice {
                            value: "nvidia".into(),
                            label: t.get("rec_forward_nvidia"),
                        },
                        Choice {
                            value: String::new(),
                            label: t.get("rec_forward_none"),
                        },
                    ],
                    enabled: rec_installed,
                },
                Control::Select {
                    id: "setting:rec:record_max_seconds".into(),
                    label: t.get("rec_max_seconds"),
                    value: val("record_max_seconds", "300"),
                    choices: ["60", "180", "300", "600"]
                        .into_iter()
                        .map(|s| Choice {
                            value: s.into(),
                            label: format!("{s} s"),
                        })
                        .collect(),
                    enabled: rec_installed,
                },
                Control::Select {
                    id: "setting:rec:record_free_gb".into(),
                    label: t.get("rec_free_gb"),
                    value: val("record_free_gb", "50"),
                    choices: ["10", "25", "50", "100"]
                        .into_iter()
                        .map(|g| Choice {
                            value: g.into(),
                            label: format!("{g} GB"),
                        })
                        .collect(),
                    enabled: rec_installed,
                },
            ],
        });
    }

    sections.push(Section {
        columns: false,
        title: t.get("section_record_tools"),
        controls: vec![
            btn_ctrl(
                "open-viewer",
                t.get("open_viewer"),
                rt.record_ready,
                false,
                None,
            ),
            Control::Path {
                label: t.get("open_recordings"),
                path: rt.recordings_dir.clone(),
            },
        ],
    });

    Category {
        id: "record".into(),
        label: t.get("record"),
        icon: "connection-platform".into(),
        hint: t.get("record_hint"),
        enabled: true,
        sections,
    }
}

fn usage_category(t: &Text, game: Option<&GameState>) -> Category {
    Category {
        id: "usage".into(),
        label: t.get("usage"),
        icon: "hub-info".into(),
        hint: t.get("usage_hint"),
        enabled: true,
        sections: vec![Section {
            columns: false,
            title: String::new(),
            controls: vec![
                text_ctrl(t.get("usage_step_1"), t.get("usage_step_1_text")),
                text_ctrl(t.get("usage_step_2"), t.get("usage_step_2_text")),
                text_ctrl(t.get("usage_step_3"), t.get("usage_step_3_text")),
                Control::Host {
                    id: "pelirun".into(),
                    label: t.get("usage_prepare_command"),
                    enabled: game.is_some_and(|g| g.mode != InstallMode::None),
                },
            ],
        }],
    }
}

pub fn action(
    service: &mut Service,
    host: peligames_addon_kit::protocol::ServiceContext,
    request: ui::Request,
) -> Result<Value, RpcError> {
    let action = request
        .action
        .ok_or_else(|| error("Missing native action"))?;
    let root = host.component_directory;
    let value = action.value.unwrap_or_default();

    match action.id.as_str() {
        "download-anr" => {
            let id = service.jobs.start(&service.runtime, move |progress| async move {
                download_anr_runtime(root, progress)
                    .await
                    .map(|_| Value::Null)
            })?;
            return Ok(json!({"job": id}));
        }
        "remove-anr-runtime" => {
            service.jobs.idle()?;
            let _ = fs::remove_dir_all(root.join("anr-fg"));
            return Ok(Value::Null);
        }
        "download-record" => {
            let id = service.jobs.start(&service.runtime, move |progress| async move {
                download_record_runtime(root, progress)
                    .await
                    .map(|_| Value::Null)
            })?;
            return Ok(json!({"job": id}));
        }
        "remove-record-runtime" => {
            service.jobs.idle()?;
            let _ = fs::remove_dir_all(root.join("nr-record"));
            return Ok(Value::Null);
        }
        "open-viewer" => {
            launch_viewer(&root).map_err(error)?;
            return Ok(Value::Null);
        }
        _ => {}
    }

    if request.context.native_game {
        return Err(error("ANR FG requer um jogo Windows executado via Proton."));
    }
    let directory = request
        .context
        .directory
        .ok_or_else(|| error("Selecione a pasta do executável do jogo."))?;
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(error("Pasta do jogo inválida."));
    }

    match action.id.as_str() {
        "install" => {
            let line = install_full(&root, &directory, &value).map_err(error)?;
            Ok(json!({"launch": {"line": line}}))
        }
        "install-addon" => {
            install_addon(&root, &directory).map_err(error)?;
            Ok(Value::Null)
        }
        "upgrade-game" => {
            let state = game_state(&directory);
            if state.mode == InstallMode::Full {
                let proxy = state.proxy.as_deref().unwrap_or("dxgi.dll");
                let line = install_full(&root, &directory, proxy).map_err(error)?;
                Ok(json!({"launch": {"line": line}}))
            } else {
                install_addon(&root, &directory).map_err(error)?;
                Ok(Value::Null)
            }
        }
        "remove-game" => {
            let line = remove_game(&directory).map_err(error)?;
            Ok(json!({"launch": {"line": line}}))
        }
        "install-record" => {
            install_record(&root, &directory).map_err(error)?;
            Ok(Value::Null)
        }
        "remove-record" => {
            remove_record(&directory).map_err(error)?;
            Ok(Value::Null)
        }
        id if id.starts_with("setting:") => {
            update_setting(&directory, &id[8..], &value).map_err(error)?;
            Ok(Value::Null)
        }
        _ => Err(error("Unknown native action")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn full_install_settings_and_record_roundtrip_cleanly() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "peligames-anr-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = base.join("component");
        let game = base.join("game");
        fs::create_dir_all(root.join("anr-fg/full/game/OptiScaler/custom/Apex Neural Rendering FG/dlssg")).unwrap();
        fs::create_dir_all(root.join("anr-fg/addon/Apex Neural Rendering FG/dlssg")).unwrap();
        fs::create_dir_all(root.join("nr-record/nr-record-1.2-linux/capture/OptiScaler/custom/nr-record/dlssg")).unwrap();
        fs::create_dir_all(&game).unwrap();
        fs::write(root.join("anr-fg/full/game/dxgi.dll"), b"opti-dll").unwrap();
        fs::write(
            root.join("anr-fg/full/game/OptiScaler.ini"),
            "[Menu]\nShortcutKey=0x2D\nScale=1.0\n[FrameGen]\nFGInput=nvngxfg\nFGNvngxReplacement=Apex Neural Rendering FG\nDisableHudless=false\n",
        )
        .unwrap();
        let nr_ini = "path = auto\nmax_generated = 5\nscene_cut = 1\nnits_per_unit = 80\n";
        fs::write(
            root.join("anr-fg/full/game/OptiScaler/custom/Apex Neural Rendering FG/dlssg/nr_fg.ini"),
            nr_ini,
        )
        .unwrap();
        fs::write(
            root.join("anr-fg/full/game/OptiScaler/custom/Apex Neural Rendering FG/dlssg/nvngx_dlssg.dll"),
            b"anr-dll",
        )
        .unwrap();
        fs::write(
            root.join("anr-fg/addon/Apex Neural Rendering FG/dlssg/nr_fg.ini"),
            nr_ini,
        )
        .unwrap();
        fs::write(
            root.join("anr-fg/addon/Apex Neural Rendering FG/dlssg/nvngx_dlssg.dll"),
            b"anr-dll",
        )
        .unwrap();
        fs::write(
            root.join("nr-record/nr-record-1.2-linux/capture/OptiScaler/custom/nr-record/dlssg/nr_record.ini"),
            "record_key = F6\ncapture_key = F9\ncapture_frames = 8\nforward =\n",
        )
        .unwrap();

        let mut s = Service::new().unwrap();
        let call = |s: &mut Service, method: &str, id: Option<&str>, val: Option<&str>| {
            s.handle(
                method,
                json!({
                    "host": {"component_directory": root},
                    "request": {
                        "context": {"language": "pt-BR", "directory": game, "native_game": false},
                        "action": id.map(|i| json!({"id": i, "value": val}))
                    }
                }),
            )
            .unwrap()
        };

        let v0: View = serde_json::from_value(call(&mut s, "native.view", None, None)).unwrap();
        ui::validate(&v0).unwrap();
        assert!(!v0.categories[1].enabled);

        let res = call(&mut s, "native.action", Some("install"), Some("dxgi.dll"));
        assert_eq!(res["launch"]["line"], "WINEDLLOVERRIDES=\"dxgi=n,b\" %command%");

        let v1: View = serde_json::from_value(call(&mut s, "native.view", None, None)).unwrap();
        ui::validate(&v1).unwrap();
        assert!(v1.categories[1].enabled);
        assert!(v1.categories[2].enabled);

        call(&mut s, "native.action", Some("setting:anr:max_generated"), Some("3"));
        call(&mut s, "native.action", Some("setting:opti:Menu.ShortcutKey"), Some("0x70"));
        let gs = game_state(&game);
        assert_eq!(gs.nr_fg["max_generated"], "3");
        assert_eq!(gs.optiscaler["Menu.ShortcutKey"], "0x70");

        call(&mut s, "native.action", Some("install-record"), None);
        let gs = game_state(&game);
        assert!(gs.record_installed);
        assert_eq!(gs.optiscaler["FrameGen.FGNvngxReplacement"], "nr-record");

        call(&mut s, "native.action", Some("setting:rec:record_key"), Some("F8"));
        assert_eq!(game_state(&game).nr_record["record_key"], "F8");

        call(&mut s, "native.action", Some("remove-record"), None);
        assert!(!game_state(&game).record_installed);
        assert_eq!(
            game_state(&game).optiscaler["FrameGen.FGNvngxReplacement"],
            "Apex Neural Rendering FG"
        );

        let rem = call(&mut s, "native.action", Some("remove-game"), None);
        assert!(rem["launch"]["line"].is_null());
        assert_eq!(game_state(&game).mode, InstallMode::None);

        let _ = fs::remove_dir_all(&base);
    }
}
