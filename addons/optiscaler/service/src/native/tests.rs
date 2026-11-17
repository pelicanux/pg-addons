use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
struct Fixture {
    root: PathBuf,
    game: PathBuf,
    component: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "peligames-native-optiscaler-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let game = root.join("game with spaces");
        let component = root.join("component");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(component.join("official/OptiScaler")).unwrap();
        let mut pe = vec![0; 256];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&128u32.to_le_bytes());
        pe[128..132].copy_from_slice(b"PE\0\0");
        pe[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
        fs::write(game.join("Fixture.exe"), &pe).unwrap();
        fs::write(component.join("official/OptiScaler.dll"), pe).unwrap();
        fs::write(
            component.join("official/OptiScaler_LICENSE.txt"),
            "fixture license",
        )
        .unwrap();
        fs::write(component.join("official/OptiScaler.ini"),"; preserve\n[Menu]\nScale=1.25\nShortcutKey=0x2D\n[Upscalers]\nDx12Upscaler=auto\n[FrameGen]\nEnabled=false\nFGInput=nofg\nFGOutput=nofg\nFGNvngxReplacement=Nukems\n[Plugins]\nLoadAsiPlugins=true\n[Custom]\nKeep=hello\n").unwrap();
        Self {
            root,
            game,
            component,
        }
    }
    fn call(
        &self,
        s: &mut Service,
        method: &str,
        language: &str,
        id: Option<&str>,
        value: Option<&str>,
    ) -> Result<Value, RpcError> {
        s.handle(method,json!({"host":{"component_directory":self.component},"request":{"context":{"language":language,"directory":self.game,"native_game":false},"action":id.map(|id|json!({"id":id,"value":value}))}}))
    }
    fn view(&self, s: &mut Service, language: &str) -> View {
        serde_json::from_value(self.call(s, "native.view", language, None, None).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn native_flow_gates_configuration_reads_ini_and_roundtrips_install_edit_remove() {
    let f = Fixture::new();
    let mut s = Service::new().unwrap();
    let view = f.view(&mut s, "pt-BR");
    ui::validate(&view).unwrap();
    assert_eq!(
        view.categories
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>(),
        [
            "management",
            "main",
            "upscaling",
            "framegeneration",
            "latency",
            "usage"
        ]
    );
    assert!(!view.categories[1].enabled);
    assert!(!f.game.join("OptiScaler.ini").exists());
    let installed = f
        .call(
            &mut s,
            "native.action",
            "pt-BR",
            Some("install"),
            Some("winmm.dll"),
        )
        .unwrap();
    assert!(installed["launch"]["line"]
        .as_str()
        .unwrap()
        .contains("winmm=n,b"));
    let view = f.view(&mut s, "pt-BR");
    assert!(view.categories[1].enabled);
    assert_eq!(view.categories[1].label, "Menu e compatibilidade");
    ui::validate(&view).unwrap();
    let scale = view.categories[1]
        .sections
        .iter()
        .flat_map(|section| &section.controls)
        .find(|c| matches!(c,Control::Select{id,..}if id=="setting:Menu.Scale"))
        .unwrap();
    assert!(matches!(scale,Control::Select{value,..}if value=="1.25"));
    f.call(
        &mut s,
        "native.action",
        "pt-BR",
        Some("setting:Menu.Scale"),
        Some("1.5"),
    )
    .unwrap();
    assert_eq!(
        optiscaler::state(&f.game, &f.component.join("official"))
            .unwrap()
            .settings["Menu.Scale"],
        "1.5"
    );
    assert!(fs::read_to_string(f.game.join("OptiScaler.ini"))
        .unwrap()
        .contains("Keep=hello"));
    let before = fs::read(f.game.join("OptiScaler.ini")).unwrap();
    assert!(f
        .call(
            &mut s,
            "native.action",
            "pt-BR",
            Some("setting:Menu.Scale"),
            Some("1\nInjected=1")
        )
        .is_err());
    assert_eq!(fs::read(f.game.join("OptiScaler.ini")).unwrap(), before);
    let en = f.view(&mut s, "en");
    assert_eq!(en.categories[0].label, "Manage");
    fs::write(f.game.join("save.dat"), "keep save").unwrap();
    let removed = f
        .call(&mut s, "native.action", "en", Some("remove-game"), None)
        .unwrap();
    assert!(removed["launch"]["line"].is_null());
    assert!(!f.game.join("winmm.dll").exists());
    assert_eq!(
        fs::read_to_string(f.game.join("save.dat")).unwrap(),
        "keep save"
    );
}
#[test]
fn native_game_cannot_mutate_windows_files() {
    let f = Fixture::new();
    let mut s = Service::new().unwrap();
    let result=s.handle("native.action",json!({"host":{"component_directory":f.component},"request":{"context":{"language":"en","directory":f.game,"native_game":true},"action":{"id":"install","value":"dxgi.dll"}}}));
    assert!(result.is_err());
    assert!(!f.game.join("dxgi.dll").exists());
}
