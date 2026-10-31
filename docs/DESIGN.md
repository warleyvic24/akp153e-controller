# Design system — AKP153E

Regras para a interface do editor (egui). O código das escalas está em `src/editor/tokens.rs`, as cores em
`src/editor/theme.rs` (`Palette`) e as peças reutilizáveis em `src/editor/widgets.rs`. O que não está num teste é
convenção: confira com os olhos nos dois temas antes de concluir. Só em build debug:
`AKP153E_PREVIEW_THEME=light|dark` força o tema sem gravar, e `AKP153E_CONFIG_DIR=<pasta>` aponta o
editor para uma cópia da config (com `config.json` dentro), para testar clicando, editando e
excluindo sem tocar no arquivo real. O release não tem nenhum dos dois desvios.

## Princípios

1. **Uma coisa brilha por vez.** O brilho (glow) significa "selecionado" (tecla) e nada mais. Hover é
   tinta, foco é anel de dois tons, ação primária é o único preenchimento sólido.
2. **Estado nunca só por cor.** Conectado/Salvo/Falha levam texto; aba e pílula ativas levam traço ou
   contorno além da cor; o estado vai aos leitores de tela (`widget_info`).
3. **O destaque é do usuário, a legibilidade não.** Qualquer cor de destaque tem de funcionar nos dois
   temas: o destaque como traço/ícone passa por `theme::ink`, que o ajusta só quando falta contraste.
4. **Nada abaixo de 12 px**, nenhum alvo de clique abaixo de 24 px.

## Tokens (`src/editor/tokens.rs`)

| Grupo | Valores |
|---|---|
| Espaço | `XS 4` · `SM 8` · `MD 12` |
| Raio | `KEY 10` · `CONTROL 8` · `CARD 12` · `DEVICE 22` · `PILL 99` |
| Texto | `CAPTION 12` · `BODY 14` · `TITLE 20` · `DISPLAY 22` |
| Alvo mínimo | `TARGET_MIN 24` |
| Contraste | texto `4,5:1` · componentes (borda, ícone, traço) `3:1` |

Tipografia: **Bahnschrift** (display, títulos e rótulos de seção), **Segoe UI** (corpo),
**Consolas** (valores, estados e números). Rótulos em caixa alta usam a fonte display, sem
espaçamento artificial entre letras (só o logotipo "AKP153E" o tem).

## Cores (`theme::Palette`)

| Papel | Uso |
|---|---|
| `bg` / `panel` / `raise` / `field` | fundo do palco e da bandeja / barra superior / inspetor e cartões / campos |
| `fg` / `muted` | texto / texto secundário e dicas (≥ 4,5:1 sobre `bg`, `panel` e `raise`) |
| `line` | separadores e decoração. **Não** use como borda de controle |
| `control` | borda de controles interativos, derivada de `line` até dar 3:1 |
| `ok` / `bad` | sucesso / erro. Gravar atalho **não** é erro |
| `device_fg` | símbolos sobre as teclas, que são pretas nos dois temas |
| destaque (`accent`) | escolhido pelo usuário; preenchimento direto, traço e ícone via `theme::ink` |

## Componentes (`src/editor/widgets.rs`)

| Peça | Regra |
|---|---|
| `status_chip` | ponto + texto; cor do texto garantida em 4,5:1 sobre o fundo tingido |
| `pill` | opção selecionável: ativa = contorno + véu do destaque. Anuncia `selected` |
| `tab` | sublinhado de 2 px em `ink`. Anuncia `selected` |
| `swatch` | amostra de cor ≥ 24 px, anel afastado quando escolhida |
| `section` | rótulo de seção, fonte display 12 px em caixa alta |
| `hint` | placeholder com cor própria; sem isso o tema o pinta como valor digitado |
| `paint_key` | tecla do aparelho; selecionada = brilho do destaque + traço de 2,5 px. Vazia em repouso: só o "+" apagado (≥ 3:1 sobre o preto, teste em `theme.rs`); cantos e "+" no destaque ao passar o mouse ou selecionar |
| botão "Testar" | única ação sólida da tela (cor de destaque + texto preto ou branco por contraste). Sem o que testar fica neutro (fundo de campo, borda, texto apagado), não um destaque esmaecido |
| ícone de app ou site na tecla | se o contraste médio com o fundo da tecla ficar abaixo de 2:1 (o logo escuro do GitHub na tecla preta), ganha uma placa arredondada clara ou escura (`renderer::plate_for`) |
| `search_field` | campo de busca com lupa na margem esquerda; a dica é o nome falado |
| grade de ícones (`editor/icon_picker.rs`) | 2000 ícones Lucide com busca (português ou inglês); sem busca, 40 sugestões para live; o ícone atual fica sempre visível na grade |
| linha da biblioteca | sem alça de arrastar em repouso; ao passar o mouse, o selo "+ Tecla N" e o cursor de arrastar |

## Retorno, orientação e preferências

- **Rótulo da tecla**: até 3 linhas (Enter quebra), posição Topo/Meio/Base; no meio, por cima do ícone, leva sombra. O número da tecla só aparece em tecla vazia ou ao apontar, sobre plaqueta escura; a dica ao apontar diz nome e ação.
- **Teclas vivas**: relógio (redesenhado na virada do minuto) e teclas do OBS, que mudam de cor com o estado do OBS.
  Volume (Vol+, Vol−, Mudo: porcentagem em cima, barra no pé; Mudo ligado fica vermelho e diz MUDO), tocando
  agora (Play/Pause com a capa escurecida, estado no canto, título e artista) e "Monitor" (CPU, RAM, temperatura;
  barra verde, âmbar a partir de 60 %, vermelha a partir de 85 %), consultados a cada 2 s só se a página os usa
  (`service/runtime/live.rs`, desenho em `graphics/live_art.rs`). Sem dado, a tecla sai como uma tecla comum.
- **Degradê e "Embelezar página"** (`editor/beautify.rs`): fundo mais claro em cima e mais escuro embaixo
  (±18 %); o botão no rodapé do palco dá a cada tecla configurada a cor da categoria da biblioteca, degradê,
  rótulo e ícone quando faltam (teclas vivas ficam sem rótulo), e avisa com "Desfazer".
- **Toasts** (`editor/toast.rs`): ações sem resultado visível (copiar, colar, limpar, trocar teclas, excluir
  página ou perfil) avisam por 4 s; as que se desfazem trazem o botão "Desfazer".
- **Estado vazio** (`editor/stage.rs::empty_guide`): página sem nenhuma tecla configurada mostra o passo a
  passo em vez de uma dica de uma linha.
- **Arrastar**: uma ação da bandeja solta numa tecla a aplica; uma tecla solta noutra troca as duas.
  O cartão "Personalizada" cria um atalho vazio e abre a aba Ação.
- **Páginas**: duplo clique renomeia; clique direito abre Renomear, Mover e Excluir.
- **Preferências** (`editor/settings.rs`): tema, destaque, iniciar com o Windows, brilho do aparelho
  (5 a 100 %, vai ao aparelho pelo `reload` que o salvamento já envia), reconectar, pasta de logs.
- O chip de conexão abre um menu com Reconectar e a pasta de logs.
- **Janela estreita** (`editor/layout.rs`): abaixo de biblioteca + inspetor + palco mínimo (984 pt), por
  exemplo numa faixa de um gerenciador lado a lado, a biblioteca sai e o botão "Ações" da barra a abre no
  lugar do inspetor (escolhida a ação, o inspetor volta). O inspetor encolhe até 260 pt; abaixo de 860 pt a
  barra esconde o logotipo e o rótulo "Perfil"; dicas de atalho que não cabem ficam de fora.

## Ícones de sites

Tecla "Abrir site" com o globo padrão troca o globo pelo ícone do site (`editor/favicon.rs`) quando a URL
fica parada por 1,5 s: lê o `<head>` do próprio site (nenhum serviço de terceiros), prefere o maior ícone
declarado ou SVG, depois `/apple-touch-icon.png` e `/favicon.ico`. Um ícone escolhido pelo usuário não é
trocado.

## Texto

- "Tecla", nunca "botão", para as 15 teclas do aparelho ("botão" é controle da interface).
- Dicas apontam para o que está na tela pelo nome visível ("Ações", "Aparência"), não por posição
  ("à esquerda") nem por jargão ("bandeja"): no modo compacto a posição muda.
- Ausência ("Sem nome") vai apagada, como placeholder, não com o peso de um conteúdo.

## Idiomas (`src/strings.rs`)

O texto em português é a chave: `t("Salvo")` e `tf("Tecla {} copiada", &[&n])` devolvem o original ou
a tradução em inglês (tabela `EN`); o que falta cai em português. Os componentes (`section`, `pill`,
`tab`, `hint`, `name`, `status_chip`) traduzem o próprio texto, então rótulos passados a eles não
precisam de `t(...)`. Texto novo na interface: use `t`/`tf`, acrescente a entrada em `EN`. Um teste
varre os arquivos-fonte e falha se algum texto usado não tiver tradução, ou se os `{}` não baterem.
O idioma é escolhido em Preferências (`ui.language`, padrão português). O menu da bandeja pertence ao
serviço e só muda de idioma quando ele reinicia. Nomes de dados do usuário (páginas, perfis) não
são traduzidos.

## Teclado e leitores de tela

- Teclas do palco e cartões da bandeja recebem foco por Tab; Enter e Espaço ativam. As setas movem o
  foco pela grade (navegação nativa do egui) e a seleção acompanha a tecla com foco.
- Clicar numa tecla também dá foco (o egui só dá foco por Tab), para as setas funcionarem em seguida.
- Anel de foco: dois tons, branco por dentro e escuro por fora, 2 px cada, afastado 2 px
  (`widgets::focus_ring`). Um anel só na cor do texto some sobre as teclas no tema claro.
- Teclas anunciam "Tecla N, rótulo", "sem texto" (tem imagem ou ícone) ou "vazia", e se estão
  selecionadas; cartões anunciam o nome da ação.
- Todo campo tem nome: os do inspetor via `labelled_by` no rótulo da seção (`widgets::section`
  devolve o id), os sem rótulo visível via `widgets::name` (Perfil, Buscar ação, Buscar app, Nova
  página). Botões com o mesmo texto visível ganham nome falado único ("Limpar imagem", "Limpar
  atalho", "Limpar botão"), sempre contendo o texto visível (WCAG 2.5.3).
- Não use espaçamento artificial entre letras: o leitor soletra ("A K P 1 5 3 E").
- Excluir página ou perfil não pede confirmação porque o desfazer (Ctrl+Z) cobre a exclusão.

## O que os testes impõem (`cargo test`)

`editor/theme.rs`: contraste de `fg`, `muted`, `ok` e `bad` ≥ 4,5:1 sobre `panel`, `raise` e `bg`, nos dois
temas; `control` ≥ 3:1; símbolos da tecla ≥ 4,5:1; `ink` ≥ 3:1 para todo destaque sugerido nos dois
temas e inalterado no escuro; texto sobre cada destaque sugerido ≥ 4,5:1. Mudou a paleta e quebrou o
contraste? O teste diz qual par.

Também varre 4.096 cores de destaque e exige texto ≥ 4,5:1 sobre qualquer uma (preto ou branco,
o que der mais contraste), então o seletor de cor livre é seguro para texto sobre o preenchimento.
O destaque usado como traço ou ícone sempre passa por `ink`.

## Teste automático de nomes acessíveis

`editor/mod.rs::every_interactive_control_has_an_accessible_name` roda o editor sem janela (AccessKit
ligado), com as três abas, cada tipo de ação e as Preferências, e falha listando todo botão, caixa,
seletor, controle deslizante ou campo sem nome. Foi ele que achou os seletores de destino sem nome.
A caixa numérica interna dos sliders (`SpinButton`) fica fora da checagem: é do egui.

## Como verificar a acessibilidade

Com o editor aberto (debug), despeje a árvore do Windows UI Automation
(`System.Windows.Automation`, `FromHandle` da janela, `FindAll(Descendants)`) e confira que cada
`Edit`, `Slider`, `ComboBox` e `Button` tem `Name`. Foi assim que se achou o logotipo lido
letra a letra e os campos sem nome. É um proxy do que um leitor de tela recebe, não substitui ouvir
o NVDA ou o Narrador.

## Lacunas conhecidas

- Só português e inglês. O logotipo "AKP153E" e o título da janela não são traduzidos. Os nomes padrão `Default` e `Page 1`
  vêm do esquema da config (`schema.rs`) e continuam em inglês para não mexer em configs existentes.
- Ninguém ouviu a interface num leitor de tela de verdade; só a árvore de acessibilidade foi
  verificada. A caixa numérica embutida dos sliders continua sem nome (é interna do egui).
- O nome da janela e da bandeja é "Controle AKP153E" (`shell::WINDOW_TITLE`), descritivo, sem a marca
  do fabricante. Um nome e uma marca próprios continuam sendo decisão de produto. A pasta de config
  `AJazz\AKP153E` e o texto do `CLAUDE.md` ainda citam o fabricante.
