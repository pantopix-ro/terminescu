let ctx: AudioContext | undefined;

/** A short two-note chime, made with WebAudio so no sound file ships. Off by default. */
export function playChime(): void {
  try {
    ctx ??= new AudioContext();
    const t = ctx.currentTime;
    for (const [i, freq] of [880, 1320].entries()) {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.frequency.value = freq;
      gain.gain.setValueAtTime(0.0001, t + i * 0.12);
      gain.gain.exponentialRampToValueAtTime(0.15, t + i * 0.12 + 0.02);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + i * 0.12 + 0.3);
      osc.connect(gain).connect(ctx.destination);
      osc.start(t + i * 0.12);
      osc.stop(t + i * 0.12 + 0.32);
    }
  } catch {
    // No audio device or no WebAudio: the badge and the status line still show the wait.
  }
}
