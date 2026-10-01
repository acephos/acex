//! Offline fixture benchmark of shipped reducers/rendering, not live agent throughput.
use acex_model::Store;
use acex_ui::{render, App};
use herdr_types::{AgentState, AgentSummary, Event, SessionSnapshot};
use ratatui::{backend::TestBackend, Terminal};
use serde_json::json;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

fn percentiles(mut samples: Vec<u128>) -> serde_json::Value {
    samples.sort_unstable();
    let at = |percent: usize| samples[(samples.len() - 1) * percent / 100];
    json!({"unit":"nanoseconds", "samples":samples.len(), "p50":at(50), "p95":at(95), "p99":at(99)})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    for count in [16, 64, 256] {
        let mut store = Store::default();
        store.apply_snapshot(SessionSnapshot {
            agents: (0..count)
                .map(|id| AgentSummary {
                    id: format!("fixture-{id}"),
                    pane_id: Some(format!("fixture:{id}")),
                    name: Some(format!("synthetic agent {id}")),
                    state: AgentState::Working,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        });
        let events: Vec<Event> = (0..count)
            .map(|id| Event {
                event: "pane_agent_status_changed".into(),
                data: json!({"pane_id":format!("fixture:{id}"),"agent_status":"working"}),
                extra: Default::default(),
            })
            .collect();
        let mut reduce = Vec::new();
        for index in 0..2000 {
            let start = Instant::now();
            store.apply_event(&events[index % count]);
            let elapsed = start.elapsed().as_nanos();
            if index >= 200 {
                reduce.push(elapsed);
            }
        }
        let (tx, _rx) = mpsc::channel();
        let app = App::with_shared(Arc::new(Mutex::new(Store::default())), tx);
        let mut terminal = Terminal::new(TestBackend::new(156, 48))?;
        let mut frames = Vec::new();
        for index in 0..220 {
            let start = Instant::now();
            terminal.draw(|frame| render(frame, &store, &app))?;
            let elapsed = start.elapsed().as_nanos();
            if index >= 20 {
                frames.push(elapsed);
            }
        }
        if count == 16 {
            if let Some(path) = std::env::args().nth(1) {
                let buffer = terminal.backend().buffer();
                let mut text = String::from("OFFLINE SYNTHETIC FIXTURE — no live agent activity\n");
                for y in 0..buffer.area.height {
                    for x in 0..buffer.area.width {
                        text.push_str(buffer[(x, y)].symbol());
                    }
                    text.push('\n');
                }
                std::fs::write(path, text)?;
            }
        }
        results
            .push(json!({"agents":count,"reduce":percentiles(reduce),"frame":percentiles(frames)}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"scope":"offline synthetic fixture; no network, PTYs or live agents", "terminal":[156,48],"results":results})
        )?
    );
    Ok(())
}
