mod font;
mod models;
mod session;
mod state;

use models::{AppSnapshot, BatchCompletion, BatchProgress, BatchResult, ExitResult};
use state::FontCore;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, RunEvent, Size, State,
    WindowEvent,
};

const TRAY_ID: &str = "main-tray";
const MIN_WINDOW_WIDTH: u32 = 760;
const MIN_WINDOW_HEIGHT: u32 = 480;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowStatus {
    Closed,
    Creating,
    Destroying { reopen: bool },
    Open,
}

#[derive(Clone, Copy, Debug)]
struct WindowBounds {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

struct AppState {
    core: Arc<FontCore>,
    cached_snapshot: Arc<Mutex<AppSnapshot>>,
    batch_progress: Arc<Mutex<Option<BatchProgress>>>,
    pending_batch_completion: Arc<Mutex<Option<BatchCompletion>>>,
    window_status: Mutex<WindowStatus>,
    window_bounds: Mutex<Option<WindowBounds>>,
    exit_requested: AtomicBool,
    exiting: AtomicBool,
}

impl AppState {
    fn new(core: FontCore) -> Self {
        let core = Arc::new(core);
        let cached_snapshot = core.snapshot();
        Self {
            core,
            cached_snapshot: Arc::new(Mutex::new(cached_snapshot)),
            batch_progress: Arc::new(Mutex::new(None)),
            pending_batch_completion: Arc::new(Mutex::new(None)),
            window_status: Mutex::new(WindowStatus::Creating),
            window_bounds: Mutex::new(None),
            exit_requested: AtomicBool::new(false),
            exiting: AtomicBool::new(false),
        }
    }
}

#[tauri::command]
async fn get_initial_state(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSnapshot, String> {
    let snapshot = state.cached_snapshot.lock().unwrap().clone();
    update_loaded_count(&app, snapshot.fonts.len());
    Ok(snapshot)
}

#[tauri::command]
fn get_batch_progress(state: State<'_, AppState>) -> Option<BatchProgress> {
    state.batch_progress.lock().unwrap().clone()
}

#[tauri::command]
fn take_pending_batch_completion(state: State<'_, AppState>) -> Option<BatchCompletion> {
    state.pending_batch_completion.lock().unwrap().take()
}

#[tauri::command]
async fn load_fonts(
    app: AppHandle,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<BatchResult, String> {
    let total = paths.len();
    let batch_progress = Arc::clone(&state.batch_progress);
    let pending_batch_completion = Arc::clone(&state.pending_batch_completion);
    clear_pending_batch_completion(&pending_batch_completion);
    update_batch_progress(&batch_progress, "load", 0, total);
    emit_batch_progress(&app, "load", 0, total);
    let core = Arc::clone(&state.core);
    let progress_app = app.clone();
    let progress_state = Arc::clone(&batch_progress);
    let result = tauri::async_runtime::spawn_blocking(move || {
        core.load_fonts_with_progress(paths, |completed| {
            update_batch_progress(&progress_state, "load", completed, total);
            emit_batch_progress(&progress_app, "load", completed, total);
        })
    })
    .await
    .map_err(|error| error.to_string());
    clear_batch_progress(&batch_progress);
    let result = result?;
    update_cached_snapshot(&state.cached_snapshot, result.snapshot.clone());
    let completion = store_batch_completion(&pending_batch_completion, "load", &result);
    emit_batch_completion(&app, &completion);
    update_loaded_count(&app, result.snapshot.fonts.len());
    Ok(result)
}

#[tauri::command]
async fn unload_fonts(
    app: AppHandle,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<BatchResult, String> {
    let total = paths.len();
    let batch_progress = Arc::clone(&state.batch_progress);
    let pending_batch_completion = Arc::clone(&state.pending_batch_completion);
    clear_pending_batch_completion(&pending_batch_completion);
    update_batch_progress(&batch_progress, "unload", 0, total);
    emit_batch_progress(&app, "unload", 0, total);
    let core = Arc::clone(&state.core);
    let progress_app = app.clone();
    let progress_state = Arc::clone(&batch_progress);
    let result = tauri::async_runtime::spawn_blocking(move || {
        core.unload_fonts_with_progress(paths, |completed| {
            update_batch_progress(&progress_state, "unload", completed, total);
            emit_batch_progress(&progress_app, "unload", completed, total);
        })
    })
    .await
    .map_err(|error| error.to_string());
    clear_batch_progress(&batch_progress);
    let result = result?;
    update_cached_snapshot(&state.cached_snapshot, result.snapshot.clone());
    let completion = store_batch_completion(&pending_batch_completion, "unload", &result);
    emit_batch_completion(&app, &completion);
    update_loaded_count(&app, result.snapshot.fonts.len());
    Ok(result)
}

#[tauri::command]
async fn restore_last_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BatchResult, String> {
    let total = state.core.snapshot().restore_candidates.len();
    let batch_progress = Arc::clone(&state.batch_progress);
    let pending_batch_completion = Arc::clone(&state.pending_batch_completion);
    clear_pending_batch_completion(&pending_batch_completion);
    update_batch_progress(&batch_progress, "load", 0, total);
    emit_batch_progress(&app, "load", 0, total);
    let core = Arc::clone(&state.core);
    let progress_app = app.clone();
    let progress_state = Arc::clone(&batch_progress);
    let result = tauri::async_runtime::spawn_blocking(move || {
        core.restore_last_session_with_progress(|completed| {
            update_batch_progress(&progress_state, "load", completed, total);
            emit_batch_progress(&progress_app, "load", completed, total);
        })
    })
    .await
    .map_err(|error| error.to_string());
    clear_batch_progress(&batch_progress);
    let result = result?;
    update_cached_snapshot(&state.cached_snapshot, result.snapshot.clone());
    let completion = store_batch_completion(&pending_batch_completion, "load", &result);
    emit_batch_completion(&app, &completion);
    update_loaded_count(&app, result.snapshot.fonts.len());
    Ok(result)
}

#[tauri::command]
fn dismiss_last_session(state: State<'_, AppState>) -> AppSnapshot {
    let snapshot = state.core.dismiss_last_session();
    update_cached_snapshot(&state.cached_snapshot, snapshot.clone());
    snapshot
}

#[tauri::command]
async fn request_exit(
    app: AppHandle,
    state: State<'_, AppState>,
    force: bool,
) -> Result<ExitResult, String> {
    if state.exit_requested.swap(true, Ordering::SeqCst) {
        return Ok(ExitResult {
            exiting: false,
            errors: state.core.snapshot().exit_errors,
        });
    }
    let total = state.core.snapshot().fonts.len();
    let batch_progress = Arc::clone(&state.batch_progress);
    update_batch_progress(&batch_progress, "unload", 0, total);
    emit_batch_progress(&app, "unload", 0, total);
    let core = Arc::clone(&state.core);
    let progress_app = app.clone();
    let progress_state = Arc::clone(&batch_progress);
    let result = match tauri::async_runtime::spawn_blocking(move || {
        core.prepare_exit_with_progress(force, |completed| {
            update_batch_progress(&progress_state, "unload", completed, total);
            emit_batch_progress(&progress_app, "unload", completed, total);
        })
    })
    .await
    {
        Ok(result) => result,
        Err(error) => {
            clear_batch_progress(&batch_progress);
            state.exit_requested.store(false, Ordering::SeqCst);
            return Err(error.to_string());
        }
    };
    clear_batch_progress(&batch_progress);
    update_cached_snapshot(&state.cached_snapshot, state.core.snapshot());
    if result.exiting {
        state.exiting.store(true, Ordering::SeqCst);
        let app_handle = app.clone();
        let _ = app.run_on_main_thread(move || app_handle.exit(0));
    } else {
        state.exit_requested.store(false, Ordering::SeqCst);
    }
    Ok(result)
}

#[tauri::command]
fn window_ready(app: AppHandle, state: State<'_, AppState>) {
    let mut status = state.window_status.lock().unwrap();
    if !matches!(*status, WindowStatus::Creating | WindowStatus::Open) {
        return;
    }
    *status = WindowStatus::Open;
    drop(status);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn loaded_count_label(count: usize) -> String {
    format!("FontLoader · {count}")
}

const BATCH_PROGRESS_EVENT: &str = "font-batch-progress";
const BATCH_COMPLETION_EVENT: &str = "font-batch-complete";

fn emit_batch_progress(app: &AppHandle, operation: &str, completed: usize, total: usize) {
    let _ = app.emit(
        BATCH_PROGRESS_EVENT,
        BatchProgress {
            operation: operation.to_string(),
            completed,
            total,
        },
    );
}

fn emit_batch_completion(app: &AppHandle, completion: &BatchCompletion) {
    let _ = app.emit(BATCH_COMPLETION_EVENT, completion);
}

fn update_batch_progress(
    progress_state: &Mutex<Option<BatchProgress>>,
    operation: &str,
    completed: usize,
    total: usize,
) {
    *progress_state.lock().unwrap() = Some(BatchProgress {
        operation: operation.to_string(),
        completed,
        total,
    });
}

fn clear_batch_progress(progress_state: &Mutex<Option<BatchProgress>>) {
    *progress_state.lock().unwrap() = None;
}

fn store_batch_completion(
    completion_state: &Mutex<Option<BatchCompletion>>,
    operation: &str,
    result: &BatchResult,
) -> BatchCompletion {
    let completion = BatchCompletion {
        operation: operation.to_string(),
        result: result.clone(),
    };
    *completion_state.lock().unwrap() = Some(completion.clone());
    completion
}

fn clear_pending_batch_completion(completion_state: &Mutex<Option<BatchCompletion>>) {
    *completion_state.lock().unwrap() = None;
}

fn update_cached_snapshot(snapshot_state: &Mutex<AppSnapshot>, snapshot: AppSnapshot) {
    *snapshot_state.lock().unwrap() = snapshot;
}

fn update_loaded_count(app: &AppHandle, count: usize) {
    let label = loaded_count_label(count);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_title(&label);
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(&label));
    }
}

fn create_main_window(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.exit_requested.load(Ordering::SeqCst) {
        return;
    }
    let mut status = state.window_status.lock().unwrap();
    if *status != WindowStatus::Closed {
        return;
    }
    *status = WindowStatus::Creating;
    drop(status);

    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "main")
        .cloned()
    else {
        *state.window_status.lock().unwrap() = WindowStatus::Closed;
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        match tauri::WebviewWindowBuilder::from_config(&app, &config)
            .and_then(|builder| builder.build())
        {
            Ok(window) => {
                let state = app.state::<AppState>();
                if state.exit_requested.load(Ordering::SeqCst) {
                    *state.window_status.lock().unwrap() = WindowStatus::Closed;
                    let _ = window.destroy();
                    return;
                }
                if let Some(bounds) = *app.state::<AppState>().window_bounds.lock().unwrap() {
                    let _ = window.set_position(Position::Physical(bounds.position));
                    let _ = window.set_size(Size::Physical(bounds.size));
                }
                *app.state::<AppState>().window_status.lock().unwrap() = WindowStatus::Open;
                let _ = window.show();
                let _ = window.set_focus();
            }
            Err(_) => {
                *app.state::<AppState>().window_status.lock().unwrap() = WindowStatus::Closed;
            }
        }
    });
}

fn open_main_window(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.exit_requested.load(Ordering::SeqCst) {
        return;
    }
    let mut status = state.window_status.lock().unwrap();

    match *status {
        WindowStatus::Open => {
            drop(status);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        WindowStatus::Creating => {}
        WindowStatus::Closed => {
            drop(status);
            create_main_window(app);
        }
        WindowStatus::Destroying { .. } => {
            *status = WindowStatus::Destroying { reopen: true };
        }
    }
}

fn destroy_minimized_window(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut status = state.window_status.lock().unwrap();
    if *status != WindowStatus::Open {
        return;
    }
    *status = WindowStatus::Destroying { reopen: false };
    drop(status);

    let Some(window) = app.get_webview_window("main") else {
        *state.window_status.lock().unwrap() = WindowStatus::Closed;
        return;
    };
    if window.destroy().is_err() {
        *state.window_status.lock().unwrap() = WindowStatus::Open;
        return;
    }

    let app = app.clone();
    std::thread::spawn(move || {
        while app.get_webview_window("main").is_some() {
            std::thread::sleep(Duration::from_millis(20));
        }

        let should_reopen = {
            let state = app.state::<AppState>();
            let mut status = state.window_status.lock().unwrap();
            match *status {
                WindowStatus::Destroying { reopen } => {
                    *status = WindowStatus::Closed;
                    reopen
                }
                _ => false,
            }
        };
        if should_reopen {
            create_main_window(&app);
        }
    });
}

fn request_application_exit(app: AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if state.exit_requested.swap(true, Ordering::SeqCst) {
        return;
    }

    let has_loaded_fonts = !state.core.snapshot().fonts.is_empty();
    let window_destroy_requested = if has_loaded_fonts {
        match app.get_webview_window("main") {
            Some(window) => {
                {
                    let mut status = state.window_status.lock().unwrap();
                    if !matches!(*status, WindowStatus::Closed) {
                        *status = WindowStatus::Destroying { reopen: false };
                    }
                }
                if window.destroy().is_err() {
                    *state.window_status.lock().unwrap() = WindowStatus::Open;
                    false
                } else {
                    true
                }
            }
            None => {
                *state.window_status.lock().unwrap() = WindowStatus::Closed;
                false
            }
        }
    } else {
        false
    };

    std::thread::spawn(move || {
        if window_destroy_requested {
            while app.get_webview_window("main").is_some() {
                std::thread::sleep(Duration::from_millis(20));
            }
            let state = app.state::<AppState>();
            let mut status = state.window_status.lock().unwrap();
            if matches!(*status, WindowStatus::Destroying { .. }) {
                *status = WindowStatus::Closed;
            }
        }

        let state = app.state::<AppState>();
        let result = state.core.prepare_exit(false);
        update_cached_snapshot(&state.cached_snapshot, state.core.snapshot());
        if result.exiting {
            state.exiting.store(true, Ordering::SeqCst);
            app.exit(0);
        } else {
            state.exit_requested.store(false, Ordering::SeqCst);
            open_main_window(&app);
        }
    });
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "卸载全部并退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon())
        .tooltip(loaded_count_label(0))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_main_window(app),
            "quit" => request_application_exit(app.clone()),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn tray_icon() -> Image<'static> {
    let width = 32;
    let height = 32;
    let mut rgba = vec![0u8; width * height * 4];
    for y in 3..29 {
        for x in 3..29 {
            let offset = (y * width + x) * 4;
            rgba[offset] = 30;
            rgba[offset + 1] = 136;
            rgba[offset + 2] = 229;
            rgba[offset + 3] = 255;
        }
    }
    for y in 8..25 {
        for x in 10..14 {
            let offset = (y * width + x) * 4;
            rgba[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    for y in 8..12 {
        for x in 10..23 {
            let offset = (y * width + x) * 4;
            rgba[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    for y in 15..19 {
        for x in 10..20 {
            let offset = (y * width + x) * 4;
            rgba[offset..offset + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    Image::new_owned(rgba, width as u32, height as u32)
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            open_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            get_batch_progress,
            take_pending_batch_completion,
            load_fonts,
            unload_fonts,
            restore_last_session,
            dismiss_last_session,
            request_exit,
            window_ready,
        ])
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let core = FontCore::new(app_data_dir)?;
            app.manage(AppState::new(core));
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            let state = window.app_handle().state::<AppState>();
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    if !state.exiting.load(Ordering::SeqCst) {
                        api.prevent_close();
                        request_application_exit(window.app_handle().clone());
                    }
                }
                WindowEvent::Moved(position) => {
                    let size = window.outer_size().unwrap_or(PhysicalSize::new(920, 620));
                    if size.width >= MIN_WINDOW_WIDTH && size.height >= MIN_WINDOW_HEIGHT {
                        *state.window_bounds.lock().unwrap() = Some(WindowBounds {
                            position: *position,
                            size,
                        });
                    }
                }
                WindowEvent::Resized(size) => {
                    if window.is_minimized().unwrap_or(false) {
                        destroy_minimized_window(window.app_handle());
                    } else if size.width >= MIN_WINDOW_WIDTH && size.height >= MIN_WINDOW_HEIGHT {
                        let position = window
                            .outer_position()
                            .unwrap_or(PhysicalPosition::new(0, 0));
                        *state.window_bounds.lock().unwrap() = Some(WindowBounds {
                            position,
                            size: *size,
                        });
                    }
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("构建 FontLoader 失败");

    app.run(|app, event| {
        if let RunEvent::ExitRequested { api, .. } = event {
            let state = app.state::<AppState>();
            if !state.exiting.load(Ordering::SeqCst) {
                api.prevent_exit();
            }
        }
    });
}
