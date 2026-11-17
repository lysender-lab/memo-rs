use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateNoteDto {
    #[validate(length(min = 1))]
    pub name: String,

    #[validate(length(min = 2, max = 10_000))]
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct UpdateNoteDto {
    #[validate(length(min = 2, max = 10_000))]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDto {
    pub id: String,
    pub file_id: String,
    pub content: String,
    pub next_revision: String,
    pub checksum: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteContentDto {
    pub id: String,
    pub file_id: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteRevisionDto {
    pub id: String,
    pub file_id: String,
    pub created_at: i64,
}

// Add a from conversion from NoteDto to NoteContentDto
impl From<NoteDto> for NoteContentDto {
    fn from(note: NoteDto) -> Self {
        NoteContentDto {
            id: note.id,
            file_id: note.file_id,
            content: note.content,
            created_at: note.created_at,
        }
    }
}

// Add a from conversion from NoteDto to NoteRevisionDto
impl From<NoteDto> for NoteRevisionDto {
    fn from(note: NoteDto) -> Self {
        NoteRevisionDto {
            id: note.id,
            file_id: note.file_id,
            created_at: note.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_note_validation() {
        let data = CreateNoteDto {
            name: "Shopping list".to_string(),
            content: "Milk".to_string(),
        };
        assert!(data.validate().is_ok());

        let data = CreateNoteDto {
            name: "Shopping list".to_string(),
            content: "x".repeat(10_000),
        };
        assert!(data.validate().is_ok());

        let data = CreateNoteDto {
            name: "".to_string(),
            content: "Milk".to_string(),
        };
        assert!(data.validate().is_err());

        let data = CreateNoteDto {
            name: "Shopping list".to_string(),
            content: "x".to_string(),
        };
        assert!(data.validate().is_err());

        let data = CreateNoteDto {
            name: "Shopping list".to_string(),
            content: "x".repeat(10_001),
        };
        assert!(data.validate().is_err());
    }
}
