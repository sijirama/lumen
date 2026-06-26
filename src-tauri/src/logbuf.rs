//INFO: In-memory ring buffer of recent log lines so Lumen can read her OWN
//      runtime output via the `view_runtime_logs` tool (self-debugging). The
//      `applog!` macro (defined in lib.rs) tees every line BOTH to stdout (so
//      the dev terminal still works) and into this buffer, which keeps the last
//      MAX_LINES lines in memory. Nothing is written to disk — logs evaporate on
//      restart, which is the point: it's a live execution trace, not an audit log.

use std::collections::VecDeque;
use std::sync::Mutex;

//INFO: How many recent lines to retain. ~600 covers several full chat turns
//      (each turn logs tool calls, results, timings) without unbounded growth.
const MAX_LINES: usize = 600;

//INFO: (timestamp, line). const-constructible so it can be a plain `static`
//      (same pattern as vision.rs's LAST_SCREENSHOT).
static LOG_BUFFER: Mutex<VecDeque<(String, String)>> = Mutex::new(VecDeque::new());

//INFO: Append a line; timestamp is captured here. Called by the `applog!` macro
//      on every log. A poisoned lock is silently ignored — logging must never
//      panic the thing it's observing.
pub fn push(line: String) {
    let ts = chrono::Local::now().format("%H:%M:%S%.3f").to_string();
    if let Ok(mut buf) = LOG_BUFFER.lock() {
        while buf.len() >= MAX_LINES {
            buf.pop_front();
        }
        buf.push_back((ts, line));
    }
}

//INFO: Return up to `limit` of the most-recent lines, oldest-first, optionally
//      keeping only lines that contain `filter` (case-insensitive). Each line is
//      rendered as "[HH:MM:SS.mmm] <msg>".
pub fn tail(limit: usize, filter: Option<&str>) -> Vec<String> {
    let buf = match LOG_BUFFER.lock() {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let needle = filter.map(|f| f.to_lowercase());
    let mut lines: Vec<String> = buf
        .iter()
        .filter(|(_, msg)| match &needle {
            Some(n) => msg.to_lowercase().contains(n),
            None => true,
        })
        .map(|(ts, msg)| format!("[{}] {}", ts, msg))
        .collect();
    let n = lines.len();
    if n > limit {
        lines = lines.split_off(n - limit);
    }
    lines
}
