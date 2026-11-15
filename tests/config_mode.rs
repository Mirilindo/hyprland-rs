//! Checks what actually goes over the socket, in both config modes.
//!
//! No compositor is needed: a stub socket answers like Hyprland does and
//! records every request it received, so these hold in CI and on any machine,
//! whatever its config.

use hyprland::config_mode::ConfigMode;
use hyprland::dispatch::{Dispatch, DispatchType, WorkspaceIdentifierWithSpecial};
use hyprland::instance::Instance;
use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};

/// Spawns a stub `.socket.sock` and returns the instance talking to it, plus
/// the list of the requests it receives.
fn stub_hyprland(name: &str, provider: &'static str) -> (Instance, Arc<Mutex<Vec<String>>>) {
    let dir = std::env::temp_dir().join(format!("hyprland-rs-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let listener = UnixListener::bind(dir.join(".socket.sock")).unwrap();

    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let recorder = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            // One read, not read_to_string: the client keeps its write half
            // open while it waits for the answer.
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let answer = if request == "j/status" {
                format!(
                    "\n{{\n    \"configProvider\": \"{provider}\",\n    \"backend\": \"drm\"\n}}\n"
                )
            } else {
                "ok".to_string()
            };
            // Recorded before the answer goes out, so that the caller finds it
            // as soon as its request returns.
            recorder.lock().unwrap().push(request);
            stream.write_all(answer.as_bytes()).unwrap();
        }
    });

    (Instance::from_base_socket_path(dir).unwrap(), seen)
}

fn workspace_1() -> DispatchType<'static> {
    DispatchType::Workspace(WorkspaceIdentifierWithSpecial::Id(1))
}

#[test]
fn a_legacy_config_still_gets_the_legacy_syntax() {
    let (instance, seen) = stub_hyprland("legacy", "hyprlang");

    assert_eq!(instance.config_mode().unwrap(), ConfigMode::Legacy);
    Dispatch::instance_call(&instance, workspace_1()).unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(*seen, ["j/status", "j/dispatch workspace 1"]);
}

#[test]
fn a_lua_config_gets_lua() {
    let (instance, seen) = stub_hyprland("lua", "lua");

    assert_eq!(instance.config_mode().unwrap(), ConfigMode::Lua);
    Dispatch::instance_call(&instance, workspace_1()).unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(
        *seen,
        [
            "j/status",
            r#"j/dispatch hl.dsp.focus({ workspace = "1" })"#
        ]
    );
}

#[test]
fn the_mode_is_asked_once_per_instance() {
    let (instance, seen) = stub_hyprland("cached", "lua");

    for _ in 0..3 {
        Dispatch::instance_call(&instance, DispatchType::Exec("true")).unwrap();
    }
    // A clone shares the answer rather than asking again.
    let clone = instance.clone();
    Dispatch::instance_call(&clone, DispatchType::Exec("true")).unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.iter().filter(|r| *r == "j/status").count(), 1);
    assert_eq!(seen.len(), 5, "one status request for four dispatches");
}

#[test]
fn an_untranslated_dispatcher_sends_nothing_on_a_lua_config() {
    let (instance, seen) = stub_hyprland("untranslated", "lua");

    let err = Dispatch::instance_call(&instance, DispatchType::ToggleFakeFullscreen).unwrap_err();
    assert!(err.to_string().contains("ToggleFakeFullscreen"), "{err}");

    let seen = seen.lock().unwrap();
    assert_eq!(*seen, ["j/status"], "only the status request went out");
}

#[test]
fn an_untranslated_dispatcher_still_works_on_a_legacy_config() {
    let (instance, seen) = stub_hyprland("untranslated-legacy", "hyprlang");

    Dispatch::instance_call(&instance, DispatchType::ToggleFakeFullscreen).unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(*seen, ["j/status", "j/dispatch fakefullscreen"]);
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn the_async_path_follows_the_mode_too() {
    let (instance, seen) = stub_hyprland("async", "lua");

    Dispatch::instance_call_async(&instance, workspace_1())
        .await
        .unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(
        *seen,
        [
            "j/status",
            r#"j/dispatch hl.dsp.focus({ workspace = "1" })"#
        ]
    );
}
