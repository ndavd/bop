use std::{
    io,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use crate::utils::text::StylizedText;

static FRAMES: [&str; 8] = ["⣷", "⣯", "⣟", "⡿", "⢿", "⣻", "⣽", "⣾"];

pub struct Spinner {
    running: Arc<AtomicBool>,
    pub frame_duration: Duration,
    progress: Arc<AtomicUsize>,
    total: Arc<AtomicUsize>,
    desc: Arc<Mutex<Option<String>>>,
}

impl Default for Spinner {
    fn default() -> Self {
        Spinner {
            running: Arc::new(AtomicBool::new(false)),
            frame_duration: Duration::from_millis(40),
            progress: Arc::new(AtomicUsize::new(0)),
            total: Arc::new(AtomicUsize::new(0)),
            desc: Arc::new(Mutex::new(None)),
        }
    }
}

impl Spinner {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn start(&self, extra_msg: Option<&'static str>) {
        self.running.store(true, Ordering::Relaxed);
        let running = self.running.clone();
        let duration = self.frame_duration;
        let progress = self.progress.clone();
        let total = self.total.clone();
        let desc = self.desc.clone();
        let show_progress = total.load(Ordering::Relaxed) != 0;
        thread::spawn(move || {
            let mut i = 0;
            let len = FRAMES.len();
            let mut desc_len = 0;
            while running.load(Ordering::Relaxed) {
                let desc = desc.lock().unwrap().clone();
                let output = format!(
                    "\r{}{}{}{}",
                    FRAMES[i].to_colored(),
                    extra_msg
                        .and_then(|s| Some(format!(" {s}")))
                        .unwrap_or_default(),
                    if show_progress {
                        format!(
                            " {}/{}",
                            progress.load(Ordering::Relaxed),
                            total.load(Ordering::Relaxed)
                        )
                    } else {
                        String::new()
                    },
                    if let Some(desc) = desc {
                        let old_desc_len = desc_len;
                        desc_len = desc.len();
                        format!(" {desc:<old_desc_len$}")
                    } else {
                        String::new()
                    }
                );
                print!("{output} ");
                io::Write::flush(&mut io::stdout()).unwrap();
                thread::sleep(duration);
                i = (i + 1) % len;
            }
        });
    }
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.set_progress(0);
        self.set_total(0);
        self.set_desc(None);
        self.cleanup();
    }
    pub fn set_progress(&self, value: usize) {
        self.progress.store(value, Ordering::Relaxed);
    }
    pub fn inc_progress(&self) {
        self.progress
            .store(self.progress.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
    }
    pub fn set_total(&self, value: usize) {
        self.total.store(value, Ordering::Relaxed);
    }
    pub fn set_desc(&self, desc: Option<String>) {
        let mut guard = self.desc.lock().unwrap();
        *guard = desc;
    }
    fn cleanup(&self) {
        print!("\r{}\r", " ".repeat(80));
        io::Write::flush(&mut io::stdout()).unwrap();
        thread::sleep(self.frame_duration);
    }
}
