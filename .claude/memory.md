# Decisões do projeto (o porquê)

O `CLAUDE.md` diz como trabalhar aqui; este arquivo guarda por que as coisas são como são.

- **Dois processos (serviço e editor):** esconder uma janela eframe mantém a CPU em 100%. O serviço sem janela fica em 0% e o editor é aberto só quando preciso.
- **Controle por TCP local, não por memória compartilhada:** simples de testar (`tests/cli.rs`, `src/service/mod.rs`) e de reiniciar um lado sem o outro. Custo: qualquer processo local conecta, por isso toda entrada é limitada.
- **O serviço só grava perfil/página ativos:** o editor é dono do resto da config. Gravar a cópia inteira do serviço apagou edições do editor (corrida entre `save` e `reload`).
- **`load_existing()` para regravar:** `load()` devolve a config padrão quando o arquivo está ilegível; usá-la para regravar sobrescreveria a config do usuário.
- **Testes isolados da config real:** `App` grava ao ser descartado; antes do isolamento, `cargo test` sobrescrevia o `config.json` do usuário.
- **"Abrir site" usa `explorer <url>`, não `cmd /C start`:** o `cmd` trata `&` como separador e corta URLs com vários parâmetros.
- **Webhook só aceita métodos conhecidos:** um método inválido virava GET em silêncio e fazia uma requisição diferente da configurada.
- **Reinício após pânico:** o release aborta no pânico e o serviço não tem janela; sem reinício a bandeja e o aparelho ficavam mortos até o próximo login.
- **"Sair" na bandeja usa `exit(0)`:** trocar por `PostQuitMessage` limparia o ícone mais cedo, mas não há como provar sem clicar no menu; não mexer sem poder testar.
- **Item "Aplicativo..." da bandeja:** arrastar abre o seletor de programas; o botão ganha o nome do app e fundo preto.
