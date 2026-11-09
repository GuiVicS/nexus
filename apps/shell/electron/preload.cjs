'use strict';
const { contextBridge, ipcRenderer } = require('electron');

// Única superfície exposta à página: enviar/receber linhas JSON-RPC do Core.
contextBridge.exposeInMainWorld('nexusBridge', {
  send: (text) => ipcRenderer.send('nexus:send', String(text)),
  onMessage: (cb) => ipcRenderer.on('nexus:message', (_e, m) => cb(m)),
  onStatus: (cb) => ipcRenderer.on('nexus:status', (_e, up) => cb(!!up)),
  status: () => ipcRenderer.invoke('nexus:status'),
});
