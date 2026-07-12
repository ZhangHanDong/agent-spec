use requirements_noteapp::NoteStore;

#[test]
fn note_create_adds_note() {
    let mut store = NoteStore::new();

    let note = store.create_note("compiler notes", "capture requirement decisions");

    assert_eq!(note.id, 1);
    assert_eq!(store.list_notes(), vec![note]);
}

#[test]
fn note_list_returns_created_notes() {
    let mut store = NoteStore::new();
    let first = store.create_note("first", "body one");
    let second = store.create_note("second", "body two");

    let notes = store.list_notes();

    assert_eq!(notes, vec![first, second]);
}
