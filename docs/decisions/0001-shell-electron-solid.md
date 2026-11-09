# ADR 0001 — Shell em Electron + SolidJS (com gate)
Escolhido por reaproveitar o protótipo aprovado e compartilhar o design system com o celular.
**Gate S4** (hardware real): RAM ociosa ≤ 400 MB, primeiro quadro ≤ 3 s, ≥ 55 fps. Medido em Xvfb/GL por software:
primeiro quadro ~0,3 s; RAM ~750 MB (processos somados). Se a GPU real não baixar isso, trocar a Shell por Qt/QML —
a fronteira é o protocolo JSON-RPC (`packages/protocol`), então o Core não muda.
