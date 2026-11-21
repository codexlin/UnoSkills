use std::{
    collections::HashSet,
    fs,
    path::Path,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

use nsh_core::{
    ApproxTextEngine, Command, EventKind, EventPayload, MountBatch, MountItem, NodeId, ROOT_ID,
    file_dialog::{FileRequest, OpenDialogKind},
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
            file_requests: true,
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

fn folder_request(item: &MountItem, title: &str) -> Option<u32> {
    let MountItem::Command {
        command: Command::FileRequest { id, request },
        ..
    } = item
    else {
        return None;
    };
    let FileRequest::OpenDialog(dialog) = request.as_ref() else {
        return None;
    };
    (dialog.kind == OpenDialogKind::Folders && dialog.title.as_deref() == Some(title))
        .then_some(*id)
}

fn press(engine: &Engine, node: NodeId) {
    for event in [EventKind::PressDown, EventKind::PressUp, EventKind::Press] {
        engine
            .control()
            .dispatch_event(node, event, EventPayload::None);
    }
}

#[test]
fn native_folder_pick_leads_from_scoped_project_to_stack_preview_and_library() {
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
    let project_dir = temp.path().join("Routing smoke project");
    let scope = Path::new("apps").join("desktop");
    fs::create_dir_all(project_dir.join(&scope)).unwrap();
    fs::write(
        project_dir.join(&scope).join("Cargo.toml"),
        "[package]\nname = \"smoke-app\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    let (engine, rx) = start(&root);
    let mut items = Vec::new();
    let open = |items: &mut Vec<MountItem>, label: &str| {
        press(&engine, wait_for(&engine, &rx, items, label, button))
    };
    open(&mut items, "Add Project");
    let request = wait_for(&engine, &rx, &mut items, "Add Project", folder_request);
    engine.control().dispatch_event(
        ROOT_ID,
        EventKind::FileRequest,
        EventPayload::file_answer(request, vec![project_dir]),
    );
    wait_for(&engine, &rx, &mut items, "Project Root", visible_text);
    let projects = manager.list_projects().unwrap();
    assert_eq!(projects.len(), 1);
    let project = &projects[0];
    assert!(
        manager
            .project_module_paths(&project.id)
            .unwrap()
            .contains(&scope)
    );
    open(&mut items, "Scan Scope");
    open(&mut items, &scope.to_string_lossy());
    wait_for(&engine, &rx, &mut items, "2 selected", visible_text);
    open(&mut items, "Save as Stack");
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
    let open_library = wait_for(&engine, &rx, &mut items, "Open in Library", button);
    items.clear();
    press(&engine, open_library);
    wait_for(&engine, &rx, &mut items, "Project Overview", button);
    wait_for(&engine, &rx, &mut items, &stack_name, visible_text);
}

#[test]
fn unreadable_skill_cannot_open_another_skills_editor() {
    use unoskills_core::SourceType;

    let temp = tempdir().unwrap();
    let root = temp.path().join("library");
    let manager = UnoSkillsManager::open_at(root.clone()).unwrap();
    manager.set_setting("language", "en").unwrap();
    manager.set_http_proxy(Some("http://127.0.0.1:1")).unwrap();

    let good_markdown = "---\nname: Alpha editable\ndescription: Fixture\n---\n\n# Alpha body\n";
    let broken_markdown =
        "---\nname: Bravo unreadable\ndescription: Fixture\n---\n\n# Bravo body\n";
    let import = |slug: &str, content: &str| {
        let source = temp.path().join(slug);
        fs::create_dir(&source).unwrap();
        fs::write(source.join("SKILL.md"), content).unwrap();
        manager
            .import_directory(&source, Some(slug), SourceType::Local, None, None, None)
            .unwrap()
    };
    let good = import("alpha-editable", good_markdown);
    let broken = import("bravo-unreadable", broken_markdown);
    let unreadable = b"\xff\xfe";
    let broken_path = broken.central_path.join("SKILL.md");
    fs::write(&broken_path, unreadable).unwrap();

    let (engine, rx) = start(&root);
    let mut items = Vec::new();
    wait_for(&engine, &rx, &mut items, "Added in the last 7 days", button);
    let good_link = wait_for(&engine, &rx, &mut items, &good.name, button);
    press(&engine, good_link);
    let preview = wait_for(&engine, &rx, &mut items, "Preview", visible_text);
    let tab = items
        .iter()
        .rev()
        .find_map(|item| match item {
            MountItem::Insert { child, parent, .. } if *child == preview => Some(*parent),
            _ => None,
        })
        .expect("Preview tab contains its label");
    press(&engine, tab);
    wait_for(&engine, &rx, &mut items, "Edit SKILL.md", |item, label| {
        edit_button(item, label, false)
    });

    let broken_row = wait_for(&engine, &rx, &mut items, &broken.name, button);
    press(&engine, broken_row);
    wait_for(
        &engine,
        &rx,
        &mut items,
        "UTF-8",
        |item, expected| match item {
            MountItem::Text { id, text, .. } if text.text.contains(expected) => Some(*id),
            _ => None,
        },
    );
    let mut removed = HashSet::new();
    for item in items.iter().rev() {
        match item {
            MountItem::Delete { id } | MountItem::Remove { id } => {
                removed.insert(*id);
            }
            MountItem::ViewProps { id, props }
                if !removed.contains(id)
                    && props.accessibility.label.as_deref() == Some("Edit SKILL.md") =>
            {
                assert!(props.accessibility.state.disabled);
                break;
            }
            _ => {}
        }
    }

    assert_eq!(
        fs::read_to_string(good.central_path.join("SKILL.md")).unwrap(),
        good_markdown
    );
    assert_eq!(fs::read(broken_path).unwrap(), unreadable);
}

fn edit_button(item: &MountItem, label: &str, disabled: bool) -> Option<NodeId> {
    match item {
        MountItem::ViewProps { id, props }
            if props.accessibility.label.as_deref() == Some(label)
                && props.accessibility.state.disabled == disabled =>
        {
            Some(*id)
        }
        _ => None,
    }
}
