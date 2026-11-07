// The facts of Help › Privacy and About (add-public-release D7).

/** The points of Help › Privacy. PRIVACY.md says the same; a test keeps them in step. */
export const PRIVACY_POINTS: readonly string[] = [
  "Terminescu sends no telemetry and no crash reports.",
  "Its only automatic network call is the update check, a download of the newest release's latest.json from GitHub at launch and once a day.",
  "The Claude Code hook reads the events Claude Code sends it and the session transcripts on your Mac. Nothing leaves your Mac.",
  "Saved variables, commands, prompts and the layout live in a local database in your user Library. Secret values live only in memory and are never written to disk.",
  "Feedback leaves your Mac only when you send the email or the GitHub issue yourself. The app log goes along only when you tick the box and attach it.",
];

export const NOT_AFFILIATED = "Works with Claude Code. Not affiliated with Anthropic.";
