# Roteiro até a nota 10

Data: 2026-10-31. Este arquivo manda na **ordem**. Os detalhes de cada passo estão em:
- `docs/PLANO-APARELHOS.md`: fases A0 a A8 (aparelhos);
- `docs/PLANO-UX.md`: etapas U0 a U9 (interface);
- `evals/hardware-checklist.md`: checklist do aparelho.

Regra: um degrau só começa quando o **portão** do anterior está cumprido. A nota é a do programa inteiro, não só
da interface.

## Onde estamos

- **Feito:** A0 (nome Keyforge), A1 (aparelho em arquivo), A2 (drivers), A3 (config por controle), A4 em parte
  (palco a partir do arquivo), primeiro passo de U6 (tipo da ação compacto).
- **Nota de hoje: 5,5.**
- **Checklist do aparelho:** 2 de 21 itens provados (10 e 13).

---

## Degrau 1: confiança no que já existe → nota 6

Uma tecla que às vezes falha anula todo o resto. Vem primeiro porque é barato e decisivo.

1. **Checklist do aparelho, itens 1 a 21**, feita com você. Eu preparo uma página de teste com tudo configurado
   e você aperta e anota. Cada falha vira conserto antes de seguir.
2. **Windows:** abrir o programa num Windows de verdade (seu ou de alguém) e repetir os itens 1 a 10.
3. **Janela opaca** (de U1): sem ela, toda captura e avaliação de interface fica errada.

**Portão:** 21 de 21 itens "ok" no Linux; 1 a 10 "ok" no Windows.

## Degrau 2: fundação da interface → nota 6,5

4. **U0, régua:** script de capturas no repositório e as 5 tarefas escritas, com a linha de base medida.
5. **U1, fundação visual:** fonte, escala de texto, componentes únicos, espaçamentos e raios.
6. **U2, janelas de verdade:** o componente Modal, Preferências em seções com brilho ao vivo, lista de apps com
   ícones reais.
7. **U3, topo:** um selo de sincronia e um seletor de perfil.

**Portão:** galeria "antes e depois" aprovada por você; nenhuma `egui::Window` crua; testes de contraste passando.

## Degrau 3: o editor bom de usar → nota 7

8. **U6, inspetor:** prévia grande, menu "⋯", "Avançado", seções de "Ao segurar" e "Dois estados" (a aba
   Alternar some), erros no próprio campo.
9. **U4, palco:** abas de página, ajuda que some, "Organizar cores" com prévia, uso da largura.
10. **U5, biblioteca:** "Recentes" e "Seus apps" no topo, linhas maiores, fantasma da tecla ao arrastar.

**Portão:** uma tecla de app se configura sem rolar o inspetor em 1366x768; você usa por uma semana e lista
o que incomoda, e a lista vira correção.

## Degrau 4: qualquer aparelho → nota 8

11. **A4, resto:** escolher o modelo à mão sem aparelho; dividir as páginas (`Page::split_for`) ao trocar
    para um modelo menor.
12. **A5, assistente "Ensinar meu aparelho"**, já feito com o Modal e os componentes do Degrau 2.
13. **A7, Elgato:** Mini, MK.2, XL, Neo; o Plus com giratórios e faixa de toque. Inclui os giratórios no
    palco e no inspetor (a parte de A4 que ficou para cá). Conferir a licença do crate `elgato-streamdeck`.
    Tudo nasce `experimental`.
14. **A6, catálogo da comunidade**, com validação no CI.

**Portão:** pelo menos um aparelho Elgato provado por alguém de fora (foto ou vídeo mais a checklist);
o assistente gera um arquivo igual ao embutido com o AKP153E fingindo ser desconhecido.

## Degrau 5: fácil de instalar e manter → nota 8,5

15. **Windows:** instalador assinado (sem o alerta do SmartScreen) e atualização automática com aviso no app.
16. **Linux:** Flatpak (Ubuntu, Fedora, Mint) com a regra de permissão do USB explicada num passo só; AUR para
    o Arch.
17. **U7, primeira execução:** pontos de partida (Streaming, Trabalho, Mídia, Em branco) e estados vazios com
    uma ação clara.
18. **Site e documentação de usuário:** uma página, um vídeo de 1 minuto, perguntas frequentes e onde pedir
    ajuda.

**Portão:** uma pessoa de fora instala e põe a primeira tecla em menos de 5 minutos, sem você ajudar.

## Degrau 6: faz mais que os outros → nota 9

19. **A8, plugins:** o formato (JSON por stdin/stdout, para ações e para aparelhos), com um aviso claro de que
    plugin roda código de terceiros.
20. **10 a 15 integrações populares**, como plugins oficiais: Spotify, Discord (mudo e ensurdecer), Home
    Assistant, Twitch, OBS avançado, Philips Hue, timer/Pomodoro, VoiceMeeter (Windows), PipeWire avançado
    (Linux).
21. **U8, acabamento:** tudo pelo teclado, inglês sem texto cortado, animações iguais.

**Portão:** um plugin feito por alguém de fora, seguindo só a documentação.

## Degrau 7: provado por pessoas → nota 9,5 a 10

22. **U9, teste com pessoas:** 3 pessoas, as 5 tarefas, tempo, erros e o questionário SUS (alvo ≥ 85). O que
    aparecer em 2 ou mais pessoas é corrigido e as tarefas são refeitas.
23. **Usuários reais:** lançamento público (Reddit r/streamdeck e r/linux, fórum do Omarchy, Discord), um
    canal de retorno e correção contínua por um mês.

**Portão (nota 10):**
- SUS ≥ 85;
- primeira tecla em menos de 30 s;
- nenhum problema aberto de "tecla não responde";
- três aparelhos de fabricantes diferentes provados.

---

## Resumo

| Degrau | Nota | O que destrava |
|---|---|---|
| 1 Confiança | 6 | Saber que funciona |
| 2 Fundação | 6,5 | Base visual para tudo o que vem depois |
| 3 Editor | 7 | Bom de usar no dia a dia |
| 4 Aparelhos | 8 | O nome "Keyforge" passa a ser verdade |
| 5 Instalação | 8,5 | Gente de fora consegue usar |
| 6 Plugins | 9 | Mais que os concorrentes |
| 7 Pessoas | 9,5–10 | Provado, não opinado |

**O que depende só de mim:** quase tudo do 2, do 3 e do 6. **O que depende de você ou de outras pessoas:**
apertar as teclas (1), um Windows (1), um Elgato emprestado ou um voluntário (4), um certificado de assinatura,
que é pago (5), e as pessoas do teste e do lançamento (7).
