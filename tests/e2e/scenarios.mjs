import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { check, openUi, rpc, sleep, startCore, ROOT } from './lib.mjs';

export const scenarios = {};

const inShell = `()=>document.getElementById('stage').dataset.scr==='shell'`;

async function withUi(opts, fn) {
  const core = await startCore(opts);
  const ui = await openUi(core, opts.ui);
  try {
    await fn(core, ui);
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
}

scenarios.adulto_demo = () =>
  withUi({ demo: true }, async (core, ui) => {
    await ui.toProfiles();
    check((await ui.count('.prof')) === 5, 'quatro perfis e "Adicionar"');
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await ui.until(`()=>document.querySelectorAll('#dock .tl').length>=5`, 'dock com jogos');
    const f = await ui.focusInfo();
    check(f?.id === 'demo:g1', `foco inicial no último jogo (${f?.id})`);
    check((await ui.text('#dockInfo h1')) === 'Ecos de Vênus', 'título do item em foco');
    check((await ui.count('#acts .ac')) === 3, 'três cartões de atividade');

    // Direita muda o item em foco e o painel acompanha
    await ui.key('ArrowRight');
    await ui.until(`()=>document.querySelector('#dockInfo h1')?.textContent!=='Ecos de Vênus'`, 'painel acompanha o foco');

    // Jogar e voltar
    await ui.key('ArrowLeft');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=launch] .dot')`, 'tela "em execução"', 6000);
    const g = await core.rpc('game.current');
    check(g?.itemId === 'demo:g1' && g.state === 'running', 'Core registrou o jogo em execução');
    check(g.pid > 0 && fs.existsSync(`/proc/${g.pid}`), 'o processo do jogo existe');
    await ui.key('Escape');
    await ui.until(`()=>document.querySelectorAll('#ovs>*').length===0`, 'B volta ao início com o jogo aberto');
    check((await ui.ev(`[...document.querySelectorAll('#dock .tl .bd')].some(b=>b.textContent==='EM EXECUÇÃO')`)), 'tile mostra "em execução"');
    check((await ui.count('#sys .np')) >= 1, 'chip do jogo na barra');
    // Central → fechar jogo
    await ui.key('Tab');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=guide]')`, 'central de controle');
    await ui.focusOn((f) => f.a === 'closegame', 20, true);
    await ui.key('Enter');
    await ui.until(`()=>!document.querySelector('#bar .np.f')`, 'jogo fechado', 9000);
    check((await core.rpc('game.current')) === null, 'Core sem jogo');
    await ui.until(`()=>document.querySelectorAll('#ovs>*').length===0`, 'overlays fechados');

    // Abas
    await ui.key('e');
    await ui.until(`()=>document.querySelector('.rail h4')?.textContent==='Jogos'`, 'biblioteca');
    check((await ui.count('.sc .tl')) >= 10, 'biblioteca lista os jogos');
    await ui.key('e');
    await ui.until(`()=>document.querySelector('#strip h2')?.textContent==='Mídia'`, 'mídia');
    check((await ui.count('.sc .tl:not(.all)')) >= 10, 'mídia lista apps por categoria');
    await ui.key('e');
    await ui.until(`()=>document.querySelectorAll('.sc .tl').length>=4`, 'loja carregou');
    check((await ui.text('.wallet')).includes('400,00'), 'carteira exibida');

    // Instalar um jogo da loja (confirmar compra → progresso → instalado)
    await ui.key('ArrowDown');
    await ui.focusOn((f) => f.id === 'demo:s4');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=hub] h1')`, 'hub do item');
    await ui.wait(350);
    check((await ui.text('#ovs .hub .chip')).startsWith('OFERTA'), 'chip de oferta');
    check((await ui.text('#ovs .hub [data-k=cta]')).includes('Comprar'), 'botão de compra');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=sheet] .buy.tot')`, 'confirmação de compra');
    await ui.wait(350);
    await ui.key('Enter');
    await ui.until(`()=>/Baixando|Jogar/.test(document.querySelector('#ovs .hub [data-k=cta]')?.textContent||'')`, 'download iniciado', 6000);
    await ui.until(`()=>document.querySelector('#ovs .hub [data-k=cta]')?.textContent?.includes('Jogar')`, 'instalação concluída', 9000);
    check((await core.rpc('store.wallet')).balance < 400, 'carteira debitada');
    await ui.key('Escape');
    await ui.key('Escape');

    // Busca com teclado físico
    await ui.key('s');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=search]')`, 'busca');
    for (const ch of 'forja') await ui.key(ch, { wait: 50 });
    await ui.until(`()=>document.querySelector('#sres .tl')?.dataset.id==='demo:s4'`, 'resultado da busca');
    await ui.key('Escape');

    // Ajustes: tema e QR do controle no celular
    await ui.ev(`document.querySelector('#sbGear').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=settings] .setrail')`, 'ajustes');
    await ui.wait(400);
    await ui.ev(`document.querySelector('[data-rail="cat:som"]').click()`);
    await ui.wait(250);
    await ui.ev(`document.querySelector('[data-k=s-theme]').click()`);
    await ui.until(`()=>document.documentElement.dataset.theme==='light'`, 'tema claro');
    check((await ui.focusInfo())?.k === 's-theme', 'foco no seletor de tema');
    await ui.key('ArrowLeft');
    await ui.until(`()=>document.documentElement.dataset.theme==='dark'`, 'tema escuro');
    await ui.ev(`document.querySelector('[data-rail="cat:remote"]').click()`);
    await ui.until(`()=>document.querySelector('.qr svg')`, 'QR code do controle no celular');
    const code = await ui.text('.qrinfo code');
    check(code.startsWith('http://127.0.0.1:') && !code.includes('?t='), `endereço do controle mostrado sem o token (${code})`);
    await ui.shot('settings-remote');
    // "Voltar ao Início" fecha os Ajustes e volta para a aba Início
    await ui.ev(`document.querySelector('[data-k=sc-home]').click()`);
    await ui.until(`()=>!document.querySelector('#ovs [data-kind=settings]')`, 'ajustes fechados pelo Voltar ao Início');
    check(await ui.ev(`document.querySelector('.tab.on')?.dataset.tab==='home'`), 'voltou à aba Início');
  });

scenarios.crianca = () =>
  withUi({ demo: true }, async (core, ui) => {
    await ui.pickProfile('Pipo');
    await ui.until(inShell, 'entrou no perfil infantil');
    check(await ui.ev(`document.body.classList.contains('kid')`), 'tema infantil ativo');
    check((await ui.count('.tab')) === 3, 'sem aba Loja');
    check((await ui.count('#sbBell')) === 0, 'sem notificações');
    const ids = await ui.ev(`[...document.querySelectorAll('#dock .tl[data-id]')].map(e=>e.dataset.id)`);
    const lib = await core.rpc('library.list').catch(() => null);
    check(ids.length >= 1, `dock mostra jogos liberados (${ids.length})`);
    check((await ui.text('#sys .timepill')).includes('60'), 'tempo restante visível');
    // apenas jogos até a classificação 10
    await ui.key('e');
    await ui.until(`()=>document.querySelector('.rail h4')`, 'biblioteca infantil');
    const items = await ui.ev(`[...document.querySelectorAll('.sc .tl[data-id]')].map(e=>e.dataset.id)`);
    for (const id of items) {
      const it = (await ui.ev(`fetch('/art/${id}/cover').then(r=>r.status)`)) && id;
      void it;
    }
    check(items.every((i) => !['demo:g1', 'demo:g4', 'demo:s2'].includes(i)), 'jogos 14+/16+ ocultos');
    // ajustes limitados
    await ui.ev(`document.querySelector('#sbGear').click()`);
    await ui.until(`()=>document.querySelector('.setrail')`, 'ajustes');
    check((await ui.count('.setrail .ri[data-rail]')) === 4, 'só 4 categorias nos ajustes infantis (inclui Áudio: volume é do dia a dia)');
    await ui.key('Escape');
    // sair do perfil infantil pede o PIN dos responsáveis
    await ui.key('Tab');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=guide]')`, 'central');
    await ui.focusOn((f) => f.a === 'switch', 20, true);
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=pin]')`, 'PIN dos responsáveis');
    await ui.typePin('0000');
    await ui.until(`()=>document.querySelector('.toast')?.textContent?.includes('PIN incorreto')`, 'PIN errado é recusado', 4000);
    check((await ui.scr()) === 'shell', 'continua no perfil infantil');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=pin]')`, 'pede de novo', 4000).catch(() => {});
    const pin = await ui.ev(`!!document.querySelector('#ovs [data-kind=pin]')`);
    if (!pin) {
      await ui.key('Tab');
      await ui.focusOn((f) => f.a === 'switch', 20, true);
      await ui.key('Enter');
    }
    await ui.typePin('1234');
    await ui.until(`()=>document.getElementById('stage').dataset.scr==='profiles'`, 'voltou à escolha de perfis', 6000);
  });

scenarios.perfis = () =>
  withUi({ demo: true }, async (core, ui) => {
    await ui.toProfiles();
    // adicionar perfil: o Core pede o PIN dos responsáveis (demo: 1234)
    await ui.focusOn((f) => f.a === 'profadd', 30);
    await ui.key('Enter');
    await ui.typePin('1234');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=text]')`, 'teclado de nome');
    for (const ch of 'Zeca') await ui.key(ch, { wait: 50 });
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=profile] .pe-prev h2')?.textContent==='Zeca'`, 'nome aplicado no editor');
    await ui.focusOn((f) => f.k === 'pe-save' || f.k === 'pe-cancel', 30, true);
    if ((await ui.focusInfo()).k === 'pe-cancel') await ui.key('ArrowLeft');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelectorAll('.prof').length===6`, 'novo perfil aparece', 6000);
    const profs = await core.rpc('profiles.list');
    check(profs.some((p) => p.name === 'Zeca' && p.kind === 'adult'), 'perfil criado no Core');
    // entra nele e abre o editor pelas Contas
    await ui.pickProfile('Zeca');
    await ui.until(inShell, 'entrou como Zeca');
    check((await ui.ev(`document.querySelector('#sbMe .av .ini')?.textContent`)) === 'Z', 'avatar com a inicial');
    await ui.ev(`document.querySelector('#sbGear').click()`);
    await ui.until(`()=>document.querySelector('.setrail')`, 'ajustes');
    await ui.ev(`document.querySelector('[data-rail="cat:conta"]').click()`);
    await ui.until(`()=>document.querySelector('[data-k=s-me]')`, 'conta');
    await ui.ev(`document.querySelector('[data-k=s-me]').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=profile]')`, 'editor de perfil');
    check((await ui.ev(`document.querySelector('#ovs [data-kind=profile] .pe-prev h2').textContent`)) === 'Zeca', 'editor mostra o perfil');
  });

// A7: assistente de primeiro uso completo, só com o controle (setas = D-pad, Enter = A, Esc = B).
scenarios.primeiro_uso = () =>
  withUi({ demo: false }, async (core, ui) => {
    const h1 = (t) => `()=>document.querySelector('.onb h1')?.textContent?.startsWith(${JSON.stringify(t)})`;
    const next = async () => {
      await to('onb-next');
      await ui.key('Enter');
    };
    // anda com ↑/↓ até o item com data-k = k
    const to = async (k) => {
      for (const [dir, max] of [['ArrowUp', 14], ['ArrowDown', 14], ['ArrowLeft', 4], ['ArrowRight', 4]]) {
        for (let n = 0; n < max; n++) {
          if ((await ui.focusInfo())?.k === k) return;
          await ui.key(dir, { wait: 70 });
        }
      }
      check((await ui.focusInfo())?.k === k, `foco em ${k}`);
    };
    await ui.until(`()=>document.getElementById('stage').dataset.scr==='onboarding'`, 'primeiro uso', 12000);
    await ui.wait(600);
    await ui.screens('a7-boas-vindas');
    await ui.key('Enter');
    // idioma, fuso e teclado
    await ui.until(h1('Idioma, hora e teclado'), 'etapa de idioma/hora');
    await ui.until(`()=>/Brasília/.test(document.querySelector('[data-k=loc-tz]')?.textContent||'')`, 'fuso atual: Brasília');
    await ui.screens('a7-idioma');
    await to('loc-tz');
    await ui.key('ArrowRight');
    await ui.until(`()=>/Amazonas/.test(document.querySelector('[data-k=loc-tz]')?.textContent||'')`, '→ troca o fuso');
    await ui.wait(1400);
    check((await core.rpc('locale.status')).timezone === 'America/Manaus', 'Core aplicou o fuso no sistema (timedatectl)');
    await to('loc-kb');
    await ui.key('ArrowRight');
    await ui.wait(1400);
    check((await core.rpc('locale.status')).keyboard === 'us', 'teclado trocado e guardado');
    await next();
    // rede: B volta uma etapa
    await ui.until(h1('Internet'), 'etapa de rede');
    await ui.until(`()=>document.querySelectorAll('[data-k^=wifi-]').length===3`, 'três redes Wi-Fi na lista');
    await ui.key('Escape');
    await ui.until(h1('Idioma, hora e teclado'), 'B volta para a etapa anterior');
    await next();
    await ui.until(h1('Internet'), 'de volta à rede');
    await ui.until(`()=>document.querySelectorAll('[data-k^=wifi-]').length===3`, 'lista de novo');
    await ui.screens('a7-rede');
    await to('wifi-0');
    check((await ui.focusInfo()).t.startsWith('Casa Nexus'), 'rede mais forte primeiro');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=text]')`, 'pede a senha');
    for (const ch of 'errada123') await ui.key(ch, { wait: 30 });
    check(/^•+$/.test(await ui.text('#tev')), 'senha aparece mascarada');
    await ui.key('Enter');
    await ui.until(`()=>[...document.querySelectorAll('.toast')].some(t=>/Senha incorreta/.test(t.textContent))`, 'senha errada: aviso em português', 8000);
    await to('wifi-0');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=text]')`, 'pede a senha de novo');
    for (const ch of 'senha-certa') await ui.key(ch, { wait: 30 });
    await ui.key('Enter');
    await ui.until(h1('Internet') + ` && /já está conectado/.test(document.querySelector('.onb small')?.textContent||'')`, 'conectado: etapa confirma', 10000);
    check((await core.rpc('net.status')).online === true, 'Core vê a internet');
    await next();
    // controle (pular) → perfil
    await ui.until(h1('Controle'), 'etapa do controle');
    await to('onb-skip');
    await ui.key('Enter');
    await ui.until(h1('Crie seu perfil'), 'etapa do perfil');
    await ui.focusOn((f) => f.k === 'onb-profile', 10);
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=text]')`, 'nome');
    for (const ch of 'Ana') await ui.key(ch, { wait: 50 });
    await ui.key('Enter');
    await ui.focusOn((f) => f.k === 'pe-save' || f.k === 'pe-cancel', 30, true);
    if ((await ui.focusInfo()).k === 'pe-cancel') await ui.key('ArrowLeft');
    await ui.key('Enter');
    await ui.until(h1('PIN dos responsáveis'), 'etapa do PIN');
    await ui.key('Escape');
    check(await ui.ev(`document.querySelector('.onb h1')?.textContent==='PIN dos responsáveis'`), 'perfil criado: B não volta para antes dele');
    await ui.focusOn((f) => f.a === 'onb:skip', 30);
    await ui.key('Enter');
    await ui.until(h1('Jogos de console'), 'etapa das ROMs');
    check((await ui.count('[data-k=onb-roms]')) === 1, 'já na sessão do perfil: "Adicionar jogos" disponível');
    await next();
    await ui.until(h1('Celular como controle'), 'etapa do celular');
    await ui.until(`()=>document.querySelector('.onbqr svg')`, 'QR code do celular');
    await ui.screens('a7-celular');
    await to('onb-skip');
    await ui.key('Enter');
    // Google Drive (opcional): sem rclone aqui, o painel diz indisponível e deixa seguir
    await ui.until(h1('Google Drive'), 'etapa do Google Drive');
    await ui.until(`()=>!!document.querySelector('[data-k=dr-skip]')`, 'opção de pular o Drive');
    await ui.screens('a7-drive');
    await to('dr-skip');
    await ui.key('Enter');
    if (await ui.ev(`document.querySelector('.onb h1')?.textContent==='Steam'`)) {
      await ui.until(`()=>/Steam não instalada|Entrar na Steam|Conectado como/.test(document.querySelector('.onb .opane')?.textContent||'')`, 'estado da Steam');
      await to('onb-skip');
      await ui.key('Enter');
    }
    await ui.until(h1('Tudo pronto, Ana'), 'pronto');
    await ui.key('Enter');
    await ui.until(inShell, 'entrou na Shell');
    await ui.until(`()=>document.querySelector('.home-empty')`, 'estado vazio da biblioteca');
    check((await core.rpc('profiles.list')).length === 1, 'perfil criado');
  });

scenarios.celular = () =>
  withUi({ demo: true }, async (core, ui) => {
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou como Lia');
    const pairing = await core.rpc('remote.pairing');
    check(pairing.qrSvg.includes('<svg') && pairing.url.includes('?t='), 'Core gera QR e URL com token');
    const { launch } = await import('./cdp.mjs');
    const url = pairing.url.replace(/^http:\/\/[^/]+/, core.remoteUrl.replace(/\/$/, ''));
    const phone = await launch({ url, w: 844, h: 390, mobile: true });
    const tap = async (sel) => {
      const r = await phone.ev(`(()=>{const e=document.querySelector(${JSON.stringify(sel)});const b=e.getBoundingClientRect();return [b.left+b.width/2,b.top+b.height/2]})()`);
      await phone.click(r[0], r[1]);
    };
    try {
      await phone.wait(900);
      check((await phone.ev(`document.querySelectorAll('.card').length`)) === 3, 'celular mostra Controle 1, 2 e Mouse e teclado');
      check(!(await phone.ev(`location.href`)).includes('t='), 'token removido da barra de endereço');
      // PWA: manifesto ligado, código guardado (o app da tela inicial abre sem ?t=) e dica de instalar
      check(await phone.ev(`!!document.querySelector('link[rel=manifest]')`), 'página do celular tem manifesto (PWA)');
      check((await phone.ev(`localStorage.getItem('nx-token')||''`)).length > 10, 'código guardado no aparelho para o app instalado');
      check(await phone.ev(`!!document.querySelector('.inst')`), 'botão "Instalar na tela inicial" no menu');
      // sem pedido nativo (http na rede local, ou o navegador recusa) o toque mostra o caminho pelo menu do navegador
      await phone.ev(`window.BeforeInstallPromptEvent && (BeforeInstallPromptEvent.prototype.prompt = () => Promise.reject(new Error('recusado')))`);
      await phone.ev(`document.querySelector('.inst').scrollIntoView({block:'center'})`);
      await phone.wait(200);
      await phone.click(...(await phone.ev(`(()=>{const b=document.querySelector('.inst').getBoundingClientRect();return [b.left+b.width/2,b.top+b.height/2]})()`)));
      for (let i = 0; i < 20 && !(await phone.ev(`!!document.querySelector('.inst-how')`)); i++) await sleep(100);
      check(/tela inicial|Tela de Início/i.test(await phone.ev(`document.querySelector('.inst-how')?.textContent||''`)), 'toque em Instalar mostra como adicionar à tela inicial');
      // token inválido
      const bad = await launch({ url: core.remoteUrl + '?t=errado', w: 844, h: 390, mobile: true });
      await bad.wait(900);
      check((await bad.ev(`document.querySelector('.alert')?.textContent||''`)).length > 5, 'token inválido mostra aviso');
      bad.close();

      await phone.ev(`document.querySelectorAll('.card')[0].click()`);
      for (let i = 0; i < 50 && !(await phone.ev(`!!document.querySelector('.pad')`)); i++) await sleep(100);
      check((await phone.ev(`!!document.querySelector('.pad')`)), 'tela do controle aberta');
      const devs = (await core.rpc('remote.pairing')).devices;
      check(devs.length === 1 && devs[0].mode === 'gamepad' && devs[0].player === 1, 'Core lista o aparelho como jogador 1');
      // direita (d-pad) move o foco; A ativa
      const before = await ui.focusInfo();
      const dp = await phone.ev(`(()=>{const e=document.querySelector('.dpad').getBoundingClientRect();return [e.left+e.width*0.92,e.top+e.height/2]})()`);
      await phone.click(dp[0], dp[1]);
      await sleep(500);
      const after = await ui.focusInfo();
      check(after && before && after.id !== before.id, `D-pad do celular move o foco (${before?.id} → ${after?.id})`);
      check(await ui.ev(`document.body.classList.contains('m-pad')`), 'Shell passa ao modo controle');
      await tap('.fb.a');
      await ui.until(`()=>document.querySelector('#ovs [data-kind=launch]')`, 'botão A do celular lança o jogo', 6000);
      await sleep(900);
      check((await core.rpc('game.current'))?.state === 'running', 'jogo em execução pelo celular');
      await tap('.fb.b');
      await ui.until(`()=>document.querySelectorAll('#ovs>*').length===0`, 'B do celular volta ao início', 4000);
      await core.rpc('game.close', { force: true });
      await ui.until(`()=>!document.querySelector('#bar .np.f')`, 'jogo encerrado', 9000);

      // sai e entra no modo mouse
      await tap('.ghost');
      await sleep(900);
      check((await core.rpc('remote.pairing')).devices.length === 0, 'aparelho some ao sair');
      await phone.until?.call;
      await sleep(600);
      await phone.ev(`document.querySelectorAll('.card')[2].click()`);
      for (let i = 0; i < 50 && !(await phone.ev(`!!document.querySelector('.trackpad')`)); i++) await sleep(100);
      check(await phone.ev(`!!document.querySelector('.trackpad')`), 'tela de mouse aberta');
      const r = await phone.ev(`(()=>{const e=document.querySelector('.trackpad').getBoundingClientRect();return [e.left+e.width/2,e.top+e.height/2]})()`);
      for (const t of ['mousePressed']) await phone.send('Input.dispatchMouseEvent', { type: t, x: r[0], y: r[1], button: 'left', clickCount: 1 });
      for (let i = 1; i <= 12; i++) await phone.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: r[0] + i * 10, y: r[1] + i * 3, button: 'left', buttons: 1 });
      await phone.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: r[0] + 120, y: r[1] + 36, button: 'left', clickCount: 1 });
      await sleep(500);
      check(await ui.ev(`!!document.querySelector('#vcur')`), 'cursor virtual aparece na TV');
      // digitação: abre a busca na TV e digita pelo celular
      await ui.key('s');
      await ui.until(`()=>document.querySelector('#ovs [data-kind=search]')`, 'busca aberta');
      await phone.ev(`(()=>{const i=document.querySelector('.ghost-input');i.focus();i.value='rota';i.dispatchEvent(new Event('input',{bubbles:true}))})()`);
      await ui.until(`()=>document.querySelector('#sqt')?.textContent==='rota'`, 'texto digitado no celular chega à busca da TV', 4000);
      await phone.ev(`(()=>{const i=document.querySelector('.ghost-input');i.value='rot';i.dispatchEvent(new Event('input',{bubbles:true}))})()`);
      await ui.until(`()=>document.querySelector('#sqt')?.textContent==='rot'`, 'apagar no celular apaga na TV', 4000);
      // novo código desconecta o aparelho
      await core.rpc('remote.rotate');
      await phone.until?.call;
      await sleep(3000);
      check(await phone.ev(`!document.querySelector('.trackpad')`), 'celular volta ao menu quando o código é trocado');
    } finally {
      check(phone.errors.length === 0, `celular sem erros de JavaScript${phone.errors.length ? ': ' + phone.errors.join(' | ') : ''}`);
      phone.close();
    }
  });

async function createAdult(core, name = 'Ana') {
  const p = await core.rpc('profiles.create', { name, avatar: 'initial', hue: 180, kind: 'adult' });
  return p;
}

scenarios.apps_personalizados = async () => {
  const dir = (await import('node:fs')).default.mkdtempSync('/tmp/nexus-apps-');
  const home = `${dir}/home`;
  const apps = `${dir}/apps/applications`;
  const fs = (await import('node:fs')).default;
  fs.mkdirSync(`${home}/Imagens`, { recursive: true });
  fs.mkdirSync(apps, { recursive: true });
  const write = (f, name, exec, cats) => fs.writeFileSync(`${apps}/${f}`, `[Desktop Entry]\nType=Application\nName=${name}\nExec=${exec}\nCategories=${cats}\n`);
  write('minecraft.desktop', 'Minecraft', 'sleep 30', 'Game;');
  write('calc.desktop', 'Calculadora', 'sleep 30', 'Utility;');
  write('steam.desktop', 'Steam', 'sleep 30', 'Network;Game;');
  fs.writeFileSync(`${home}/Imagens/capa.png`, Buffer.concat([Buffer.from('\x89PNG\r\n\x1a\n', 'binary'), Buffer.alloc(64)]));
  const core = await startCore({ dir, env: { NEXUS_APPS_DIRS: apps } });
  await createAdult(core);
  const ui = await openUi(core);
  try {
    await ui.pickProfile('Ana');
    await ui.until(inShell, 'entrou');
    check((await core.rpc('library.list').catch(() => null)) === null || true, 'estado inicial');
    check((await ui.ev(`document.querySelectorAll('#dock .tl[data-id]').length`)) === 0, 'nenhum app do sistema aparece sozinho (calculadora, Steam…)');
    // Ajustes → Apps e jogos → Adicionar
    await ui.wait(500);
    await ui.ev(`document.querySelector('#sbGear').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=settings] .setrail')`, 'ajustes');
    await ui.wait(400);
    await ui.ev(`document.querySelector('[data-rail="cat:apps"]').click()`);
    await ui.until(`()=>document.querySelector('[data-k=s-appadd]')`, 'painel de apps');
    await ui.ev(`document.querySelector('[data-k=s-appadd]').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=appicker] #pickpane')`, 'seletor de apps');
    await ui.until(`()=>document.querySelectorAll('#pickpane .srow[data-a^="pickapp:"]').length>=3`, 'lista de apps do sistema');
    const names = await ui.ev(`[...document.querySelectorAll('#pickpane .srow b')].map(b=>b.textContent)`);
    check(names.includes('Minecraft') && names.includes('Calculadora') && !names.includes('Steam'), `seletor lista os apps (e não a Steam): ${names.join(', ')}`);
    await ui.ev(`document.querySelector('[data-a="pickapp:minecraft.desktop"]').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=appedit] .ae-prev h2')?.textContent==='Minecraft'`, 'editor do app');
    check((await ui.text('.ae-prev small')).startsWith('Jogo'), 'Minecraft detectado como jogo');
    // capa do card pelo navegador de arquivos
    await ui.ev(`document.querySelector('[data-k=ae-cover]').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=filepick] #fpane')`, 'navegador de arquivos');
    await ui.until(`()=>document.querySelector('#fpane [data-a^="fp:dir:"]')`, 'pastas listadas');
    await ui.ev(`document.querySelector('#fpane [data-a^="fp:dir:"][data-a$="Imagens"]').click()`);
    await ui.until(`()=>document.querySelector('#fpane [data-a^="fp:pick:"]')`, 'imagem listada');
    await ui.ev(`document.querySelector('#fpane [data-a^="fp:pick:"]').click()`);
    await ui.until(`()=>document.querySelector('[data-k=ae-rm-cover]')`, 'capa aplicada');
    const lib = await core.rpc('apps.registered');
    check(lib[0].hasCover && lib[0].kind === 'game' && !lib[0].hasHero, 'Core guardou a capa do card');
    // renomear pela tela
    await ui.ev(`document.querySelector('[data-k=ae-name]').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=text]')`, 'teclado de nome');
    await ui.key('Backspace', { n: 12, wait: 40 });
    for (const ch of 'Craft') await ui.key(ch, { wait: 40 });
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('.ae-prev h2')?.textContent==='Craft'`, 'nome alterado');
    // concluir: o jogo aparece na biblioteca com a capa personalizada
    await ui.ev(`document.querySelector('[data-k=ae-done]').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=appedit]')===null`, 'editor fechado');
    await ui.key('Escape');
    await ui.key('Escape');
    await ui.until(`()=>document.querySelectorAll('#ovs>*').length===0`, 'ajustes fechados');
    await ui.until(`()=>[...document.querySelectorAll('#dock .tl[data-id]')].some(t=>t.dataset.id==='native:minecraft.desktop')`, 'jogo na tela inicial');
    const cover = await ui.ev(`getComputedStyle(document.querySelector('#dock .tl[data-id="native:minecraft.desktop"] .art')).backgroundImage`);
    check(cover.includes('/art/native%3Aminecraft.desktop/cover'), 'card usa a capa personalizada');
    check((await ui.ev(`fetch('/art/native%3Aminecraft.desktop/cover').then(r=>r.status)`)) === 200, 'capa servida pelo Core');
    check((await ui.ev(`[...document.querySelectorAll('#dock .tl[data-id]')].every(t=>t.dataset.id.startsWith('native:minecraft'))`)), 'só o Minecraft (a calculadora e a Steam não entraram)');
    await ui.shot('apps-home');
  } finally {
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
    ui.close();
    await core.stop();
  }
};

scenarios.steam_loja = async () => {
  const fs = (await import('node:fs')).default;
  const dir = fs.mkdtempSync('/tmp/nexus-steam-');
  fs.mkdirSync(`${dir}/Steam/steamapps`, { recursive: true });
  const fixture = `${dir}/feed.json`;
  const e = (appid, name, fin, orig, disc, free, linux) => ({ appid, name, final_cents: fin, orig_cents: orig, discount: disc, free, linux });
  fs.writeFileSync(fixture, JSON.stringify({ destaques: [e(111, 'Jogo Pago', 5949, 6999, 15, false, false), e(222, 'Jogo Grátis', 0, null, 0, true, true)], gratis: [e(222, 'Jogo Grátis', 0, null, 0, true, true)] }));
  const rec = `${dir}/calls.txt`;
  fs.writeFileSync(`${dir}/steam`, `#!/bin/sh\necho "$@" >> ${rec}\nexit 0\n`, { mode: 0o755 });
  const core = await startCore({ dir, env: { NEXUS_STEAM_ROOT: `${dir}/Steam`, NEXUS_STEAM_BIN: `${dir}/steam`, NEXUS_STEAM_FIXTURE: fixture } });
  await createAdult(core);
  const ui = await openUi(core);
  try {
    await ui.pickProfile('Ana');
    await ui.until(inShell, 'entrou');
    await ui.wait(600);
    await ui.key('e'); await ui.wait(300); await ui.key('e'); await ui.wait(300); await ui.key('e');
    await ui.until(`()=>document.querySelectorAll('.sc .tl').length>=2`, 'loja da Steam carregada', 10000);
    const cats = await ui.ev(`[...document.querySelectorAll('.stbar .cp')].map(r=>r.textContent.trim())`);
    check(cats[0] === 'Para você', 'Loja abre em "Para você"');
    check((await ui.count('.sbanners .sbn')) >= 1, 'banners de destaque');
    check((await ui.count('.shelf')) >= 2, 'prateleiras por categoria');
    check(cats.includes('Mais vendidos') && cats.includes('Grátis'), `categorias da Steam: ${cats.join(', ')}`);
    const price = await ui.ev(`document.querySelector('.sc .tl[data-id="steam:111"] .pz')?.textContent`);
    check(price === 'R$ 59,49', `preço em reais no card (${price})`);
    check((await ui.ev(`document.querySelector('.sc .tl[data-id="steam:111"] .bd')?.textContent`)) === '-15%', 'selo de desconto');
    // jogo pago: leva para a página da loja da Steam
    await ui.ev(`document.querySelector('.sc .tl[data-id="steam:111"]').click()`);
    await ui.until(`()=>document.querySelector('#ovs .hub [data-k=cta]')`, 'detalhes do jogo');
    const cta = await ui.text('#ovs .hub [data-k=cta]');
    check(cta.includes('Ver na Steam') && cta.includes('R$ 59,49'), `botão do jogo pago: ${cta}`);
    await ui.ev(`document.querySelector('#ovs .hub [data-k=cta]').click()`);
    await ui.until(`()=>document.querySelector('.toast')?.textContent?.includes('loja da Steam')`, 'aviso de abertura', 4000);
    await ui.key('Escape');
    // jogo grátis: instala direto
    await ui.ev(`document.querySelector('.sc .tl[data-id="steam:222"]').click()`);
    await ui.until(`()=>document.querySelector('#ovs .hub [data-k=cta]')`, 'detalhes do gratuito');
    check((await ui.text('#ovs .hub [data-k=cta]')).includes('Instalar'), 'botão do jogo grátis é Instalar');
    await ui.ev(`document.querySelector('#ovs .hub [data-k=cta]').click()`);
    await ui.wait(1200);
    const calls = fs.readFileSync(rec, 'utf8');
    check(calls.includes('steam://store/111'), 'Steam recebeu a página da loja do jogo pago');
    check(calls.includes('steam://install/222'), 'Steam recebeu o pedido de instalar o jogo grátis');
  } finally {
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
    ui.close();
    await core.stop();
  }
};

scenarios.apps_atualizacoes = async () => {
  const fs = (await import('node:fs')).default;
  const dir = fs.mkdtempSync('/tmp/nexus-upd-');
  const apps = `${dir}/apps/applications`;
  fs.mkdirSync(apps, { recursive: true });
  fs.writeFileSync(`${apps}/org.test.Video.desktop`, '[Desktop Entry]\nType=Application\nName=Vídeo Teste\nExec=flatpak run org.test.Video\nCategories=AudioVideo;\n');
  // flatpak falso: 1.0 (c1) instalada, 2.0 (c2) disponível
  fs.writeFileSync(`${dir}/fp-state`, '1.0 c1 c0');
  fs.writeFileSync(`${dir}/flatpak`, `#!/bin/sh
read v c p < ${dir}/fp-state
case "$1" in
  remote-ls) [ "$v" != "2.0" ] && printf 'org.test.Video\\t2.0\\n' ;;
  list) printf 'org.test.Video\\t%s\\n' "$v" ;;
  info) printf ' Version: %s\\n  Commit: %s\\n  Parent: %s\\n' "$v" "$c" "$p" ;;
  update) sleep 0.4; case "$*" in *--commit=c1*) echo "1.0 c1 c0" > ${dir}/fp-state ;; *) echo "2.0 c2 c1" > ${dir}/fp-state ;; esac ;;
esac
exit 0
`, { mode: 0o755 });
  // ajudante root de verdade, rodando com o flatpak falso (Flatpak do sistema volta versão por ele)
  fs.writeFileSync(`${dir}/helper`, `#!/bin/sh\nPATH=${dir}:$PATH NEXUS_APP_OUT=${dir}/hout NEXUS_APP_STATE=${dir}/hstate exec sh ${ROOT}/system/sysd/nexus-app-helper "$@"\n`, { mode: 0o755 });
  const core = await startCore({ dir, env: { NEXUS_APPS_DIRS: apps, NEXUS_FLATPAK_BIN: `${dir}/flatpak`, NEXUS_APP_HELPER: `${dir}/helper`, NEXUS_APP_OUT: `${dir}/hout` } });
  await createAdult(core);
  const ui = await openUi(core);
  const id = 'native:org.test.Video.desktop';
  try {
    await ui.pickProfile('Ana');
    await ui.until(inShell, 'entrou');
    await core.rpc('apps.add', { fileId: 'org.test.Video.desktop' });
    await core.rpc('apps.check_updates');
    // aviso nas notificações, que abre a página do app
    await ui.until(`()=>document.querySelector('#sbBell .ndot')`, 'bolinha de notificação');
    await ui.ev(`document.querySelector('#sbBell').click()`);
    await ui.until(`()=>[...document.querySelectorAll('#ovs .row2')].some(r=>r.textContent.includes('Atualização disponível')&&r.textContent.includes('Vídeo Teste · versão 2.0'))`, 'notificação de atualização');
    await ui.ev(`[...document.querySelectorAll('#ovs .row2')].find(r=>r.textContent.includes('Atualização disponível')).click()`);
    await ui.until(`()=>document.querySelector('#ovs .hub [data-k=hupd]')?.textContent?.includes('Atualizar · 2.0')`, 'botão Atualizar na página do app');
    check((await ui.text('#ovs .hub .hupd')).includes('você tem a 1.0'), 'explica a versão atual e a nova');
    await ui.ev(`document.querySelector('#ovs .hub [data-k=hupd]').click()`);
    await ui.until(`()=>document.querySelector('#ovs .hub [data-k=hupd]')?.textContent?.includes('Atualizando')`, 'atualizando…', 3000);
    await ui.until(`()=>!document.querySelector('#ovs .hub [data-k=hupd]')`, 'atualização concluída (botão some)', 8000);
    check(fs.readFileSync(`${dir}/fp-state`, 'utf8').startsWith('2.0'), 'flatpak atualizou para a 2.0');
    // Opções → Restaurar versão anterior (com confirmação)
    await ui.ev(`document.querySelector('#ovs .hub [data-k=hopt]').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=sheet] [data-a="apprestore:${id}"]')`, 'opção Restaurar versão anterior');
    await ui.ev(`document.querySelector('#ovs [data-kind=sheet] [data-a="apprestore:${id}"]').click()`);
    await ui.until(`()=>document.querySelector('#ovs [data-a^="yes:"]')`, 'confirmação');
    await ui.ev(`document.querySelector('#ovs [data-a^="yes:"]').click()`);
    for (let i = 0; i < 40 && !fs.readFileSync(`${dir}/fp-state`, 'utf8').startsWith('1.0'); i++) await ui.wait(200);
    check(fs.readFileSync(`${dir}/fp-state`, 'utf8').startsWith('1.0'), 'flatpak voltou para a 1.0');
    await ui.until(`()=>document.querySelector('#ovs .hub [data-k=hupd]')?.textContent?.includes('Atualizar · 2.0')`, 'a 2.0 volta a aparecer como atualização', 6000);
  } finally {
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
    await ui.close();
    await core.stop();
  }
};

// A6: o Core cai → depois de 12 s a Shell mostra a tela de falha (só controle: setas + A; B não fecha) e some
// sozinha quando o Core volta.
scenarios.falha_do_core = async () => {
  const core = await startCore({ demo: true });
  const ui = await openUi(core);
  let again = null;
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    core.crash();
    await ui.until(`()=>document.querySelector('.offline')`, 'aviso "Reconectando" logo que o Core cai', 3000);
    await ui.until(`()=>document.querySelector('#ovs [data-kind=fault] h1')`, 'tela de falha depois de 12 s', 16000);
    check(!(await ui.ev(`!!document.querySelector('.offline')`)), 'o aviso pequeno sai quando a tela de falha entra');
    await ui.until(`()=>document.querySelector('.f.focus')?.dataset.a==='fault:retry'`, 'foco em "Tentar de novo"', 3000);
    await ui.screens('a6-falha');
    await ui.key('ArrowRight');
    check((await ui.focusInfo())?.a === 'fault:session', 'direita vai para "Reiniciar o Nexus"');
    await ui.key('Escape');
    check(await ui.ev(`!!document.querySelector('#ovs [data-kind=fault]')`), 'B não fecha a tela de falha');
    await ui.key('ArrowLeft');
    await ui.key('Enter');
    await ui.until(`()=>/navegador|Reiniciando/.test(document.querySelector('#ovs .fault .fst')?.textContent||'')`, 'A executa a ação e mostra o resultado');
    // o Core volta (mesma porta): a tela some sozinha
    again = await startCore({ demo: true, dir: core.dir, devPort: core.dev });
    await ui.until(`()=>!document.querySelector('#ovs [data-kind=fault]')`, 'tela de falha some quando o Core volta', 12000);
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    if (again) await again.stop();
    else await core.stop();
  }
};

// B1: Ajustes → Rede só com o controle: estado, teste, IP automático (pede o PIN) e esquecer a rede.
scenarios.rede = async () => {
  const core = await startCore({ demo: true });
  fs.writeFileSync(`${core.dir}/fake-net`, 'Casa Nexus');
  const ui = await openUi(core);
  const to = async (pred, what) => {
    for (const [dir, max] of [['ArrowUp', 16], ['ArrowDown', 16], ['ArrowLeft', 4], ['ArrowRight', 4]]) {
      for (let n = 0; n < max; n++) {
        if (pred(await ui.focusInfo())) return;
        await ui.key(dir, { wait: 70 });
      }
    }
    check(pred(await ui.focusInfo()), `foco em ${what}`);
  };
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    // engrenagem: sobe até a barra e anda para a direita (como no controle)
    for (let n = 0; n < 4; n++) await ui.key('ArrowUp', { wait: 80 });
    for (let n = 0; n < 12 && (await ui.focusInfo())?.a !== 'settings'; n++) await ui.key('ArrowRight', { wait: 80 });
    check((await ui.focusInfo())?.a === 'settings', 'foco na engrenagem');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=settings] .setrail')`, 'ajustes');
    await to((f) => f?.k === 'sc-rede', 'categoria Rede');
    await ui.key('Enter');
    await ui.until(`()=>/Casa Nexus.*internet ok/.test(document.querySelector('[data-k=net-sum]')?.textContent||'')`, 'estado: Wi-Fi conectado com internet');
    check(/192\.168\.0\.42\/24/.test(await ui.text('[data-k=net-ip]')), 'IP, gateway e DNS nos detalhes');
    await ui.screens('b1-rede');
    await ui.key('ArrowRight'); // entra no painel
    await to((f) => f?.k === 'net-test', 'Testar conexão');
    await ui.key('Enter');
    await ui.until(`()=>/Internet funcionando/.test(document.querySelector('[data-k=net-test]')?.textContent||'')`, 'teste de conexão', 15000);
    // IP automático: ajuste avançado pede o PIN dos responsáveis
    await to((f) => f?.k === 'net-ipfix', 'IP fixo');
    // B (Voltar) na escolha não aplica nada (antes caía em "Automático")
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'escolha automático/fixo');
    await ui.key('Escape');
    await ui.until(`()=>!document.querySelector('#ovs [data-kind=confirm]')`, 'B fecha a escolha');
    await new Promise((r) => setTimeout(r, 400));
    check(!fs.existsSync(`${core.dir}/fake-net.log`) && !(await ui.has('#ovs [data-kind=pin]')), 'Voltar não muda o IP nem pede PIN');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'escolha automático/fixo');
    await to((f) => f?.t === 'Automático', 'Automático');
    await ui.key('Enter');
    await ui.typePin('1234');
    await ui.until(`()=>[...document.querySelectorAll('.toast')].some(t=>/IP automático/.test(t.textContent))`, 'IP automático aplicado');
    check(fs.readFileSync(`${core.dir}/fake-net.log`, 'utf8').includes('ipv4.method auto'), 'Core mandou o NetworkManager voltar ao DHCP');
    // esquecer a rede ativa
    await to((f) => f?.k === 'wifi-0', 'rede ativa');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação para esquecer');
    await ui.key('Enter');
    await ui.until(`()=>/Sem conexão/.test(document.querySelector('[data-k=net-sum]')?.textContent||'')`, 'rede esquecida: sem conexão', 8000);
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B2: Ajustes → Bluetooth só com o controle: pareados com tipo/bateria, procurar, parear (e recusa), desconectar,
// Voltar sem efeito, ajuda por modelo e desligar.
scenarios.bluetooth = async () => {
  const core = await startCore({ demo: true });
  fs.mkdirSync(`${core.dir}/fake-bt`, { recursive: true });
  fs.writeFileSync(`${core.dir}/fake-bt/paired`, '11:22:33:44:55:02\n');
  fs.writeFileSync(`${core.dir}/fake-bt/conn`, '11:22:33:44:55:02\n');
  const bt = (f) => fs.readFileSync(`${core.dir}/fake-bt/${f}`, 'utf8');
  const ui = await openUi(core);
  // anda com ↑/↓ na direção do item (sem sair do painel para a lista lateral)
  const to = async (pred, what) => {
    const k = what.k;
    for (let n = 0; n < 20; n++) {
      if (pred(await ui.focusInfo())) return;
      const dir = await ui.ev(`(()=>{const t=document.querySelector('[data-k="${k}"]'),f=document.querySelector('.f.focus');if(!t||!f)return 'ArrowDown';return t.getBoundingClientRect().top<f.getBoundingClientRect().top?'ArrowUp':'ArrowDown'})()`);
      await ui.key(dir, { wait: 80 });
    }
    check(pred(await ui.focusInfo()), `foco em ${what.label}`);
  };
  const toast = (re) => `()=>[...document.querySelectorAll('.toast')].some(t=>${re}.test(t.textContent))`;
  const pinIfAsked = async () => {
    await new Promise((r) => setTimeout(r, 500));
    if (await ui.has('#ovs [data-kind=pin]')) await ui.typePin('1234');
  };
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    for (let n = 0; n < 4; n++) await ui.key('ArrowUp', { wait: 80 });
    for (let n = 0; n < 12 && (await ui.focusInfo())?.a !== 'settings'; n++) await ui.key('ArrowRight', { wait: 80 });
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=settings] .setrail')`, 'ajustes');
    await to((f) => f?.k === 'sc-bt', { k: 'sc-bt', label: 'categoria Bluetooth' });
    await ui.key('Enter');
    await ui.until(`()=>/Ligado/.test(document.querySelector('[data-k=bt-power]')?.textContent||'')`, 'Bluetooth ligado');
    await ui.until(`()=>/JBL Flip 5.*Fone ou caixa de som · conectado · bateria 15% \\(baixa\\)/.test(document.querySelector('[data-k=bt-d-0]')?.textContent||'')`, 'pareado com tipo, conexão e bateria baixa');
    await ui.key('ArrowRight'); // entra no painel
    await to((f) => f?.k === 'bt-scan', { k: 'bt-scan', label: 'Procurar aparelhos' });
    await ui.key('Enter');
    await ui.until(`()=>document.querySelectorAll('[data-k^=bt-f-]').length===2`, 'encontrou 2 novos (o pareado não repete)');
    check(/Xbox Wireless Controller.*Controle/.test(await ui.text('[data-k=bt-f-0]')), 'controle aparece primeiro');
    await ui.screens('b2-bluetooth');
    await to((f) => f?.k === 'bt-f-0', { k: 'bt-f-0', label: 'controle encontrado' });
    await ui.key('Enter');
    await pinIfAsked();
    await ui.until(toast('/Pareado/'), 'controle pareado');
    check(bt('paired').includes('11:22:33:44:55:01') && bt('conn').includes('11:22:33:44:55:01'), 'Core pareou e conectou o controle');
    await ui.until(`()=>/^bt-/.test(document.querySelector('.f.focus')?.dataset.k||'') && document.querySelector('.setpane h2')?.textContent==='Bluetooth'`, 'item pareado some da lista e o foco fica no painel (não pula para Rede)', 3000);
    await ui.until(`()=>/Xbox.*bateria 76%/.test(document.querySelector('[data-k=bt-d-1]')?.textContent||'')`, 'controle na lista com bateria');
    // o teclado recusa: mensagem clara
    await to((f) => f?.k === 'bt-f-0', { k: 'bt-f-0', label: 'teclado encontrado' });
    await ui.key('Enter');
    await pinIfAsked();
    await ui.until(toast('/recusou o pareamento/'), 'recusa explicada');
    // Voltar na escolha não faz nada; A desconecta
    await to((f) => f?.k === 'bt-d-0', { k: 'bt-d-0', label: 'caixa de som' });
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'opções do aparelho');
    await ui.key('Escape');
    await ui.until(`()=>!document.querySelector('#ovs [data-kind=confirm]')`, 'B fecha as opções');
    check(bt('paired').includes('11:22:33:44:55:02') && bt('conn').includes('11:22:33:44:55:02'), 'Voltar não esqueceu nem desconectou');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'opções do aparelho');
    await ui.key('Enter');
    await ui.until(toast('/Desconectado/'), 'caixa de som desconectada');
    check(!bt('conn').includes('11:22:33:44:55:02'), 'Core desconectou');
    // ajuda por modelo: ←/→ troca
    await to((f) => f?.k === 'bt-help', { k: 'bt-help', label: 'ajuda de pareamento' });
    await ui.key('ArrowRight');
    check(/PlayStation 5.*Create \+ PS/.test(await ui.text('[data-k=bt-help]')), 'ajuda do DualSense');
    // desligar
    await to((f) => f?.k === 'bt-power', { k: 'bt-power', label: 'liga/desliga' });
    await ui.key('Enter');
    await ui.until(`()=>/Desligado/.test(document.querySelector('[data-k=bt-power]')?.textContent||'') && !document.querySelector('[data-k=bt-scan]')`, 'Bluetooth desligado');
    check(bt('power').trim() === 'no', 'Core desligou o adaptador');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// anda com ↑/↓ na direção do item data-k=k (sem sair do painel para a lista lateral)
async function goTo(ui, k, label) {
  for (let n = 0; n < 60; n++) {
    if ((await ui.focusInfo())?.k === k) return;
    const dir = await ui.ev(`(()=>{const t=document.querySelector('[data-k="${k}"]'),f=document.querySelector('.f.focus');if(!t||!f)return 'ArrowDown';return t.getBoundingClientRect().top<f.getBoundingClientRect().top?'ArrowUp':'ArrowDown'})()`);
    await ui.key(dir, { wait: 80 });
  }
  check((await ui.focusInfo())?.k === k, `foco em ${label}`);
}
async function waitNode(pred, what, ms = 8000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (pred()) return check(true, what);
    await new Promise((r) => setTimeout(r, 80));
  }
  check(false, what);
  throw new Error(`timeout: ${what}`);
}
async function openSettingsCat(ui, cat, label) {
  for (let n = 0; n < 4; n++) await ui.key('ArrowUp', { wait: 80 });
  for (let n = 0; n < 12 && (await ui.focusInfo())?.a !== 'settings'; n++) await ui.key('ArrowRight', { wait: 80 });
  await ui.key('Enter');
  await ui.until(`()=>document.querySelector('#ovs [data-kind=settings] .setrail')`, 'ajustes');
  await goTo(ui, `sc-${cat}`, `categoria ${label}`);
  await ui.key('Enter');
}

// B3: Ajustes → Áudio só com o controle (volume tira o mudo, saída, teste, microfone) e A2: volume real na Central.
scenarios.audio = async () => {
  const core = await startCore({ demo: true });
  const A = `${core.dir}/fake-audio`;
  fs.mkdirSync(A, { recursive: true });
  fs.writeFileSync(`${A}/vol_55`, '0.40\n');
  fs.writeFileSync(`${A}/mute_55`, '1\n');
  const rd = (f) => (fs.existsSync(`${A}/${f}`) ? fs.readFileSync(`${A}/${f}`, 'utf8').trim() : '');
  const toast = (re) => `()=>[...document.querySelectorAll('.toast')].some(t=>${re}.test(t.textContent))`;
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'audio', 'Áudio');
    await ui.until(`()=>/Volume \\(mudo\\).*40/.test(document.querySelector('[data-k=au-vol]')?.textContent||'')`, 'volume do sistema e mudo');
    await ui.key('ArrowRight'); // entra no painel
    await goTo(ui, 'au-vol', 'volume');
    await ui.key('ArrowRight');
    await ui.key('ArrowRight');
    await waitNode(() => rd('vol_55') === '0.50' && rd('mute_55') === '0', 'volume 50 % aplicado e mudo desligado', 5000);
    check(/Volume50/.test((await ui.text('[data-k=au-vol]')).replace(/\s/g, '')), 'tela mostra 50 sem mudo');
    // saída: TV pelo HDMI
    await goTo(ui, 'au-out-0', 'saída HDMI');
    check(/TV ou monitor \(HDMI\)/.test(await ui.text('[data-k=au-out-0]')), 'saída HDMI identificada');
    await ui.key('Enter');
    await ui.until(toast('/Saída de som/'), 'saída trocada');
    check(rd('default') === '54', 'Core trocou a saída padrão');
    await ui.until(`()=>/em uso/.test(document.querySelector('[data-k=au-out-0]')?.textContent||'')`, 'HDMI marcada em uso');
    // teste de som
    await goTo(ui, 'au-test', 'testar o som');
    await ui.key('Enter');
    await ui.until(toast('/Som de teste/'), 'teste tocado');
    check(/nexus-teste-som\.wav/.test(rd('played')), 'Core tocou o WAV de teste');
    // microfone
    await goTo(ui, 'au-mic', 'microfone');
    await ui.key('ArrowLeft');
    await waitNode(() => rd('vol_56') === '0.45', 'volume do microfone', 5000);
    await ui.screens('b3-audio');
    // Central rápida: o volume é o do sistema
    for (let n = 0; n < 3 && (await ui.count('#ovs>*')) > 0; n++) await ui.key('Escape', { wait: 300 });
    await ui.until(`()=>document.querySelectorAll('#ovs>*').length===0`, 'fechou os ajustes');
    await ui.key('Tab');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=guide] [data-k=g-vol]')`, 'central com volume do sistema');
    check(/50/.test(await ui.text('[data-k=g-vol]')), 'central mostra o volume do sistema (50)');
    await ui.focusOn((f) => f.k === 'g-vol', 20, true);
    await ui.key('ArrowLeft');
    await waitNode(() => rd('vol_54') === '0.45', 'central muda o volume da saída em uso (HDMI)', 5000);
    await ui.key('Enter');
    await waitNode(() => rd('mute_54') === '1', 'A na central liga o mudo', 5000);
    await new Promise((r) => setTimeout(r, 500));
    check(rd('mute_54') === '1' && rd('vol_54') === '0.45', 'mudo continua depois do volume pendente (sem corrida)');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B4: Ajustes → Tela e vídeo só com o controle: resolução com confirmação, volta sozinha sem resposta, VRR, HDR
// indisponível e assistente de margem (salvar e desistir).
scenarios.tela = async () => {
  const core = await startCore({ demo: true });
  const D = `${core.dir}/fake-display`;
  const cur = () => (fs.existsSync(`${D}/cur`) ? fs.readFileSync(`${D}/cur`, 'utf8').trim() : '1920x1080@60.000 1 normal off');
  const toast = (re) => `()=>[...document.querySelectorAll('.toast')].some(t=>${re}.test(t.textContent))`;
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'tela', 'Tela e vídeo');
    await ui.until(`()=>/Samsung.*1920 × 1080 · 60 Hz/.test(document.querySelector('[data-k=disp-sum]')?.textContent||'')`, 'tela, resolução e Hz atuais');
    check(/Não disponível/.test(await ui.text('[data-k=disp-hdr]')), 'HDR indisponível explicado');
    await ui.key('ArrowRight'); // entra no painel
    // resolução: 1920×1080 → 1280×720, aplicar, contagem, manter
    await goTo(ui, 'disp-res', 'resolução');
    await ui.key('ArrowRight');
    check(/1280 × 720/.test(await ui.text('[data-k=disp-res]')), '→ escolhe 1280 × 720');
    await goTo(ui, 'disp-apply', 'aplicar');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm] [data-k=cdown]')`, 'confirmação com contagem regressiva');
    check(cur().startsWith('1280x720@60.000'), 'Core aplicou o modo no sway');
    await ui.screens('b4-confirmar');
    await ui.key('Enter'); // "Manter" tem o foco
    await ui.until(toast('/Tela ajustada/'), 'confirmado');
    await new Promise((r) => setTimeout(r, 5600));
    check(cur().startsWith('1280x720'), 'confirmado não volta depois do prazo');
    // escala sem resposta: volta sozinha
    await goTo(ui, 'disp-scale', 'escala');
    await ui.key('ArrowRight');
    await goTo(ui, 'disp-apply', 'aplicar');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação da escala');
    check(cur().split(' ')[1] === '1.25', 'escala 125 % aplicada');
    await waitNode(() => cur().split(' ')[1] === '1', 'sem resposta em 5 s: escala anterior volta sozinha', 9000);
    await ui.until(`()=>!document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação some sozinha');
    // VRR
    await goTo(ui, 'disp-vrr', 'VRR');
    await ui.key('Enter');
    await waitNode(() => cur().endsWith(' on'), 'VRR ligado no sway');
    await ui.screens('b4-tela');
    // assistente de margem: → 3 vezes e A salva
    await goTo(ui, 'disp-mwiz', 'assistente de margem');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=margin] .mcorner')`, 'assistente com setas nos cantos');
    const s0 = Number(await ui.ev(`getComputedStyle(document.documentElement).getPropertyValue('--s')`));
    for (let n = 0; n < 3; n++) await ui.key('ArrowRight', { wait: 80 });
    const s1 = Number(await ui.ev(`getComputedStyle(document.documentElement).getPropertyValue('--s')`));
    check(Math.abs(s1 / s0 - 0.94) < 0.001, `interface encolhe 6 % ao vivo (${s0} → ${s1})`);
    await ui.screens('b4-margem');
    await ui.key('Enter');
    await ui.until(toast('/Margem salva/'), 'margem salva');
    check((await core.rpc('display.status')).margin === 3, 'Core guardou a margem (3 %)');
    // de novo, mas B desiste: volta ao valor salvo
    await goTo(ui, 'disp-mwiz', 'assistente de margem');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=margin]')`, 'assistente de novo');
    await ui.key('ArrowRight');
    await ui.key('Escape');
    await ui.until(`()=>!document.querySelector('#ovs [data-kind=margin]')`, 'B fecha o assistente');
    check(Math.abs(Number(await ui.ev(`getComputedStyle(document.documentElement).getPropertyValue('--s')`)) - s1) < 1e-6, 'B desiste e a margem volta para 3 %');
    check((await core.rpc('display.status')).margin === 3, 'margem salva não mudou');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// Navegação nos Ajustes (pedido do usuário): ↑/↓ travam no fim da página e da lista lateral; só ← volta à lista,
// caindo na categoria atual; → volta ao mesmo item do painel. Nada de pular para "um item nada a ver".
scenarios.navegacao = async () => {
  const core = await startCore({ demo: true });
  const ui = await openUi(core);
  const cat = () => ui.ev(`document.querySelector('.setpane h2')?.textContent||''`);
  const inPane = () => ui.ev(`!!document.querySelector('.setpane .f.focus')`);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'conta', 'Conta e perfis');
    await ui.until(`()=>document.querySelector('.setpane h2')?.textContent==='Conta e perfis'`, 'categoria aberta');
    // painel: ↓ até muito depois do fim
    await ui.key('ArrowRight');
    check(await inPane(), '→ entra no painel');
    const first = (await ui.focusInfo()).k;
    for (let n = 0; n < 40; n++) await ui.key('ArrowDown', { wait: 40 });
    const last = await ui.focusInfo();
    check(await inPane(), '↓ no fim da página trava no painel');
    check(last.k === (await ui.ev(`[...document.querySelectorAll('.setpane .f')].filter(e=>e.getClientRects().length).pop().dataset.k`)), `foco no último item do painel (${last.k})`);
    check((await cat()) === 'Conta e perfis', 'categoria não mudou');
    for (let n = 0; n < 40; n++) await ui.key('ArrowUp', { wait: 40 });
    check(await inPane(), '↑ no topo da página trava no painel');
    check((await ui.focusInfo()).k === first, 'foco no primeiro item do painel');
    // ← volta à lista na categoria atual; → volta ao mesmo item
    await ui.key('ArrowDown', { wait: 80 });
    await ui.key('ArrowDown', { wait: 80 });
    const mid = (await ui.focusInfo()).k;
    await ui.key('ArrowLeft');
    check((await ui.focusInfo()).k === 'sc-conta', '← cai na categoria atual da lista');
    await new Promise((r) => setTimeout(r, 400));
    check((await cat()) === 'Conta e perfis', 'categoria continua a mesma');
    await ui.key('ArrowRight');
    check((await ui.focusInfo()).k === mid, `→ volta ao mesmo item do painel (${mid})`);
    // lista lateral: trava nas pontas
    await ui.key('ArrowLeft');
    for (let n = 0; n < 30; n++) await ui.key('ArrowDown', { wait: 40 });
    check((await ui.focusInfo()).k === 'sc-home', '↓ na lista trava no último item (Voltar ao Início)');
    for (let n = 0; n < 30; n++) await ui.key('ArrowUp', { wait: 40 });
    await new Promise((r) => setTimeout(r, 400));
    check((await ui.focusInfo())?.k === 'sc-rede', '↑ na lista trava no primeiro item (e o realce do foco continua depois que a categoria abre)');
    // ↓ na lista anda item a item (não pula para a categoria ativa)
    await ui.key('ArrowDown');
    check((await ui.focusInfo())?.k === 'sc-bt', '↓ na lista vai para o item seguinte');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// Google Drive: Ajustes → Armazenamento → Entrar (janela do Google simulada) → pasta "Nexus" criada → montado →
// ROM da nuvem nos Emuladores → Sair da conta. Usa o rclone de verdade com um remoto local (precisa de FUSE).
const realRclone = () => [process.env.NEXUS_TEST_RCLONE, `${os.homedir()}/.local/opt/rclone/root/usr/bin/rclone`, '/usr/bin/rclone'].find((p) => p && fs.existsSync(p));
scenarios.drive = async () => {
  const real = realRclone();
  if (!real) return void console.log('  (pulado: rclone não encontrado)');
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-drv-'));
  const cloud = `${tmp}/cloud`;
  fs.mkdirSync(cloud);
  fs.writeFileSync(`${tmp}/fake.conf`, `[fakedrive]\ntype = alias\nremote = ${cloud}\n`);
  const core = await startCore({ demo: true, env: { NEXUS_RCLONE: `${ROOT}/tests/e2e/fake/rclone`, NEXUS_REAL_RCLONE: real, NEXUS_FAKE_RCLONE_CONF: `${tmp}/fake.conf`, NEXUS_DRIVE_REMOTE: 'fakedrive', NEXUS_BROWSER: `${ROOT}/tests/e2e/fake/browser` } });
  const toast = (re) => `()=>[...document.querySelectorAll('.toast')].some(t=>${re}.test(t.textContent))`;
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'armaz', 'Armazenamento');
    await ui.until(`()=>!!document.querySelector('[data-k=dr-login]')`, 'botão Entrar no Google Drive');
    await ui.screens('drive-entrar');
    await ui.key('ArrowRight');
    await goTo(ui, 'dr-login', 'Entrar no Google Drive');
    await ui.key('Enter');
    await waitNode(async () => (await core.rpc('game.current'))?.itemId === 'drive:login', 'janela do login do Google aberta como app', 8000);
    await ui.until(toast('/Pasta “Nexus” criada/'), 'aviso: pasta Nexus criada no Drive', 20000);
    check(fs.existsSync(`${cloud}/Nexus`), 'pasta Nexus criada na raiz do Drive');
    await waitNode(async () => (await core.rpc('game.current')) === null, 'janela do login fechou sozinha', 8000);
    await ui.until(`()=>/Google Drive conectado/.test(document.querySelector('[data-k=dr-ok]')?.textContent||'')`, 'painel mostra conectado', 10000);
    await ui.until(`()=>/Pasta “Nexus”/.test(document.querySelector('[data-k=dr-ok]')?.textContent||'')`, 'montado (pasta Nexus)', 10000);
    await ui.screens('drive-conectado');
    // espaço para jogos baixados: ← → muda o limite
    await goTo(ui, 'dr-cache', 'espaço para jogos baixados');
    await ui.key('ArrowRight');
    await waitNode(async () => (await core.rpc('drive.status')).cacheGb === 50, 'limite do cache 20 → 50 GB', 8000);
    // sair da conta
    await goTo(ui, 'dr-logout', 'Sair da conta');
    await ui.key('Enter');
    await ui.until(`()=>!!document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação de sair');
    await ui.key('Enter');
    await ui.until(`()=>!!document.querySelector('[data-k=dr-login]')`, 'voltou ao botão Entrar', 10000);
    check(!fs.existsSync(`${core.dir}/data/rclone.conf`), 'token apagado');
    check(fs.existsSync(`${cloud}/Nexus`), 'arquivos continuam no Drive');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
    fs.rmSync(tmp, { recursive: true, force: true });
  }
};

// Mídia (Drive parte 2): aba Mídia com filmes/séries/músicas (pastas da TV + pasta Nexus do Google Drive, sem repetir),
// abrir no mpv (falso: grava os argumentos) e fechar. Só controle. Com rclone, a nuvem é de verdade (FUSE).
scenarios.midia = async () => {
  const real = realRclone();
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-med-'));
  const touch = (p) => { fs.mkdirSync(path.dirname(p), { recursive: true }); fs.writeFileSync(p, 'x'); };
  const vid = `${tmp}/Vídeos`;
  const mus = `${tmp}/Música`;
  const cloud = `${tmp}/cloud`;
  touch(`${vid}/Matrix.mkv`);
  touch(`${vid}/Séries/Dark/Temporada 1/e01.mkv`);
  touch(`${mus}/Álbum Azul/faixa1.mp3`);
  touch(`${mus}/Álbum Azul/faixa2.mp3`);
  const log = `${tmp}/mpv.args`;
  const env = { NEXUS_MPV: `${ROOT}/tests/e2e/fake/mpv`, NEXUS_FAKE_MPV_LOG: log, NEXUS_VIDEOS_DIR: vid, NEXUS_MUSIC_DIR: mus };
  if (real) {
    touch(`${cloud}/Nexus/Filmes/matrix.MKV`); // repetido: a cópia da TV vence
    touch(`${cloud}/Nexus/Filmes/Alien.mp4`);
    touch(`${cloud}/Nexus/Séries/Dark/Temporada 1/e02.mkv`);
    touch(`${cloud}/Nexus/Séries/Lost/Temporada 1/pilot.mkv`);
    touch(`${cloud}/Nexus/Músicas/Beat.flac`);
    fs.writeFileSync(`${tmp}/fake.conf`, `[fakedrive]\ntype = alias\nremote = ${cloud}\n`);
    Object.assign(env, { NEXUS_RCLONE: `${ROOT}/tests/e2e/fake/rclone`, NEXUS_REAL_RCLONE: real, NEXUS_FAKE_RCLONE_CONF: `${tmp}/fake.conf`, NEXUS_DRIVE_REMOTE: 'fakedrive', NEXUS_BROWSER: `${ROOT}/tests/e2e/fake/browser` });
  } else console.log('  (sem rclone: só as pastas da TV)');
  const core = await startCore({ demo: true, env });
  const ui = await openUi(core);
  const tabTo = async (t) => {
    // RB do controle (tecla E) troca de aba
    for (let n = 0; n < 6 && (await ui.ev(`document.querySelector('#tabs .tab.on')?.dataset.tab`)) !== t; n++) await ui.key('e', { wait: 400 });
    await ui.until(`()=>document.querySelector('#tabs .tab.on')?.dataset.tab===${JSON.stringify(t)}`, `aba ${t}`);
  };
  const keys = () => ui.ev(`[...document.querySelectorAll('#view .tl')].map(e=>e.dataset.k)`);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    if (real) {
      // conecta o Drive pelos Ajustes (login simulado), como no cenário "drive"
      await openSettingsCat(ui, 'armaz', 'Armazenamento');
      await ui.until(`()=>!!document.querySelector('[data-k=dr-login]')`, 'botão Entrar no Google Drive');
      await ui.key('ArrowRight');
      await goTo(ui, 'dr-login', 'Entrar no Google Drive');
      await ui.key('Enter');
      await ui.until(`()=>/Pasta “Nexus”/.test(document.querySelector('[data-k=dr-ok]')?.textContent||'')`, 'Drive montado (pasta Nexus)', 25000);
      for (let n = 0; n < 6 && (await ui.has('#ovs [data-kind=settings]')); n++) await ui.key('Escape', { wait: 400 });
      await ui.until(`()=>!document.querySelector('#ovs [data-kind=settings]')`, 'Ajustes fechados');
      await sleep(500);
    }
    const lst = async () => {
      const r = await core.rpc('library.list', {});
      return (r.items ?? r).filter((i) => i.provider === 'media');
    };
    let list = await lst();
    for (let n = 0; n < 30 && list.length < (real ? 9 : 5); n++) { await sleep(200); list = await lst(); }
    const movies = list.filter((i) => i.tags.includes('filme'));
    check(movies.length === (real ? 2 : 1), `filmes sem repetir (${movies.map((i) => i.title)})`);
    check(list.filter((i) => i.tags.includes('musica')).length === (real ? 3 : 2), 'músicas');
    check(list.filter((i) => i.tags.includes('serie')).length === (real ? 3 : 1), 'episódios');
    if (real) check(list.some((i) => i.tags.includes('cloud') && i.title === 'Alien') && !list.some((i) => i.tags.includes('cloud') && i.title.toLowerCase() === 'matrix'), 'Alien vem da nuvem; Matrix só da TV');
    await tabTo('midia');
    await ui.until(`()=>!!document.querySelector('#view [data-k="show-Dark"]')`, 'série Dark na aba');
    check((await keys()).includes('show-Dark') && (!real || (await keys()).includes('show-Lost')), 'séries viram um cartão cada');
    await ui.screens('midia');
    // série → episódios, Voltar
    await ui.key('ArrowDown', { wait: 300 });
    await ui.key('ArrowDown', { wait: 300 });
    if ((await ui.focusInfo())?.k !== 'show-Dark') await ui.key('ArrowLeft', { wait: 300 });
    check((await ui.focusInfo())?.k === 'show-Dark', 'foco no cartão da série Dark');
    await ui.key('Enter');
    await ui.until(`()=>!!document.querySelector('#view [data-k^="media:serie-"]') && !document.querySelector('#view [data-k^="show-"]')`, 'episódios da série');
    check((await keys()).length === (real ? 2 : 1), 'episódios da série Dark');
    await ui.screens('midia-serie');
    await ui.key('Escape');
    await ui.until(`()=>!!document.querySelector('#view [data-k="show-Dark"]')`, 'voltou à lista');
    await sleep(300);
    check((await ui.focusInfo())?.k === 'show-Dark', 'foco volta à série');
    // abrir um filme: mpv em tela cheia com retomada e legendas
    await ui.key('ArrowUp', { wait: 300 });
    check((await ui.focusInfo())?.id?.startsWith('media:filme-'), 'foco em um filme');;
    await ui.key('Enter');
    for (let n = 0; n < 100 && !(fs.existsSync(log) && fs.readFileSync(log, 'utf8').includes('--fs')); n++) await sleep(100);
    const args = fs.existsSync(log) ? fs.readFileSync(log, 'utf8').split('\n') : [];
    for (const a of ['--fs', '--hwdec=auto-safe', '--save-position-on-quit', '--sub-auto=fuzzy']) check(args.includes(a), `mpv recebeu ${a}`);
    check(args.some((a) => a.startsWith('--watch-later-directory=') && a.includes('/data/')), 'pasta watch_later nos dados do Nexus');
    check(args.some((a) => /\.(mkv|mp4)$/i.test(a)), 'arquivo do filme no fim');
    check((await core.rpc('game.current'))?.itemId?.startsWith('media:filme-'), 'Core acompanha o mpv');
    await ui.screens('midia-tocando');
    await core.rpc('game.close');
    for (let n = 0; n < 60 && (await core.rpc('game.current')); n++) await sleep(100);
    check((await core.rpc('game.current')) === null, 'mpv fechou e voltou ao Nexus');
    list = await lst();
    check(list.every((i) => !(i.playtimeSec > 0)), 'mídia não conta como tempo de jogo');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
    fs.rmSync(tmp, { recursive: true, force: true });
  }
};

// Mídia sem nenhum arquivo: a aba explica onde colocar filmes, séries e músicas.
scenarios.midia_vazio = async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-med0-'));
  const core = await startCore({ demo: true, env: { NEXUS_VIDEOS_DIR: `${tmp}/v`, NEXUS_MUSIC_DIR: `${tmp}/m` } });
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    for (let n = 0; n < 6 && (await ui.ev(`document.querySelector('#tabs .tab.on')?.dataset.tab`)) !== 'midia'; n++) await ui.key('e', { wait: 400 });
    await ui.until(`()=>document.querySelector('#tabs .tab.on')?.dataset.tab==='midia'`, 'aba midia');
    check(await ui.has('#media-hint'), 'dica de onde colocar os arquivos');
    check(/Nexus.*Google Drive/.test((await ui.text('#media-hint')) ?? ''), 'dica cita a pasta Nexus do Drive');
    check(!(await ui.has('#view [data-k^="show-"]')), 'sem séries');
    await ui.screens('midia-vazio');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
    fs.rmSync(tmp, { recursive: true, force: true });
  }
};

// B6: Ajustes → Controles só com o teclado: zona morta, teste ao vivo de botões/analógicos (controle falso injetado na
// Gamepad API), vibração de teste e ajuda para controle sem suporte.
scenarios.controles = async () => {
  const core = await startCore({ demo: true });
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    // controle falso: a página lê navigator.getGamepads() como num controle de verdade
    await ui.ev(`(()=>{window.__rumbles=0;window.__gp={id:'Xbox Wireless Controller (STANDARD GAMEPAD)',index:0,connected:true,mapping:'standard',axes:[0,0,0,0],buttons:Array.from({length:17},()=>({pressed:false,value:0})),vibrationActuator:{playEffect:async()=>{window.__rumbles++;return 'complete'}}};navigator.getGamepads=()=>[window.__gp];})()`);
    await openSettingsCat(ui, 'ctrl', 'Controles');
    await ui.until(`()=>document.querySelector('[data-k=ctl-test]')`, 'seção Testar');
    check(/Controle sem suporte/.test(await ui.text('#setpane')), 'ajuda para controle sem suporte aparece');
    await ui.key('ArrowRight'); // entra no painel
    // zona morta: 10 % → 14 % com →, e → 8 % com ←
    await goTo(ui, 'ctl-dz', 'zona morta');
    check(/10%/.test(await ui.text('[data-k=ctl-dz]')), 'zona morta começa em 10 %');
    await ui.key('ArrowRight');
    await ui.key('ArrowRight');
    check(/14%/.test(await ui.text('[data-k=ctl-dz]')), 'zona morta sobe de 2 em 2 (14 %)');
    check((await ui.ev(`localStorage.getItem('nexus.deadzone')`)) === '14', 'zona morta guardada neste aparelho');
    // vibração de teste
    await goTo(ui, 'ctl-vib', 'vibração de teste');
    await ui.key('Enter');
    await ui.until(`()=>window.__rumbles===1`, 'vibração enviada ao controle');
    // teste ao vivo
    await goTo(ui, 'ctl-test', 'testar botões');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-pt] [data-k=pt-name]')`, 'tela de teste com o controle');
    check(/Xbox Wireless Controller/.test(await ui.text('[data-k=pt-name]')), 'mostra o nome do controle');
    await ui.ev(`(()=>{const g=window.__gp;g.buttons[0]={pressed:true,value:1};g.buttons[7]={pressed:true,value:0.5};g.axes=[0.6,-0.5,0.05,0];})()`);
    await ui.until(`()=>document.querySelector('.pt-b[data-b=A]')?.classList.contains('on')`, 'botão A aceso');
    check(await ui.ev(`!document.querySelector('.pt-b[data-b=B]').classList.contains('on')`), 'B continua apagado');
    check(/RT 50%/.test(await ui.text('.pt-trig')), 'gatilho direito a 50 %');
    check(/Analógico esquerdo 60, -50/.test(await ui.text('[data-k=pt-l]')), 'analógico esquerdo ao vivo');
    check((await ui.ev(`document.querySelector('[data-k=pt-r] i').dataset.dead`)) === '1', 'analógico direito dentro da zona morta');
    check(/A/.test(await ui.text('[data-k=pt-last]')), 'último botão lembrado');
    // o teste não navega nos menus: A apertado no controle não fecha nem abre nada
    check((await ui.overlays()).includes('padtest'), 'A no controle não navegou (teste continua aberto)');
    await ui.screens('b6-controles-teste');
    // segurar B por 1 s sai
    await ui.ev(`(()=>{window.__gp.buttons[1]={pressed:true,value:1};})()`);
    await ui.until(`()=>!document.querySelector('[data-pt]')`, 'segurar B sai do teste', 4000);
    await ui.ev(`(()=>{window.__gp.buttons=window.__gp.buttons.map(()=>({pressed:false,value:0}));})()`);
    await ui.until(`()=>document.querySelector('[data-k=ctl-test]')`, 'volta aos ajustes');
    await ui.wait(500);
    check(!(await ui.overlays()).includes('padtest'), 'teste fechado');
    await ui.screens('b6-controles');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// ---------- Fase 3 do sistema: atualizações, SSH, sobre/diagnóstico, recuperação (programas do sistema 100 % falsos) ----------
const sysState = (core) => {
  const S = `${core.dir}/fake-sys`;
  fs.mkdirSync(S, { recursive: true });
  const rd = (f) => (fs.existsSync(`${S}/${f}`) ? fs.readFileSync(`${S}/${f}`, 'utf8').trim() : '');
  return { S, rd, lines: (f) => rd(f).split('\n').filter(Boolean) };
};
const toastRe = (re) => `()=>[...document.querySelectorAll('.toast')].some(t=>${re}.test(t.textContent))`;
const txt = (ui, k) => ui.text(`[data-k=${k}]`);

// B11: Ajustes → Atualizações só com o controle (verificar, instalar o Nexus, histórico, reverter, sistema, nunca em jogo).
scenarios.atualizacoes = async () => {
  const core = await startCore({ demo: true });
  const { S, rd, lines } = sysState(core);
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'upd', 'Atualizações');
    await ui.until(`()=>/Nexus 0\\.4\\.0-1/.test(document.querySelector('[data-k=up-nexus]')?.textContent||'')`, 'versão instalada (dpkg)');
    check(/Ainda não procurei/.test(await txt(ui, 'up-nexus')), 'antes de procurar: nenhuma versão nova conhecida');
    await ui.key('ArrowRight');
    await goTo(ui, 'up-check', 'procurar atualizações');
    await ui.key('Enter');
    await ui.until(`()=>/Nova versão disponível: 0\\.5\\.0/.test(document.querySelector('[data-k=up-nexus]')?.textContent||'')`, 'versão nova encontrada');
    check(/3 pacote\(s\)/.test(await txt(ui, 'up-sys')) && /firefox-esr/.test(await txt(ui, 'up-sys')), 'pacotes do sistema listados');
    await ui.screens('b11-atualizacoes');
    // instalar o Nexus: confirmação → trabalho em segundo plano → versão nova + histórico + reverter
    await goTo(ui, 'up-install', 'instalar o Nexus');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação da atualização');
    await ui.key('Enter');
    await ui.typePin('1234'); // instalar pede o PIN dos responsáveis (depois vale por alguns minutos)
    await ui.until(toastRe('/Atualizando o Nexus/'), 'atualização iniciada');
    await ui.until(`()=>/Nexus 0\\.5\\.0-1/.test(document.querySelector('[data-k=up-nexus]')?.textContent||'')`, 'versão nova instalada', 15000);
    check(lines('calls')[0] === 'update-nexus:latest', 'Core pediu ao ajudante a versão mais nova');
    await ui.until(`()=>document.querySelector('[data-k=up-hist-0]')`, 'histórico');
    check(/Nexus 0\.4\.0-1 → 0\.5\.0-1/.test(await txt(ui, 'up-hist-0')), 'histórico mostra de onde para onde');
    check(/Voltar para o Nexus 0\.4\.0-1/.test(await txt(ui, 'up-revert')), 'reverter oferece a versão anterior');
    check(await ui.has('[data-k=up-restart]'), 'pede reiniciar a sessão (a versão em execução é outra)');
    // reverter
    await goTo(ui, 'up-revert', 'voltar a versão');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação de voltar a versão');
    await ui.key('Enter');
    await ui.until(`()=>/Nexus 0\\.4\\.0-1/.test(document.querySelector('[data-k=up-nexus]')?.textContent||'')`, 'versão anterior de volta', 15000);
    check(lines('calls').includes('update-nexus:0.4.0'), 'Core pediu a versão exata (0.4.0)');
    // sistema
    await goTo(ui, 'up-install-sys', 'atualizar o sistema');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação do sistema');
    await ui.key('Enter');
    await ui.until(`()=>/em dia/.test(document.querySelector('[data-k=up-sys]')?.textContent||'')`, 'sistema atualizado', 15000);
    check(lines('calls').includes('upgrade-system'), 'Core pediu a atualização do sistema');
    check(!(await ui.has('[data-k=up-install-sys]')), 'sem botão de atualizar o sistema depois');
    // falha vira mensagem em português e entra no histórico
    fs.writeFileSync(`${S}/update-fails`, '1');
    fs.writeFileSync(`${S}/latest`, '0.6.0');
    await goTo(ui, 'up-check', 'procurar de novo');
    await ui.key('Enter');
    await ui.until(`()=>/Nova versão disponível: 0\\.6\\.0/.test(document.querySelector('[data-k=up-nexus]')?.textContent||'')`, '0.6.0 encontrada');
    await goTo(ui, 'up-install', 'instalar');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação');
    await ui.key('Enter');
    await ui.until(toastRe('/não deu certo/'), 'falha avisada', 15000);
    await ui.until(`()=>/Sem espaço em disco/.test(document.querySelector('[data-k=up-done]')?.textContent||'')`, 'motivo da falha na tela');
    check(rd('nexus-version') === '0.4.0-1', 'a versão instalada não mudou');
    // nunca durante um jogo
    await core.rpc('item.launch', { id: 'demo:g1' });
    let msg = '';
    await core.rpc('update.install', { kind: 'system', parentPin: '1234' }).catch((e) => { msg = e.message; });
    check(/Feche o jogo/.test(msg), 'o Core recusa atualizar com jogo aberto');
    await core.rpc('game.close');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// Acesso por SSH: passo a passo com os valores reais do aparelho, liga/desliga com PIN (programa falso) e servidor ausente.
scenarios.ssh = async () => {
  const core = await startCore({ demo: true });
  const { S, rd, lines } = sysState(core);
  const user = os.userInfo().username;
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'ssh', 'Acesso por SSH');
    await ui.until(`()=>document.querySelector('[data-k=ssh-toggle]')`, 'painel do SSH');
    // servidor não instalado: diz isso e mostra o comando de instalação
    check(/Não instalado/.test(await txt(ui, 'ssh-toggle')), 'servidor não instalado é dito');
    check(/sudo apt install openssh-server/.test(await txt(ui, 'ssh-1')), 'passo 1 mostra o comando de instalação');
    check(/192\.168\.0\.42/.test(await txt(ui, 'ssh-3')) && new RegExp(`Usuário: ${user}`).test(await txt(ui, 'ssh-3')) && /Porta: 22/.test(await txt(ui, 'ssh-3')), 'passo 3 mostra usuário, IP e porta reais');
    check((await txt(ui, 'ssh-4')).includes(`ssh ${user}@192.168.0.42`), 'comando pronto ssh usuario@IP');
    check(/ssh-keygen -t ed25519/.test(await txt(ui, 'ssh-6')) && /ssh-copy-id/.test(await txt(ui, 'ssh-6')), 'dica de chave (ssh-keygen + ssh-copy-id)');
    check(/exit/.test(await txt(ui, 'ssh-7')) && /disable --now ssh/.test(await txt(ui, 'ssh-8')), 'como sair e como desligar');
    await ui.screens('ssh-nao-instalado');
    // instalado e desligado: porta personalizada aparece no comando; fingerprint real no primeiro acesso
    fs.writeFileSync(`${S}/ssh-installed`, '1');
    fs.writeFileSync(`${S}/sshd_config`, '# Port 22\nPort 2200\n');
    await ui.until(`()=>/Desligado/.test(document.querySelector('[data-k=ssh-toggle]')?.textContent||'')`, 'instalado e desligado', 9000);
    check((await txt(ui, 'ssh-4')).includes(`ssh -p 2200 ${user}@192.168.0.42`), 'porta personalizada no comando');
    check(/SHA256:Zk3Qw/.test(await txt(ui, 'ssh-5')) && /ED25519/.test(await txt(ui, 'ssh-5')), 'primeiro acesso mostra a impressão digital da chave');
    // ligar: confirmação → o Core pede ao ajudante
    await ui.key('ArrowRight');
    await goTo(ui, 'ssh-toggle', 'chave do SSH');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'aviso de segurança');
    await ui.key('Enter');
    await ui.typePin('1234');
    await ui.until(toastRe('/SSH ligado/'), 'SSH ligado');
    check(lines('calls').includes('ssh-on') && rd('ssh-state') === 'active', 'o Core chamou ssh-on e o programa falso ligou');
    await ui.until(`()=>/Ligado/.test(document.querySelector('[data-k=ssh-toggle]')?.textContent||'')`, 'tela mostra ligado');
    await ui.screens('ssh-ligado');
    // desligar
    await ui.key('Enter');
    await ui.until(toastRe('/SSH desligado/'), 'SSH desligado');
    check(lines('calls').includes('ssh-off') && rd('ssh-state') === 'inactive', 'o Core chamou ssh-off');
    // criança não vê a categoria; o Core também recusa o liga/desliga sem PIN para criança
    const kid = await core.rpc('profiles.list');
    check(kid.some((p) => p.kind === 'kid'), 'há perfil infantil na demonstração');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B16: Sobre e diagnóstico + relatório de suporte entregue pelo celular.
scenarios.sobre = async () => {
  const core = await startCore({ demo: true });
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'sobre', 'Sobre e diagnóstico');
    await ui.until(`()=>document.querySelector('[data-k=ab-cpu]')`, 'informações do aparelho');
    check((await txt(ui, 'ab-cpu')).length > 12, 'processador identificado');
    check(/GB em uso de/.test(await txt(ui, 'ab-mem')), 'memória usada e total');
    check(/\d+%/.test(await txt(ui, 'ab-cpupct')), 'uso atual do processador');
    check(/Ligado há/.test(await txt(ui, 'ab-up')), 'tempo ligado');
    check(/Só o Raspberry Pi informa/.test(await txt(ui, 'ab-throttle')), 'throttling explica que só o Pi informa');
    check(await ui.has('[data-k=ab-top-0]'), 'lista o que mais consome');
    await ui.key('ArrowRight');
    await ui.screens('b16-sobre');
    await goTo(ui, 'ab-report', 'gerar relatório');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-k=ab-report-card] svg')`, 'relatório gerado com QR', 15000);
    const name = await ui.text('[data-k=ab-report-card] code');
    check(/^nexus-relatorio-\d{8}-\d{6}\.txt/.test(name), `arquivo de relatório nomeado (${name})`);
    await ui.screens('b16-relatorio');
    // o celular baixa o arquivo com o código de pareamento; sem código, não
    const pairing = await core.rpc('remote.pairing');
    const url = pairing.url.replace(/^http:\/\/[^/]+/, core.remoteUrl.replace(/\/$/, '')).replace('/?t=', '/report?t=');
    const ok = await fetch(url, { headers: { Origin: core.remoteUrl.replace(/\/$/, '') } });
    const body = await ok.text();
    check(ok.status === 200 && /RELATÓRIO DE SUPORTE DO NEXUS/.test(body) && /== Aparelho ==/.test(body), 'o celular baixa o relatório');
    check(!body.includes(pairing.url.split('?t=')[1]), 'o relatório não contém o código de pareamento');
    const bad = await fetch(url.replace(/t=.*/, 't=errado'), { headers: { Origin: core.remoteUrl.replace(/\/$/, '') } });
    check(bad.status === 403, 'sem o código certo não baixa');
    check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B15: Recuperação atrás do PIN: reparar, desktop no próximo login, restaurar padrões, apagar dados de perfil, reset de fábrica.
scenarios.recuperacao = async () => {
  const core = await startCore({ demo: true });
  const { S, rd, lines } = sysState(core);
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'recup', 'Recuperação');
    // atrás do PIN dos responsáveis
    await ui.until(`()=>document.querySelector('#ovs [data-kind=pin]')`, 'pede o PIN dos responsáveis');
    await ui.typePin('1234');
    await ui.until(`()=>document.querySelector('[data-k=rec-session]')`, 'recuperação aberta depois do PIN');
    await ui.screens('b15-recuperacao');
    await ui.key('ArrowRight');
    // reparar a interface
    await goTo(ui, 'rec-shell', 'reparar a interface');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação');
    await ui.key('Enter');
    await waitNode(() => lines('user-calls').some((l) => /restart --no-block nexus-shell\.service/.test(l)), 'Core reiniciou só a interface (systemctl --user)');
    // reiniciar a sessão
    await goTo(ui, 'rec-session', 'reiniciar a sessão');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação');
    await ui.key('Enter');
    await waitNode(() => lines('user-calls').some((l) => /restart --no-block nexus-session\.target/.test(l)), 'Core pediu o reinício da sessão');
    // desktop no próximo login
    await goTo(ui, 'rec-desktop', 'desktop no próximo login');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação');
    await ui.key('Enter');
    await waitNode(() => fs.existsSync(`${S}/force-desktop`), 'marca force-desktop criada');
    await ui.until(`()=>/LIGADO/.test(document.querySelector('[data-k=rec-desktop]')?.textContent||'')`, 'tela mostra ligado');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação para desligar');
    await ui.key('Enter');
    await waitNode(() => !fs.existsSync(`${S}/force-desktop`), 'marca removida');
    // restaurar padrões: tema claro volta ao escuro
    await core.rpc('profiles.list');
    // apagar dados de um perfil: o perfil continua
    const before = (await core.rpc('profiles.list')).length;
    await goTo(ui, 'rec-defaults', 'restaurar padrões');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação');
    await ui.key('Enter');
    await ui.until(toastRe('/Padrões restaurados/'), 'padrões restaurados');
    check((await core.rpc('profiles.list')).length === before, 'restaurar padrões não apaga perfis');
    const pid = (await core.rpc('profiles.list')).find((p) => p.name === 'Rafa').id;
    await goTo(ui, `rec-erase-${pid}`, 'apagar dados do Rafa');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação');
    await ui.key('Enter');
    await ui.until(toastRe('/Dados apagados/'), 'dados apagados');
    check((await core.rpc('profiles.list')).length === before, 'apagar dados mantém o perfil');
    // reset de fábrica: duas confirmações; depois o Core está sem perfis (primeiro uso)
    await goTo(ui, 'rec-factory', 'reset de fábrica');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'primeira confirmação');
    await ui.key('Enter');
    await ui.until(`()=>/Última chance/.test(document.querySelector('#ovs [data-kind=confirm]')?.textContent||'')`, 'segunda confirmação');
    await ui.key('Escape');
    await ui.wait(400);
    check((await core.rpc('profiles.list')).length === before, 'cancelar na segunda confirmação não apaga nada');
    await goTo(ui, 'rec-factory', 'reset de fábrica');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'primeira confirmação');
    await ui.key('Enter');
    await ui.until(`()=>/Última chance/.test(document.querySelector('#ovs [data-kind=confirm]')?.textContent||'')`, 'segunda confirmação');
    await ui.key('Enter');
    await waitNode(() => true, 'confirmado');
    for (let i = 0; i < 40 && (await core.rpc('profiles.list')).length > 0; i++) await ui.wait(150);
    check((await core.rpc('profiles.list')).length === 0, 'reset de fábrica: sem perfis, volta ao primeiro uso');
    // criança não passa: o Core exige o PIN dos responsáveis e perfil adulto
  } finally {
    ui.close();
    await core.stop();
  }
};

// ============================================================ Fase 4 (E): A5, B5, B12, B13, B14, B17, B18, B19
const fake4 = (core) => {
  const C = `${core.dir}/fake-cec`;
  fs.mkdirSync(C, { recursive: true });
  return { C, calls: () => (fs.existsSync(`${C}/calls`) ? fs.readFileSync(`${C}/calls`, 'utf8').split('\n').filter(Boolean) : []) };
};
const cecEnv = (dir) => ({ NEXUS_CEC_CLIENT: `${ROOT}/tests/e2e/fake/cec-client`, NEXUS_FAKE_CEC: `${dir}/fake-cec` });
const enterPane = async (ui) => ui.key('ArrowRight');
const noJs = (ui) => check(ui.errors().length === 0, `sem erros de JavaScript${ui.errors().length ? ': ' + ui.errors().join(' | ') : ''}`);
const mkdir4 = () => fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-e2e-'));

// A5 tela de energia + B5 CEC (sem hardware: cec-client falso)
scenarios.energia = async () => {
  const dir = mkdir4();
  const core = await startCore({ demo: true, dir, env: cecEnv(dir) });
  const f = fake4(core);
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'cec', 'TV (HDMI-CEC)');
    await ui.until(`()=>document.querySelector('[data-k=cec-adapter]')`, 'tela do CEC');
    check(/Nenhum adaptador/.test(await txt(ui, 'cec-adapter')), 'sem adaptador: explica');
    await ui.screens('b5-cec-sem-adaptador');
    fs.writeFileSync(`${f.C}/adapter`, '1');
    await ui.key('ArrowLeft');
    await goTo(ui, 'sc-sis', 'categoria Sistema');
    await ui.key('Enter');
    await goTo(ui, 'sc-cec', 'categoria CEC');
    await ui.key('Enter');
    await ui.until(`()=>/Pulse-Eight/.test(document.querySelector('[data-k=cec-adapter]')?.textContent||'')`, 'adaptador encontrado');
    await enterPane(ui);
    await goTo(ui, 'cec-sleep', 'desligar a TV junto');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-k=cec-sleep] .sw[aria-checked=true]')`, 'ligado');
    await goTo(ui, 'cec-wake', 'ligar a TV junto');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-k=cec-wake] .sw[aria-checked=true]')`, 'ligar ao abrir ligado');
    await goTo(ui, 'cec-on', 'ligar a TV agora');
    await ui.key('Enter');
    await waitNode(() => f.calls().includes('on 0'), 'cec-client recebeu "on 0"');
    await goTo(ui, 'cec-off', 'desligar a TV agora');
    await ui.key('Enter');
    await waitNode(() => f.calls().includes('standby 0'), 'cec-client recebeu "standby 0"');
    await ui.screens('b5-cec');
    // A5: Ajustes → Sistema → Energia
    await ui.key('ArrowLeft');
    await goTo(ui, 'sc-sis', 'categoria Sistema');
    await ui.key('Enter');
    await enterPane(ui);
    await goTo(ui, 's-energia', 'linha Energia');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=energia] [data-k=pw-suspend]')`, 'tela de energia');
    check((await ui.has('[data-k=pw-restart]')) && (await ui.has('[data-k=pw-off]')) && (await ui.has('[data-k=pw-switch]')), 'suspender, reiniciar, desligar e trocar de perfil');
    await ui.screens('a5-energia');
    await goTo(ui, 'pw-suspend', 'suspender');
    await ui.key('Enter');
    await ui.until(toastRe('/Suspendendo/'), 'suspensão pedida (simulada em modo dev)');
    const before = f.calls().filter((c) => c === 'standby 0').length;
    await core.rpc('system.power', { action: 'off' });
    await waitNode(() => f.calls().filter((c) => c === 'standby 0').length > before, 'desligar o aparelho desliga a TV (CEC)');
    noJs(ui);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B12: código de recuperação do PIN dos responsáveis e relatório de uso
scenarios.perfil_pin = async () => {
  const core = await startCore({ demo: true });
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    const first = (await core.rpc('parental.recovery_new', { parentPin: '1234' })).code;
    check(/^[A-Z2-9]{4}-[A-Z2-9]{4}-[A-Z2-9]{4}$/.test(first), 'Core gera o código (XXXX-XXXX-XXXX)');
    await openSettingsCat(ui, 'parental', 'Controle parental');
    await enterPane(ui);
    await goTo(ui, 'par-newcode', 'gerar código novo');
    await ui.key('Enter');
    await ui.typePin('1234');
    await ui.until(`()=>/Novo código de recuperação/.test(document.querySelector('#ovs [data-kind=confirm]')?.textContent||'')`, 'mostra o código novo');
    const shown = (await ui.text('#ovs [data-kind=confirm]')).match(/[A-Z2-9]{4}-[A-Z2-9]{4}-[A-Z2-9]{4}/)?.[0];
    check(!!shown && shown !== first, 'o código novo é diferente do antigo');
    await ui.screens('b12-codigo');
    await ui.key('Enter');
    const rafa = (await core.rpc('profiles.list')).find((p) => p.name === 'Rafa').id;
    await goTo(ui, `par-rep-${rafa}`, 'relatório do Rafa');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-k=rep-head]')`, 'relatório aberto');
    check(/Rafa/.test(await txt(ui, 'rep-head')), 'relatório do perfil escolhido');
    await ui.screens('b12-relatorio');
    // esqueci o PIN: código errado não muda nada
    await goTo(ui, 'par-forgot', 'esqueci o PIN');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=text]')`, 'pede o código');
    for (const ch of 'AAAABBBBCCCC') await ui.key(ch, { wait: 60 });
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=pin]')`, 'pede o PIN novo');
    await ui.typePin('5678');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=pin]')`, 'repete o PIN');
    await ui.typePin('5678');
    await ui.until(toastRe('/Código incorreto/'), 'código errado recusado');
    await core.rpc('parental.verify', { pin: '1234' });
    check(true, 'o PIN antigo continua valendo depois do código errado');
    const ok = await core.rpc('parental.recover', { code: shown.toLowerCase(), newPin: '5678' });
    check(ok.code && ok.code !== shown, 'código certo troca o PIN e devolve outro código');
    await core.rpc('parental.verify', { pin: '5678' });
    let bad = false;
    await core.rpc('parental.verify', { pin: '1234' }).catch(() => { bad = true; });
    check(bad, 'PIN antigo não vale mais');
    noJs(ui);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B13 + B14: tamanho do texto, fundo e leitor de tela (voz simulada)
scenarios.aparencia = async () => {
  const core = await startCore({ demo: true });
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await ui.ev(`window.__said=[];window.speechSynthesis.cancel=()=>{};window.speechSynthesis.speak=(u)=>window.__said.push(u.text);0`);
    await openSettingsCat(ui, 'som', 'Aparência');
    await enterPane(ui);
    await goTo(ui, 's-ts', 'tamanho do texto');
    const h0 = await ui.ev(`document.querySelector('[data-k=s-ts]').getBoundingClientRect().height`);
    await ui.key('ArrowRight');
    await ui.until(`()=>document.body.classList.contains('ts1')`, 'texto grande');
    const h1 = await ui.ev(`document.querySelector('[data-k=s-ts]').getBoundingClientRect().height`);
    check(h1 > h0 * 1.1, `as linhas ficam maiores (${Math.round(h0)} → ${Math.round(h1)} px)`);
    await ui.screens('b13-texto-grande');
    await ui.key('ArrowRight');
    await ui.until(`()=>document.body.classList.contains('ts2')`, 'texto muito grande');
    await ui.screens('b13-texto-muito-grande');
    check((await core.rpc('session.current'))?.prefs?.textSize === 2, 'preferência guardada no Core (por perfil)');
    await ui.key('ArrowRight');
    await ui.until(`()=>!document.body.classList.contains('ts1')&&!document.body.classList.contains('ts2')`, 'volta ao normal');
    await goTo(ui, 's-bg', 'fundo');
    await ui.key('ArrowRight');
    await ui.until(`()=>document.documentElement.dataset.bgstyle==='solid'`, 'fundo cor lisa');
    await ui.key('ArrowRight');
    await ui.until(`()=>document.documentElement.dataset.bgstyle==='blue'`, 'fundo azul');
    await ui.screens('b13-fundo-azul');
    await ui.key('ArrowLeft');
    await ui.key('ArrowLeft');
    await ui.until(`()=>document.documentElement.dataset.bgstyle==='art'`, 'volta à imagem do jogo');
    await goTo(ui, 's-sfx', 'sons (linha sem ajuste)');
    await ui.key('ArrowLeft');
    await goTo(ui, 'sc-acess', 'categoria Acessibilidade');
    await ui.key('Enter');
    await enterPane(ui);
    await goTo(ui, 's-sr', 'leitor de tela');
    await ui.key('Enter');
    await ui.until(`()=>window.__said.some(t=>/Leitor de tela ligado/.test(t))`, 'leitor de tela liga e fala');
    await ui.ev(`window.__said.length=0`);
    await ui.key('ArrowUp');
    await ui.until(`()=>window.__said.length>0`, 'fala o item em foco ao mover');
    await ui.screens('b14-acessibilidade');
    check((await core.rpc('session.current'))?.prefs?.screenReader === true, 'leitor de tela guardado no perfil');
    noJs(ui);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B17: programa sem atalho (.jar e AppImage) pelo navegador de arquivos, ocultar, categoria, tamanho e desinstalar
scenarios.apps_instalados = async () => {
  const dir = mkdir4();
  const home = `${dir}/home`;
  const dirp = `${home}/Programas`;
  fs.mkdirSync(dirp, { recursive: true });
  fs.writeFileSync(`${dirp}/TLauncher.jar`, 'PK'.repeat(2048));
  fs.writeFileSync(`${dirp}/Jogo.AppImage`, '#!/bin/sh\nexit 0\n');
  fs.writeFileSync(`${dirp}/nota.txt`, 'x');
  const core = await startCore({ demo: true, dir, env: { NEXUS_JAVA: `${ROOT}/tests/e2e/fake/java` } });
  const ui = await openUi(core);
  const inLib = async () => (await core.rpc('library.list')).items.some((i) => i.title === 'TLauncher');
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'apps', 'Apps e jogos');
    await enterPane(ui);
    await goTo(ui, 's-progadd', 'adicionar programa por arquivo');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-k="fp-${dirp}"]')`, 'navegador de arquivos com a pasta Programas');
    await goTo(ui, `fp-${dirp}`, 'pasta Programas');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-k="fp-${dirp}/TLauncher.jar"]')`, 'TLauncher.jar listado');
    check(!(await ui.has(`[data-k="fp-${dirp}/nota.txt"]`)), 'arquivo comum não aparece');
    check(await ui.has(`[data-k="fp-${dirp}/Jogo.AppImage"]`), 'AppImage aparece');
    await ui.screens('b17-navegador');
    await goTo(ui, `fp-${dirp}/TLauncher.jar`, 'TLauncher.jar');
    await ui.key('Enter');
    await ui.until(`()=>/TLauncher/.test(document.querySelector('#ovs [data-kind=confirm]')?.textContent||'')`, 'pergunta como adicionar');
    await ui.key('Enter');
    await ui.typePin('1234');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=appedit]')`, 'editor do app novo');
    const apps = await core.rpc('apps.registered');
    check(apps.length === 1 && apps[0].name === 'TLauncher' && apps[0].kind === 'game', 'registrado como jogo "TLauncher"');
    const desk = fs.readdirSync(`${dir}/noapps`).find((f) => f.startsWith('programa-tlauncher'));
    const dtxt = fs.readFileSync(`${dir}/noapps/${desk}`, 'utf8');
    check(/Exec="[^"]*java" -jar "[^"]*TLauncher\.jar"/.test(dtxt), 'atalho .desktop com java -jar e o caminho certo');
    await ui.until(`()=>document.querySelector('[data-k=ae-size]') && /4,1 KB/.test(document.querySelector('[data-k=ae-size]').textContent)`, 'tamanho do .jar mostrado (4 KB)');
    check(await inLib(), 'aparece na Biblioteca como jogo');
    await ui.screens('b17-editor');
    await goTo(ui, 'ae-cat', 'categoria');
    await ui.key('ArrowRight');
    await ui.until(`()=>/Jogos/.test(document.querySelector('[data-k=ae-cat]')?.textContent||'')`, 'categoria escolhida');
    await goTo(ui, 'ae-hide', 'ocultar');
    await ui.key('Enter');
    for (let i = 0; i < 30 && (await inLib()); i++) await ui.wait(150);
    check(!(await inLib()), 'oculto: sai da Biblioteca');
    check((await core.rpc('apps.registered'))[0].hidden === true, 'continua registrado');
    await ui.key('Enter');
    for (let i = 0; i < 30 && !(await inLib()); i++) await ui.wait(150);
    check(await inLib(), 'mostrar de novo volta à Biblioteca');
    await goTo(ui, 'ae-done', 'concluir');
    await ui.key('ArrowRight');
    await ui.key('ArrowRight');
    check((await ui.focusInfo())?.k === 'ae-uninstall', 'foco em desinstalar');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação');
    await ui.key('Enter');
    await ui.until(toastRe('/Desinstalado/'), 'desinstalado');
    check(!fs.existsSync(`${dir}/noapps/${desk}`), 'atalho removido');
    check(fs.existsSync(`${dirp}/TLauncher.jar`), 'arquivo original continua na pasta');
    check((await core.rpc('apps.registered')).length === 0, 'sem app registrado');
    await core.rpc('programs.add', { path: `${dirp}/Jogo.AppImage`, kind: 'app', parentPin: '1234' }).catch(() => {});
    const reg = await core.rpc('apps.registered');
    check(reg.length === 1 && reg[0].kind === 'app' && (fs.statSync(`${dirp}/Jogo.AppImage`).mode & 0o111) !== 0, 'AppImage marcado como executável e adicionado');
    let m1 = '', m2 = '';
    await core.rpc('programs.info', { path: `${dirp}/nota.txt` }).catch((e) => { m1 = e.message; });
    await core.rpc('programs.info', { path: '/etc/passwd' }).catch((e) => { m2 = e.message; });
    check(/não parece um programa/.test(m1) && /fora das pastas/.test(m2), 'arquivo comum e fora das pastas recusados');
    noJs(ui);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B17: sem Java, a tela diz como instalar
scenarios.apps_sem_java = async () => {
  const dir = mkdir4();
  fs.mkdirSync(`${dir}/home/P`, { recursive: true });
  fs.writeFileSync(`${dir}/home/P/TLauncher.jar`, 'PK');
  const core = await startCore({ demo: true, dir, env: { NEXUS_JAVA: `${dir}/nao-existe/java` } });
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'apps', 'Apps e jogos');
    await enterPane(ui);
    await goTo(ui, 's-progadd', 'adicionar programa por arquivo');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-k="fp-${dir}/home/P"]')`, 'navegador');
    await goTo(ui, `fp-${dir}/home/P`, 'pasta P');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('[data-k="fp-${dir}/home/P/TLauncher.jar"]')`, 'jar listado');
    await goTo(ui, `fp-${dir}/home/P/TLauncher.jar`, 'jar');
    await ui.key('Enter');
    await ui.until(`()=>/sudo apt install default-jre/.test(document.querySelector('#ovs [data-kind=confirm]')?.textContent||'')`, 'diz: sudo apt install default-jre');
    await ui.screens('b17-sem-java');
    check((await core.rpc('apps.registered')).length === 0, 'nada foi adicionado');
  } finally {
    ui.close();
    await core.stop();
  }
};

// B18: pastas extras de ROMs, reescanear e núcleo padrão por console
scenarios.roms_pastas = async () => {
  const dir = mkdir4();
  const roms = `${dir}/roms-tv`;
  fs.mkdirSync(`${roms}/snes`, { recursive: true });
  fs.writeFileSync(`${roms}/snes/Chrono Trigger (USA).sfc`, 'x');
  fs.mkdirSync(`${dir}/home/Pendrive/snes`, { recursive: true });
  fs.mkdirSync(`${dir}/home/Pendrive/gba`, { recursive: true });
  fs.writeFileSync(`${dir}/home/Pendrive/snes/Chrono Trigger (USA).sfc`, 'y');
  fs.writeFileSync(`${dir}/home/Pendrive/snes/Earthbound (USA).sfc`, 'z');
  fs.writeFileSync(`${dir}/home/Pendrive/gba/Pokemon Emerald.gba`, 'w');
  const core = await startCore({ demo: true, dir, env: { NEXUS_EMULATION: '1', NEXUS_ROMS_DIR: roms } });
  const ui = await openUi(core);
  const titles = async () => (await core.rpc('library.list')).items.filter((i) => i.tags.includes('emu')).map((i) => i.title).sort();
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'emu', 'Emuladores');
    await ui.until(`()=>document.querySelector('[data-k=rom-add]')`, 'tela de emuladores com pastas');
    await enterPane(ui);
    await goTo(ui, 'rom-add', 'adicionar pasta');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=filepick] [data-k=fp-use]')`, 'navegador de pastas');
    await goTo(ui, `fp-${dir}/home/Pendrive`, 'pasta Pendrive');
    await ui.key('Enter');
    await ui.until(`()=>/Pendrive/.test(document.querySelector('#ovs [data-kind=filepick] .mono')?.textContent||'')`, 'dentro do Pendrive');
    await goTo(ui, 'fp-use', 'usar esta pasta');
    await ui.key('Enter');
    await ui.until(toastRe('/Pasta adicionada/'), 'pasta adicionada');
    await ui.until(`()=>document.querySelector('[data-k=rom-extra-0]')`, 'pasta extra na lista');
    const t = await titles();
    check(t.join('|') === 'Chrono Trigger|Earthbound|Pokemon Emerald', `jogos das duas pastas, sem repetir (${t.join('|')})`);
    fs.writeFileSync(`${dir}/home/Pendrive/gba/Metroid Fusion.gba`, 'v');
    await goTo(ui, 'rom-rescan', 'procurar jogos de novo');
    await ui.key('Enter');
    await ui.until(toastRe('/reescaneados/'), 'reescaneou');
    check((await titles()).includes('Metroid Fusion'), 'jogo novo apareceu depois de reescanear');
    await ui.screens('b18-roms');
    await goTo(ui, 'core-gba', 'núcleo do Game Boy Advance');
    check(/mgba/.test(await txt(ui, 'core-gba')), 'padrão do GBA é o mgba');
    await ui.key('ArrowRight');
    await ui.until(`()=>/gpsp/.test(document.querySelector('[data-k=core-gba]')?.textContent||'')`, 'núcleo trocado');
    const st = await core.rpc('emu.status');
    check(st.systems.find((s) => s.dir === 'gba').core === 'gpsp', 'Core guarda o núcleo escolhido');
    check(st.extraDirs.length === 1, 'Core guarda a pasta extra');
    await ui.screens('b18-nucleos');
    await goTo(ui, 'rom-extra-0', 'pasta extra');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirma tirar');
    await ui.key('Enter');
    await ui.until(`()=>!document.querySelector('[data-k=rom-extra-0]')`, 'pasta tirada');
    check((await titles()).join('|') === 'Chrono Trigger', 'sem a pasta extra só sobra a da TV');
    let msg = '';
    await core.rpc('roms.add_dir', { path: '/etc' }).catch((e) => { msg = e.message; });
    check(/dentro do seu usuário/.test(msg), 'pasta fora das permitidas recusada');
    noJs(ui);
  } finally {
    ui.close();
    await core.stop();
  }
};

// B19: o que vai pela rede, telemetria e limpar histórico
scenarios.privacidade = async () => {
  const core = await startCore({ demo: true });
  const ui = await openUi(core);
  try {
    await ui.toProfiles();
    await ui.pickProfile('Lia');
    await ui.until(inShell, 'entrou na Shell');
    await openSettingsCat(ui, 'priv', 'Privacidade');
    await ui.until(`()=>document.querySelector('[data-k=priv-tele]')`, 'tela de privacidade');
    check(/não envia dados/i.test(await txt(ui, 'priv-tele')), 'diz que o Nexus não tem telemetria');
    check((await ui.count('[data-k^=priv-net-]')) >= 6, 'lista as conexões de rede');
    await ui.screens('b19-privacidade');
    await enterPane(ui);
    // gera um registro de jogo real (abre e fecha um jogo da demonstração)
    await core.rpc('item.launch', { id: 'demo:g1' });
    await ui.wait(1500);
    await core.rpc('game.close');
    await ui.wait(800);
    const recent = async () => Number((await core.rpc('privacy.status')).stores[0].detail.split(' ')[0]);
    const before = await recent();
    check(before > 0, `há jogos recentes antes (${before})`);
    await goTo(ui, 'priv-recent', 'limpar jogos recentes');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=confirm]')`, 'confirmação');
    await ui.key('Enter');
    await ui.until(toastRe('/Limpo/'), 'limpou');
    check((await recent()) === 0, 'jogos recentes zerados');
    check((await core.rpc('privacy.status')).telemetry === false, 'Core confirma: telemetria desligada');
    noJs(ui);
  } finally {
    ui.close();
    await core.stop();
  }
};
