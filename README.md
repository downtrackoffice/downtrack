# DownTrack

Premium media file manager and downloader for desktop.

## Product pillars
- Workspace-first filesystem: explicit authorized root folders, recursive below each root only.
- Media-first workflow: Add Media targets the folder currently open in the explorer.
- Quiet native engine: media tooling runs behind the desktop UI with no terminal window exposed.
- Download queue: progress, speed, ETA, pause/resume/cancel states and history.
- Premium organization: grid/list views, search, breadcrumbs, thumbnails and file operations.
- International architecture: 20-language localization with RTL support.
- Security by construction: native filesystem commands validate targets against authorized roots.

## Stack
React + TypeScript + Vite, Tauri 2 + Rust, yt-dlp + FFmpeg behind the native engine boundary.

Production packaging should bundle pinned, license-reviewed media-engine sidecars for each supported platform.
