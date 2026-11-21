use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

use nsh_core::{
    ApproxTextEngine, EventKind, EventPayload, MountBatch, MountItem, NodeId,
    geometry::{Size, px},
};
use nsh_engine::{Engine, EngineConfig, MountSink};
use tempfile::tempdir;
use unoskills_core::UnoSkillsManager;

enum Observation {
    Batch(MountBatch),
    Error(String),
}

struct Sink(Sender<Observation>);

impl MountSink for Sink {
    fn submit(&self, batch: MountBatch) {
        let _ = self.0.send(Observation::Batch(batch));
    }

    fn report_error(&self, message: &str) {
        let _ = self.0.send(Observation::Error(message.to_owned()));
    }
}

fn wait_for(
    rx: &Receiver<Observation>,
    items: &mut Vec<MountItem>,
    label: &str,
    predicate: impl Fn(&MountItem, &str) -> Option<NodeId>,
) -> NodeId {
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if let Some(found) = items.iter().find_map(|item| predicate(item, label)) {
            return found;
        }
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Observation::Batch(batch)) => items.extend(batch.items),
            Ok(Observation::Error(error)) => panic!("NSH reported {error}"),
            Err(error) => {
                let observed = items
                    .iter()
                    .filter_map(|item| match item {
                        MountItem::Text { text, .. } => Some(text.text.as_ref()),
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
        MountItem::Text { id, text, .. } if text.text.as_ref() == label => Some(*id),
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
    let host = crate::nsh::channel_for(move || UnoSkillsManager::open_at(root.clone())).unwrap();
    let config = EngineConfig {
        host,
        viewport: Size::new(px(1240.0), px(780.0)),
        ui_sets: vec![nsh_base::registry(), crate::nsh::dialogs::registry()],
        ..EngineConfig::new(crate::bundle().unwrap())
    };
    let (tx, rx) = mpsc::channel();
    let engine = Engine::start(config, || ApproxTextEngine, Sink(tx)).unwrap();
    let mut items = Vec::new();
    let settings = wait_for(&rx, &mut items, "Settings", button);
    wait_for(&rx, &mut items, "Local workspace", visible_text);
    engine
        .control()
        .dispatch_event(settings, EventKind::Press, EventPayload::None);
    wait_for(&rx, &mut items, "Central Library Root", visible_text);
}
