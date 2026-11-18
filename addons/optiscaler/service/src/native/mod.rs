//! Native presentation of the original OptiScaler categories. Component logic stays in core.
mod fields;
mod management;
#[cfg(test)]
mod tests;
use super::*;
use peligames_addon_kit::native_ui::{self as ui, Category, Control, Section, View};
use std::{collections::BTreeMap, path::Path};
pub(super) struct Text(BTreeMap<String, String>);
impl Text {
    fn new(language: &str) -> Self {
        Self(
            serde_json::from_str(if language.starts_with("pt") {
                include_str!("../../../ui/locales/pt-BR.json")
            } else {
                include_str!("../../../ui/locales/en.json")
            })
            .expect("bundled OptiScaler translations"),
        )
    }
    fn get(&self, key: &str) -> String {
        self.0.get(key).cloned().unwrap_or_else(|| key.into())
    }
}
pub(super) fn text(label: String, text: String) -> Control {
    Control::Text { label, text }
}
pub(super) fn button(
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
pub fn view(service: &Service, root: &Path, request: ui::Request) -> Result<Value, RpcError> {
    let t = Text::new(&request.context.language);
    let runtime = service
        .runtime
        .block_on(runtime::state(root))
        .map_err(error)?;
    let game = if request.context.native_game {
        None
    } else {
        request
            .context
            .directory
            .as_ref()
            .map(|directory| {
                optiscaler::state(directory, &runtime::component(root).map_err(error)?)
                    .map_err(error)
            })
            .transpose()?
    };
    let game_scope = request.context.directory.is_some();
    let mut categories = Vec::new();
    if game_scope {
        categories.extend(fields::categories(
            &t,
            game.as_ref(),
            request.context.directory.as_deref(),
        ));
        categories.push(management::category(&t, &runtime, true));
        categories.push(Category {
            id: "usage".into(),
            label: t.get("usage"),
            icon: "hub-info".into(),
            hint: t.get("usage_hint"),
            enabled: true,
            sections: vec![Section {
                columns: false,
                title: String::new(),
                controls: vec![
                    text(t.get("usage_install"), t.get("usage_install_hint")),
                    text(t.get("usage_configure"), t.get("usage_configure_hint")),
                    text(t.get("usage_launch"), t.get("usage_launch_hint")),
                    text(
                        t.get("usage_other_launcher"),
                        t.get("usage_other_launcher_hint"),
                    ),
                    Control::Host {
                        id: "pelirun".into(),
                        label: t.get("usage_prepare_command"),
                        enabled: game.as_ref().is_some_and(|g| g.installed),
                    },
                ],
            }],
        });
    } else {
        categories.push(management::category(&t, &runtime, false));
    }
    let result = View {
        title: "OptiScaler".into(),
        categories,
        notice: request
            .context
            .native_game
            .then(|| t.get("native_unavailable")),
    };
    ui::validate(&result).map_err(error)?;
    serde_json::to_value(result).map_err(|_| error("Cannot encode native view"))
}
pub fn action(
    service: &mut Service,
    host: peligames_addon_kit::protocol::ServiceContext,
    request: ui::Request,
) -> Result<Value, RpcError> {
    let action = request
        .action
        .ok_or_else(|| error("Missing native action"))?;
    if request.context.native_game
        && !["download", "channel", "remove-package"].contains(&action.id.as_str())
    {
        return Err(error("OptiScaler requires a Windows game"));
    }
    let value = action.value.unwrap_or_default();
    let channel = channel_value(&host.component_directory)?;
    let directory = request.context.directory.clone();
    let game = |operation: Value| -> Result<Value, RpcError> {
        Ok(
            json!({"directory":directory.clone().ok_or_else(||error("Missing game directory"))?,"action":operation}),
        )
    };
    let (method, params) = match action.id.as_str() {
        "download" => ("runtime.download", json!({"channel":channel})),
        "channel" => (
            "runtime.channel",
            json!({"channel":if value=="true" {"nightly"} else {"stable"}}),
        ),
        "remove-package" => ("runtime.remove", json!({"channel":channel})),
        "install" => (
            "game.mutate",
            game(json!({"operation":"install","proxy":value,"settings":{}}))?,
        ),
        "upgrade" => ("game.mutate", game(json!({"operation":"upgrade"}))?),
        "remove-game" => ("game.mutate", game(json!({"operation":"remove"}))?),
        "patcher" => (
            "game.mutate",
            game(json!({"operation":"patcher","enabled":value=="true"}))?,
        ),
        id if id.starts_with("setting:") => (
            "game.mutate",
            game(json!({"operation":"update","key":&id[8..],"value":value}))?,
        ),
        _ => return Err(error("Unknown native action")),
    };
    let result = service.handle(method, json!({"host":host,"request":params}))?;
    if ["install", "upgrade", "remove-game"].contains(&action.id.as_str()) {
        Ok(json!({"launch":{"line":result.as_str()}}))
    } else {
        Ok(result)
    }
}
fn channel_value(root: &Path) -> Result<Channel, RpcError> {
    optiscaler::channel::read(root).map_err(error)
}
