# Pesquisa: o que leva o Keyforge a 10 (mercado, UX, recursos)

Data: 2026-11-02. Complementa `PLANO-PRODUTO.md` (não repete o que já está lá; aponta o que o mercado
mostra e onde o plano atual tem lacunas). Cada item traz a fonte.

## 1. O que a concorrência já entrega (a régua)

| Produto | O que define a régua | Fonte |
|---|---|---|
| Elgato Stream Deck 7.x | **Key Logic**: até 3 ações por tecla (toque, toque duplo, segurar) em um único bloco | https://www.elgato.com/us/en/explorer/products/stream-deck/key-logic-stream-deck/ |
| Elgato | **Smart Profiles**: troca de perfil pelo app em foco | https://help.elgato.com/hc/en-us/articles/360053419071 |
| Elgato | **Multi Actions** (sequência com intervalos) e Multi Action Switch (alternada) | https://help.elgato.com/hc/en-us/articles/360027960912 |
| Elgato | Perfis, páginas e **pastas** como os 3 níveis de organização | https://marketplace.elgato.com/learn/inspiration/Profiles-Pages-Folders |
| Elgato | Backup/restauração `.StreamDeckProfilesBackup`, perfil importado com duplo clique | https://help.elgato.com/hc/en-us/articles/360048424432 |
| Elgato 7.6 | **Infobar** (tela de informação no Neo) com ações arrastadas: tocando agora, CPU/GPU, clima, status de agente de IA | https://help.elgato.com/hc/en-us/articles/51156609278737 |
| Elgato | **Virtual Stream Deck** (até 8×8, invocado no cursor) e app de celular | https://www.elgato.com/us/en/explorer/products/stream-deck/virtual-stream-deck-beta-overview/ |
| Elgato SDK | Regras de UX para plugins: feedback visual em toda ação, consolidar ações parecidas, inspetor enxuto | https://docs.elgato.com/guidelines/stream-deck/plugins |
| Bitfocus Companion | **Feedbacks** (estado do aparelho muda o estilo da tecla), **variáveis**, **expressões** (fórmulas), **triggers** (por tempo, intervalo, evento), botões via HTTP/OSC/WebSocket, emulador e página web | https://bitfocus.io/companion · https://companion.free/user-guide/beta/expressions/ |
| Touch Portal | Estados dinâmicos, eventos ("quando X mudar, faça Y"), ações dinâmicas de plugins | https://www.touch-portal.com/api/index.php?section=description_file_actions |
| Macro Deck | Variáveis `{{ vars.x }}`, eventos, provedores de música/clima/calendário/e-mail, cliente web no celular, "problemas da integração" explicados ao usuário | https://docs.macro-deck.app/features/ |
| Loupedeck/Logi Options+ | Perfis por app automáticos, Actions Ring (anel na tela), ajustes por parâmetro | https://support.loupedeck.com/profiles.html · https://support.logi.com/hc/en-us/articles/17844647906967 |
| OpenDeck (Rust, Linux) | Roda **plugins do SDK da Elgato** e da API aberta **OpenAction**; perfis portáveis entre SOs; Multi/Toggle Actions, troca por app | https://github.com/nekename/OpenDeck |
| StreamController (Linux) | Pacotes de ícones e fundos, vídeo de fundo, troca de página pela janela ativa, **trava o deck quando o sistema trava**, rotação | https://streamcontroller.github.io/docs/latest/ |

**Conclusão:** o Keyforge já empata em muita coisa (pastas, perfil automático, teclas vivas, OBS, lateral
com alertas, PressHold/DoubleTap, sons, multi-ação, exportar/importar). Ele perde em quatro frentes:
**ecossistema (plugins)**, **automação reativa (variáveis/condições/gatilhos)**, **outras superfícies
(celular/virtual)** e **acabamento de configuração (Key Logic unificado, macro gravável, backup)**.

## 2. Recomendações de recursos (ordem de valor/custo)

### P0: fecham a distância para o Stream Deck
1. **Key Logic unificado.** Hoje `PressHold` e `DoubleTap` são tipos de ação separados. Trocar por uma
   seção "Como apertar" em toda tecla: Toque / Toque duplo / Segurar, cada uma com sua ação. Mesmo modelo
   mental da Elgato 7.0 e evita escolher "tipo" antes da ação. (Fonte: Key Logic, acima.)
2. **Multi-ação de verdade** (já marcada nota 5 no plano): lista de passos arrastáveis, passo "Esperar",
   modo **Alternada** (liga/desliga com dois ícones) e **"Testar passo a passo"**. (Elgato Multi Actions.)
3. **Gravador de macro.** Botão "Gravar" que captura teclas (e opcionalmente cliques) e gera passos de
   multi-ação editáveis, com tempos arredondados e "ignorar pausas longas". Linux: `evdev`/libinput leitura
   via `/dev/input` (exige grupo `input`) ou portal `org.freedesktop.portal.GlobalShortcuts`/`RemoteDesktop`
   no Wayland; Windows: `SetWindowsHookEx(WH_KEYBOARD_LL)`. Mostrar aviso de privacidade antes de gravar.
   (Mercado: plugin Macro Recorder da GRAMSHIFT, https://gramshift.com/apps/macro-recorder/.)
4. **Perfil automático completo**: escolher o app pela lista com ícone (não digitar), várias regras por
   perfil, regra por **título da janela** (regex) além da classe, e "voltar ao anterior". No Hyprland ler o
   evento `activewindow` do socket `.socket2.sock` em vez de sondar. (Smart Profiles; StreamController.)
5. **Backup automático + "Restaurar…"** (últimas N versões, com data e prévia) e exportação de
   **pacote** (`.keyforge` = zip com config + ícones + sons), importação por arrastar o arquivo, com lista
   "o que este perfil executa" (comandos, webhooks) antes de aceitar. (Elgato backup; segurança: perfis
   com `RunCommand` são vetor de ataque.)

### P1: passam o Stream Deck (onde o Companion/Touch Portal ganham)
6. **Variáveis e estados** com sintaxe simples (`{obs.gravando}`, `{volume}`, `{hora}`, `{clip}`) usáveis
   no rótulo da tecla, em Texto, Webhook e Comando. O `platform/live.rs` já coleta os dados: é expor.
   (Companion variables; Macro Deck `{{ vars.x }}`.)
7. **Feedbacks/condições**: "se `{obs.ao_vivo}` então fundo vermelho" e "se X faz A senão B" numa
   multi-ação. Começar com 3 operadores (igual, maior, contém), sem linguagem de expressões completa.
8. **Gatilhos (triggers)** sem tecla: horário, intervalo, app abriu/fechou, OBS começou a gravar, aparelho
   conectou, sessão travou. Ex.: "às 22h brilho 20%"; "ao travar a sessão, apagar o deck" (StreamController
   faz isso). (Companion triggers: https://github.com/bitfocus/companion/blob/adf71c2f/companion/lib/Resources/EventDefinitions.ts)
9. **Compatibilidade OpenAction / plugins Elgato.** O maior multiplicador de valor: o OpenDeck (também Rust)
   prova que dá para hospedar plugins do SDK da Elgato via WebSocket. Mesmo que o plano A8 crie um formato
   próprio, adotar o protocolo OpenAction dá centenas de integrações (Spotify, Discord, Home Assistant,
   Twitch) sem escrevê-las. Avaliar antes de desenhar o formato próprio. (https://openaction.amankhanna.me/)
10. **API local** (HTTP/WebSocket em `127.0.0.1`, com token): "apertar tecla N", "mudar página",
    "definir variável". Permite automação externa (scripts, Home Assistant) e é a base do deck no celular.
    O protocolo JSON versionado editor↔serviço já existe: expor um subconjunto. (Companion.)

### P2: superfícies e conforto
11. **Deck no celular / navegador** (página web local servida pelo serviço, QR code no editor). Elgato,
    Macro Deck e Companion têm. Dá teclas extras e controle remoto sem custo de hardware.
12. **Deck virtual na tela** invocado por atalho, no cursor (Virtual Stream Deck); no Omarchy, como
    janela flutuante Hyprland com regra de posição.
13. **Sincronização entre PCs sem conta na nuvem** (o plano proíbe conta: bom). Alternativa: escolher
    uma "pasta de sincronização" (Syncthing/Dropbox/Nextcloud/Git) onde a config vive; o serviço observa o
    arquivo (já faz hot-reload?) e resolve conflito por versão + backup. Formato texto estável e ordenado
    para diffs limpos no Git.
14. **Biblioteca da comunidade de perfis/ícones** mínima: importar pacote por URL; compatibilidade de
    importação com pacotes de ícones do Stream Deck (PNG 144×144). (Elgato icon packs; Streamline.)
15. **Tela lateral como "Infobar" com slots**: arrastar uma ação para a lateral como numa tecla (Elgato
    7.6 fez exatamente isso no Neo). O Keyforge já tem modos; transformar em slots componíveis
    (relógio + mic + música) é o passo seguinte.

## 3. UX para hardware com telas pequenas (teclas de ~85–100 px)

- **Uma informação por tecla**, no máximo ícone + 1 rótulo curto (≤ 8–10 caracteres); números grandes
  para valores vivos. Rótulo nunca sobre área de alto contraste do ícone. (Guia de ícones Elgato/SDK:
  https://docs.elgato.com/streamdeck/sdk/guides/keys/)
- **Feedback em toda ação** (Elgato exige isso de plugins): ✓ curto ao concluir, ⚠ ao falhar (o SDK usa
  `showOk`/`showAlert`). O Keyforge deve piscar a tecla em erro de webhook/OBS em vez de falhar calado.
- **Estado por cor + forma, não só cor**: vermelho + ícone riscado para mudo; daltonismo afeta ~8% dos
  homens. Já seguir isso no "MIC MUDO" (texto + cor: bom).
- **Contraste**: fundo escuro, ícone claro, contraste ≥ 4.5:1 (WCAG 1.4.3) também no aparelho; o "Embelezar"
  deve validar contraste do rótulo contra o degradê.
- **Hierarquia por posição**: as ações mais usadas na fileira de cima/canto; "Voltar" sempre na mesma
  posição (pastas já fazem: manter fixo em todas).
- **Navegação previsível**: indicador de página (pontos) na lateral, e "segurar Pág+ = ir à página 1".
- **Latência**: aparelho só aceita imagens pequenas por vez; priorizar a tecla apertada no envio
  (feedback < 100 ms) e só depois as teclas vivas.
- **Modo bloqueado/privacidade**: apagar ou trocar por relógio quando a sessão trava (StreamController).

## 4. Editor (egui): design system e acessibilidade

- **AccessKit já vem ligado** (feature padrão do `eframe` 0.37) e os testes `egui_kittest` usam a mesma
  árvore. Garantir: todo botão só-ícone com `on_hover_text` **e** rótulo acessível
  (`response.widget_info(|| WidgetInfo::labeled(...))`); teclas da grade anunciadas como
  "Tecla 3: Mic mudo, ligado". Testar com Orca (Linux) e Narrador (Windows).
  (https://accesskit.dev/looking-back-looking-forward/ · https://github.com/emilk/egui/issues/167 ·
  https://github.com/rerun-io/kittest)
- **Teclado primeiro** (Linear): setas movem a seleção na grade, Enter abre o inspetor, Del limpa, Ctrl+C/V,
  Ctrl+K abre a CommandPalette com **todas** as ações (inclusive "ir para perfil X", "testar tecla").
  Mostrar o atalho ao lado de cada item do menu. (https://blakecrosley.com/guides/design/linear)
- **Foco visível** em todo controle (anel de 2 px na cor de destaque); ordem de Tab = ordem visual.
- **Escala**: respeitar `pixels_per_point` do sistema e oferecer 90–150% nas Preferências; textos ≥ 13 px.
- **Movimento reduzido**: opção que desliga deslize de página/fade (as animações já existem).
- **Tokens**: uma cor de destaque usada com disciplina (só ação primária, seleção, foco), neutros em
  escala, 4/8 px de grade, raio único. Linear/Vercel/Stripe convergem nisso; o projeto já tem
  `editor/tokens.rs` + `DESIGN.md`: auditar usos de cor fora dos tokens.
- **Estados vazios que ensinam** (já há "pontos de partida"); **erros com ação** ("OBS fechado → Abrir
  OBS / Configurar") como os "Integration issues" do Macro Deck.
- **Ligar o Onboarding e a CommandPalette** que existem mas não estão conectados: são ganho barato.

## 5. Roteiro sugerido (encaixe no `ROTEIRO.md`)

| Degrau | Adições desta pesquisa |
|---|---|
| 4 Editor bom | Key Logic unificado (1), multi-ação completa (2), teclado-primeiro + foco + AccessKit (seção 4), ligar Onboarding/Palette |
| 6 Fácil de ter | Backup/restaurar e pacote `.keyforge` com prévia de segurança (5), pasta de sincronização (13) |
| 7 Mais que os outros | Variáveis (6), condições (7), gatilhos (8), gravador de macro (3), lateral com slots (15) |
| 7b Ecossistema | Decidir OpenAction vs. formato próprio (9); API local (10) |
| fora do escopo atual | Deck no celular (11), deck virtual (12), biblioteca da comunidade (14) |

Maior alavanca isolada: **(9) plugins compatíveis com OpenAction/Elgato**. Maior ganho de UX barato:
**(1) Key Logic unificado + ligar Palette/Onboarding**.
