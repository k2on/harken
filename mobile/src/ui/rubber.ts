/**
 * The one rubber band, because two things are dragged here and both need a
 * wall.
 *
 * A row can be pushed aside to act on it and the play bar can be pushed aside
 * to see the track next to it, and in both the finger can keep going long
 * after the gesture has decided everything it is going to. Unresisted that is
 * a control sliding off the screen with nothing to say it has finished; a hard
 * clamp is a control that stops dead under a moving thumb, which reads as the
 * gesture having been dropped.
 *
 * So: exact up to `free`, then the remaining `limit - free` spent on an
 * exponential that never quite arrives. `1 - e^-x` rather than a divisor,
 * because a divisor keeps growing — half as fast is still as far as you like
 * given a long enough screen — and what is wanted is something you can lean
 * on. The two halves meet at `free` with the same value, so nothing jumps at
 * the hand-off, and the slope goes smoothly to nothing rather than to a stop.
 */
export function rubber(dx: number, free: number, limit: number): number {
  'worklet';
  const away = Math.abs(dx);
  if (away <= free) return dx;
  const room = Math.max(1, limit - free);
  return Math.sign(dx) * (free + room * (1 - Math.exp(-(away - free) / room)));
}

/**
 * …and the same wall with no free travel at all, for an edge there is nothing
 * beyond: the first track's "previous", the last one's "next". It moves enough
 * to answer the finger and never enough to look like it is going anywhere.
 */
export function wall(dx: number, limit: number): number {
  'worklet';
  return rubber(dx, 0, limit);
}
