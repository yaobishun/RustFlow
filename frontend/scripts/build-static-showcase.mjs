import { access, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const frontendRoot = path.resolve(scriptDir, "..");
const singleHtml = path.join(frontendRoot, "dist-single", "RustFlow.html");
const outDir = path.join(frontendRoot, "dist-showcase");
const outputHtml = path.join(outDir, "RustFlow全页面静态展示.html");
const profileDir = path.join(frontendRoot, ".showcase-edge-profile");
const apiBase = "http://127.0.0.1:3100/api";
const debugPort = 9333;

const pages = [
  { title: "登录页", route: "/login", description: "系统登录入口与产品能力概览", public: true },
  { title: "综合仪表盘", route: "/dashboard", description: "项目进度、成本、风险与成员负载总览" },
  { title: "项目管理", route: "/projects", description: "项目范围、预算、周期与状态管理" },
  { title: "项目详情", route: "/projects/1", description: "项目指标、成员和里程碑信息" },
  { title: "任务看板", route: "/projects/1/board", description: "任务从待处理到完成的完整协作流程" },
  { title: "任务详情", route: "/tasks/2", description: "任务说明、参与人、评审、评论与依赖关系" },
  { title: "团队负载", route: "/team", description: "团队成员、角色、工时与负载分配" },
  { title: "工时与成本", route: "/costs", description: "工时、支出、实际成本与 EVM 指标" },
  { title: "风险预警", route: "/risks", description: "延期、阻塞、超载与预算风险" },
  { title: "技术方案决策", route: "/decisions", description: "技术方案指标、经济评价和决策结果" },
];

const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

async function firstExisting(paths) {
  for (const candidate of paths) {
    try {
      await access(candidate);
      return candidate;
    } catch {
      // Continue checking the known Edge installation paths.
    }
  }
  throw new Error("未找到 Microsoft Edge，无法生成静态页面快照。");
}

async function waitForDebugger() {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/list`);
      const targets = await response.json();
      const page = targets.find((target) => target.type === "page");
      if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
    } catch {
      // Edge is still starting.
    }
    await sleep(125);
  }
  throw new Error("连接 Edge 调试端口超时。");
}

function createCdpClient(webSocketUrl) {
  const socket = new WebSocket(webSocketUrl);
  let nextId = 0;
  const pending = new Map();

  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (!message.id) return;
    const request = pending.get(message.id);
    if (!request) return;
    pending.delete(message.id);
    if (message.error) request.reject(new Error(message.error.message));
    else request.resolve(message.result);
  });

  const opened = new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });

  return {
    async send(method, params = {}) {
      await opened;
      const id = ++nextId;
      const result = new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
      socket.send(JSON.stringify({ id, method, params }));
      return result;
    },
    close() {
      socket.close();
    },
  };
}

async function waitForPage(client) {
  await sleep(250);
  for (let attempt = 0; attempt < 80; attempt += 1) {
    const result = await client.send("Runtime.evaluate", {
      expression: `document.readyState === "complete" &&
        document.body.innerText.trim().length > 20 &&
        !document.querySelector(".el-skeleton")`,
      returnByValue: true,
    });
    if (result.result.value) {
      await sleep(900);
      return;
    }
    await sleep(150);
  }
  throw new Error("等待页面数据渲染超时。");
}

async function setRoute(client, route, reload = false) {
  await client.send("Runtime.evaluate", {
    expression: `location.hash = ${JSON.stringify(`#${route}`)};${reload ? " location.reload();" : ""}`,
  });
  await waitForPage(client);
}

async function capturePage(client) {
  const dimensions = await client.send("Runtime.evaluate", {
    expression: `({
      width: Math.max(document.documentElement.scrollWidth, document.body.scrollWidth, 1440),
      height: Math.max(document.documentElement.scrollHeight, document.body.scrollHeight, 900)
    })`,
    returnByValue: true,
  });
  const width = Math.min(Math.max(dimensions.result.value.width, 1440), 1920);
  const height = Math.min(Math.max(dimensions.result.value.height, 900), 5000);
  await client.send("Emulation.setDeviceMetricsOverride", {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await sleep(450);
  const screenshot = await client.send("Page.captureScreenshot", {
    format: "png",
    fromSurface: true,
    captureBeyondViewport: true,
    clip: { x: 0, y: 0, width, height, scale: 1 },
  });
  return screenshot.data;
}

function escapeHtml(value) {
  return value.replace(/[&<>"']/g, (character) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  })[character]);
}

function buildShowcase(captures) {
  const navigation = captures
    .map((item, index) => `<a href="#page-${index + 1}">${String(index + 1).padStart(2, "0")} ${escapeHtml(item.title)}</a>`)
    .join("\n");
  const sections = captures
    .map((item, index) => `
      <section class="screen" id="page-${index + 1}">
        <div class="screen-heading">
          <span>${String(index + 1).padStart(2, "0")}</span>
          <div><h2>${escapeHtml(item.title)}</h2><p>${escapeHtml(item.description)}</p></div>
          <code>${escapeHtml(item.route)}</code>
        </div>
        <img src="data:image/png;base64,${item.image}" alt="${escapeHtml(item.title)}页面" />
      </section>`)
    .join("\n");

  return `<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>RustFlow · 全页面静态展示</title>
  <style>
    :root { color: #172033; background: #eef2f7; font-family: "Microsoft YaHei", "PingFang SC", Arial, sans-serif; }
    * { box-sizing: border-box; }
    html { scroll-behavior: smooth; }
    body { margin: 0; }
    .hero { padding: 72px max(28px, calc((100vw - 1240px) / 2)); color: white; background: radial-gradient(circle at 85% 10%, #2f63bc, transparent 34%), linear-gradient(135deg, #10192d, #1a3159); }
    .hero small { color: #85a9ef; font-weight: 700; letter-spacing: 1.5px; }
    .hero h1 { margin: 18px 0 12px; font-size: clamp(34px, 5vw, 62px); }
    .hero p { max-width: 760px; color: #c5d0e2; font-size: 16px; line-height: 1.8; }
    nav { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 30px; }
    nav a { padding: 8px 13px; border: 1px solid #ffffff2c; border-radius: 999px; color: #e8effb; text-decoration: none; font-size: 13px; background: #ffffff0d; }
    main { width: min(1320px, calc(100% - 40px)); margin: 42px auto 80px; }
    .screen { margin: 0 0 52px; padding: 22px; border: 1px solid #dfe5ee; border-radius: 18px; background: white; box-shadow: 0 14px 42px #17203312; scroll-margin-top: 18px; }
    .screen-heading { display: grid; grid-template-columns: 52px 1fr auto; gap: 16px; align-items: center; padding: 0 4px 18px; }
    .screen-heading > span { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 12px; color: white; background: #2161ee; font-weight: 800; }
    .screen-heading h2 { margin: 0 0 5px; font-size: 22px; }
    .screen-heading p { margin: 0; color: #667085; font-size: 13px; }
    .screen-heading code { color: #667085; background: #f2f4f7; border-radius: 7px; padding: 7px 10px; }
    .screen img { display: block; width: 100%; height: auto; border: 1px solid #e1e6ee; border-radius: 10px; background: white; }
    footer { padding: 26px; text-align: center; color: #7b8494; font-size: 13px; }
    @media (max-width: 720px) {
      .hero { padding-block: 46px; }
      main { width: min(100% - 20px, 1320px); }
      .screen { padding: 10px; border-radius: 12px; }
      .screen-heading { grid-template-columns: 44px 1fr; }
      .screen-heading code { display: none; }
    }
  </style>
</head>
<body>
  <header class="hero">
    <small>RUST-POWERED PROJECT INTELLIGENCE</small>
    <h1>RustFlow 全页面静态展示</h1>
    <p>以下内容按照实际使用流程依次展示系统主要页面，所有画面均已内嵌在当前 HTML 中。无需登录、无需启动后端，也不依赖外部图片或样式文件。</p>
    <nav>${navigation}</nav>
  </header>
  <main>${sections}</main>
  <footer>RustFlow · 基于 Rust 的研发团队协同与工程决策支持系统</footer>
</body>
</html>`;
}

await access(singleHtml);
const health = await fetch(`${apiBase}/health`).catch(() => null);
if (!health?.ok) {
  throw new Error("生成静态展示版前需要在 127.0.0.1:3100 启动 RustFlow 后端。");
}

const loginResponse = await fetch(`${apiBase}/auth/login`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ username: "manager", password: "RustFlow123!" }),
});
if (!loginResponse.ok) throw new Error("无法登录演示账号。");
const auth = await loginResponse.json();

const edgePath = await firstExisting([
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
]);
await rm(profileDir, { recursive: true, force: true });
const browser = spawn(edgePath, [
  "--headless=new",
  "--disable-gpu",
  "--hide-scrollbars",
  "--allow-file-access-from-files",
  `--remote-debugging-port=${debugPort}`,
  `--user-data-dir=${profileDir}`,
  "--window-size=1440,900",
  "about:blank",
], { stdio: "ignore", windowsHide: true });

let client;
try {
  const socketUrl = await waitForDebugger();
  client = createCdpClient(socketUrl);
  await client.send("Page.enable");
  await client.send("Runtime.enable");
  await client.send("Page.navigate", { url: `${pathToFileURL(singleHtml).href}#/login` });
  await waitForPage(client);

  const captures = [];
  captures.push({ ...pages[0], image: await capturePage(client) });

  await client.send("Runtime.evaluate", {
    expression: `localStorage.setItem("rustflow_token", ${JSON.stringify(auth.token)});
      localStorage.setItem("rustflow_user", ${JSON.stringify(JSON.stringify(auth.user))});
      localStorage.setItem("rustflow_project_id", "1");`,
  });

  await setRoute(client, pages[1].route, true);
  captures.push({ ...pages[1], image: await capturePage(client) });
  for (const page of pages.slice(2)) {
    await setRoute(client, page.route);
    captures.push({ ...page, image: await capturePage(client) });
    console.log(`Captured: ${page.title}`);
  }

  const showcase = buildShowcase(captures);
  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });
  await writeFile(outputHtml, showcase, "utf8");

  const verification = await readFile(outputHtml, "utf8");
  if ((verification.match(/data:image\/png;base64,/g) ?? []).length !== pages.length) {
    throw new Error("静态展示版未包含全部页面截图。");
  }
  console.log(`Static showcase generated: ${outputHtml}`);
  console.log(`Pages: ${pages.length}; size: ${(Buffer.byteLength(verification) / 1024 / 1024).toFixed(2)} MiB`);
} finally {
  client?.close();
  browser.kill();
  await sleep(600);
  await rm(profileDir, { recursive: true, force: true });
}
