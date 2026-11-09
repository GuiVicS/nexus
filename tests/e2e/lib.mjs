// Utilidades dos testes de interface: sobem um Core isolado e falam com ele pelo socket Unix.
import { spawn } from 'node:child_process';
import net from 'node:net';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launch } from './cdp.mjs';

export const ROOT = path.resolve(new URL('../..', import.meta.url).pathname);
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export async function freePort() {
  return new Promise((res) => {
    const s = net.createServer();
    s.listen(0, '127.0.0.1', () => {
      const p = s.address().port;
      s.close(() => res(p));
    });
  });
}

export async function startCore({ demo = false, env = {}, args = [], dir: given } = {}) {
  const dir = given ?? fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-e2e-'));
  const dev = await freePort();
  const remote = await freePort();
  const a = ['--dev', '--dev-http', `127.0.0.1:${dev}`, '--remote-bind', `127.0.0.1:${remote}`, '--runtime-dir', `${dir}/run`, '--data-dir', `${dir}/data`, '--shell-dir', `${ROOT}/apps/shell/dist`, '--remote-dir', `${ROOT}/apps/remote/dist`, '--demo-assets', `${ROOT}/assets/demo`, ...(demo ? ['--demo'] : []), ...args];
  const p = spawn(`${ROOT}/target/debug/nexus-core`, a, { env: { ...process.env, NEXUS_UINPUT: '0', NEXUS_GUIDE_WATCH: '0', NEXUS_CACHE_DIR: `${dir}/cache`, NEXUS_DEMO_DL_SECS: '2', NEXUS_REMOTE_HOST: '127.0.0.1', NEXUS_APPS_DIRS: `${dir}/noapps`, NEXUS_STEAM_ROOT: `${dir}/nosteam`, ...env }, stdio: ['ignore', 'pipe', 'pipe'] });
  let log = '';
  p.stdout.on('data', (d) => (log += d));
  p.stderr.on('data', (d) => (log += d));
  const sock = `${dir}/run/core.sock`;
  for (let i = 0; i < 100 && !fs.existsSync(sock); i++) await sleep(50);
  await sleep(150);
  return {
    dir, dev, remote, sock, p, log: () => log,
    url: `http://127.0.0.1:${dev}/`,
    remoteUrl: `http://127.0.0.1:${remote}/`,
    rpc: (method, params = {}) => rpc(sock, method, params),
    async stop() {
      p.kill('SIGTERM');
      await sleep(250);
      try { fs.rmSync(dir, { recursive: true, force: true }); } catch { /* já removido */ }
    },
  };
}

export function rpc(sock, method, params = {}) {
  return new Promise((resolve, reject) => {
    const s = net.createConnection(sock);
    let buf = '';
    s.on('error', reject);
    s.on('data', (d) => {
      buf += d;
      const i = buf.indexOf('\n');
      if (i >= 0) {
        const m = JSON.parse(buf.slice(0, i));
        s.end();
        m.error ? reject(Object.assign(new Error(m.error.message), m.error)) : resolve(m.result);
      }
    });
    s.write(JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }) + '\n');
  });
}

export async function openUi(core, o = {}) {
  const c = await launch({ url: core.url, w: o.w ?? 1920, h: o.h ?? 1080 });
  const ui = {
    c,
    ev: (e) => c.ev(e),
    key: (...a) => c.key(...a),
    wait: sleep,
    shot: (n) => c.shot(`/tmp/nx-e2e-${n}.png`),
    async until(fn, what, timeout = 8000) {
      const end = Date.now() + timeout;
      let last;
      while (Date.now() < end) {
        try { last = await c.ev(`(${fn})()`); if (last) return last; } catch (e) { last = e.message; }
        await sleep(80);
      }
      throw new Error(`timeout: ${what} (último: ${JSON.stringify(last)})`);
    },
    scr: () => c.ev(`document.getElementById('stage').dataset.scr`),
    focusInfo: () => c.ev(`(()=>{const e=document.querySelector('.f.focus');return e?{a:e.dataset.a||'',k:e.dataset.k||'',id:e.dataset.id||'',t:(e.textContent||'').trim().slice(0,40),cls:e.className}:null})()`),
    overlays: () => c.ev(`[...document.querySelectorAll('#ovs>[data-kind]')].map(e=>e.dataset.kind)`),
    text: (sel) => c.ev(`document.querySelector(${JSON.stringify(sel)})?.textContent?.trim() ?? null`),
    count: (sel) => c.ev(`document.querySelectorAll(${JSON.stringify(sel)}).length`),
    /** varre a tela em "cobrinha" (direita/esquerda e desce nas pontas) até o foco satisfazer `match` */
    async focusOn(match, max = 80, vertical = false) {
      const sig = (f) => (f ? `${f.a}|${f.k}|${f.t}` : '');
      let dir = 'ArrowRight';
      let cur = await ui.focusInfo();
      for (let i = 0; i < max; i++) {
        if (cur && match(cur)) return cur;
        if (vertical) {
          await c.key('ArrowDown', { wait: 60 });
          cur = await ui.focusInfo();
          continue;
        }
        await c.key(dir, { wait: 60 });
        let next = await ui.focusInfo();
        if (sig(next) === sig(cur)) {
          await c.key('ArrowDown', { wait: 60 });
          next = await ui.focusInfo();
          dir = dir === 'ArrowRight' ? 'ArrowLeft' : 'ArrowRight';
          if (sig(next) === sig(cur)) {
            await c.key('ArrowUp', { wait: 60 });
            await c.key('ArrowUp', { wait: 60 });
            next = await ui.focusInfo();
          }
        }
        cur = next;
      }
      throw new Error(`foco não chegou ao elemento esperado (parou em ${JSON.stringify(cur)})`);
    },
    async toProfiles() {
      await ui.until(`()=>document.getElementById('stage').dataset.scr==='profiles'`, 'tela de perfis', 12000);
      await ui.wait(250);
    },
    async pickProfile(name) {
      await ui.toProfiles();
      for (let i = 0; i < 8; i++) await c.key('ArrowLeft', { wait: 40 });
      for (let i = 0; i < 8; i++) {
        const f = await ui.focusInfo();
        if (f && f.a.startsWith('prof:') && f.t.includes(name)) break;
        await c.key('ArrowRight');
      }
      await c.key('Enter');
    },
    async typePin(pin) {
      await ui.until(`()=>document.querySelector('#ovs [data-kind=pin]')`, 'teclado de PIN');
      for (const d of pin) await c.key(d, { wait: 60 });
      await c.key('Enter');
    },
    errors: () => c.errors,
    close: () => c.close(),
  };
  return ui;
}

let failures = 0;
export function check(cond, msg) {
  if (cond) console.log('  ok  ', msg);
  else {
    failures++;
    console.log('  FAIL', msg);
  }
}
export const failed = () => failures;
