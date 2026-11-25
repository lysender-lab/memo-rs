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

## Job Queue

The queue uses the `jobs` table in the main database. Producers enqueue a job
with a type, JSON payload, maximum attempts, and availability time. A single
dispatcher polls for eligible jobs and conditionally claims them before
passing them to available workers. Workers execute handlers concurrently.

Workers are disabled by default. Set `JOB_WORKERS=2` to allow two concurrent
jobs in the API process. The dispatcher recovers expired leases; workers renew
leases while running and record completion or failure. Failed attempts are
retried after a delay until the maximum is reached. Jobs may execute more than
once, so handlers must tolerate retries. Completed and failed jobs remain in
the table; automatic cleanup is not currently scheduled.

The initial `logger_job_handler` accepts a JSON payload such as
`{"message":"Hello"}` and logs the message at info level. Creating a note
enqueues a log job on a best-effort basis: an enqueue failure is logged but
does not undo note creation.

Image uploads create a file record with status `processing` and enqueue a
`create_image_versions` job. The response is returned before image versions
are available. A worker downloads the original, generates and uploads versions,
and marks the file `active`. Failed jobs are retried by the queue; a file stays
in `processing` if all attempts fail or enqueueing fails. Workers must be enabled
for images to be processed.

Deleting a non-note file marks an active file `inactive` and queues a
`delete_file` job. The worker deletes the record and then its storage objects.
Only active files can be deleted. If enqueueing fails, the file remains inactive
without a job; storage failures are retried using the metadata in the job.

## Uploads

**Planned; not yet implemented.** Today, the API signs a direct S3 upload URL
and returns a token containing the upload metadata. File creation trusts that
metadata and assumes the upload succeeded. Upload reservations and abandoned
upload cleanup are not yet wired into the API.

The intended lifecycle uses the `uploads` table as the authoritative reservation:

1. When an upload URL is requested, validate the request and persist a `pending`
   reservation before returning a usable URL. Store the organization and directory
   IDs, original and generated filenames, content type, declared size, and exact
   S3 object key. Cleanup uses the configured bucket; this assumes it remains the
   correct bucket for the reservation's lifetime.
2. Set `created_at` to the current Unix time in seconds and `expires_at` to that
   time plus the upload lifetime. Sign a token containing the upload ID, purpose
   (`upload`), and expiry. The token and S3 PUT URL must expire no later than
   `expires_at`; calculate the URL lifetime from the time remaining. Do not
   return the URL if the reservation could not be persisted or has expired.
   The current token and PUT URL lifetimes differ and must be aligned.
3. After uploading, the client requests file creation with the token. Verify its
   signature, purpose, and expiry; load the reservation; and check its status,
   expiry, organization, and directory against the authorized request. Confirm
   the directory still exists. Use the stored metadata, not client-supplied
   metadata, for the file record. Decide whether to verify the S3 object's
   existence and metadata before finalization.
4. In one database transaction, conditionally consume the pending reservation,
   create the file record, and enqueue image processing if needed. Image
   processing runs separately. A failed transaction leaves the reservation
   available for retry or cleanup. A consumed token cannot create another file.
   Decide whether to retain completed reservations with a file ID if idempotent
   retries of successful requests are needed.
5. A scheduled cleanup finds reservations whose expiry is older than a configured
   grace period. It atomically changes `pending` to `cleaning` before deleting
   the exact stored key, so cleanup and finalization cannot both succeed. A
   `cleaning` reservation must never become finalizable again. Delete the
   reservation only after storage deletion succeeds; retry failures and recover
   interrupted cleanup. Deletion and duplicate cleanup attempts must be
   idempotent. If cleanup uses queue workers, they must be enabled.

The grace period allows time for an upload started before URL expiry to finish.
It is not stored per reservation; changing it affects existing reservations.
It reduces but does not eliminate the risk of a late upload appearing after
cleanup. Stronger guarantees require an upload-duration limit or later
reconciliation. This cleanup is separate from cleanup of file records stuck in
image processing.

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

Upload:
- id
- org_id, dir_id
- original_filename, filename, content_type, size
- storage_key
- status (`pending` or `cleaning`)
- created_at, expires_at (Unix seconds)

Note:
- id
- file_id
- content
- next_revision ('latest' or the next revision's notes.id)
- created_at

Job:
- id (`job_` prefix)
- job_type
- payload (JSON text)
- status (`pending`, `running`, `completed`, or `failed`)
- attempts, max_attempts
- available_at
- lease_token, lease_expires_at (nullable)
- last_error (nullable)
- created_at, updated_at

Job timestamps are Unix seconds. A lease identifies the current claim and
expires if the worker cannot renew it; an expired job can be retried if it has
attempts remaining.
