Download the `.dmg`, open it and drag Terminescu to Applications.

**First launch.** This build is not notarized by Apple yet, so macOS says it "could not verify"
Terminescu. Open it once this way:

1. Double-click Terminescu in Applications and close the warning.
2. Open System Settings › Privacy & Security.
3. Under Security, click **Open Anyway** next to the Terminescu line, then confirm.

macOS remembers the choice. Later versions install through Terminescu › Check for Updates.

Check the download against `SHA256SUMS` with `shasum -a 256 -c --ignore-missing SHA256SUMS`.
