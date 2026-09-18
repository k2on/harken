// The tab's title and the platform's media controller — the same two facts
// twice: what is playing, and whether it is. Written here rather than in
// Rust because both are the page's, the way the <audio> element is.
//
// The remote is a mailbox and not a callback. A handler runs on the
// browser's stack, and the app's state lives behind iced's update loop, so
// what a lock-screen button can do is leave a note; the tick that already
// watches for the end of a track collects it. Last one wins: two presses
// inside 50ms are one instruction, which is what a person pressing twice
// meant anyway.
let pending = "";
let wired = false;

function on(name, fn) {
  // An action the browser does not know throws rather than being ignored,
  // and the ones it knows differ per platform, so each is set on its own.
  try {
    navigator.mediaSession.setActionHandler(name, fn);
  } catch (e) {
    /* not on this platform */
  }
}

function wire() {
  if (wired) return;
  wired = true;
  on("play", () => { pending = "play"; });
  on("pause", () => { pending = "pause"; });
  on("stop", () => { pending = "pause"; });
  on("nexttrack", () => { pending = "next"; });
  on("previoustrack", () => { pending = "prev"; });
  on("seekto", (d) => {
    if (d && typeof d.seekTime === "number") pending = "seek:" + d.seekTime;
  });
}

export function announce(title, artist, album, playing) {
  // Windows draws the tab's title in its own window list, so the tab is the
  // one place this has to be right even where there is no media session.
  document.title = title ? (artist ? title + " — " + artist : title) : "harken";
  if (!("mediaSession" in navigator)) return;
  wire();
  navigator.mediaSession.metadata = title
    ? new MediaMetadata({ title: title, artist: artist, album: album })
    : null;
  navigator.mediaSession.playbackState = !title
    ? "none"
    : playing
      ? "playing"
      : "paused";
}

export function position(duration, at) {
  if (!("mediaSession" in navigator)) return;
  if (!navigator.mediaSession.setPositionState) return;
  // The dictionary is validated: a position past the duration, or a duration
  // that is not a number yet, throws rather than being clamped.
  if (!(duration > 0) || !(at >= 0) || at > duration) return;
  try {
    navigator.mediaSession.setPositionState({
      duration: duration,
      position: at,
      playbackRate: 1,
    });
  } catch (e) {
    /* the element has not read enough of the stream to agree yet */
  }
}

export function take_remote() {
  const p = pending;
  pending = "";
  return p;
}
