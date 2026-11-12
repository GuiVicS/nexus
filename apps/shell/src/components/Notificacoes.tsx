// Ajustes → Notificações (A4): "silenciar durante jogos" e avisos de teste. A fila curta (no máximo 3 avisos na tela,
// sem repetir) está em `toast()`; o Core não deixa o mesmo aviso entrar duas vezes em 2 minutos.
import { onCleanup, onMount, Show } from 'solid-js';
import { api } from '../lib/api';
import { askConfirm, fail, guarded, S, setS, toast } from '../lib/state';
import { sfx } from '@nexus/ui';
import { Icon, Switch } from './ui';

export function NotifPanel() {
  onMount(() => {
    void api.notificationSettings().then((s) => setS('notifSilence', s.silenceInGame)).catch(fail);
    const h = async (e: Event) => {
      const a = (e as CustomEvent<string>).detail;
      try {
        if (a === 'silence') {
          const on = !S.notifSilence;
          const r = await guarded((x) => api.notificationSet({ silenceInGame: on, ...x }), 'Conta da casa: PIN dos responsáveis');
          if (r) {
            setS('notifSilence', r.silenceInGame);
            sfx(r.silenceInGame ? 'on' : 'off');
          }
        } else if (a === 'test' || a === 'testi') {
          await api.notificationTest(a === 'testi');
        } else if (a === 'clear') {
          if (!S.notifs.length) return toast('Nada para limpar', '', 'check');
          if (!(await askConfirm({ title: 'Limpar as notificações?', sub: `${S.notifs.length} aviso(s) serão apagados.`, yes: 'Limpar' }))) return;
          await api.clearNotifications();
          setS('notifs', []);
          toast('Notificações limpas', '', 'trash');
        }
      } catch (e2) {
        fail(e2);
      }
    };
    document.addEventListener('nexus:ntf', h);
    onCleanup(() => document.removeEventListener('nexus:ntf', h));
  });
  return (
    <>
      <div class="srow f" data-k="nt-silence" data-r="22" data-a="ntf:silence" data-verb="Alternar">
        <div><b>Silenciar durante jogos</b><small>Com um jogo aberto, só os avisos importantes aparecem por cima dele (bateria baixa, disco cheio, erros). O resto espera no sino.</small></div>
        <Switch on={S.notifSilence} />
      </div>
      <div class="srow f" data-k="nt-test" data-r="22" data-a="ntf:test" data-verb="Enviar"><div><b>Enviar um aviso de teste</b><small>Aparece no canto da tela e fica no sino</small></div><Icon n="bell" /></div>
      <div class="srow f" data-k="nt-testi" data-r="22" data-a="ntf:testi" data-verb="Enviar"><div><b>Enviar um aviso importante de teste</b><small>Passa por cima do jogo mesmo com “Silenciar durante jogos” ligado</small></div><Icon n="info" /></div>
      <div class="srow f" data-k="nt-clear" data-r="22" data-a="ntf:clear" data-verb="Limpar"><div><b>Limpar notificações</b><small>{S.notifs.length ? `${S.notifs.length} no sino` : 'Nenhuma no sino'}</small></div><Icon n="trash" /></div>
      <Show when={S.notifs.length}>
        <h3 class="sh2">Recentes</h3>
        {S.notifs.slice(0, 5).map((n) => (
          <div class="srow info"><div><b>{n.title}</b><small>{n.body}</small></div></div>
        ))}
      </Show>
    </>
  );
}
