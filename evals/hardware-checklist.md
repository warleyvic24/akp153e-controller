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
| 8 | Teclas laterais | Aperte as 3 teclas laterais | Nenhuma ação roda (são ignoradas de propósito) | não verificado |
| 9 | Brilho | Preferências > Brilho do aparelho: arraste de 100% até 5% e volte | O aparelho escurece e clareia ao salvar, sem reiniciar nem apagar as imagens; após reconectar o cabo, volta ao último valor | não verificado |

Para registrar um resultado, troque "não verificado" por `ok AAAA-MM-DD` ou descreva o defeito. Um defeito vira uma issue (modelo "Defeito") com a saída de `akp153e.exe --probe`.
