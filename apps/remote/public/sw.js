// Service worker do controle no celular: só existe para o app poder ser instalado (Android/Chrome exigem um com
// "fetch") e abrir rápido. A tela vem da rede quando a TV responde; sem TV, mostra a última versão guardada
// (que então avisa "Sem conexão com o console"). Nunca guarda o que é do Core (/remote/*: hello, ws, upload).
const C = 'nexus-remote-v1';
self.addEventListener('install', (e) => { self.skipWaiting(); e.waitUntil(caches.open(C).then((c) => c.addAll(['./', './manifest.webmanifest', './icon-192.png']))); });
self.addEventListener('activate', (e) => e.waitUntil(caches.keys().then((ks) => Promise.all(ks.filter((k) => k !== C).map((k) => caches.delete(k)))).then(() => self.clients.claim())));
self.addEventListener('fetch', (e) => {
  const u = new URL(e.request.url);
  if (e.request.method !== 'GET' || u.origin !== location.origin || u.pathname.startsWith('/remote/') || u.pathname.startsWith('/roms')) return;
  e.respondWith(
    fetch(e.request)
      .then((r) => { if (r.ok) { const cp = r.clone(); caches.open(C).then((c) => c.put(e.request, cp)); } return r; })
      .catch(() => caches.match(e.request, { ignoreSearch: true }).then((r) => r || caches.match('./'))),
  );
});
