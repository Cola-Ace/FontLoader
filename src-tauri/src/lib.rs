mod font;
mod models;
mod session;
mod state;

use models::{
    AppSnapshot, BatchCompletion, BatchProgress, BatchResult, ExitResult, InitialState,
    OperationError,
};
use state::FontCore;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use std::time::{Duration, Instant};
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
    core: FontCore,
    ui: Mutex<UiState>,
    operation: tauri::async_runtime::Mutex<()>,
    window_status: Mutex<WindowStatus>,
    window_bounds: Mutex<Option<WindowBounds>>,
    exit_requested: AtomicBool,
    exiting: AtomicBool,
}

struct UiState {
    snapshot: AppSnapshot,
    progress: Option<BatchProgress>,
    completion: Option<BatchCompletion>,
    batch_id: u64,
}

impl AppState {
    fn new(core: FontCore) -> Self {
        let snapshot = core.snapshot();
        Self {
            core,
            ui: Mutex::new(UiState {
                snapshot,
                progress: None,
                completion: None,
                batch_id: 0,
            }),
            operation: tauri::async_runtime::Mutex::new(()),
            window_status: Mutex::new(WindowStatus::Creating),
            window_bounds: Mutex::new(None),
            exit_requested: AtomicBool::new(false),
            exiting: AtomicBool::new(false),
        }
    }

    fn try_operation(&self) -> Result<impl std::ops::Deref<Target = ()> + '_, String> {
        let guard = self
            .operation
            .try_lock()
            .map_err(|_| "当前正在处理字体，请稍后再试".to_string())?;
        if self.exit_requested.load(Ordering::SeqCst) {
            return Err("FontLoader 正在退出".to_string());
        }
        Ok(guard)
    }
}

#[tauri::command]
async fn get_initial_state(state: State<'_, AppState>) -> Result<InitialState, String> {
    let ui = state.ui.lock().unwrap();
    Ok(InitialState {
        snapshot: ui.snapshot.clone(),
        progress: ui.progress.clone(),
        completion: ui.completion.clone(),
    })
}

#[tauri::command]
async fn get_batch_completion(
    id: u64,
    state: State<'_, AppState>,
) -> Result<Option<BatchCompletion>, String> {
    let ui = state.ui.lock().unwrap();
    Ok(ui
        .completion
        .as_ref()
        .filter(|completion| completion.id == id)
        .cloned())
}

#[tauri::command]
fn acknowledge_batch_completion(id: u64, state: State<'_, AppState>) {
    let mut ui = state.ui.lock().unwrap();
    if ui
        .completion
        .as_ref()
        .is_some_and(|completion| completion.id == id)
    {
        ui.completion = None;
    }
}

enum Batch {
    Load(Vec<String>),
    Unload(Vec<String>),
    Restore,
}

#[tauri::command]
async fn load_fonts(app: AppHandle, paths: Vec<String>) -> Result<BatchCompletion, String> {
    run_batch(app, Batch::Load(paths)).await
}

#[tauri::command]
async fn unload_fonts(app: AppHandle, paths: Vec<String>) -> Result<BatchCompletion, String> {
    run_batch(app, Batch::Unload(paths)).await
}

#[tauri::command]
async fn restore_last_session(app: AppHandle) -> Result<BatchCompletion, String> {
    run_batch(app, Batch::Restore).await
}

async fn run_batch(app: AppHandle, batch: Batch) -> Result<BatchCompletion, String> {
    let state = app.state::<AppState>();
    let _operation = state.try_operation()?;
    let progress = {
        let mut ui = state.ui.lock().unwrap();
        let (operation, total) = match &batch {
            Batch::Load(paths) => ("load", paths.len()),
            Batch::Unload(paths) => ("unload", paths.len()),
            Batch::Restore => ("load", ui.snapshot.restore_candidates.len()),
        };
        ui.batch_id += 1;
        let progress = BatchProgress {
            id: ui.batch_id,
            operation: operation.to_string(),
            completed: 0,
            total,
        };
        ui.progress = Some(progress.clone());
        ui.completion = None;
        progress
    };
    let id = progress.id;
    let operation = progress.operation.clone();
    let _ = app.emit(BATCH_PROGRESS_EVENT, &progress);
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let state = worker_app.state::<AppState>();
        let mut progress = progress;
        let mut last_emit = Instant::now();
        let report = |completed| {
            progress.completed = completed;
            let mut ui = state.ui.lock().unwrap();
            ui.progress.as_mut().unwrap().completed = completed;
            drop(ui);
            if completed == progress.total || last_emit.elapsed() >= BATCH_PROGRESS_INTERVAL {
                let _ = worker_app.emit(BATCH_PROGRESS_EVENT, &progress);
                last_emit = Instant::now();
            }
        };
        match batch {
            Batch::Load(paths) => state.core.load_fonts_with_progress(paths, report),
            Batch::Unload(paths) => state.core.unload_fonts_with_progress(paths, report),
            Batch::Restore => state.core.restore_last_session_with_progress(report),
        }
    })
    .await;
    let completion = {
        let mut ui = state.ui.lock().unwrap();
        let result = result.unwrap_or_else(|error| BatchResult {
            snapshot: ui.snapshot.clone(),
            errors: vec![OperationError {
                path: String::new(),
                code: "operation_failed".to_string(),
                message: error.to_string(),
            }],
            ..BatchResult::default()
        });
        ui.snapshot = result.snapshot.clone();
        ui.progress = None;
        let completion = BatchCompletion {
            id,
            operation,
            result,
        };
        ui.completion = Some(completion.clone());
        completion
    };
    update_loaded_count(&app, completion.result.snapshot.fonts.len());
    let _ = app.emit(BATCH_COMPLETION_EVENT, id);
    Ok(completion)
}

#[tauri::command]
fn dismiss_last_session(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    let _operation = state.try_operation()?;
    let snapshot = state
        .core
        .dismiss_last_session()
        .map_err(|error| error.to_string())?;
    let mut ui = state.ui.lock().unwrap();
    ui.snapshot = snapshot.clone();
    ui.completion = None;
    Ok(snapshot)
}

#[tauri::command]
async fn request_exit(app: AppHandle, force: bool) -> Result<ExitResult, String> {
    exit_application(app, force, false).await
}

async fn exit_application(
    app: AppHandle,
    force: bool,
    destroy_window: bool,
) -> Result<ExitResult, String> {
    let state = app.state::<AppState>();
    if state.exit_requested.swap(true, Ordering::SeqCst) {
        return Ok(ExitResult {
            exiting: false,
            errors: state.ui.lock().unwrap().snapshot.exit_errors.clone(),
        });
    }
    let (ui_dismissed, mut dismissal_completed) = tauri::async_runtime::channel(1);
    let ui_app = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        let _ = ui_dismissed.try_send(dismiss_exit_ui(&ui_app, destroy_window));
    }) {
        state.exit_requested.store(false, Ordering::SeqCst);
        return Err(error.to_string());
    }
    let window_destroy_requested = match dismissal_completed.recv().await.unwrap() {
        Ok(requested) => requested,
        Err(error) => {
            recover_failed_exit(&app);
            return Err(error.to_string());
        }
    };
    let operation = state.operation.lock().await;
    let cleanup_app = app.clone();
    let result = match tauri::async_runtime::spawn_blocking(move || {
        if window_destroy_requested {
            while cleanup_app.get_webview_window("main").is_some() {
                std::thread::sleep(Duration::from_millis(20));
            }
            let state = cleanup_app.state::<AppState>();
            let mut status = state.window_status.lock().unwrap();
            if matches!(*status, WindowStatus::Destroying { .. }) {
                *status = WindowStatus::Closed;
            }
        }
        cleanup_app.state::<AppState>().core.prepare_exit(force)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => {
            drop(operation);
            recover_failed_exit(&app);
            return Err(error.to_string());
        }
    };
    let snapshot = state.core.snapshot();
    {
        let mut ui = state.ui.lock().unwrap();
        ui.snapshot = snapshot;
        ui.completion = None;
    }
    drop(operation);
    if result.exiting {
        state.exiting.store(true, Ordering::SeqCst);
        let app_handle = app.clone();
        let _ = app.run_on_main_thread(move || app_handle.exit(0));
    } else {
        recover_failed_exit(&app);
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
        let count = state.ui.lock().unwrap().snapshot.fonts.len();
        let _ = window.set_title(&loaded_count_label(count));
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn loaded_count_label(count: usize) -> String {
    format!("FontLoader · {count}")
}

const BATCH_PROGRESS_EVENT: &str = "font-batch-progress";
const BATCH_COMPLETION_EVENT: &str = "font-batch-complete";
const BATCH_PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

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
    tauri::async_runtime::spawn(async move {
        if let Err(error) = exit_application(app, false, true).await {
            eprintln!("退出失败: {error}");
        }
    });
}

fn dismiss_exit_ui(app: &AppHandle, destroy_window: bool) -> tauri::Result<bool> {
    drop(app.remove_tray_by_id(TRAY_ID));
    let state = app.state::<AppState>();
    let Some(window) = app.get_webview_window("main") else {
        *state.window_status.lock().unwrap() = WindowStatus::Closed;
        return Ok(false);
    };
    if !destroy_window {
        // Keep the invoking WebView alive until its retry response is delivered.
        window.hide()?;
        return Ok(false);
    }
    *state.window_status.lock().unwrap() = WindowStatus::Destroying { reopen: false };
    if let Err(error) = window.destroy() {
        *state.window_status.lock().unwrap() = WindowStatus::Open;
        return Err(error);
    }
    Ok(true)
}

fn recover_failed_exit(app: &AppHandle) {
    if let Err(error) = setup_tray(app) {
        eprintln!("恢复托盘图标失败: {error}");
    }
    app.state::<AppState>()
        .exit_requested
        .store(false, Ordering::SeqCst);
    open_main_window(app);
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "打开", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "卸载全部并退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let count = app
        .state::<AppState>()
        .ui
        .lock()
        .unwrap()
        .snapshot
        .fonts
        .len();

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon())
        .tooltip(loaded_count_label(count))
        .menu(&menu)
        .show_menu_on_left_click(false)
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
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_main_window(app),
            "quit" => request_application_exit(app.clone()),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_initial_state,
            get_batch_completion,
            acknowledge_batch_completion,
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
            setup_tray(app.handle())?;
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
