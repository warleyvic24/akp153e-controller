# Plano de produto: interface e funções até a nota 10

Data: 2026-10-31. Escrito como um designer sênior de produto: cada tela e cada função com nota, problemas e o que
é "10". Baseado no código, nas capturas do editor (temas claro e escuro, 15 e 6 teclas) e no uso no aparelho.
Onde a nota vem só da leitura do código, e não de uso real, está marcado *(código)*.

Este arquivo diz **o que** fazer. A **ordem** está em `docs/ROTEIRO.md`; os detalhes visuais em
`docs/PLANO-UX.md`; os aparelhos em `docs/PLANO-APARELHOS.md`.

**Regra de "10":** a pessoa faz a tarefa sem ler nada, sem erro, e o resultado aparece no aparelho na hora.
Se precisar de explicação, não é 10.

---

## Parte A: interface, tela por tela

### A1. Estrutura da janela (hoje 5)
**Problemas:** três colunas fixas (biblioteca, palco, inspetor) que disputam espaço. Em 1366 px tudo
aperta; em 1920 px sobra vazio no meio. A janela era translúcida no Omarchy (corrigido na sua máquina, não
no pacote).
**10:**
- Colunas que se ajustam: biblioteca recolhível (ícone na borda), inspetor de 360 a 420 px, palco com o resto.
- Abaixo de 1100 px a biblioteca vira gaveta.
- A regra de janela opaca vai no pacote como `packaging/linux/hypr-keyforge.lua`, com instrução de uma linha.
- Tamanho e posição da janela lembrados.
**Aceite:** capturas em 1280, 1366, 1920 e 2560 sem corte e sem área vazia maior que um terço do palco.

### A2. Barra do topo (hoje 5)
**Problemas:**
- os selos "CONECTADO" e "Salvo" são iguais e em fonte de máquina de escrever;
- "Salvo" é ruído, porque a gravação é automática;
- "Perfil" aparece duas vezes (o menu e o botão "Perfil e página");
- não existe a informação que importa: o que está no aparelho é igual ao que está na tela?

**10:** `Keyforge` · `[ Streaming ▾ ]` · ··· · `● No aparelho` · `↶ ↷` · `⚙`.
- O selo de sincronia tem quatro estados: "No aparelho" (verde), "Enviando…", "Desconectado · Reconectar"
  (clicável) e "Serviço parado · Iniciar".
- O menu de perfil traz os perfis, "Novo perfil", "Duplicar", "Gerenciar…".
- Erro de gravação vira uma faixa vermelha fixa até ser resolvido.

**Aceite:** no máximo 6 elementos no topo; nenhum texto em caixa alta.

### A3. Biblioteca de ações (hoje 5)
**Problemas:**
- lista plana com 40+ itens e o mesmo peso para tudo;
- "Copiar", "Colar" e "Recortar" no topo (ninguém compra stream deck para Ctrl+C);
- os atalhos do Windows (Calculadora, Bloco de notas, Explorador, Win+D) já somem no Linux (`WINDOWS_ONLY`),
  mas o Linux não ganha equivalentes (Omarchy: menu, captura, workspace);
- "Clique para aplicar" não diz em qual tecla;
- linhas de 27 px.

**10:**
- Ordem nova:
  1. **Busca**.
  2. **Recentes** (as 6 últimas usadas).
  3. **Seus apps** (os 8 mais usados do sistema, com ícone real).
  4. Mídia e som; Transmissão (OBS); Páginas e perfis; Sistema (brilho, relógio, monitor, bloquear, captura);
     Web; Avançado (comando, texto, multi-ação, webhook); Edição (copiar, colar…) por último e recolhida.
- Atalhos e apps por sistema operacional (Linux: `hyprctl`/`omarchy`; Windows: os de hoje).
- Linha de 32 px com ícone e nome; ao passar o mouse: "Pôr na tecla 3 ↵".
- Arrastar mostra a tecla desenhada.
- Seção recolhível e lembrada.

**Aceite:** pôr o Discord numa tecla em 2 gestos; nenhum item que não funciona no sistema atual.

### A4. Palco: o aparelho (hoje 6)
**Problemas:**
- os "25%" das teclas de volume ficam fora da tecla;
- o número aparece só na tecla vazia;
- a barra fixa de atalhos (DEL, CTRL C…) nunca some;
- "Embelezar página" é vago e muda tudo de uma vez;
- falta foco visível ao navegar por teclado (a conferir);
- tecla vazia com "+" e o número: bom, mas o "+" é discreto demais.

**10:**
- **O palco mostra o que o aparelho mostra**, nem mais nem menos; números só ao passar o mouse.
- Tecla vazia: "+" claro; clicar abre a biblioteca já filtrada; arrastar um arquivo de imagem funciona (já
  existe).
- Menu de contexto (botão direito): Testar, Copiar, Colar, Duplicar em…, Limpar, Trocar com…
- Barra de atalhos só nas 3 primeiras aberturas; depois um "?" (folha de atalhos).
- "Embelezar" vira **"Organizar cores"**, com prévia lado a lado e "Aplicar"/"Cancelar".
- Seleção de várias teclas (Shift/Ctrl): mudar cor, limpar ou mover em grupo.

**Aceite:** a captura do palco e uma foto do aparelho são iguais.

### A5. Abas de página (hoje 4)
**Problemas:** só "Live +", pequena e longe do aparelho; renomear, reordenar e excluir ficam em outra janela.
**10:**
- Fileira de abas colada ao aparelho: nome, um ponto na aba ativa no aparelho, "+".
- Duplo clique renomeia; arrastar reordena; botão direito: Duplicar, Excluir (com Desfazer), Tela lateral
  desta página.
- Com muitas páginas, rolagem com setas.
- Atalhos Ctrl+PgUp/PgDn.

**Aceite:** criar, renomear e reordenar uma página sem abrir janela.

### A6. Inspetor: cabeçalho e ações da tecla (hoje 6)
**Problemas:**
- o "Testar" ciano é o elemento mais forte da tela;
- copiar, colar e apagar são só ícones, e o apagar é vermelho e fica colado ao Testar;
- a prévia é pequena;
- o nome só se edita na aba Aparência.

**10:**
- Prévia de 96 px, igual ao aparelho, e o nome editável ali mesmo (clicar no título).
- "Testar" vira botão secundário; o menu "⋯" traz Copiar, Colar, Duplicar e Limpar, com o atalho ao lado.
- Resumo de uma linha ("Abre: Discord").

### A7. Inspetor: aba Ação (hoje 6,5; melhorou em 2026-10-31)
**Feito:** o tipo virou uma linha compacta; a grade aparece só na tecla vazia.
**Problemas:**
- campos técnicos na primeira camada: caminho do programa, argumentos, método HTTP, atalho digitado
  ("ctrl+shift+m");
- dois botões de larguras diferentes para escolher o app;
- erro só aparece como aviso na tecla.

**10:**
- Cada tipo tem um campo principal e um "Avançado" recolhido (detalhe por tipo na Parte B).
- Validação no próprio campo, em linguagem humana.
- "Ao segurar" e "Dois estados" viram **seções recolhíveis** dentro de Ação, com resumo quando fechadas
  ("Ao segurar: abre o OBS").

**Aceite:** tecla de app configurada sem rolar em 1366x768.

### A8. Inspetor: aba Aparência (hoje 6)
**Problemas:**
- a grade de ícones não tem ordem por relevância;
- "Outra" e "Texto" são quadrados de cor sem explicação;
- faltam fonte e tamanho do texto (a conferir *(código)*);
- não há "voltar ao automático".

**10:**
- Prévia ao vivo no topo, compartilhada com o cabeçalho.
- Rótulo com posição (Topo/Meio/Base, já existe) e tamanho (P/M/G).
- Cor:
  - 8 cores da paleta com nome ao passar o mouse;
  - "Cor personalizada…";
  - "Cor por categoria" (o automático do "Organizar cores");
  - cor do texto automática pelo contraste, com troca manual em "Avançado".
- Ícone:
  - busca;
  - "Sugeridos para esta ação" (o mixer sugere alto-falante);
  - "Recentes";
  - categorias da biblioteca Lucide;
  - "Imagem do computador…";
  - o ícone do app/site, que já vem sozinho.
- Fundo em degradê (já existe), imagem de fundo e **GIF animado** (ver C).
- "Restaurar aparência automática".

### A9. Seção "Dois estados" (hoje 4, como aba "Alternar")
**Problemas:** uma aba inteira para um interruptor e um parágrafo; não se vê os dois estados juntos.
**10:**
- Seção dentro de Ação: interruptor "Tecla de dois estados".
- Ligada, mostra as duas prévias lado a lado (Normal | Ligado), a ação de cada estado e a aparência do
  segundo, com um botão "Gerar automaticamente" (vermelho/ativo).
- Para OBS e Mudo, o estado vem da realidade (já funciona) e a seção diz isso.

### A10. Tela lateral (hoje 6)
**Problemas:**
- abre num balão separado *(código)*;
- os modos (Desligada, Nome da página, Relógio, Sistema, Imagem) não mostram prévia antes de escolher.

**10:** clicar na faixa abre o inspetor em modo "Tela lateral", com cartões de prévia de cada modo; a imagem
tem recorte ajustável. Modos novos: "Agora tocando" (capa e título) e "Texto livre".

### A11. Perfis e páginas: gerenciar (hoje 4)
**Problemas:** janela crua do egui; o perfil automático é uma regra de texto (nome do .exe ou classe da
janela) que o usuário comum não entende.
**10:**
- Modal "Gerenciar perfis": lista à esquerda; à direita nome, ícone, "Ativar sozinho quando…" com
  **escolha do app pela lista de apps abertos** (com ícone, sem digitar), Exportar, Duplicar, Excluir.
- Importar mostra a prévia e **lista o que o perfil vai executar** (comandos, webhooks) antes de aceitar
  (segurança).

### A12. Preferências (hoje 3)
**Problemas:** janela crua presa à esquerda; tudo num rolo; brilho só "ao salvar"; o primeiro item da cor de
destaque é um quadrado e os outros são círculos; "Diagnóstico" mistura versão, reconectar, logs e autoteste.
**10:** modal com seções:
- **Geral:** tema (Escuro, Claro, Omarchy, Sistema), cor, idioma, iniciar com o sistema, tamanho do texto.
- **Aparelho:** modelo conectado (com o selo verificado/experimental), brilho **ao vivo**, descanso, "Testar
  aparelho", "Ensinar meu aparelho" e "Reconectar".
- **Integrações:** OBS (estado ao vivo + "Testar conexão"), outras quando houver plugins.
- **Sobre:** versão, novidades, atualizações, logs, licenças.

### A13. Lista de apps (hoje 3)
**Problemas:** janela crua; letras coloridas no lugar dos ícones; não há recentes.
**10:** modal com ícones reais (o projeto já extrai), "Abertos agora" e "Mais usados" no topo, busca com
setas e Enter, e a tecla de prévia mudando ao passar o mouse.

### A14. Avisos, erros e estados vazios (hoje 6)
**10:**
- avisos com uma ação ("Limpo · Desfazer"), já existe;
- erros sempre dizem o que fazer;
- sem aparelho: ilustração do aparelho apagado + "Conecte o cabo" + "Meu aparelho não aparece";
- aparelho desconhecido: leva ao assistente;
- página vazia: os pontos de partida (Parte C).

### A15. Primeira execução (hoje 4)
**10:** em 3 telas:
1. Aparelho encontrado (foto do modelo).
2. Ponto de partida: Streaming, Trabalho, Mídia ou Em branco.
3. "Pronto: aperte uma tecla". A página escolhida já aparece no aparelho.

Nunca mais aparece; "Começar de novo" fica nas Preferências.

### A16. Bandeja do sistema (hoje 6 *(código)*)
**10:** menu com Abrir editor, a lista de perfis (trocar com um clique), Pausar aparelho, Testar aparelho e
Sair. O ícone muda com o estado: normal, desconectado ou pausado.

### A17. Tema, cor e Omarchy (hoje 7)
Bom: segue o tema do Omarchy e tem testes de contraste.
**10:** opção "Sistema" (segue claro/escuro do sistema); o tema claro com menos cinza sobre cinza; animação de
troca de tema.

### A18. Teclado e acessibilidade (hoje 6)
**10:**
- Tab circula entre as áreas, setas andam entre as teclas, Enter edita, Esc volta, F2 renomeia, Del limpa,
  Ctrl+D duplica.
- Folha de atalhos (Ctrl+/).
- Leitor de tela testado (NVDA, Orca).
- "Reduzir movimento" respeitado.

### A19. Idiomas (hoje 5)
**10:** chaves estáveis, plural, espanhol e alemão, nenhum texto cortado em inglês ou alemão, e as datas e
horas do relógio no idioma escolhido.

---

## Parte B: funções existentes, uma por uma

Formato: **nota de hoje → o que falta para o 10.**

### Ações das teclas

| Ação | Hoje | O que falta para 10 |
|---|---|---|
| **App** | 6 | Campo único com ícone e nome; caminho e argumentos em "Avançado"; "Abrir ou trazer para frente se já aberto"; no Linux, abrir pelo `.desktop` (respeita o Omarchy/uwsm); ícone do app sempre na tecla |
| **Atalho** | 6 | Só o gravador na primeira camada; teclas como chips; "Segurar enquanto a tecla estiver apertada"; aviso quando o atalho é do sistema (Super+…) e pode não chegar |
| **Abrir site** | 5 | Hoje é um "App" com URL disfarçado: vira tipo próprio. Ícone do site (favicon, já existe); "abrir no navegador X" em Avançado |
| **Webhook** | 6 | URL na primeira camada; método, corpo e cabeçalhos em Avançado; botão "Testar" com a resposta (código e 200 primeiros caracteres); tokens no chaveiro |
| **Mídia** | 7 | Já tem capa e título ao vivo. Falta escolher o player (Spotify ou o que tocar por último); tecla "Gostei" (MPRIS) |
| **Volume / Mudo** | 7 | Já ao vivo. Falta escolher o **dispositivo** (fone, caixa) e o **microfone**: mudo do microfone com a tecla vermelha é a ação nº 1 de quem faz live |
| **Mixer** | 6 | Funciona, mas a explicação é longa demais: o desenho no aparelho tem de se explicar sozinho; prévia do mixer no palco |
| **Texto** | 6 | Variáveis ({data}, {hora}, {área de transferência}); "Colar em vez de digitar" para textos longos |
| **Multi-ação** | 5 | Editor de passos com arrastar; passos de todos os tipos com resumo; espera; **"multi-ação alternada"** (liga/desliga); testar passo a passo |
| **Página / Pág+ / Pág−** | 7 | "Voltar à página anterior"; o destino mostra a miniatura da página |
| **Perfil** | 6 | Destino com miniatura; "voltar ao perfil anterior" |
| **Comando** | 5 | Útil para quem sabe. Falta: rodar num terminal visível (opção), mostrar a saída na tecla (ex.: um número), aviso de segurança ao importar |
| **Brilho** | 7 | Mais claro, mais escuro, ou um valor fixo; mostrar a % na tecla |
| **Relógio** | 7 | Formato 12/24 h, segundos, fuso, cronômetro e **temporizador** (contagem regressiva com alerta) |
| **Monitor** | 7 | CPU, RAM, temperatura (já); GPU, disco, rede; limite de cor configurável |
| **OBS** | 7 | 9 comandos com o estado na tecla. Falta: lista de cenas com miniatura, mudo por fonte com o estado, "tempo de gravação" na tecla, status "OBS fechado" claro |
| **Som** | 6 | Volume do som; parar se tocar de novo; biblioteca de sons curtos embutida |
| **Nenhuma** | — | Some da grade aberta (feito) |

### Funções do sistema

| Função | Hoje | O que falta para 10 |
|---|---|---|
| **Páginas** | 6 | Tudo em A5; páginas de modelos (Parte C) |
| **Perfis** | 6 | Tudo em A11; ícone por perfil |
| **Perfil automático** | 5 | Escolher o app pela lista, sem digitar; várias regras por perfil; "voltar ao anterior" ao sair (já existe *(código)*) |
| **Pressão longa** | 6 | Seção dentro de Ação; tempo ajustável por tecla com um controle simples; não provada no aparelho ainda |
| **Dois estados** | 5 | Tudo em A9 |
| **Teclas vivas** | 7 | Boas. Falta: provar no aparelho (itens 17 a 19) e escolher a frequência (economia de USB) |
| **Tela lateral** | 6 | A10; geometria ainda não conferida neste aparelho (item 16) |
| **Descanso da tela** | 6 | Escurecer antes de apagar; "nunca durante uma live" (se o OBS estiver transmitindo) |
| **Brilho** | 5 | Ao vivo (A12); brilho por horário (mais fraco à noite) |
| **Exportar/importar perfil** | 6 | Prévia e lista do que executa; arrastar o arquivo para a janela; formato com versão (já existe) |
| **Organizar cores (Embelezar)** | 5 | Prévia antes/depois; escolher o estilo (por categoria, uma cor só, degradê) |
| **Desfazer/refazer** | 8 | Já bom. Falta: o histórico visível ("Desfazer: limpar tecla 3") |
| **Ícones Lucide** | 7 | Categorias, sugeridos pela ação, recentes |
| **Ícone do site** | 7 | Já busca sozinho; mostrar "buscando…" e permitir trocar |
| **Copiar/colar teclas** | 7 | Colar em várias teclas; copiar uma página inteira |
| **Autoteste** | 6 | Placar na tela final; "Teste 2 de 4" e uma dica em cada passo; abrir o resultado no editor |
| **Serviço/bandeja/autostart** | 6 | A16; reinício limpo após atualização |
| **Atualização** | 0 | Não existe (Parte C) |

---

## Parte C: funções novas que o 10 exige

Comparado com o app da Elgato (referência de mercado) e o Companion (referência de poder). Em ordem de valor
para quem usa:

1. **Pastas:** uma tecla abre um grupo de teclas e a tecla "voltar" aparece sozinha. É o recurso mais pedido
   e mais usado no app da Elgato.
2. **Pontos de partida** (Streaming, Trabalho, Mídia, Omarchy) prontos, com ícones e cores, aplicáveis a uma
   página nova.
3. **Mudo do microfone de verdade**, com escolha do dispositivo e a tecla vermelha (ação do item B, citada aqui
   pela importância).
4. **Plugins** (Fase A8) e as primeiras integrações oficiais:
   - Spotify;
   - Discord (mudo, ensurdecer, com o estado);
   - Home Assistant (luzes, cenas);
   - Twitch (marcador, anúncio, chat lento);
   - Philips Hue;
   - Pomodoro/temporizador;
   - VoiceMeeter (Windows) e PipeWire avançado (Linux);
   - Hyprland/Omarchy (workspace, tema, captura de tela, gravação).
5. **Atualização automática** com novidades.
6. **GIF/animação na tecla** (curtos, com limite de quadros: o aparelho só aceita imagens de até 3 KB por vez,
   então a animação é lenta; deixar isso claro).
7. **Variáveis e condições:** "se o OBS está ao vivo, faz X; senão, Y"; contador numa tecla.
8. **Backup automático** da config (as últimas 10 versões) e "Restaurar…".
9. **Deck no celular** (página web local com as mesmas teclas), que vira "mais um aparelho" (ver
   `PLANO-APARELHOS.md`, fora do escopo).
10. **Giratórios e faixa de toque** (Elgato Plus, AKP03), com ações de girar.
11. **Dois aparelhos ao mesmo tempo** (fora do escopo hoje; a config já guarda o modelo por página).

O que **não** fazer, para não inchar: loja paga, conta na nuvem, editor de ícones próprio, chat.

---

## Parte D: onde cada item entra no roteiro

| Degrau do `ROTEIRO.md` | Itens deste plano |
|---|---|
| 1 Confiança | Autoteste completo (B); provar pressão longa, lateral e teclas vivas |
| 2 Alicerce | Abrir site como tipo próprio (B); chaves de idioma estáveis (A19) |
| 3 Fundação visual | A1, A2, A12, A13, A14 |
| 4 Editor bom | A3 a A11, A18; Ações App, Atalho, Webhook, Multi-ação, Dois estados (B) |
| 5 Qualquer aparelho | A10 com a geometria provada; giratórios (C10) |
| 6 Fácil de ter | A15, A16, atualização (C5), pontos de partida (C2), backup (C8) |
| 7 Mais que os outros | Pastas (C1), plugins e integrações (C4), mudo de microfone (C3), variáveis (C7), GIF (C6), A19 |
| 8 Provado | Teste com pessoas em todas as tarefas; o que falhar volta para cá |

## A verdade sobre o tamanho

- Parte A: umas 8 a 12 sessões longas.
- Parte B: umas 6 a 10.
- Parte C: as pastas e os pontos de partida são 1 a 2 sessões cada; plugins e integrações são semanas.

Nada disso chega a 10 sem o Degrau 8 do roteiro: **pessoas reais usando**. O plano leva a um 9 bem defendido;
o último ponto vem do retorno de quem usa.
