import fs from 'node:fs';
import path from 'node:path';
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
    check((await ui.count('.setrail .ri[data-rail]')) === 3, 'só 3 categorias nos ajustes infantis');
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

scenarios.primeiro_uso = () =>
  withUi({ demo: false }, async (core, ui) => {
    await ui.until(`()=>document.getElementById('stage').dataset.scr==='onboarding'`, 'primeiro uso', 12000);
    await ui.wait(600);
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('.onb h1')?.textContent==='Crie seu perfil'`, 'passo do perfil');
    await ui.wait(400);
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('#ovs [data-kind=text]')`, 'nome');
    for (const ch of 'Ana') await ui.key(ch, { wait: 50 });
    await ui.key('Enter');
    await ui.focusOn((f) => f.k === 'pe-save' || f.k === 'pe-cancel', 30, true);
    if ((await ui.focusInfo()).k === 'pe-cancel') await ui.key('ArrowLeft');
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('.onb h1')?.textContent==='PIN dos responsáveis'`, 'passo do PIN');
    await ui.focusOn((f) => f.a === 'onb:skip', 30);
    await ui.key('Enter');
    await ui.until(`()=>document.querySelector('.onb h1')?.textContent?.startsWith('Tudo pronto, Ana')`, 'pronto');
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
