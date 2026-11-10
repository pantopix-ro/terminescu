# Privacy

Terminescu runs on your Mac. This note lists every piece of data it reads, keeps or sends.
The app shows the same points under Help › Privacy.

- Terminescu sends no telemetry and no crash reports.
- Its own automatic network call is the update check, a download of the newest release's
  `latest.json` from GitHub at launch and once a day. With the git plugin on and `gh` logged
  in, `gh` asks GitHub for each branch's pull request; `gh` keeps its own login.
- The Claude Code hook reads the events Claude Code sends it and the session transcripts on
  your Mac. Nothing leaves your Mac.
- Saved variables, commands, prompts and the layout live in a local database in your user
  Library. Secret values live only in memory and are never written to disk.
- When a Claude session starts, the Claude plugin keeps a baseline of the files it may change:
  a snapshot of the working tree in a git repository, else a copy of each file before Claude's
  first edit. Baselines stay in Terminescu's folder in your user Library, readable only by you,
  go 7 days after the session ends or past 500 MB (Settings › Claude changes both), and never
  touch the repository.
- Feedback leaves your Mac only when you send the email or the GitHub issue yourself. The app
  log goes along only when you tick the box and attach it.

Terminescu is made by PANTOPIX RO (https://pantopix.ro). Questions about this note:
salut@pantopix.com.
