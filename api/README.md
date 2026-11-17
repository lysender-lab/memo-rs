# memo-api: Make Memories

`memo-api` is a simple file storage service and is the backend service for `memo-website`.

It is designed for personal use and not indended for large number of concurrent users.
The goal of the service is to provide an economical way to store and retrieve
files in the cloud.

Uses cases:
- Store personal files and documents
- Online photo album

## Features 

- [x] JSON API endpoints to manage files
- [x] Multi-tenant
- [x] AWS S3
- [x] Turso database
- [x] JWT with Role-based authorization

## Routes

- GET / - API metadata
- GET /health/liveness - liveness health check
- GET /health/readiness - readiness health check
- All routes under `/{dir_type}` require authentication. Valid directory types are photos, videos, documents, and notes.
- GET /{dir_type} - list directories
- POST /{dir_type} - create a directory
- GET /{dir_type}/{dir_id} - get a directory
- PATCH /{dir_type}/{dir_id} - update a directory
- DELETE /{dir_type}/{dir_id} - delete a directory
- POST /{dir_type}/{dir_id}/upload-url - create a pre-signed file upload URL
- GET /{dir_type}/{dir_id}/files - list files
- POST /{dir_type}/{dir_id}/files - create a file record
- GET /{dir_type}/{dir_id}/files/{file_id} - get a file
- DELETE /{dir_type}/{dir_id}/files/{file_id} - delete a file
- GET /{dir_type}/{dir_id}/entries - list note entries
- POST /{dir_type}/{dir_id}/entries - create a note entry
- GET /{dir_type}/{dir_id}/entries/{file_id} - get a note file metadata
- DELETE /{dir_type}/{dir_id}/entries/{file_id} - delete a note entry
- GET /{dir_type}/{dir_id}/entries/{file_id}/revisions - list note revisions metadata
- GET /{dir_type}/{dir_id}/entries/{file_id}/content - get a note content
- GET /{dir_type}/{dir_id}/entries/{file_id}/content?revision=xxx - get a note content from a revision
- PUT /{dir_type}/{dir_id}/entries/{file_id}/content - updates a note content

## Workflow

- Organizations and users a managed by YAAS - https://github.com/lysender-lab/yaas
- Each organizations will have their directory in a unified bucket
- Users will share access to files/notes within the organization
- Files are categorized as photos, documents and videos
- Notes are backed by database but will be treated as files as well
- Files are organized by directories (like albums for photos)
- Each directory have the following sub-directories for image files:
    - orig - original image
    - preview - web optimized image preview
    - thumb - web optimized image thumbnail
- File metadata is collected
  - Content type
  - Size
  - Image dimension for each version
  - Date picture is taken

## Notes

Notes are treated as files although they are actually stored in the database.
Every time you modify a note, a new revision is created.

- Create note: creates a file and a note record
- Read note: returns the latest note revision
- Update note: creates a new note record and updates the file record with updated_at
- Delete note: deletes both the note revisions and the file entry

## Models

Dir:
- id
- org_id
- dir_type (Photos, Videos, Documents, Notes)
- name
- label
- created_at
- updated_at

File:
- id
- org_id
- dir_id
- file_type (file, image, video, note)
- name
- filename
- content_type
- size
- is_image
- img_versions
- img_taken_at
- created_at
- updated_at

Note:
- id
- file_id
- content
- next_revision ('latest' or the next revision's notes.id)
- created_at

