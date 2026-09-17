const { chromium } = require("playwright");

(async () => {
  const browser = await chromium.launch({
    executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
    args: [
      "--use-gl=swiftshader",
      "--enable-unsafe-swiftshader",
      "--disable-gpu-sandbox",
    ],
  });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const console_lines = [];
  page.on("console", (m) => console_lines.push(`${m.type()}: ${m.text()}`));
  page.on("pageerror", (e) => console_lines.push(`pageerror: ${e.message}`));
  page.on("requestfailed", (r) => console_lines.push(`requestfailed: ${r.url()}`));
  page.on("response", (r) => { if (r.status() >= 400) console_lines.push(`http ${r.status()}: ${r.url()}`); });

  await page.goto("http://localhost:8099/index.html", { waitUntil: "load" });
  // The module is 11MB and iced boots a compositor; give it room.
  await page.waitForTimeout(15000);

  const state = await page.evaluate(() => ({
    booted: window.__booted,
    fatal: window.__fatal || null,
    log: (window.__log || []).slice(-25),
    canvases: [...document.querySelectorAll("canvas")].map((c) => ({
      w: c.width,
      h: c.height,
      css: c.getAttribute("style"),
    })),
  }));

  console.log(JSON.stringify(state, null, 2));
  console.log("--- browser console ---");
  console.log(console_lines.slice(-25).join("\n"));

  await page.screenshot({ path: process.argv[2] || "shot.png" });
  await browser.close();
})();
