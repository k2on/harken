/**
 * The two pieces of formatting the screens share.
 *
 * `clock` is `iced/src/main.rs`'s, to the character: `m:ss`, which is how long
 * a piece of music is written down, and an empty string rather than `0:00` for
 * a length nothing knows yet — an unknown duration should read as absent, not
 * as zero.
 */

export function clock(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '';
  const whole = Math.floor(seconds);
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`;
}

/** The same, from the milliseconds the library stores. */
export function clockMs(ms: number | bigint): string {
  return clock(Number(ms) / 1000);
}
