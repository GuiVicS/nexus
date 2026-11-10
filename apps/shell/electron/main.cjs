'use strict';
// Processo principal do Nexus Shell: janela em tela cheia, ponte segura com o Core e protocolo de arte.
// Nada da página tem acesso a Node; a única ponte é window.nexusBridge (preload.cjs).
const { app, BrowserWindow, ipcMain, protocol, session, net: enet } = require('electron');
const net = require('node:net');
const http = require('node:http');
const path = require('node:path');
const fs = require('node:fs');

app.setName('nexus-shell');

const runtime = process.env.NEXUS_RUNTIME_DIR || path.join(process.env.XDG_RUNTIME_DIR || `/tmp/nexus-${process.getuid()}`, 'nexus');
const RPC_SOCK = path.join(runtime, 'core.sock');
const HTTP_SOCK = path.join(runtime, 'http.sock');
const DEV_URL = process.env.NEXUS_SHELL_URL || '';
const KIOSK = process.env.NEXUS_KIOSK !== '0';

protocol.registerSchemesAsPrivileged([{ scheme: 'nexus-art', privileges: { standard: true, secure: true, supportFetchAPI: true } }]);

let win = null;
let up = false;
let sock = null;
let buf = '';
let retry = 300;

function toRenderer(ch, payload) {
  if (win && !win.isDestroyed()) win.webContents.send(ch, payload);
}

function connectCore() {
  sock = net.createConnection(RPC_SOCK);
  sock.setEncoding('utf8');
  sock.on('connect', () => {
    up = true;
    retry = 300;
    toRenderer('nexus:status', true);
  });
  sock.on('data', (d) => {
    buf += d;
    let i;
    while ((i = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, i);
      buf = buf.slice(i + 1);
      if (line) toRenderer('nexus:message', line);
    }
  });
  const lost = () => {
    if (up) toRenderer('nexus:status', false);
    up = false;
    buf = '';
    setTimeout(connectCore, (retry = Math.min(retry * 1.6, 3000)));
  };
  sock.on('error', () => {});
  sock.on('close', lost);
}

ipcMain.on('nexus:send', (e, text) => {
  if (e.sender !== win?.webContents) return;
  if (typeof text !== 'string' || text.length > 256 * 1024) return;
  if (up && sock && !sock.destroyed) sock.write(text + '\n');
});
ipcMain.handle('nexus:status', (e) => (e.sender === win?.webContents ? up : false));

function artHandler(request) {
  // nexus-art://art/<id>/<kind> → GET /art/<id>/<kind> no socket HTTP do Core
  const u = new URL(request.url);
  if (u.hostname !== 'art') return new Response(null, { status: 404 });
  return new Promise((resolve) => {
    const req = http.request({ socketPath: HTTP_SOCK, path: '/art' + u.pathname, method: 'GET', headers: { Host: '127.0.0.1' } }, (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => resolve(new Response(Buffer.concat(chunks), { status: res.statusCode, headers: { 'content-type': res.headers['content-type'] || 'application/octet-stream', 'cache-control': 'max-age=86400' } })));
    });
    req.on('error', () => resolve(new Response(null, { status: 502 })));
    req.setTimeout(15000, () => req.destroy());
    req.end();
  });
}

function createWindow() {
  win = new BrowserWindow({
    fullscreen: KIOSK,
    kiosk: KIOSK,
    frame: !KIOSK,
    width: 1920,
    height: 1080,
    backgroundColor: '#0b1012',
    show: false,
    autoHideMenuBar: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.cjs'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      webSecurity: true,
      spellcheck: false,
      backgroundThrottling: false,
    },
  });
  win.setMenu(null);
  win.once('ready-to-show', () => win.show());
  win.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  win.webContents.on('will-navigate', (e, url) => {
    const ok = DEV_URL ? url.startsWith(DEV_URL) : url.startsWith('file://');
    if (!ok) e.preventDefault();
  });
  win.webContents.on('render-process-gone', () => setTimeout(() => !win?.isDestroyed() && win.reload(), 800));
  if (DEV_URL) void win.loadURL(DEV_URL);
  else void win.loadFile(path.join(__dirname, '..', 'dist', 'index.html'));
  win.on('closed', () => (win = null));
}

app.on('web-contents-created', (_e, wc) => {
  wc.on('will-attach-webview', (e) => e.preventDefault());
});

// SIGTERM (systemd ao parar a sessão/reiniciar a Shell): sair pelo caminho normal do Electron. Sem isso o
// processo morria no meio e os filhos do Chromium caíam com SIGTRAP → aviso de erro do Ubuntu (apport) na tela.
for (const sig of ['SIGTERM', 'SIGINT', 'SIGHUP']) process.on(sig, () => app.quit());

if (!app.requestSingleInstanceLock()) app.quit();
else {
  app.whenReady().then(() => {
    session.defaultSession.setPermissionRequestHandler((_wc, _perm, cb) => cb(false));
    session.defaultSession.webRequest.onHeadersReceived((d, cb) => cb({ responseHeaders: d.responseHeaders }));
    protocol.handle('nexus-art', artHandler);
    connectCore();
    createWindow();
  });
  app.on('window-all-closed', () => app.quit());
}

void enet;
void fs;
