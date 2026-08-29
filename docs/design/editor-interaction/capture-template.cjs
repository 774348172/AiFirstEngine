const { app, BrowserWindow } = require("electron");
const fs = require("fs");
const path = require("path");

async function main() {
  await app.whenReady();

  const baseDir = __dirname;
  const input = path.join(baseDir, "ai-editor-interaction-template.html");
  const output = path.join(baseDir, "ai-editor-interaction-template.png");

  const win = new BrowserWindow({
    width: 1600,
    height: 1000,
    show: false,
    backgroundColor: "#0f1115",
    webPreferences: {
      offscreen: true,
    },
  });

  await win.loadFile(input);
  await new Promise((resolve) => setTimeout(resolve, 350));

  const image = await win.webContents.capturePage();
  fs.writeFileSync(output, image.toPNG());

  win.destroy();
  app.quit();
}

main().catch((error) => {
  console.error(error);
  app.exit(1);
});
