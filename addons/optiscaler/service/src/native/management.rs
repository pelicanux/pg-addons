use super::*;
pub(super) fn game_controls(
    t: &Text,
    game: Option<&optiscaler::State>,
    directory: Option<&Path>,
) -> Vec<Control> {
    let mut controls = Vec::new();
    if let Some(directory) = directory {
        if let Some(game) = game {
            controls.push(text(
                "OptiScaler".into(),
                format!(
                    "{}{}",
                    t.get(if game.installed {
                        "installed"
                    } else {
                        "not_installed"
                    }),
                    game.proxy
                        .as_ref()
                        .map(|p| format!(" · {p}"))
                        .unwrap_or_default()
                ),
            ));
            if let Some(route) = &game.nr_route {
                controls.push(text(
                    String::new(),
                    t.get(match route {
                        optiscaler::NrRoute::Optiscaler => "nr_optiscaler",
                        optiscaler::NrRoute::Reshade => "nr_reshade",
                        _ => "nr_unknown",
                    }),
                ));
                controls.push(Control::Host {
                    id: "manage-nr".into(),
                    label: t.get("manage_nr"),
                    enabled: true,
                });
            } else if !game.installed && !game.managed {
                let values: Vec<String> =
                    serde_json::from_str(include_str!("../../../core/proxies.json"))
                        .expect("proxy catalogue");
                controls.push(Control::Select {
                    id: "proxy".into(),
                    label: t.get("proxy"),
                    value: "dxgi.dll".into(),
                    choices: values
                        .into_iter()
                        .map(|value| ui::Choice {
                            label: value.clone(),
                            value,
                        })
                        .collect(),
                    enabled: true,
                });
                controls.push(button(
                    "install",
                    t.get("install"),
                    game.component_ready,
                    false,
                    None,
                ));
            } else if game.managed {
                controls.push(button(
                    "upgrade",
                    t.get("game_update"),
                    game.component_ready,
                    false,
                    None,
                ));
                controls.push(button(
                    "remove-game",
                    t.get("remove"),
                    true,
                    true,
                    Some(t.get("remove_confirm")),
                ));
            } else {
                controls.push(text(String::new(), t.get("external_installation")));
            }
            if !game.installed {
                controls.push(text(String::new(), t.get("configuration_locked")));
            }
            controls.push(Control::Path {
                label: t.get("destination"),
                path: directory.into(),
            });
            if game.installed {
                inventory(&mut controls, &game.software, t, "software_game");
            }
        }
    }
    controls
}

pub(super) fn category(t: &Text, runtime: &runtime::RuntimeState, game_scope: bool) -> Category {
    let mut controls = Vec::new();
    controls.push(text(
        "OptiScaler".into(),
        format!(
            "{} · {}",
            t.get(if runtime.channel == Channel::Nightly {
                "nightly_channel"
            } else {
                "stable_channel"
            }),
            runtime
                .version
                .clone()
                .unwrap_or_else(|| t.get("not_installed"))
        ),
    ));
    controls.push(Control::Row {
        label: t.get("runtime_files"),
        text: t.get("channel_games_hint"),
        controls: vec![
            button(
                "download",
                t.get(if runtime.ready {
                    "runtime_update"
                } else {
                    "download"
                }),
                true,
                false,
                None,
            ),
            Control::Switch {
                appearance: ui::SwitchStyle::Rail,
                id: "channel".into(),
                label: t.get("nightly_label"),
                hint: String::new(),
                checked: runtime.channel == Channel::Nightly,
                enabled: true,
                on: "true".into(),
                off: "false".into(),
            },
        ],
    });
    if let Some(path) = &runtime.directory {
        controls.push(Control::Row {
            label: t.get("package_folder"),
            text: t.get("package_remove_hint"),
            controls: vec![
                Control::Path {
                    label: t.get("open_package"),
                    path: path.into(),
                },
                button(
                    "remove-package",
                    t.get("remove_download"),
                    true,
                    true,
                    Some(t.get("package_remove_confirm").replace(
                        "{{channel}}",
                        &t.get(if runtime.channel == Channel::Nightly {
                            "nightly_channel"
                        } else {
                            "stable_channel"
                        }),
                    )),
                ),
            ],
        });
    }
    inventory(&mut controls, &runtime.software, t, "software_package");
    #[derive(Deserialize)]
    struct Project {
        name: String,
        url: String,
    }
    let projects: Vec<Project> = serde_json::from_str(include_str!("../../../ui/projects.json"))
        .expect("bundled project links");
    controls.push(Control::Row {
        label: t.get("projects_title"),
        text: String::new(),
        controls: projects
            .into_iter()
            .map(|p| Control::Link {
                label: p.name,
                url: p.url,
            })
            .collect(),
    });

    Category {
        id: "management".into(),
        label: if game_scope {
            t.get("management")
        } else {
            "OptiScaler".into()
        },
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
fn inventory(
    controls: &mut Vec<Control>,
    items: &[optiscaler::software::Component],
    t: &Text,
    scope: &str,
) {
    let values = items
        .iter()
        .map(|item| ui::StatusItem {
            label: match item.id {
                "optiscaler" => "OptiScaler",
                "dlss" => "DLSS",
                "fsr" => "FSR",
                "xess" => "XeSS",
                "fakenvapi" => "FakeNVAPI",
                "streamline" => "Streamline SDK",
                "optipatcher" => "OptiPatcher",
                other => other,
            }
            .into(),
            value: if item.integrated {
                t.get("software_integrated")
            } else if !item.present {
                t.get("software_absent")
            } else {
                item.version
                    .clone()
                    .unwrap_or_else(|| t.get("software_present"))
            },
            present: item.present || item.integrated,
        })
        .collect();
    controls.push(Control::Inventory {
        label: t.get(scope),
        hint: t.get("software_hint"),
        items: values,
    });
}
