const { chromium } = require("playwright");
(async () => {
  const browser = await chromium.launch({
    executablePath: "/opt/pw-browsers/chromium-1194/chrome-linux/chrome",
    args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader", "--disable-gpu-sandbox"],
  });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  page.on("pageerror", (e) => console.log("pageerror:", e.message));
  await page.goto("http://localhost:8099/index.html", { waitUntil: "load" });
  await page.waitForTimeout(12000);
  // Squarely on the table row, which is inside the ContextMenu's content.
  await page.mouse.move(600, 200);
  await page.waitForTimeout(200);
  await page.mouse.down({ button: "right" });
  await page.waitForTimeout(150);
  await page.mouse.up({ button: "right" });
  for (const ms of [300, 1200, 3000]) {
    await page.waitForTimeout(ms === 300 ? 300 : ms - 300);
    await page.screenshot({ path: `menu-${ms}.png` });
  }
  await browser.close();
})();
