# Verificação no aparelho

O que os testes automáticos não provam. Faça com o serviço rodando (`akp153e.exe --background`) e o aparelho conectado; anote o resultado e a data em cada item.

Preparação: abra o editor, ponha uma ação visível em cada botão testado (por exemplo "Mudo" na tecla 1).

| # | O que verificar | Como | Esperado | Resultado |
|---|---|---|---|---|
| 1 | Imagem em todas as 15 teclas | `akp153e.exe --probe`, depois olhe o aparelho | Cada tecla mostra a imagem configurada, sem corte nas bordas | não verificado |
| 2 | Aperto chega ao programa | Aperte uma tecla com a ação "Mudo" | O volume muda uma vez | não verificado |
| 3 | Soltura chega ao programa | Segure a tecla por 2 s e solte | Nada acontece antes de soltar; a ação roda ao soltar | não verificado |
| 4 | Pressão longa | Configure "Ao segurar" em outra ação; segure por mais que o limite (500 ms) | Roda a ação longa, não a curta | não verificado |
| 5 | Troca de página pelo aparelho | Tecla com "Página 2" | O aparelho redesenha as 15 teclas da página 2 e o editor aberto não desfaz a troca ao salvar | não verificado |
| 6 | Reconexão | Desligue e religue o cabo USB | O editor mostra CONECTADO de novo e as imagens voltam sozinhas | não verificado |
| 7 | Encerrar o serviço à mão | Encerre o processo `--background` no Gerenciador de Tarefas | Nada reinicia (isso não é pânico); abra de novo pela bandeja ou pelo início automático | não verificado |
| 8 | Lateral sem teclas | Toque e aperte a faixa lateral, à direita das 15 teclas | Nada acontece: no AKP153E ela é só uma tela (ver abaixo) | não verificado |
| 9 | Brilho | Preferências > Brilho do aparelho: arraste de 100% até 5% e volte | O aparelho escurece e clareia ao salvar, sem reiniciar nem apagar as imagens; após reconectar o cabo, volta ao último valor | não verificado |
| 10 | Tecla afunda | Segure qualquer tecla com ação | Enquanto segura, a imagem fica menor sobre preto; ao soltar, volta ao normal | não verificado |
| 11 | Descanso da tela | Preferências > Apagar a tela sem uso: 1 min; espere 1 min sem tocar | A tela apaga; o primeiro toque só acende (a ação não roda); o segundo roda | não verificado |
| 12 | Teclas de brilho | Biblioteca > Aparelho > "Brilho −" numa tecla; aperte 3 vezes | O aparelho escurece 10% a cada toque; as Preferências mostram o valor novo | não verificado |
| 13 | Relógio | Biblioteca > Aparelho > "Relógio" | A tecla mostra a hora e a data e muda sozinha na virada do minuto | não verificado |
| 14 | OBS | Ligue o WebSocket do OBS (Ferramentas > Configurações do servidor WebSocket); tecla "Gravar" | O OBS começa a gravar e a tecla fica vermelha; outro toque para e ela volta | não verificado |
| 15 | Perfil automático | Perfil e página > "Ativar sozinho quando este app estiver em foco": escolha um app recente | Ao focar o app, o aparelho troca de perfil; ao sair dele, volta ao anterior | não verificado |

## Tela lateral (pesquisa de 2026-10-24, sem teste no aparelho)

- No AKP153E a lateral é uma faixa LCD decorativa, sem botões. As "3 teclas extras" do protocolo (18 = 15 + 3) são botões físicos só da revisão AKP153R. Fontes: [Aiacos/ajazz-control-center](https://github.com/Aiacos/ajazz-control-center) (`docs/protocols/streamdeck/akp153.md`) e [lukasMega/DeckBridge](https://github.com/lukasMega/DeckBridge), que mapeou só 15 teclas no `0300:3010` em hardware real.
- A única forma documentada de pôr imagem nela é o comando `LOG` ([ajazz-sdk em mistweaverco/opendeck-ajazz](https://github.com/mistweaverco/opendeck-ajazz)): grava o **logo de inicialização** de 854×480 (a tela inteira, não só a lateral). Por ser logo de inicialização, deve ir para a flash; regravar a cada troca de página a desgastaria.
- Não implementado, porque falta: o formato do `LOG` no protocolo v3 (pacote de 1024) do `0300:3010`; se a imagem é JPEG ou RGB cru (o cabeçalho v1 `LOG 00 12 C3 C0 01` anuncia 854×480×3 bytes, mas o código a marca como JPEG); e um comando confirmado que restaure o logo original. Um envio errado pode deixar um logo de inicialização corrompido.
- Como o app oficial mostra CPU/memória ao vivo na lateral não está em nenhuma documentação pública.

Para registrar um resultado, troque "não verificado" por `ok AAAA-MM-DD` ou descreva o defeito. Um defeito vira uma issue (modelo "Defeito") com a saída de `akp153e.exe --probe`.
