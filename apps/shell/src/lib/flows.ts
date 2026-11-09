import { api } from './api';
import { askConfirm, askPin, fail, S, setS, toast } from './state';

export async function setParentalPin() {
  const had = S.parental.hasPin;
  let current: string | undefined;
  if (had) {
    const c = await askPin({ title: 'PIN atual dos responsáveis' });
    if (c === null) return;
    current = c;
  }
  const n = await askPin({ title: had ? 'Novo PIN' : 'Crie o PIN dos responsáveis', sub: 'De 4 a 6 números. Ele protege perfis infantis, compras e ajustes.', confirm: true });
  if (n === null) return;
  try {
    setS('parental', await api.parentalSet(n, current));
    setS('grant', null);
    toast('PIN dos responsáveis salvo', '', 'lock');
  } catch (e) {
    fail(e);
  }
}

export async function removeParentalPin() {
  const c = await askPin({ title: 'PIN dos responsáveis', sub: 'Digite para remover a proteção' });
  if (c === null) return;
  if (!(await askConfirm({ title: 'Remover o PIN dos responsáveis?', sub: 'Perfis infantis continuam, mas ficam sem proteção para sair.', yes: 'Remover', danger: true }))) return;
  try {
    setS('parental', await api.parentalSet(null, c));
    setS('grant', null);
    toast('PIN removido', '', 'lock');
  } catch (e) {
    fail(e);
  }
}
