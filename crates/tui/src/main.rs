//! dbtool TUI — DataGrip 식 터미널 클라이언트. GUI 와 같은 엔진(dbtool-core)·같은 Data Source 를 쓴다.
mod app;
mod grid;
mod help;
mod input;
mod tree;
mod ui;

use crossterm::event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, EventStream, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use futures::StreamExt;
use ratatui::prelude::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("dbtool-tui [--config-dir DIR]\n\nTerminal UI for dbtool (PostgreSQL / MySQL / Redis). Shares data sources with the dbtool app.\nPress F1 inside for this keymap.\n{}", help::text());
        return Ok(());
    }
    let dir = args.iter().position(|a| a == "--config-dir").and_then(|i| args.get(i + 1)).map(std::path::PathBuf::from);
    // 엔진 로그가 화면을 깨지 않도록
    dbtool_core::QUIET.store(true, std::sync::atomic::Ordering::Relaxed);
    let state = Arc::new(dbtool_core::open_state(dir).map_err(|e| std::io::Error::other(e.to_string()))?);

    enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)?;
    // Ctrl+Enter 등을 구분할 수 있는 터미널(kitty, WezTerm, iTerm2, Ghostty…)에서는 확장 키 보고를 켠다
    let enhanced = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false);
    if enhanced {
        let _ = execute!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES));
    }
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        let _ = restore();
        hook(p);
    }));
    let mut term = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    let res = run(&mut term, state).await;
    if enhanced {
        let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    }
    restore()?;
    res
}

fn restore() -> std::io::Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture, DisableBracketedPaste)
}

async fn run(term: &mut Terminal<CrosstermBackend<std::io::Stdout>>, state: Arc<dbtool_core::app::AppState>) -> std::io::Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<app::Apply>();
    let mut app = app::App::new(state.clone(), tx.clone());
    // keep-alive / auto-disconnect (GUI 와 같은 규칙)
    {
        let tx = tx.clone();
        let st = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let dropped = dbtool_core::app::housekeeping(&st).await;
                if !dropped.is_empty() && tx.send(Box::new(move |a: &mut app::App| {
                    for id in &dropped {
                        a.tree.forget(id);
                        a.tree.expanded.retain(|k| !(k == id || k.starts_with(&format!("{id}|"))));
                    }
                    a.status = format!("Disconnected (idle): {}", dropped.len());
                    a.reload_conns();
                })).is_err() {
                    break;
                }
            }
        });
    }
    let mut events = EventStream::new();
    loop {
        term.draw(|f| ui::draw(f, &mut app))?;
        tokio::select! {
            ev = events.next() => match ev {
                Some(Ok(ev)) => input::on_event(&mut app, ev),
                Some(Err(e)) => return Err(e),
                None => break,
            },
            Some(apply) = rx.recv() => {
                apply(&mut app);
                while let Ok(a) = rx.try_recv() {
                    a(&mut app);
                }
            }
        }
        if app.quit {
            break;
        }
    }
    Ok(())
}
