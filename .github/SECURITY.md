# Security

## Reporting Vulnerabilities

If you discover a security issue in deck, please report it responsibly. Do not open a public GitHub issue.

**Email:** Send details to the maintainer via the email address listed on the [GitHub profile](https://github.com/thijsvos).

Please include:
- A description of the vulnerability
- Steps to reproduce it
- The potential impact

I'll acknowledge receipt within 48 hours and aim to release a fix promptly.

## Scope

Security-relevant areas of deck include:
- **Image path resolution** — deck restricts image loads to the presentation's directory to prevent path traversal
- **Sync file handling** — presenter/follower sync uses a user-private directory with atomic writes
- **Terminal escape sequences** — deck writes raw escape sequences for Kitty/Sixel image protocols
