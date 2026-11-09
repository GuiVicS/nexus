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
    check((await ui.count('.setrail .ri')) === 3, 'só 3 categorias nos ajustes infantis');
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
