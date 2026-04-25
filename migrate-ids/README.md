# Migrate IDs

Objectives:
- Migrate dir IDs and file IDs to a new format
- Ensure that relationship between files and dirs are still maintained

Migrate Dirs Workflow:
- Fetch all dir IDs and store into a HashMap where key is old ID and value is new ID
- Iterate through all dir IDs
- For each dir:
    - Fetch the dir entry
    - Insert a new record copying the old record but with the new ID
    - Update all files under that dir to reference the new dir ID
    - Mark the old dir as deleted

Migrate Files Workflow:
- iterate through all files order by created_at
- For each file:
    - Generate a new ID using the new format
    - Update file ID to the new ID
