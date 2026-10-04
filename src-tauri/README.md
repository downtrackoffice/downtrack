# DownTrack native core

The production desktop shell will expose a small, permission-first command surface to the UI. Filesystem operations must validate every path against the user's configured authorized roots before reading or writing.

Media processing is intentionally isolated behind an internal engine boundary so yt-dlp/FFmpeg can run hidden from the user and report structured progress to the queue.