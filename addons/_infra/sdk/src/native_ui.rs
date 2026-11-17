//! Versioned native view contract. Integrations own their categories, controls and operations.
//! The GPUI host renders these data models; packages contain no web frontend for this entrypoint.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    pub schema_version: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub language: String,
    pub directory: Option<std::path::PathBuf>,
    pub native_game: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub context: Context,
    pub action: Option<Action>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub value: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub title: String,
    pub categories: Vec<Category>,
    pub notice: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub id: String,
    pub label: String,
    pub icon: String,
    pub hint: String,
    pub enabled: bool,
    pub sections: Vec<Section>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub title: String,
    pub controls: Vec<Control>,
    #[serde(default)]
    pub columns: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub value: String,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusItem {
    pub label: String,
    pub value: String,
    pub present: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SwitchStyle {
    #[default]
    Round,
    Rail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Control {
    Inventory {
        label: String,
        hint: String,
        items: Vec<StatusItem>,
    },
    Row {
        label: String,
        text: String,
        controls: Vec<Control>,
    },
    Link {
        label: String,
        url: String,
    },
    Text {
        label: String,
        text: String,
    },
    Path {
        label: String,
        path: std::path::PathBuf,
    },
    Select {
        id: String,
        label: String,
        value: String,
        choices: Vec<Choice>,
        enabled: bool,
    },
    Switch {
        #[serde(default)]
        appearance: SwitchStyle,
        id: String,
        label: String,
        hint: String,
        enabled: bool,
        checked: bool,
        on: String,
        off: String,
    },
    Shortcut {
        id: String,
        label: String,
        value: String,
        enabled: bool,
    },
    Button {
        id: String,
        label: String,
        enabled: bool,
        danger: bool,
        confirmation: Option<String>,
    },
    Host {
        id: String,
        label: String,
        enabled: bool,
    },
}
#[cfg(feature = "packages")]
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub route: String,
    pub target: crate::manifest::Target,
    pub selection: crate::registry::SelectionSnapshot,
}
pub fn validate(view: &View) -> crate::Result<()> {
    use std::collections::BTreeSet;
    fn text(s: &str) -> crate::Result<()> {
        if s.len() > 8192 || s.contains('\0') {
            Err("Invalid native text".into())
        } else {
            Ok(())
        }
    }
    fn controls(
        items: &[Control],
        depth: usize,
        count: &mut usize,
        ids: &mut BTreeSet<String>,
    ) -> crate::Result<()> {
        if depth > 3 {
            return Err("Native controls are nested too deeply".into());
        }
        for c in items {
            *count += 1;
            if *count > 256 {
                return Err("Too many native controls".into());
            }
            let (id, label) = match c {
                Control::Inventory { label, hint, items } => {
                    text(hint)?;
                    if items.len() > 64 {
                        return Err("Too many status items".into());
                    }
                    for item in items {
                        text(&item.label)?;
                        text(&item.value)?;
                    }
                    (None, label)
                }
                Control::Row {
                    label,
                    text: hint,
                    controls: children,
                } => {
                    text(hint)?;
                    controls(children, depth + 1, count, ids)?;
                    (None, label)
                }
                Control::Link { label, url } => {
                    if !url.starts_with("https://")
                        || url.len() > 2048
                        || url.chars().any(char::is_control)
                    {
                        return Err("Invalid project link".into());
                    }
                    (None, label)
                }
                Control::Text { label, text: hint } => {
                    text(hint)?;
                    (None, label)
                }
                Control::Path { label, path } => {
                    if !path.is_absolute() || path.to_string_lossy().len() > 4096 {
                        return Err("Invalid native path".into());
                    }
                    (None, label)
                }
                Control::Select {
                    id,
                    label,
                    value,
                    choices,
                    ..
                } => {
                    text(value)?;
                    if choices.len() > 128 {
                        return Err("Too many native choices".into());
                    }
                    let mut keys = BTreeSet::new();
                    for choice in choices {
                        text(&choice.label)?;
                        text(&choice.value)?;
                        if !keys.insert(&choice.value) {
                            return Err("Duplicate native choice".into());
                        }
                    }
                    (Some(id), label)
                }
                Control::Switch {
                    id,
                    label,
                    hint,
                    on,
                    off,
                    ..
                } => {
                    text(hint)?;
                    text(on)?;
                    text(off)?;
                    (Some(id), label)
                }
                Control::Shortcut {
                    id, label, value, ..
                } => {
                    text(value)?;
                    (Some(id), label)
                }
                Control::Button {
                    id,
                    label,
                    confirmation,
                    ..
                } => {
                    if let Some(v) = confirmation {
                        text(v)?;
                    }
                    (Some(id), label)
                }
                Control::Host { id, label, .. } => (Some(id), label),
            };
            text(label)?;
            if let Some(id) = id {
                if id.is_empty()
                    || id.len() > 160
                    || !id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
                    || !ids.insert(id.clone())
                {
                    return Err("Invalid or duplicate native control".into());
                }
            }
        }
        Ok(())
    }
    if view.categories.len() > 16 || view.title.len() > 120 {
        return Err("Oversized native add-on view".into());
    }
    let (mut count, mut cats) = (0, BTreeSet::new());
    if let Some(notice) = &view.notice {
        text(notice)?;
    }
    for category in &view.categories {
        crate::manifest::identifier(&category.id)?;
        text(&category.label)?;
        text(&category.hint)?;
        text(&category.icon)?;
        if !cats.insert(&category.id) || category.sections.len() > 32 {
            return Err("Invalid native categories".into());
        }
        let mut ids = BTreeSet::new();
        for section in &category.sections {
            text(&section.title)?;
            controls(&section.controls, 0, &mut count, &mut ids)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> View {
        View {
            title: "Example".into(),
            notice: None,
            categories: vec![Category {
                id: "main".into(),
                label: "Settings".into(),
                icon: "settings".into(),
                hint: String::new(),
                enabled: true,
                sections: vec![Section {
                    title: String::new(),
                    columns: true,
                    controls: vec![Control::Select {
                        id: "setting:Menu.Scale".into(),
                        label: "Scale".into(),
                        value: "1".into(),
                        enabled: true,
                        choices: vec![Choice {
                            value: "1".into(),
                            label: "100%".into(),
                        }],
                    }],
                }],
            }],
        }
    }
    #[test]
    fn native_contract_bounds_labels_ids_choices_links_and_nesting() {
        let mut v = sample();
        validate(&v).unwrap();
        let c = v.categories[0].sections[0].controls[0].clone();
        v.categories[0].sections[0].controls.push(c);
        assert!(validate(&v).is_err());
        let mut v = sample();
        v.categories[0].hint = "x".repeat(8193);
        assert!(validate(&v).is_err());
        let mut v = sample();
        v.categories[0].sections[0].controls = vec![Control::Link {
            label: "Project".into(),
            url: "file:///private".into(),
        }];
        assert!(validate(&v).is_err());
        let mut c = Control::Text {
            label: String::new(),
            text: String::new(),
        };
        for _ in 0..5 {
            c = Control::Row {
                label: String::new(),
                text: String::new(),
                controls: vec![c],
            };
        }
        v.categories[0].sections[0].controls = vec![c];
        assert!(validate(&v).is_err());
    }
}
