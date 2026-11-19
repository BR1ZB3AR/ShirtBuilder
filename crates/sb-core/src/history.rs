//! Undo / redo with the web app's rules: a keyed edit repeated within 1.2 s (dragging a slider,
//! nudging with the arrow keys) is one history step; an un-keyed edit is always a new step.
//! Time is passed in by the caller (seconds), so this works the same on native and on the web.

pub struct History<T: Clone> {
    past: Vec<T>,
    future: Vec<T>,
    last_key: Option<String>,
    last_time: f64,
    limit: usize,
}

impl<T: Clone> Default for History<T> {
    fn default() -> Self {
        History { past: vec![], future: vec![], last_key: None, last_time: f64::NEG_INFINITY, limit: 150 }
    }
}

impl<T: Clone> History<T> {
    /// Record `before` (the state before an edit). `key = Some(..)` coalesces with the previous
    /// edit of the same key made within 1.2 s.
    pub fn record(&mut self, before: &T, key: Option<&str>, now: f64) {
        let coalesce = key.is_some() && key == self.last_key.as_deref() && now - self.last_time < 1.2;
        if !coalesce {
            self.past.push(before.clone());
            if self.past.len() > self.limit {
                self.past.remove(0);
            }
        }
        self.last_key = key.map(str::to_owned);
        self.last_time = now;
        self.future.clear();
    }

    pub fn undo(&mut self, current: &T) -> Option<T> {
        let prev = self.past.pop()?;
        self.future.push(current.clone());
        self.last_key = None;
        Some(prev)
    }

    pub fn redo(&mut self, current: &T) -> Option<T> {
        let next = self.future.pop()?;
        self.past.push(current.clone());
        self.last_key = None;
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_edits_coalesce_within_window() {
        let mut h = History::default();
        h.record(&0, Some("slider"), 0.0);
        h.record(&1, Some("slider"), 0.5);
        h.record(&2, Some("slider"), 1.0);
        assert_eq!(h.undo(&3), Some(0)); // one step back to before the drag
        assert!(!h.can_undo());
        assert_eq!(h.redo(&0), Some(3));
    }

    #[test]
    fn unkeyed_and_slow_edits_are_separate_steps() {
        let mut h = History::default();
        h.record(&0, None, 0.0);
        h.record(&1, None, 0.1);
        h.record(&2, Some("k"), 0.2);
        h.record(&3, Some("k"), 2.0); // too late to coalesce
        assert_eq!(h.undo(&4), Some(3));
        assert_eq!(h.undo(&3), Some(2));
        assert_eq!(h.undo(&2), Some(1));
        assert_eq!(h.undo(&1), Some(0));
        assert_eq!(h.undo(&0), None);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut h = History::default();
        h.record(&0, None, 0.0);
        let _ = h.undo(&1);
        assert!(h.can_redo());
        h.record(&0, None, 1.0);
        assert!(!h.can_redo());
    }
}
