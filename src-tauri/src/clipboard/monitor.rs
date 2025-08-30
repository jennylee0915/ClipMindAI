// src-tauri/src/clipboard/monitor.rs
use super::types::{ClipboardEvent, ClipboardError};
use super::content_detector::ContentDetector;

use arboard::Clipboard;
use log::{debug, info, warn, error};
use serde::{Serialize, Deserialize};

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use tokio::sync::{broadcast, mpsc};
use tokio::time::Instant;

// Windows API 
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM, HINSTANCE},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
        RegisterClassExW, UnregisterClassW, TranslateMessage, PostQuitMessage, DestroyWindow,
        MSG, WNDCLASSEXW, WM_CLIPBOARDUPDATE, WM_DESTROY,
        CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
    },
};

// AddClipboardFormatListener and RemoveClipboardFormatListener
extern "system" {
    fn AddClipboardFormatListener(hwnd: HWND) -> i32;
    fn RemoveClipboardFormatListener(hwnd: HWND) -> i32;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorConfig {
    pub min_content_length: usize,
    pub max_content_length: usize,
    pub ignore_duplicates: bool,
    pub ignore_short_content: bool,
    /// Debounce time (milliseconds)
    pub debounce_ms: u64,
    /// Number of retries for clipboard read operations
    pub retry_max: u32,
    /// Initial retry delay (milliseconds), will exponentially back off up to 200ms
    pub retry_initial_delay_ms: u64,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            min_content_length: 1,
            max_content_length: 100_000,
            ignore_duplicates: true,
            ignore_short_content: false,
            debounce_ms: 60,
            retry_max: 8,
            retry_initial_delay_ms: 10,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClipboardChange {
    pub event: ClipboardEvent,
    pub is_duplicate: bool,
    pub source_detection_time_ms: u64,
}

#[derive(Debug)]
enum MonitorCommand {
    Stop,
}

// Signal for the dedicated worker (only needs Pulse)
type WorkerPulseTx = std::sync::mpsc::Sender<()>;
type WorkerPulseRx = std::sync::mpsc::Receiver<()>;

/// Shared context for the Win32 message loop thread (used by the window procedure)
struct WindowsMonitorContext {
    event_sender: broadcast::Sender<ClipboardChange>,
    content_detector: ContentDetector,
    config: MonitorConfig,
    window_handle: HWND,
    // Channel to send messages to the worker (only sends () to indicate updates)
    worker_tx: Option<WorkerPulseTx>,
}

// ===== Global State =====
static mut MESSAGE_LOOP_HANDLE: Option<JoinHandle<()>> = None;
static MESSAGE_LOOP_RUNNING: AtomicBool = AtomicBool::new(false);
static mut GLOBAL_CONTEXT: Option<Arc<Mutex<WindowsMonitorContext>>> = None;

// ===== Main External Object =====
pub struct ClipboardMonitor {
    config: MonitorConfig,
    content_detector: ContentDetector,

    // Event channel
    pub event_sender: broadcast::Sender<ClipboardChange>,
    _event_receiver: broadcast::Receiver<ClipboardChange>,

    // Control channel
    control_sender: Option<mpsc::UnboundedSender<MonitorCommand>>,

    // Basic state
    start_time: Option<Instant>,
    is_running: bool,
}

impl ClipboardMonitor {
    pub fn new(config: Option<MonitorConfig>) -> std::result::Result<Self, ClipboardError> {
        let config = config.unwrap_or_default();

        // Do not initialize Clipboard here; it will be exclusively owned by the worker thread
        let (event_sender, event_receiver) = broadcast::channel(1000);

        Ok(Self {
            config,
            content_detector: ContentDetector::new(),
            event_sender,
            _event_receiver: event_receiver,
            control_sender: None,
            start_time: None,
            is_running: false,
        })
    }

        /// Start monitoring - Windows event-driven + dedicated worker
    pub async fn start_monitoring(&mut self) -> std::result::Result<broadcast::Receiver<ClipboardChange>, ClipboardError> {
        if self.is_running {
            return Err(ClipboardError::AccessError("Monitor is already running".to_string()));
        }

        self.start_time = Some(Instant::now());
        self.is_running = true;

        let (control_tx, mut control_rx) = mpsc::unbounded_channel();
        self.control_sender = Some(control_tx);

        let event_receiver = self.event_sender.subscribe();

        // Create worker channel (std mpsc, convenient for blocking recv in std::thread)
        let (worker_tx, worker_rx): (WorkerPulseTx, WorkerPulseRx) = std::sync::mpsc::channel();

        // Start worker thread (exclusive ownership of Clipboard)
        let worker_cfg = self.config.clone();
        let worker_detector = self.content_detector.clone();
        let worker_sender = self.event_sender.clone();
        std::thread::spawn(move || {
            if let Err(e) = run_worker(worker_rx, worker_sender, worker_detector, worker_cfg) {
                error!("Clipboard worker exited with error: {}", e);
            } else {
                info!("Clipboard worker exited cleanly");
            }
        });

        // Set global context for the Win32 window thread
        unsafe {
            GLOBAL_CONTEXT = Some(Arc::new(Mutex::new(WindowsMonitorContext {
                event_sender: self.event_sender.clone(),
                content_detector: self.content_detector.clone(),
                config: self.config.clone(),
                window_handle: HWND(0),
                worker_tx: Some(worker_tx),
            })));
        }

        // Start the Win32 message loop thread
        unsafe {
            MESSAGE_LOOP_RUNNING.store(true, Ordering::SeqCst);
            MESSAGE_LOOP_HANDLE = Some(std::thread::spawn(move || {
                if let Err(e) = Self::run_windows_message_loop() {
                    error!("Windows clipboard monitoring failed: {}", e);
                }
                MESSAGE_LOOP_RUNNING.store(false, Ordering::SeqCst);
            }));
        }

        // Control channel (stop)
        tokio::spawn(async move {
            while let Some(cmd) = control_rx.recv().await {
                match cmd {
                    MonitorCommand::Stop => {
                        info!("Received stop command, ending monitoring");
                        Self::stop_windows_monitoring();
                        break;
                    }
                }
            }
        });

        info!("Windows API clipboard monitoring started");
        Ok(event_receiver)
    }

    fn run_windows_message_loop() -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            let class_name = windows::core::w!("ClipMindMonitor");
            let hinstance: HINSTANCE = GetModuleHandleW(None)?.into();

            // Safety: Attempt to unregister any potentially old class first
            let _ = UnregisterClassW(class_name, hinstance);

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(Self::window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: Default::default(),
                hCursor: Default::default(),
                hbrBackground: Default::default(),
                lpszMenuName: windows::core::PCWSTR::null(),
                lpszClassName: class_name,
                hIconSm: Default::default(),
            };

            if RegisterClassExW(&wc) == 0 {
                return Err("Register window class failed".into());
            }

            // Invisible window
            let hwnd = CreateWindowExW(
                Default::default(),
                class_name,
                windows::core::w!("ClipMind Clipboard Monitor"),
                Default::default(),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                hinstance,
                None,
            );

            if hwnd.0 == 0 {
                return Err("Create window failed".into());
            }

            // Record the hwnd
            if let Some(context_arc) = &GLOBAL_CONTEXT {
                if let Ok(mut context) = context_arc.lock() {
                    context.window_handle = hwnd;
                }
            }

            // Register clipboard listener
            if AddClipboardFormatListener(hwnd) == 0 {
                DestroyWindow(hwnd);
                return Err("Register clipboard listener failed".into());
            }

            info!("Windows clipboard listener registered, hwnd: {:?}", hwnd);

            // Message loop
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if !MESSAGE_LOOP_RUNNING.load(Ordering::SeqCst) {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            info!("Windows message loop ended");
            let _ = UnregisterClassW(class_name, hinstance);
            Ok(())
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CLIPBOARDUPDATE => {
                // Only send a pulse to the worker, do not read the clipboard in this thread
                if let Some(ctx_arc) = &GLOBAL_CONTEXT {
                    if let Ok(ctx) = ctx_arc.lock() {
                        if let Some(tx) = &ctx.worker_tx {
                            // If the worker is busy or the channel is full, losing one or two pulses here is fine;
                            // the worker has debouncing to merge events
                            if let Err(e) = tx.send(()) {
                                debug!("Worker pulse send failed (likely stopping): {}", e);
                            }
                        }
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                info!("Window destroyed, removing clipboard listener");
                RemoveClipboardFormatListener(hwnd);
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, _wparam, _lparam),
        }
    }

    fn stop_windows_monitoring() {
        unsafe {
            MESSAGE_LOOP_RUNNING.store(false, Ordering::SeqCst);

            // First destroy the window (this will trigger WM_DESTROY -> PostQuitMessage)
            if let Some(context_arc) = &GLOBAL_CONTEXT {
                // Extract worker_tx and let it drop (this closes the channel, causing the worker to exit)
                let mut maybe_worker_tx: Option<WorkerPulseTx> = None;

                if let Ok(mut context) = context_arc.lock() {
                    let hwnd = context.window_handle;
                    if hwnd.0 != 0 {
                        DestroyWindow(hwnd);
                    } else {
                        PostQuitMessage(0);
                    }
                    // Drop worker_tx, the worker loop will exit after recv() returns Err
                    maybe_worker_tx = context.worker_tx.take();
                    drop(maybe_worker_tx);
                }

                GLOBAL_CONTEXT = None;
            }

            // Wait for the message loop thread to finish
            if let Some(handle) = MESSAGE_LOOP_HANDLE.take() {
                let _ = handle.join();
            }
        }
    }

    /// Stop monitoring - sync version
    pub fn stop_monitoring_sync(&mut self) -> std::result::Result<(), ClipboardError> {
        if !self.is_running {
            return Err(ClipboardError::AccessError("Monitor is not running".to_string()));
        }

        if let Some(control_sender) = &self.control_sender {
            control_sender
                .send(MonitorCommand::Stop)
                .map_err(|e| ClipboardError::AccessError(format!("Failed to send stop command: {}", e)))?;
        }

        self.control_sender = None;
        self.start_time = None;
        self.is_running = false;

        info!("Clipboard monitoring stopped");
        Ok(())
    }

    /// Stop monitoring - async version
    pub async fn stop_monitoring(&mut self) -> std::result::Result<(), ClipboardError> {
        self.stop_monitoring_sync()
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Test mode: manually trigger clipboard event
    #[cfg(test)]
    pub async fn simulate_clipboard_change(&self, content: String) -> Result<(), ClipboardError> {
        let event = self.content_detector.create_event(content, Some("test".to_string()));

        let change = ClipboardChange {
            event,
            is_duplicate: false,
            source_detection_time_ms: 0,
        };

        self.event_sender
            .send(change)
            .map_err(|e| ClipboardError::AccessError(format!("Simulate change failed: {}", e)))?;

        Ok(())
    }
}

// ====== Worker thread: exclusively owns Clipboard, handles debounce, retry, and filtering ======
fn run_worker(
    rx: WorkerPulseRx,
    event_sender: broadcast::Sender<ClipboardChange>,
    content_detector: ContentDetector,
    config: MonitorConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // Exclusive clipboard instance exists only for the lifetime of this thread
    let mut clipboard = Clipboard::new()
        .map_err(|e| ClipboardError::AccessError(format!("Clipboard initialization in worker failed: {}", e)))?;

    let debounce = std::time::Duration::from_millis(config.debounce_ms);
    let mut last_emit = std::time::Instant::now()
        .checked_sub(debounce)
        .unwrap_or_else(std::time::Instant::now);

    // Last content (to avoid duplicates)
    let mut last_content: Option<String> = None;

    info!("Clipboard worker started");

    // Simple loop: wait for pulse, debounce, read, filter, and send events
    loop {
        // Wait for the next pulse; exit if the channel is closed
        if rx.recv().is_err() {
            break; // Channel closed (stop)
        }

        // Debounce: if the time since the last processing is less than debounce, sleep to merge events
        let since = last_emit.elapsed();
        if since < debounce {
            std::thread::sleep(debounce - since);

            // Merge redundant pulses (non-blocking attempt to clear the queue)
            while rx.try_recv().is_ok() {}
        }

        let start_time = std::time::Instant::now();

        // Retry reading
        let content_opt = read_clipboard_with_retry(
            &mut clipboard,
            config.retry_max,
            config.retry_initial_delay_ms,
        );

        last_emit = std::time::Instant::now();

        let mut current_content = match content_opt {
            Some(s) => s,
            None => {
                debug!("Worker: clipboard read failed after retries, skip");
                continue;
            }
        };

        // filter
        if current_content.len() < config.min_content_length {
            debug!("Worker: content too short, ignored: {} chars", current_content.len());
            continue;
        }
        if current_content.len() > config.max_content_length {
            debug!("Worker: content too long, ignored: {} chars", current_content.len());
            continue;
        }
        let trimmed = current_content.trim();
        if trimmed.is_empty() {
            debug!("Worker: empty/whitespace content, ignored");
            continue;
        }

        // ignore duplicates
        if config.ignore_duplicates {
            if let Some(ref last) = last_content {
                if last == &current_content {
                    debug!("Worker: duplicate content, ignored");
                    continue;
                }
            }
        }
        if config.ignore_short_content && trimmed.len() <= 1 {
            debug!("Worker: very short content, ignored: '{}'", trimmed);
            continue;
        }

        // event creation and sending
        let event = content_detector.create_event(current_content.clone(), None);
        last_content = Some(std::mem::take(&mut current_content));

        info!(
            "Clipboard change detected (worker): {} characters, type: {:?}",
            event.content_length, event.content_type
        );

        let detection_ms = start_time.elapsed().as_millis() as u64;
        let change = ClipboardChange {
            event,
            is_duplicate: false,
            source_detection_time_ms: detection_ms,
        };

        if let Err(e) = event_sender.send(change) {
            warn!("Worker: failed to send clipboard event: {}", e);
        } else {
            debug!("Worker: event sent ({}ms)", detection_ms);
        }
    }

    info!("Clipboard worker stopping (channel closed)");
    Ok(())
}

/// read clipboard with retries and exponential backoff
fn read_clipboard_with_retry(
    clipboard: &mut Clipboard,
    retry_max: u32,
    initial_delay_ms: u64,
) -> Option<String> {
    let mut delay = std::time::Duration::from_millis(initial_delay_ms.max(1));
    let max_delay = std::time::Duration::from_millis(200);

    for attempt in 0..retry_max {
        match clipboard.get_text() {
            Ok(s) => return Some(s),
            Err(arboard::Error::ContentNotAvailable) => {
                // content not available (clipboard empty or non-text)
                debug!("Clipboard read: ContentNotAvailable (attempt {}/{})", attempt + 1, retry_max);
            }
            Err(e) => {
                // other errors (e.g., clipboard locked)
                debug!("Clipboard read error (attempt {}/{}): {}", attempt + 1, retry_max, e);
            }
        }

        std::thread::sleep(delay);
        delay = std::cmp::min(delay.saturating_mul(2), max_delay);
    }

    None
}

// ===== 測試 =====
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_monitor_creation() {
        let monitor = ClipboardMonitor::new(None);
        assert!(monitor.is_ok());
        println!("Windows API Monitor created successfully");
    }

    #[tokio::test]
    async fn test_monitor_start_stop() {
        let mut monitor = ClipboardMonitor::new(None).unwrap();

        assert!(!monitor.is_running());

        let _receiver = monitor.start_monitoring().await.unwrap();
        assert!(monitor.is_running());

        tokio::time::sleep(Duration::from_millis(100)).await;

        let result = monitor.stop_monitoring().await;
        assert!(result.is_ok());
        assert!(!monitor.is_running());

        println!("Start-stop test passed");
    }
}
