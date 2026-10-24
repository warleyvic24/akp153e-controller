//! Histórico de desfazer/refazer por instantâneos. Edições em sequência rápida (ex.: digitar
//! um texto) viram um único passo.
use std::time::{Duration, Instant};

const COALESCE_WINDOW: Duration = Duration::from_millis(700);
const MAX_ENTRIES: usize = 100;

pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    last_record: Option<Instant>,
}

impl<T> History<T> {
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            last_record: None,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Registra o estado de ANTES de uma edição.
    pub fn record(&mut self, before: T, now: Instant) {
        self.redo.clear();
        let coalesce = self
            .last_record
            .is_some_and(|t| now.duration_since(t) < COALESCE_WINDOW);
        self.last_record = Some(now);
        if coalesce {
            return;
        }
        if self.undo.len() == MAX_ENTRIES {
            self.undo.remove(0);
        }
        self.undo.push(before);
    }

    /// Devolve o estado anterior e guarda `current` para refazer.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let previous = self.undo.pop()?;
        self.redo.push(current);
        self.last_record = None;
        Some(previous)
    }

    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        self.last_record = None;
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_and_redo_walk_the_timeline() {
        let mut h = History::new();
        let t0 = Instant::now();
        h.record(1, t0);
        h.record(2, t0 + Duration::from_secs(5));
        assert_eq!(h.undo(3), Some(2));
        assert_eq!(h.undo(2), Some(1));
        assert_eq!(h.undo(1), None);
        assert_eq!(h.redo(1), Some(2));
        assert_eq!(h.redo(2), Some(3));
        assert_eq!(h.redo(3), None);
    }

    #[test]
    fn rapid_edits_collapse_into_one_step() {
        let mut h = History::new();
        let t0 = Instant::now();
        for i in 0..10u64 {
            h.record(i, t0 + Duration::from_millis(i * 50));
        }
        assert_eq!(h.undo(99), Some(0));
        assert!(!h.can_undo());
    }

    #[test]
    fn new_edit_discards_redo() {
        let mut h = History::new();
        let t0 = Instant::now();
        h.record(1, t0);
        h.undo(2);
        assert!(h.can_redo());
        h.record(1, t0 + Duration::from_secs(9));
        assert!(!h.can_redo());
    }

    #[test]
    fn history_is_capped() {
        let mut h = History::new();
        let t0 = Instant::now();
        for i in 0..150u64 {
            h.record(i, t0 + Duration::from_secs(i * 2));
        }
        let mut steps = 0;
        while h.undo(0).is_some() {
            steps += 1;
        }
        assert_eq!(steps, MAX_ENTRIES);
    }
}
