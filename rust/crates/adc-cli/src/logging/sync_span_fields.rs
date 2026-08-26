//! `SYNC_EVENT_SPAN_NAME`'s fields, shared by `sync_slots`/`sync_report`/`sync_debug`
//! — each attaches one of these to the span's extensions and decodes it the
//! same way, only the rendering differs.

use tracing::field::{Field, Visit};

#[derive(Default)]
pub struct SpanFields {
    pub resource_type: String,
    pub resource_name: String,
    pub event_type: String,
    pub error: Option<String>,
}

impl SpanFields {
    pub fn display(&self) -> String {
        format!(
            "{} {} \"{}\"",
            self.event_type, self.resource_type, self.resource_name
        )
    }
}

impl Visit for SpanFields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "resource_type" => self.resource_type = value.to_string(),
            "resource_name" => self.resource_name = value.to_string(),
            "event_type" => self.event_type = value.to_string(),
            "error" => self.error = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "resource_type" => self.resource_type = format!("{value:?}"),
            "resource_name" => self.resource_name = format!("{value:?}"),
            "event_type" => self.event_type = format!("{value:?}"),
            "error" => self.error = Some(format!("{value:?}")),
            _ => {}
        }
    }

    fn record_bool(&mut self, _field: &Field, _value: bool) {}
}
