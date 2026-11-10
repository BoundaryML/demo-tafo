import { build } from "esbuild";
import { readFile, writeFile, mkdir } from "node:fs/promises";
const result = await build({
  entryPoints: ["app.jsx"],
  bundle: true,
  write: false,
  minify: true,
  format: "iife",
  platform: "browser",
  target: "es2020",
  define: { "process.env.NODE_ENV": '"production"' },
});
const script = result.outputFiles[0].text.replaceAll("</script", "<\\/script");
const tokens = JSON.parse(await readFile("../design/tokens.json", "utf8"));
const palette = (name) =>
  Object.entries(tokens[name])
    .map(([key, rgb]) => {
      if (
        rgb.length !== 3 ||
        rgb.some((v) => !Number.isInteger(v) || v < 0 || v > 255)
      )
        throw Error("Invalid design color");
      const cssName = key.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
      return `--${cssName}-rgb:${rgb.join(",")};--${cssName}:rgb(var(--${cssName}-rgb));`;
    })
    .join("");
const dimensions = Object.entries(tokens.radius)
  .map(([name, value]) => `--radius-${name}:${value}px;`)
  .join("");
const theme = `:root{${palette("light")}${dimensions}}html[data-theme="dark"]{${palette("dark")}}`;
const css = theme + (await readFile("style.css", "utf8"));
const html = `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Tafo · Activity</title><style>${css}</style></head><body><div id="root"></div><script>${script}</script></body></html>`;
if (Buffer.byteLength(html) > 5 * 1024 * 1024)
  throw Error("Dashboard exceeds Boundary upload limit");
await mkdir("dist", { recursive: true });
await writeFile("dist/activity.html", html);
console.log(
  `Built self-contained dashboard (${Math.round(Buffer.byteLength(html) / 1024)} KiB)`,
);
