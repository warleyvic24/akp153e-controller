# Plano: interface nota 10

Data: 2026-10-31. Base: avaliação das telas reais (temas claro e escuro, aparelho de 15 e de 6 teclas,
Preferências, lista de apps, as três abas do inspetor). Referências: o app da Elgato (Stream Deck),
Loupedeck/Logi Options+, Bitfocus Companion.

**Ordem geral:** `docs/ROTEIRO.md` (intercala este plano com o outro e a checklist do aparelho).

## Diagnóstico honesto

Nota de hoje: **5,5**. A engenharia é boa: testes de contraste e de alvo mínimo, desfazer, busca, arrastar,
prévia real das teclas. O que falta é **direção de produto**. Tudo tem o mesmo peso, há superfícies que
parecem de depuração e a tela ensina coisas que deveria deixar implícitas.

| Área | Nota | Problema principal |
|---|---|---|
| Barra do topo | 5 | Dois selos de estado iguais; "Perfil" duas vezes; fonte monoespaçada misturada |
| Palco | 6 | Aparelho bom, mas abas de página fracas, ajuda permanente, espaço mal usado |
| Biblioteca | 5 | Lista plana sem prioridade; o mais usado (apps, recentes) escondido |
| Inspetor | 6 | "Testar" é o elemento mais forte da tela; aba "Alternar" quase vazia; campo de app confuso |
| Preferências e lista de apps | 3 | Janelas flutuantes de cara de depuração; brilho só "ao salvar"; letras no lugar de ícones |
| Primeira execução | 4 | Sem modelos prontos: o usuário encara 15 quadrados vazios |
| Consistência | 5 | Raios, alturas de campo e fontes variam; texto pequeno (12 a 14 px) com muito cinza |

**Achado extra:** o "texto fantasma" atrás do aparelho não vem do programa. A janela é **translúcida**
(regra de opacidade do Omarchy/Hyprland) e o que aparece é a área de trabalho atrás dela. Para uma
ferramenta visual, isso é inaceitável: corrigir na Etapa 1.

## O que é "nota 10" (critério de aceite do plano inteiro)

Nota 10 não é opinião. São estes fatos, medidos:

1. **Primeira tecla em menos de 30 s:** alguém que nunca viu o programa põe um app numa tecla e a vê no
   aparelho sem ler nada.
2. **Cinco tarefas sem erro** (ver Etapa 9), por 3 pessoas de fora, sem ajuda.
3. **SUS ≥ 85** (questionário padrão de usabilidade) nessas 3 pessoas.
4. **Nada parece "de programador":** nenhuma janela crua da biblioteca gráfica, nenhum texto técnico (caminho,
   JSON, "websocket") na primeira camada.
5. **Sempre se sabe o estado:** o que está no aparelho é o que está na tela, ou o programa diz por que não.
6. Contraste, alvo mínimo e navegação só por teclado continuam garantidos por testes.

Honestidade: os itens 2 e 3 dependem de **pessoas reais**. Eu consigo chegar a um 8 a 9 sozinho, com capturas
e revisão; o 10 só se prova com teste de usuário, e o resultado vai gerar uma rodada de correções.

## Princípios (decidem as dúvidas no caminho)

- **O aparelho é o herói.** Tudo o que não é o aparelho é mais discreto que ele.
- **Uma ação principal por tela, ou nenhuma.** Editar uma tecla não tem botão principal.
- **Mostrar, não explicar.** A ajuda aparece quando falta algo (estado vazio, erro), não para sempre.
- **Mudanças ao vivo.** O que mexe no aparelho (brilho, tecla) aparece nele na hora.
- **Camadas.** Primeiro o que 90% usa; o técnico (argumentos, método HTTP, porta) fica em "Avançado".

---

## Etapa 0: régua (antes de mudar qualquer coisa)

- Script de capturas no repositório (`tools/ui-shots.sh`): todas as telas, nos dois temas, em 1366x768 e
  1920x1080, com a cópia da config. Rodar antes e depois de cada etapa; as imagens vão para o PR.
- Escrever as 5 tarefas da Etapa 9 e medir a linha de base (cliques e tempo, feitos por mim).
- **Aceite:** galeria "antes" salva; tarefas escritas.

## Etapa 1: fundação visual

- **Janela opaca:** regra do Hyprland para `class:keyforge` (opacidade 1) no pacote e no `docs/LINUX.md`.
  No Windows não há o problema.
- **Tipografia:** uma família só (a sans atual). Monoespaçada apenas para teclas de atalho (`CTRL`).
  Escala 12 / 14 / 16 / 20 / 24; corpo do inspetor passa a 14 e rótulos a 13 em caixa normal (sem CAIXA
  ALTA cinza).
- **Componentes únicos:** campo de texto (32 px de altura, raio 8, borda visível), botão secundário,
  botão fantasma, segmentado, interruptor, linha de lista. Todo o editor usa só esses componentes, com um
  teste que proíbe `TextEdit`/`Button` crus fora de `widgets.rs`.
- **Espaçamento em grade de 4/8 px; raios 6 (controle), 10 (cartão), 16 (painel).**
- **Aceite:** galeria sem campos de alturas diferentes; contraste ≥ 4,5 em todo texto (teste).
- Risco: mexer em tudo ao mesmo tempo quebra telas → trocar componente por componente, com captura a cada troca.

## Etapa 2: superfícies modais (o pior ponto hoje)

- Componente `Modal`: centralizado, fundo escurecido, título, fechar com Esc e clicando fora, foco preso
  dentro, animação de entrada de 150 ms.
- **Preferências:** modal com seções à esquerda (Geral, Aparelho, Integrações, Sobre).
  - Geral: tema, cor, idioma, iniciar com o sistema.
  - Aparelho: brilho **ao vivo** (aplica enquanto arrasta), descanso da tela, modelo conectado, "Ensinar
    meu aparelho" (Fase 5).
  - Integrações: OBS com estado ("Conectado ao OBS 30.2" / "Não encontrado") e um botão "Testar conexão".
  - Sobre: versão, logs, reconectar.
- **Lista de apps:** modal com **ícones reais**, "Recentes" no topo, setas e Enter para escolher, e a tecla
  na prévia mudando ao passar o mouse.
- **Aceite:** nenhuma `egui::Window` crua no editor (teste no código); brilho muda no aparelho sem salvar.

## Etapa 3: barra do topo e estado

- Um selo só, de **sincronia**: "No aparelho" (verde), "Enviando…", "Desconectado · Reconectar"
  (vermelho, clicável), "Serviço parado · Iniciar". "Salvo" some; erro de gravação vira aviso vermelho fixo.
- Seletor de perfil único: menu com os perfis, "Novo perfil", "Gerenciar…" (renomear, app automático,
  exportar/importar). O botão "Perfil e página" sai.
- **Aceite:** o topo tem no máximo: nome, perfil, sincronia, desfazer/refazer, preferências.

## Etapa 4: palco

- **Abas de página coladas ao aparelho**, do tamanho certo, com renomear (duplo clique), arrastar para
  reordenar, menu (duplicar, excluir) e um contador quando não cabem.
- Números das teclas só ao passar o mouse e na tecla vazia (para todas igual).
- Valores ao vivo (volume, CPU) **dentro** da tecla, como saem no aparelho; nada vazando.
- Barra de atalhos: aparece nas 3 primeiras aberturas; depois vira um "?" que abre a folha de atalhos.
- "Embelezar página" vira **"Organizar cores"**, com prévia antes/depois e o aviso "Ctrl+Z desfaz".
- Usar a largura: acima de 1600 px o inspetor cresce para 400 px e a tecla pode chegar a 140 px.
- **Aceite:** em 1366x768 nada corta; em 1920 o aparelho e os painéis usam ≥ 85% da largura.

## Etapa 5: biblioteca

- Ordem nova: **Recentes**, **Seus apps** (os 8 mais abertos do sistema, com ícone), Mídia, Som e tela,
  Páginas, OBS, Sistema, e só no fim Edição (copiar, colar…).
- Linhas de 32 px, título de seção legível, contagem por seção, seção recolhível.
- Clicar aplica **na tecla selecionada**, e a linha diz isso ao passar o mouse ("Pôr na tecla 3").
- Ao arrastar, o fantasma é a própria tecla desenhada (ícone e cor), não o texto.
- **Aceite:** tarefa "pôr o Discord numa tecla" em ≤ 2 ações (arrastar de "Seus apps").

## Etapa 6: inspetor

- Cabeçalho: prévia **grande** (a imagem que vai ao aparelho), nome editável ali mesmo, resumo de uma linha.
- Ações da tecla: "Testar" vira botão secundário; copiar, colar, duplicar e limpar vão para um menu "⋯"
  com nome e atalho de cada um. Limpar mostra "Desfazer" no aviso.
- **Ação:**
  - Linha do tipo (feita em 2026-10-31).
  - **App:** um campo com ícone e nome do app e "Trocar"; o caminho e os argumentos ficam em "Avançado".
  - **Atalho:** só o gravador ("Clique e aperte as teclas"); o campo de texto vai para "Avançado".
  - **Web:** só a URL; método e corpo em "Avançado".
  - **Ao segurar** e **Dois estados** viram seções recolhíveis dentro de Ação. A aba "Alternar" deixa
    de existir.
- **Aparência:**
  - prévia dos dois estados lado a lado quando a tecla é de dois estados;
  - cores com nome ao passar o mouse; "Outra" vira "Cor personalizada…";
  - ícones com categorias e "Usados recentemente".
- Erros aparecem **no campo** ("URL precisa começar com http"), não só no aviso da tecla.
- **Aceite:** o inspetor de uma tecla de app cabe em 768 px de altura sem rolar.

## Etapa 7: primeira execução e estados

- Primeira abertura: "Aparelho encontrado: AJAZZ AKP153E" e a escolha de um ponto de partida: **Streaming**
  (OBS, microfone, cenas), **Trabalho** (apps, atalhos, volume), **Mídia** (player, volume, mixer) ou
  **Em branco**. Escolher já preenche a página, desenhada no aparelho na hora.
- Estados vazios com uma ação clara: sem aparelho ("Conecte o cabo · Meu aparelho não aparece"), aparelho
  desconhecido (leva ao assistente da Fase 5), serviço parado ("Iniciar").
- Mensagens de erro humanas: o que houve e o que fazer, sem termos técnicos na primeira linha.
- **Aceite:** critério 1 (primeira tecla em < 30 s) medido com uma pessoa.

## Etapa 8: acessibilidade e acabamento

- Mapa completo de teclado: Tab entre áreas, setas nas teclas, Enter abre a edição, Esc volta, F2 renomeia.
- Anel de foco visível e igual em tudo; "reduzir movimento" do sistema respeitado.
- Inglês: conferir que nenhum texto corta (os textos em inglês são até 30% mais longos).
- Animações com um tempo só (150 ms para entrar, 100 ms para sair) e a mesma curva.
- **Aceite:** as 5 tarefas feitas só com teclado.

## Etapa 9: teste com pessoas

Três pessoas que nunca usaram o programa, de preferência uma que já usou o app da Elgato. As tarefas:

1. Pôr um app numa tecla.
2. Fazer uma tecla de mudo do microfone que fica vermelha quando mudo.
3. Criar uma segunda página e uma tecla que leva até ela.
4. Mudar o brilho do aparelho.
5. Trocar a cor e o ícone de uma tecla.

Medir tempo, erros e o SUS. Cada problema visto em 2 ou mais pessoas é corrigido e as tarefas são refeitas.
**Aceite:** os critérios de nota 10 acima.

---

## Ordem, tamanho e riscos

| Etapa | Tamanho | Depende de |
|---|---|---|
| 0 Régua | pequeno | |
| 1 Fundação | grande | 0 |
| 2 Modais | médio | 1 |
| 3 Topo | pequeno | 1 |
| 4 Palco | médio | 1 |
| 5 Biblioteca | médio | 1 |
| 6 Inspetor | grande | 1, 2 |
| 7 Primeira execução | médio | 2, 6 |
| 8 Acabamento | médio | todas |
| 9 Teste com pessoas | pequeno (mas precisa de gente) | todas |

**Riscos → correção**
- **egui tem limites** (sombras suaves, desfoque, tipografia fina) → não imitar o macOS; apostar em
  espaçamento, hierarquia e cor, que o egui faz bem.
- **Arquivos passando de 800 linhas** (`inspector.rs`, `stage.rs`, `widgets.rs`) → separar por componente
  antes de cada etapa.
- **Regressão visual sem perceber** → galeria de capturas antes e depois em todo commit de interface.
- **Eu avalio por capturas, não clicando de verdade** → cada etapa termina com você usando por 5 minutos,
  e o que incomodar entra na etapa seguinte.
- **Plano de aparelhos parado** → as etapas 1 a 3 vêm antes da Fase 5 (o assistente "Ensinar meu aparelho"
  já nasce com o Modal e os componentes novos); o resto pode intercalar.
