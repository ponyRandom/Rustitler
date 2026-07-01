# Duplicate File Retry Design

## Context

When a user imports a file whose fingerprint matches a previous output, Rustitler marks the job as `pending` with `pendingReason: duplicateSuspected` and stores a duplicate warning in `failureReason`. The current UI can still show the item as analyzing because a later progress event overwrites the file status. The detail panel also does not make the duplicate recovery path explicit.

## Goal

Let users recover from a suspected duplicate by renaming the file manually and confirming output, using the same copy-and-history path already used for low-confidence pending files.

## Approach

- Preserve terminal and actionable statuses in the frontend store when progress events arrive. A `FileProgress` event should update progress metadata, but it should not turn `pending`, `failed`, `skipped`, `outputCreated`, or `cancelled` files back into `analyzing`.
- Treat duplicate-suspected pending files as manually confirmable. The detail panel will show the existing filename-stem editor and confirm button for these files.
- Keep the backend confirmation command unchanged. `confirm_pending_output` already accepts any `pending` file, validates the edited stem, creates a manual output copy, records history, records undo metadata, and emits `FileOutputCreated`.

## User Experience

The queue row should show the duplicate file as pending, with the duplicate warning in the reason column. Selecting it should show the warning and a filename editor prefilled from the source filename when no recognized title exists. After the user enters a new stem and confirms, the file should become output-created and show the new output path.

## Testing

- Add a store test proving progress events do not overwrite an existing duplicate pending status.
- Add or update UI coverage so a duplicate pending file displays the manual confirmation form.
- Keep existing backend tests for manual confirmation as the behavioral contract.
