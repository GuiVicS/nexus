import '@fontsource/anta/400.css';
import '@fontsource/cousine/400.css';
import './remote.css';
import { render } from 'solid-js/web';
import { App } from './App';

render(() => <App />, document.getElementById('root')!);

// service worker só existe em contexto seguro (https/localhost); na rede local por http o app instala sem ele
if ('serviceWorker' in navigator && window.isSecureContext) navigator.serviceWorker.register('./sw.js').catch(() => {});
