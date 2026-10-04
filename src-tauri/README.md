# Native architecture

DownTrack uses Tauri 2 for a native desktop shell. The UI never opens a terminal window. Media inspection/download work is performed by a background native process boundary, with stdout/stderr suppressed and structured results returned to the UI.

Security rule: every filesystem read/write command validates the target against the configured authorized roots. A root grants recursive access only below that directory; unrelated drives and folders remain unavailable.

Production packaging should bundle pinned, license-reviewed yt-dlp and FFmpeg sidecars for each supported platform rather than requiring users to install command-line tools themselves.
