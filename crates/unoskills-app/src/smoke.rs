use std::{
    fs,
    path::Path,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

use nsh_core::{
    ApproxTextEngine, EventKind, EventPayload, MountBatch, MountItem, NodeId,
    geometry::{Size, px},
};
use nsh_engine::{Engine, EngineConfig, MountSink};
use tempfile::tempdir;
use unoskills_core::{AgentCategory, CustomAgentRecord, UnoSkillsManager};

enum Observation {
    Batch(MountBatch),
    Error(String),
    Frame,
}

struct Sink(Sender<Observation>);

impl MountSink for Sink {
    fn submit(&self, batch: MountBatch) {
        let _ = self.0.send(Observation::Batch(batch));
    }

    fn request_frame(&self) {
        let _ = self.0.send(Observation::Frame);
    }

    fn report_error(&self, message: &str) {
        let _ = self.0.send(Observation::Error(message.to_owned()));
    }
}

fn wait_for(
    engine: &Engine,
    rx: &Receiver<Observation>,
    items: &mut Vec<MountItem>,
    label: &str,
    predicate: impl Fn(&MountItem, &str) -> Option<NodeId>,
) -> NodeId {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Some(found) = items.iter().rev().find_map(|item| predicate(item, label)) {
            return found;
        }
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Observation::Batch(batch)) => items.extend(batch.items),
            Ok(Observation::Frame) => engine.control().frame(),
            Ok(Observation::Error(error)) => panic!("NSH reported {error}"),
            Err(error) => {
                let observed = items
                    .iter()
                    .filter_map(|item| match item {
                        MountItem::Text { text, .. } => Some(text.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                panic!("waiting for {label:?}: {error}. Observed: {observed:?}");
            }
        }
    }
}

fn visible_text(item: &MountItem, label: &str) -> Option<NodeId> {
    match item {
        MountItem::Text { id, text, .. } if text.text.as_str() == label => Some(*id),
        _ => None,
    }
}

fn button(item: &MountItem, label: &str) -> Option<NodeId> {
    match item {
        MountItem::ViewProps { id, props }
            if props.accessibility.label.as_deref() == Some(label) =>
        {
            Some(*id)
        }
        MountItem::AccessibilityLabel { id, label: name } if name.as_deref() == Some(label) => {
            Some(*id)
        }
        _ => None,
    }
}

#[test]
fn embedded_shell_reads_the_real_workspace_and_navigates_to_settings() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("library");
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    manager.set_setting("language", "en").unwrap();
    manager.set_http_proxy(Some("http://127.0.0.1:1")).unwrap();
    let (engine, rx) = start(&root);
    let mut items = Vec::new();
    let settings = wait_for(&engine, &rx, &mut items, "Settings", button);
    wait_for(&engine, &rx, &mut items, "Local workspace", visible_text);
    press(&engine, settings);
    wait_for(
        &engine,
        &rx,
        &mut items,
        "Central Library Root",
        visible_text,
    );
}

fn start(root: &Path) -> (Engine, Receiver<Observation>) {
    let root = root.to_path_buf();
    let host = crate::nsh::channel_for(move || UnoSkillsManager::open_at(root.clone())).unwrap();
    let viewport = Size::new(px(1240.0), px(780.0));
    let config = EngineConfig {
        host,
        viewport,
        environment: nsh_engine::Environment {
            window: viewport,
            ..Default::default()
        },
        ui_sets: crate::nsh::ui_sets(),
        ..EngineConfig::new(crate::bundle().unwrap())
    };
    let (tx, rx) = mpsc::channel();
    (
        Engine::start(config, || ApproxTextEngine, Sink(tx)).unwrap(),
        rx,
    )
}

fn press(engine: &Engine, node: NodeId) {
    for event in [EventKind::PressDown, EventKind::PressUp, EventKind::Press] {
        engine
            .control()
            .dispatch_event(node, event, EventPayload::None);
    }
}

#[test]
fn nested_project_route_reaches_stack_preview_and_returns_to_library() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("library");
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    manager.set_setting("language", "en").unwrap();
    manager.set_http_proxy(Some("http://127.0.0.1:1")).unwrap();
    let agent_dir = temp.path().join("agent-skills");
    fs::create_dir_all(&agent_dir).unwrap();
    manager
        .add_custom_agent(&CustomAgentRecord {
            key: "custom_smoke".into(),
            display_name: "Smoke agent".into(),
            skills_dir: agent_dir.to_string_lossy().into_owned(),
            category: AgentCategory::Coding,
        })
        .unwrap();
    let project_dir = temp.path().join("project");
    let scope = Path::new("apps").join("desktop");
    fs::create_dir_all(project_dir.join(&scope)).unwrap();
    fs::write(
        project_dir.join(&scope).join("Cargo.toml"),
        "[package]\nname = \"smoke-app\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    let project = manager
        .add_project(&project_dir, Some("Routing smoke project"))
        .unwrap();
    assert!(
        manager
            .project_module_paths(&project.id)
            .unwrap()
            .contains(&scope)
    );

    let (engine, rx) = start(&root);
    let mut items = Vec::new();
    let open = |items: &mut Vec<MountItem>, label: &str| {
        press(&engine, wait_for(&engine, &rx, items, label, button))
    };
    open(&mut items, &project.name);
    wait_for(&engine, &rx, &mut items, "Project root", visible_text);
    open(&mut items, "Scope");
    open(&mut items, &scope.to_string_lossy());
    open(&mut items, "Save selection as a stack");
    wait_for(&engine, &rx, &mut items, "Deployment preview", visible_text);
    let stack_name = format!("{} · {}", project.name, scope.display());
    wait_for(&engine, &rx, &mut items, &stack_name, visible_text);
    assert!(
        manager
            .list_local_stacks()
            .unwrap()
            .iter()
            .any(|stack| stack.name == stack_name)
    );

    open(&mut items, "Open project");
    open(&mut items, "Open in Library");
    wait_for(&engine, &rx, &mut items, "Project Overview", button);
}
