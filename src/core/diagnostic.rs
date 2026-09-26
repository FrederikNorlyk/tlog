use std::backtrace::BacktraceStatus;
use std::fmt::Write;

/// Render operation context and every source once, with an optional captured backtrace.
#[must_use]
pub fn report(error: &anyhow::Error) -> String {
    let messages: Vec<_> = error.chain().map(ToString::to_string).collect();
    // Some dependencies embed their source in Display. Keep the more informative
    // message (including codes), without repeating a message it already contains.
    let mut report = String::new();
    for (index, message) in messages.iter().enumerate() {
        if messages.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other.contains(message)
                && (other.len() > message.len() || other_index < index)
        }) {
            continue;
        }
        if !report.is_empty() {
            report.push_str("\nCaused by: ");
        }
        report.push_str(message);
    }
    if error.backtrace().status() == BacktraceStatus::Captured {
        write!(report, "\n\nStack backtrace:\n{}", error.backtrace())
            .expect("writing to a String is infallible");
    }
    report
}
