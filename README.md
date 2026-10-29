# Renamer

Batch file renamer (Rust, egui): find / replace in file names with a live preview, remove dots,
`S01E01` → `1x01`, confirmation before every run and undo (Ctrl+Z). Same behaviour as the Python
"Renamer Portable MASTER": names are validated once (empty, invalid characters, Windows reserved
names, trailing dot), nothing is ever overwritten, duplicates are flagged in the preview.

    cargo run --release -- [folder] [find] [replace]

Licence: MIT OR Apache-2.0.
