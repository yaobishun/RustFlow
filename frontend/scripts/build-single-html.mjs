import { readFile, readdir, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(scriptDir, "..");
const outDir = path.join(frontendRoot, "dist-single");
const generatedHtml = path.join(outDir, "index.html");
const outputHtml = path.join(outDir, "RustFlow.html");

const mimeTypes = {
  ".avif": "image/avif",
  ".gif": "image/gif",
  ".jpeg": "image/jpeg",
  ".jpg": "image/jpeg",
  ".png": "image/png",
  ".svg": "image/svg+xml",
  ".webp": "image/webp",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
};

function resolveBuiltAsset(reference) {
  const relativePath = reference
    .split(/[?#]/, 1)[0]
    .replace(/^\.\//, "")
    .replace(/^\//, "");
  return path.join(outDir, relativePath);
}

function escapedClosingTag(name) {
  return `<${String.fromCharCode(92)}/${name}`;
}

async function replaceAsync(source, pattern, replacer) {
  const matches = [...source.matchAll(pattern)];
  for (const match of matches) {
    const replacement = await replacer(match);
    source = source.replace(match[0], () => replacement);
  }
  return source;
}

async function inlineCssAssets(css, cssPath) {
  const pattern = /url\((['"]?)(?!data:|https?:|#)([^'")]+)\1\)/gi;
  return replaceAsync(css, pattern, async (match) => {
    const assetPath = path.resolve(path.dirname(cssPath), match[2]);
    const mime = mimeTypes[path.extname(assetPath).toLowerCase()];
    if (!mime) return match[0];
    const data = await readFile(assetPath);
    return `url("data:${mime};base64,${data.toString("base64")}")`;
  });
}

await build({
  root: frontendRoot,
  configFile: path.join(frontendRoot, "vite.config.ts"),
  base: "./",
  build: {
    outDir,
    emptyOutDir: true,
    cssCodeSplit: false,
    assetsInlineLimit: Number.MAX_SAFE_INTEGER,
    chunkSizeWarningLimit: 3_000,
    rollupOptions: {
      output: {
        inlineDynamicImports: true,
      },
    },
  },
});

let html = await readFile(generatedHtml, "utf8");

html = await replaceAsync(
  html,
  /<script\b(?=[^>]*\bsrc=["']([^"']+)["'])[^>]*><\/script>/gi,
  async (match) => {
    const code = await readFile(resolveBuiltAsset(match[1]), "utf8");
    const safeCode = code.replace(/<\/script/gi, () => escapedClosingTag("script"));
    return `<script type="module">${safeCode}</script>`;
  },
);

html = await replaceAsync(
  html,
  /<link\b(?=[^>]*\brel=["']stylesheet["'])(?=[^>]*\bhref=["']([^"']+)["'])[^>]*>/gi,
  async (match) => {
    const cssPath = resolveBuiltAsset(match[1]);
    let css = await readFile(cssPath, "utf8");
    css = css.replace(
      /@import\s*(?:url\()?['"]https:\/\/fonts\.googleapis\.com[^'"]+['"]\)?\s*;/gi,
      "",
    );
    css = await inlineCssAssets(css, cssPath);
    const safeCss = css.replace(/<\/style/gi, () => escapedClosingTag("style"));
    return `<style>${safeCss}</style>`;
  },
);

html = html.replace(
  "</head>",
  '    <meta name="rustflow-build" content="single-file" />\n  </head>',
);

const markupShell = html
  .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, "")
  .replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, "");

if (/<script\b[^>]*\bsrc=["'][^"']+["'][^>]*>/i.test(markupShell)) {
  throw new Error("Single-file output still contains an external script or asset reference.");
}
if (/<link\b(?=[^>]*\brel=["']stylesheet["'])[^>]*\bhref=["'][^"']+["'][^>]*>/i.test(markupShell)) {
  throw new Error("Single-file output still contains an external stylesheet reference.");
}
if (/fonts\.googleapis\.com/i.test(html)) {
  throw new Error("Single-file output still depends on Google Fonts.");
}
if ((html.match(/<\/script/gi) ?? []).length !== 1) {
  throw new Error("Inline JavaScript contains an unescaped closing script tag.");
}
if ((html.match(/<\/style/gi) ?? []).length !== 1) {
  throw new Error("Inline CSS contains an unescaped closing style tag.");
}

await writeFile(outputHtml, html, "utf8");
await rm(generatedHtml, { force: true });
await rm(path.join(outDir, "assets"), { recursive: true, force: true });

const remaining = await readdir(outDir);
if (remaining.length !== 1 || remaining[0] !== "RustFlow.html") {
  throw new Error(`Unexpected files remain in dist-single: ${remaining.join(", ")}`);
}

const sizeMiB = Buffer.byteLength(html) / 1024 / 1024;
console.log(`Single HTML generated: ${outputHtml}`);
console.log(`Size: ${sizeMiB.toFixed(2)} MiB`);
