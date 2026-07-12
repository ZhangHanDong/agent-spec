#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Default)]
pub struct NoteStore {
    next_id: u64,
    notes: Vec<Note>,
}

impl NoteStore {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            notes: Vec::new(),
        }
    }

    pub fn create_note(&mut self, title: impl Into<String>, body: impl Into<String>) -> Note {
        let note = Note {
            id: self.next_id,
            title: title.into(),
            body: body.into(),
        };
        self.next_id += 1;
        self.notes.push(note.clone());
        note
    }

    pub fn list_notes(&self) -> Vec<Note> {
        self.notes.clone()
    }
}
