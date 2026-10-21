# Segurança

Este programa roda só no seu PC e não envia dados a servidores próprios. Pontos sensíveis:

- **Porta de controle** `127.0.0.1:47153` (serviço) e `47154` (editor): aceita qualquer processo local, não há autenticação. Os comandos são limitados e a entrada é validada.
- **`config.json`** (`%APPDATA%\AJazz\AKP153E`): pode guardar URLs e corpos de webhook em texto puro, e caminhos de programas que serão executados. Quem escreve nesse arquivo já tem acesso à sua conta.
- **Logs** (`akp153e.log`): URLs de webhook aparecem sem usuário, senha, query e fragmento.

Para reportar uma vulnerabilidade, abra uma issue privada ou escreva ao mantenedor (veja o autor dos commits). Não publique detalhes antes de uma correção.
