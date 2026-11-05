// SPDX-FileCopyrightText: 2026 Minux
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Renders packaging/macos/AppIcon.svg into the AppIcon set of the app
// (needs Playwright with Chromium):
//   node packaging/macos/render-icon.js packaging/macos/AppIcon.svg apps/macos/Atlas/Resources/Assets.xcassets/AppIcon.appiconset
const { chromium } = require('playwright');
const fs = require('fs');
(async () => {
  const [svgPath, outDir] = process.argv.slice(2);
  const svg = fs.readFileSync(svgPath, 'utf8');
  const browser = await chromium.launch();
  for (const size of [16, 32, 64, 128, 256, 512, 1024]) {
    const page = await browser.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 });
    await page.setContent(`<html><body style="margin:0;background:transparent">${svg.replace(/width="1024" height="1024"/, `width="${size}" height="${size}"`)}</body></html>`);
    await page.screenshot({ path: `${outDir}/icon_${size}.png`, omitBackground: true });
    await page.close();
  }
  await browser.close();
})();
