const { chromium } = require("playwright");
(async () => {
  const browser = await chromium.launch({
    executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
    args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader", "--disable-gpu-sandbox"],
  });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const lines = [];
  page.on("pageerror", (e) => lines.push(`pageerror: ${e.message}`));
  await page.goto("http://localhost:8111/index.html", { waitUntil: "load" });
  await page.waitForTimeout(30000);

  // Right-click a Goldberg row — the long album title is the one that made
  // the width rule necessary in the first place.
  await page.mouse.move(500, 548);
  await page.waitForTimeout(400);
  await page.mouse.down({ button: "right" });
  await page.waitForTimeout(150);
  await page.mouse.up({ button: "right" });
  await page.waitForTimeout(1200);
  await page.screenshot({ path: "hmenu.png" });

  // …then rest on `Add to playlist` to see whether the dwell still opens it.
  await page.mouse.move(140, 116);
  await page.waitForTimeout(2500);
  await page.screenshot({ path: "hsubmenu.png" });
  console.log(lines.join("\n") || "no page errors");
  await browser.close();
})();
