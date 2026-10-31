# Segurança

Este programa roda só no seu PC e não envia dados a servidores próprios. Pontos sensíveis:

- **Porta de controle** `127.0.0.1:47153` (serviço) e `47154` (editor): não há autenticação. No Linux o serviço recusa conexões de processos de outro usuário (dono do socket em `/proc/net/tcp`); no Windows qualquer processo local conecta. Os comandos não executam ações, são limitados e a entrada é validada.
- **`config.json`** (`%APPDATA%\AJazz\AKP153E` ou `~/.config/AJazz/AKP153E`): pode guardar URLs e corpos de webhook, a senha do WebSocket do OBS e comandos de shell em texto puro, e caminhos de programas que serão executados. No Linux é gravado com permissão `0600`. Quem escreve nesse arquivo já tem acesso à sua conta.
- **Logs** (`akp153e.log`): URLs de webhook aparecem sem usuário, senha, query e fragmento.

Para reportar uma vulnerabilidade, use o relato privado do GitHub (aba Security, "Report a vulnerability") em vez de uma issue pública. Não publique detalhes antes de uma correção.
