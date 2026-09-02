use std::collections::{HashMap, VecDeque};

use crate::model::{Annotation, Moment};

pub const HISTORY_CAP: usize = 100;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    AnnotationCreated {
        frame: usize,
        index: usize,
        annotation: Annotation,
    },
    // Symmetric with AnnotationCreated so a future delete UI can record deletions
    // that the same undo path already knows how to invert.
    #[allow(dead_code)]
    AnnotationDeleted {
        frame: usize,
        index: usize,
        annotation: Annotation,
    },
    MomentCreated {
        index: usize,
        moment: Moment,
    },
    MomentDeleted {
        index: usize,
        moment: Moment,
    },
    MomentNoteChanged {
        index: usize,
        old: String,
        new: String,
    },
    MomentBufferChanged {
        index: usize,
        old: usize,
        new: usize,
    },
}

impl Action {
    /// Frame the action affects, if any. Used for the optional auto-jump on
    /// undo/redo when the affected annotation is on a different frame.
    pub fn affected_frame(&self, moments: &[Moment]) -> Option<usize> {
        match self {
            Action::AnnotationCreated { frame, .. } | Action::AnnotationDeleted { frame, .. } => {
                Some(*frame)
            }
            Action::MomentCreated { moment, .. } | Action::MomentDeleted { moment, .. } => {
                Some(moment.frame_index)
            }
            Action::MomentNoteChanged { index, .. } | Action::MomentBufferChanged { index, .. } => {
                moments.get(*index).map(|m| m.frame_index)
            }
        }
    }

    fn apply(&self, state: &mut HistoryState<'_>) {
        match self {
            Action::AnnotationCreated {
                frame,
                index,
                annotation,
            } => {
                let list = state.annotations.entry(*frame).or_default();
                let insert_at = (*index).min(list.len());
                list.insert(insert_at, annotation.clone());
            }
            Action::AnnotationDeleted { frame, index, .. } => {
                if let Some(list) = state.annotations.get_mut(frame) {
                    if *index < list.len() {
                        list.remove(*index);
                    }
                    if list.is_empty() {
                        state.annotations.remove(frame);
                    }
                }
            }
            Action::MomentCreated { index, moment } => {
                let insert_at = (*index).min(state.moments.len());
                state.moments.insert(insert_at, moment.clone());
            }
            Action::MomentDeleted { index, .. } => {
                if *index < state.moments.len() {
                    state.moments.remove(*index);
                }
            }
            Action::MomentNoteChanged { index, new, .. } => {
                if let Some(m) = state.moments.get_mut(*index) {
                    m.note = new.clone();
                }
            }
            Action::MomentBufferChanged { index, new, .. } => {
                if let Some(m) = state.moments.get_mut(*index) {
                    m.buffer = *new;
                }
            }
        }
    }

    fn revert(&self, state: &mut HistoryState<'_>) {
        match self {
            Action::AnnotationCreated { frame, index, .. } => {
                if let Some(list) = state.annotations.get_mut(frame) {
                    if *index < list.len() {
                        list.remove(*index);
                    }
                    if list.is_empty() {
                        state.annotations.remove(frame);
                    }
                }
            }
            Action::AnnotationDeleted {
                frame,
                index,
                annotation,
            } => {
                let list = state.annotations.entry(*frame).or_default();
                let insert_at = (*index).min(list.len());
                list.insert(insert_at, annotation.clone());
            }
            Action::MomentCreated { index, .. } => {
                if *index < state.moments.len() {
                    state.moments.remove(*index);
                }
            }
            Action::MomentDeleted { index, moment } => {
                let insert_at = (*index).min(state.moments.len());
                state.moments.insert(insert_at, moment.clone());
            }
            Action::MomentNoteChanged { index, old, .. } => {
                if let Some(m) = state.moments.get_mut(*index) {
                    m.note = old.clone();
                }
            }
            Action::MomentBufferChanged { index, old, .. } => {
                if let Some(m) = state.moments.get_mut(*index) {
                    m.buffer = *old;
                }
            }
        }
    }
}

pub struct HistoryState<'a> {
    pub annotations: &'a mut HashMap<usize, Vec<Annotation>>,
    pub moments: &'a mut Vec<Moment>,
}

#[derive(Debug, Default)]
pub struct History {
    undo: VecDeque<Action>,
    redo: VecDeque<Action>,
    cap: usize,
}

impl History {
    pub fn new(cap: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    /// Record a user-initiated action. The action has already been applied to
    /// app state; this only bookkeeps history. Clears the redo stack.
    pub fn record(&mut self, action: Action) {
        self.undo.push_back(action);
        while self.undo.len() > self.cap {
            self.undo.pop_front();
        }
        self.redo.clear();
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[allow(dead_code)]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    #[allow(dead_code)]
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Pop the newest undo entry, revert it against state, push to redo. Returns
    /// the reverted action for callers that want to react (e.g. auto-jump).
    pub fn undo(&mut self, state: &mut HistoryState<'_>) -> Option<Action> {
        let action = self.undo.pop_back()?;
        action.revert(state);
        self.redo.push_back(action.clone());
        while self.redo.len() > self.cap {
            self.redo.pop_front();
        }
        Some(action)
    }

    /// Pop the newest redo entry, re-apply it, push to undo. Returns the action.
    pub fn redo(&mut self, state: &mut HistoryState<'_>) -> Option<Action> {
        let action = self.redo.pop_back()?;
        action.apply(state);
        self.undo.push_back(action.clone());
        while self.undo.len() > self.cap {
            self.undo.pop_front();
        }
        Some(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Annotation, Moment};

    fn rect(x: f32) -> Annotation {
        Annotation::Rect {
            x,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            stroke_color: [255, 0, 0, 255],
            stroke_width: 2.0,
        }
    }

    fn text(s: &str) -> Annotation {
        Annotation::Text {
            x: 0.0,
            y: 0.0,
            text: s.into(),
            font_size: 12.0,
            color: [255, 255, 255, 255],
        }
    }

    fn moment(frame: usize, note: &str, buffer: usize) -> Moment {
        Moment {
            frame_index: frame,
            buffer,
            note: note.into(),
        }
    }

    fn state<'a>(
        anns: &'a mut HashMap<usize, Vec<Annotation>>,
        moms: &'a mut Vec<Moment>,
    ) -> HistoryState<'a> {
        HistoryState {
            annotations: anns,
            moments: moms,
        }
    }

    #[test]
    fn new_history_is_empty() {
        let h = History::new(HISTORY_CAP);
        assert!(!h.can_undo());
        assert!(!h.can_redo());
        assert_eq!(h.undo_len(), 0);
        assert_eq!(h.redo_len(), 0);
    }

    #[test]
    fn undo_on_empty_is_noop() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = Vec::new();
        let mut h = History::new(HISTORY_CAP);
        assert!(h.undo(&mut state(&mut anns, &mut moms)).is_none());
        assert!(anns.is_empty());
        assert!(moms.is_empty());
    }

    #[test]
    fn redo_on_empty_is_noop() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = Vec::new();
        let mut h = History::new(HISTORY_CAP);
        assert!(h.redo(&mut state(&mut anns, &mut moms)).is_none());
    }

    #[test]
    fn annotation_created_round_trip() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = Vec::new();
        let mut h = History::new(HISTORY_CAP);

        // User draws a rect: apply state change, then record.
        anns.entry(3).or_default().push(rect(1.0));
        h.record(Action::AnnotationCreated {
            frame: 3,
            index: 0,
            annotation: rect(1.0),
        });

        h.undo(&mut state(&mut anns, &mut moms)).unwrap();
        assert!(!anns.contains_key(&3), "empty frame removed");
        assert_eq!(h.undo_len(), 0);
        assert_eq!(h.redo_len(), 1);

        h.redo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(anns.get(&3).map(|l| l.len()), Some(1));
        assert_eq!(h.undo_len(), 1);
        assert_eq!(h.redo_len(), 0);
    }

    #[test]
    fn annotation_deleted_round_trip() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        anns.insert(2, vec![rect(1.0), rect(2.0), rect(3.0)]);
        let mut moms: Vec<Moment> = Vec::new();
        let mut h = History::new(HISTORY_CAP);

        let removed = anns.get_mut(&2).unwrap().remove(1);
        h.record(Action::AnnotationDeleted {
            frame: 2,
            index: 1,
            annotation: removed,
        });

        h.undo(&mut state(&mut anns, &mut moms)).unwrap();
        let list = anns.get(&2).unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[1], rect(2.0));

        h.redo(&mut state(&mut anns, &mut moms)).unwrap();
        let list = anns.get(&2).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], rect(1.0));
        assert_eq!(list[1], rect(3.0));
    }

    #[test]
    fn moment_created_round_trip() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = vec![moment(1, "a", 5)];
        let mut h = History::new(HISTORY_CAP);

        moms.push(moment(9, "b", 5));
        h.record(Action::MomentCreated {
            index: 1,
            moment: moment(9, "b", 5),
        });

        h.undo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(moms.len(), 1);
        assert_eq!(moms[0].note, "a");

        h.redo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(moms.len(), 2);
        assert_eq!(moms[1].note, "b");
    }

    #[test]
    fn moment_deleted_round_trip() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = vec![moment(1, "a", 5), moment(2, "b", 5), moment(3, "c", 5)];
        let mut h = History::new(HISTORY_CAP);

        let removed = moms.remove(1);
        h.record(Action::MomentDeleted {
            index: 1,
            moment: removed,
        });

        h.undo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(moms.len(), 3);
        assert_eq!(moms[1].note, "b");

        h.redo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(moms.len(), 2);
        assert_eq!(moms[0].note, "a");
        assert_eq!(moms[1].note, "c");
    }

    #[test]
    fn moment_note_changed_round_trip() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = vec![moment(1, "old", 5)];
        let mut h = History::new(HISTORY_CAP);

        moms[0].note = "new".into();
        h.record(Action::MomentNoteChanged {
            index: 0,
            old: "old".into(),
            new: "new".into(),
        });

        h.undo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(moms[0].note, "old");

        h.redo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(moms[0].note, "new");
    }

    #[test]
    fn moment_buffer_changed_round_trip() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = vec![moment(1, "", 5)];
        let mut h = History::new(HISTORY_CAP);

        moms[0].buffer = 12;
        h.record(Action::MomentBufferChanged {
            index: 0,
            old: 5,
            new: 12,
        });

        h.undo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(moms[0].buffer, 5);

        h.redo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(moms[0].buffer, 12);
    }

    #[test]
    fn text_annotation_round_trip() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = Vec::new();
        let mut h = History::new(HISTORY_CAP);

        anns.entry(0).or_default().push(text("hi"));
        h.record(Action::AnnotationCreated {
            frame: 0,
            index: 0,
            annotation: text("hi"),
        });

        h.undo(&mut state(&mut anns, &mut moms)).unwrap();
        assert!(!anns.contains_key(&0));

        h.redo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(anns.get(&0).unwrap().len(), 1);
        assert_eq!(anns[&0][0], text("hi"));
    }

    #[test]
    fn record_clears_redo_stack() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = Vec::new();
        let mut h = History::new(HISTORY_CAP);

        anns.entry(0).or_default().push(rect(1.0));
        h.record(Action::AnnotationCreated {
            frame: 0,
            index: 0,
            annotation: rect(1.0),
        });
        h.undo(&mut state(&mut anns, &mut moms)).unwrap();
        assert_eq!(h.redo_len(), 1);

        anns.entry(1).or_default().push(rect(2.0));
        h.record(Action::AnnotationCreated {
            frame: 1,
            index: 0,
            annotation: rect(2.0),
        });
        assert_eq!(h.redo_len(), 0);
        assert!(!h.can_redo());
    }

    #[test]
    fn undo_cap_drops_oldest() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let _ = &mut anns;
        let mut h = History::new(HISTORY_CAP);

        for i in 0..101 {
            h.record(Action::MomentBufferChanged {
                index: 0,
                old: i,
                new: i + 1,
            });
        }
        assert_eq!(h.undo_len(), HISTORY_CAP);
        // The oldest entry (old = 0) is dropped; the newest kept has old = 100.
        let popped = h.undo.front().cloned().unwrap();
        match popped {
            Action::MomentBufferChanged { old, .. } => assert_eq!(old, 1),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn redo_cap_drops_oldest() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = (0..HISTORY_CAP + 1).map(|i| moment(i, "", 5)).collect();
        let mut h = History::new(HISTORY_CAP);

        // Fill up: record 101 delete-then-undo cycles isn't the right shape.
        // Instead, seed the redo stack by recording 101 no-ops then undoing all.
        for (i, m) in moms.iter_mut().enumerate().take(HISTORY_CAP + 1) {
            h.record(Action::MomentBufferChanged {
                index: i,
                old: 5,
                new: 6,
            });
            m.buffer = 6;
        }
        // Undo 101 times — the first undo drops nothing from redo (well
        // under cap); by the last, redo should be capped at HISTORY_CAP.
        for _ in 0..HISTORY_CAP + 1 {
            let _ = h.undo(&mut state(&mut anns, &mut moms));
        }
        assert_eq!(h.redo_len(), HISTORY_CAP);
    }

    #[test]
    fn interleaved_undo_redo_survives() {
        let mut anns: HashMap<usize, Vec<Annotation>> = HashMap::new();
        let mut moms: Vec<Moment> = Vec::new();
        let mut h = History::new(HISTORY_CAP);

        anns.entry(5).or_default().push(rect(1.0));
        h.record(Action::AnnotationCreated {
            frame: 5,
            index: 0,
            annotation: rect(1.0),
        });
        moms.push(moment(5, "note", 5));
        h.record(Action::MomentCreated {
            index: 0,
            moment: moment(5, "note", 5),
        });
        anns.entry(5).or_default().push(text("label"));
        h.record(Action::AnnotationCreated {
            frame: 5,
            index: 1,
            annotation: text("label"),
        });

        // Undo all three, then redo all three.
        for _ in 0..3 {
            assert!(h.undo(&mut state(&mut anns, &mut moms)).is_some());
        }
        assert!(!anns.contains_key(&5));
        assert!(moms.is_empty());

        for _ in 0..3 {
            assert!(h.redo(&mut state(&mut anns, &mut moms)).is_some());
        }
        assert_eq!(anns.get(&5).unwrap().len(), 2);
        assert_eq!(moms.len(), 1);
        assert_eq!(moms[0].note, "note");
    }

    #[test]
    fn affected_frame_reads_correct_source() {
        let moms = vec![moment(42, "", 5)];
        let a = Action::AnnotationCreated {
            frame: 7,
            index: 0,
            annotation: rect(1.0),
        };
        assert_eq!(a.affected_frame(&moms), Some(7));

        let a = Action::MomentNoteChanged {
            index: 0,
            old: "".into(),
            new: "x".into(),
        };
        assert_eq!(a.affected_frame(&moms), Some(42));

        let a = Action::MomentDeleted {
            index: 0,
            moment: moment(19, "", 5),
        };
        assert_eq!(a.affected_frame(&moms), Some(19));

        // Out-of-range note change on empty moments returns None.
        let a = Action::MomentNoteChanged {
            index: 3,
            old: "".into(),
            new: "".into(),
        };
        assert_eq!(a.affected_frame(&[]), None);
    }
}
