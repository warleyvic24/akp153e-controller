# Design system — AKP153E

Regras para a interface do editor (egui). O código das escalas está em `src/tokens.rs`, as cores em
`src/theme.rs` (`Palette`) e as peças reutilizáveis em `src/widgets.rs`. O que não está num teste é
convenção: confira com os olhos nos dois temas antes de concluir (`AKP153E_PREVIEW_THEME=light`
com `cargo run -- --editor`, só em debug, não grava nada).

## Princípios

1. **Uma coisa brilha por vez.** O brilho (glow) significa "selecionado" (tecla) e nada mais. Hover é
   tinta, foco é anel branco, ação primária é o único preenchimento sólido.
2. **Estado nunca só por cor.** Conectado/Salvo/Falha levam texto; aba e pílula ativas levam traço ou
   contorno além da cor; o estado vai aos leitores de tela (`widget_info`).
3. **O destaque é do usuário, a legibilidade não.** Qualquer cor de destaque tem de funcionar nos dois
   temas: o destaque como traço/ícone passa por `theme::ink`, que o ajusta só quando falta contraste.
4. **Nada abaixo de 12 px**, nenhum alvo de clique abaixo de 24 px.

## Tokens (`src/tokens.rs`)

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

## Componentes (`src/widgets.rs`)

| Peça | Regra |
|---|---|
| `status_chip` | ponto + texto; cor do texto garantida em 4,5:1 sobre o fundo tingido |
| `pill` | opção selecionável: ativa = contorno + véu do destaque. Anuncia `selected` |
| `tab` | sublinhado de 2 px em `ink`. Anuncia `selected` |
| `swatch` | amostra de cor ≥ 24 px, anel afastado quando escolhida |
| `section` | rótulo de seção, fonte display 12 px em caixa alta |
| `hint` | placeholder com cor própria; sem isso o tema o pinta como valor digitado |
| `paint_key` | tecla do aparelho; selecionada = brilho do destaque + traço de 2,5 px |
| botão "Testar" | única ação sólida da tela (cor de destaque + texto preto ou branco por contraste) |

## Teclado e leitores de tela

- Teclas do palco e cartões da bandeja recebem foco por Tab; Enter e Espaço ativam. As setas movem o
  foco pela grade (navegação nativa do egui) e a seleção acompanha a tecla com foco.
- Clicar numa tecla também dá foco (o egui só dá foco por Tab), para as setas funcionarem em seguida.
- Anel de foco: traço de 2 px na cor do texto, afastado 2 px (`main::focus_ring`).
- Teclas anunciam "Tecla N, rótulo|vazia" e se estão selecionadas; cartões anunciam o nome da ação.

## O que os testes impõem (`cargo test`)

`theme.rs`: contraste de `fg`, `muted`, `ok` e `bad` ≥ 4,5:1 sobre `panel`, `raise` e `bg`, nos dois
temas; `control` ≥ 3:1; símbolos da tecla ≥ 4,5:1; `ink` ≥ 3:1 para todo destaque sugerido nos dois
temas e inalterado no escuro; texto sobre cada destaque sugerido ≥ 4,5:1. Mudou a paleta e quebrou o
contraste? O teste diz qual par.

## Lacunas conhecidas

- Textos em português fixos no código, sem camada de tradução. Os nomes padrão `Default` e `Page 1`
  vêm do esquema da config (`schema.rs`) e continuam em inglês para não alterar arquivos existentes.
- Não há teste automatizado de leitor de tela; os nomes acessíveis foram implementados, não ouvidos
  num NVDA/Narrador.
- O título da janela e da bandeja é "AJAZZ AKP153E" (`shell::WINDOW_TITLE`), o nome do fabricante.
  A situação de marca não foi verificada; um app não oficial deveria ter nome próprio.
- Contraste do texto sobre a cor de destaque escolhida livremente (seletor de cor) não é garantido;
  só os destaques sugeridos são testados.
