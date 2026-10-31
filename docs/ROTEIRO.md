# Roteiro até a nota 10 (plano mestre)

Data: 2026-10-31. Este arquivo manda na **ordem** e cobre **todas as áreas** do programa, não só interface e
aparelhos. Os detalhes de cada passo estão em:
- `docs/PLANO-APARELHOS.md`: fases A0 a A8;
- `docs/PLANO-UX.md`: etapas U0 a U9;
- `evals/hardware-checklist.md`: checklist do aparelho.

Notas medidas ou observadas em 2026-10-31:
- 21,9 mil linhas de Rust em 81 arquivos e 305 testes;
- 494 crates na árvore de dependências;
- binário de 17 MB;
- o serviço usa 42 MB e ~0% de CPU, e o editor 158 MB;
- a checklist do aparelho tem 2 de 21 itens provados.

---

## Parte 1: o boletim, área por área

"Teto" é o máximo realista com o que temos. Quando o teto não é 10, a coluna seguinte diz o que falta para ser.

| # | Área | Hoje | Teto | O que impede o 10 |
|---|---|---|---|---|
| 1 | Funciona no aparelho | 4 | 10 | Só falta provar: a checklist inteira |
| 2 | Estabilidade | 7 | 10 | Teste de longa duração, protocolo testado com dados aleatórios, falhas raras observadas |
| 3 | Desempenho | 8 | 10 | Editor com 158 MB e redesenhos sem necessidade; tempo de abertura não medido |
| 4 | Segurança | 6 | 9 | Senhas e tokens em texto puro; porta de controle aberta a qualquer processo no Windows; sem assinatura |
| 5 | Código e arquitetura | 7,5 | 10 | Tipos de ação como texto no editor; config ainda presa ao 3x5; protocolo de texto entre os processos |
| 6 | Testes e qualidade | 6,5 | 10 | Sem teste de interface automatizado, sem medida de cobertura, teste instável no Windows |
| 7 | Visual | 5,5 | 9,5 | Limites do egui (sombras, desfoque); o resto é trabalho |
| 8 | Usabilidade | 5 | 10 | Só se prova com pessoas (U9) |
| 9 | Acessibilidade | 6 | 9 | Leitor de tela nunca testado de verdade; o AccessKit no Linux é imaturo |
| 10 | Idiomas | 5 | 9 | Só PT e EN; o texto-fonte em PT complica tradutores; sem plural |
| 11 | Aparelhos suportados | 2 | 9 | Sem aparelhos para testar: depende de voluntários |
| 12 | Sistemas | 4 | 8 | Windows sem teste recente; **macOS impossível sem um Mac** |
| 13 | Instalação e atualização | 3 | 9,5 | Instalador assinado custa dinheiro; Flatpak e HID exigem cuidado |
| 14 | Funções | 6 | 10 | Plugins, pastas, multi-ação condicional, giratórios |
| 15 | Documentação | 5 | 10 | Falta a documentação de usuário (a de desenvolvedor é boa) |
| 16 | Comunidade e alcance | 1 | 8 | Ninguém conhece o programa; depende de lançamento e tempo |
| 17 | Licenças e marca | 7 | 10 | Conferir as licenças de todas as 494 crates (`cargo deny`); nome "Keyforge" sem pesquisa de marca |

**Média de hoje: 5,2. Teto realista: 9,5.** O 10 cravado em tudo não existe para um projeto de uma pessoa
mais eu. As áreas 11, 12 e 16 dependem de gente e de equipamento que não temos. Quem disser outra coisa
está vendendo.

---

## Parte 2: o que fazer em cada área

### 1. Funciona no aparelho (4 → 10)
- Página de teste pronta com tudo configurado; você aperta e anota (21 itens).
- **Modo diagnóstico:** `keyforge --selftest` acende as teclas uma a uma e registra o que chegou. A checklist
  vira 5 minutos, não 1 hora.
- Toda falha vira teste automático com o `FakeDriver` antes do conserto.

### 2. Estabilidade (7 → 10)
- **Teste de longa duração:** o serviço roda 24 h com o `FakeDriver` disparando 100 mil apertos, trocas de
  página e reconexões; memória e tempo de resposta medidos no começo e no fim.
- **Testes com dados aleatórios** (`cargo fuzz` ou `proptest`) no leitor de relatórios do aparelho, no
  arquivo de aparelho, na config e na porta de controle.
- Revisar os `unwrap`/`expect` fora dos testes: cada um vira tratamento ou ganha comentário dizendo por que
  não falha.
- **Relatório de falha local:** o gancho de pânico grava um arquivo com versão, sistema e o rastro, e o
  editor oferece "copiar relatório" (nada é enviado sozinho).

### 3. Desempenho (8 → 10)
- Medir: abertura do editor (meta < 400 ms até o primeiro quadro), aperto até a ação (meta < 30 ms), troca
  de página no aparelho (meta < 150 ms para 15 teclas).
- Editor: a janela só redesenha quando algo muda. Hoje relógio, teclas vivas e animações pedem quadro;
  conferir que, parada, a CPU fica ~0.
- Memória do editor: as texturas das teclas e da biblioteca de ícones com limite. Meta < 100 MB.
- Testes de tempo no CI (`criterion`) para desenhar uma tecla e codificar o JPEG; avisam se piorar 20%.

### 4. Segurança (6 → 9)
- **Senhas fora da config:** a senha do OBS e os tokens de webhook vão para o chaveiro do sistema (Secret
  Service no Linux, Credential Manager no Windows); a config guarda só uma referência.
- **Porta de controle no Windows:** conferir o dono do processo do outro lado, como já é feito no Linux;
  ou trocar por um pipe nomeado com permissão só do usuário.
- **"Comando" e "Webhook" importados de perfil alheio:** ao importar, mostrar a lista do que vai rodar e
  pedir confirmação.
- `cargo deny` no CI (licenças, crates duplicadas, avisos de segurança), além do `cargo audit` que já existe.
- Builds reproduzíveis e assinados (ligado à área 13). O 10 exigiria uma auditoria externa.

### 5. Código e arquitetura (7,5 → 10)
- **Tipos de ação no editor:** trocar as strings ("launch-app") por um `enum ActionKind` derivado de
  `ButtonAction`. O compilador passa a avisar quando faltar um caso.
- **Config sem o 3x5 fixo:** `sanitized` deixa de prender linhas e colunas e passa a usar o modelo da página
  (fecha a A4).
- **Comunicação editor ↔ serviço:** o protocolo de texto vira JSON, com versão e testes de contrato dos dois
  lados.
- Separar `inspector.rs`, `stage.rs` e `widgets.rs` por componente antes de cada etapa de interface.
- Um teste de "camadas": o editor só usa `service::runtime`, o `usb` não usa a interface. Hoje isso é só
  regra escrita.

### 6. Testes e qualidade (6,5 → 10)
- **Testes de interface** com `egui_kittest`: abrir o editor sem janela, clicar, digitar, comparar capturas
  com as aprovadas. Cobre as 5 tarefas do U9.
- Cobertura medida (`cargo llvm-cov`) no CI; meta ≥ 75% fora de `platform/`.
- Consertar o teste do ícone que falha às vezes no Windows (tempo maior ou um ícone de teste próprio).
- Testes de migração com configs reais anonimizadas de cada versão (v1, v2, v3).

### 7. Visual (5,5 → 9,5) e 8. Usabilidade (5 → 10)
- Todo o `docs/PLANO-UX.md`, U0 a U9. O 10 em usabilidade só vem do teste com pessoas.

### 9. Acessibilidade (6 → 9)
- Testar de verdade com o NVDA (Windows) e o Orca (Linux) nas 5 tarefas.
- Tudo pelo teclado, com foco visível (U8).
- Tamanho de texto ajustável (100/125/150%) nas Preferências.
- Modo de alto contraste.

### 10. Idiomas (5 → 9)
- Chaves estáveis (`inspector.test`) em vez do texto em português como chave; arquivos `.ftl` (Fluent) com
  plural.
- Espanhol (grande público de stream deck na América Latina) e alemão (texto longo, bom para achar cortes).
- Teste que acusa chave sem tradução e texto que não cabe no espaço.

### 11. Aparelhos (2 → 9)
- A4, A5, A6 e A7 do plano de aparelhos.
- Pedir aparelhos emprestados ou voluntários com AKP03, AKP815, 293S e Elgato Mini/MK.2/XL/Plus.
- Selo "verificado" só com prova.

### 12. Sistemas (4 → 8)
- **Windows:** conferido a cada versão, por você ou por um voluntário, mais o CI.
- **Linux:** além do Arch, testar no Ubuntu e no Fedora (máquina virtual com o USB repassado).
- **macOS:** honestamente, fica de fora até haver um Mac para testar. Compilar sem testar é pior que não
  oferecer.

### 13. Instalação e atualização (3 → 9,5)
- **Windows:** instalador MSI ou MSIX assinado (certificado pago, ~US$ 100 a 300 por ano; ou a assinatura
  gratuita do SignPath para código aberto) e atualização automática com aviso e nota da versão.
- **Linux:** Flatpak no Flathub, AUR, `.deb` e `.rpm`. A regra de permissão do USB é instalada por um
  assistente na primeira execução, com uma explicação de uma linha.
- Versões com número (0.2, 0.3…), registro de mudanças e builds no GitHub Releases pelo CI.

### 14. Funções (6 → 10)
- Plugins (A8) com 10 a 15 integrações oficiais.
- **Pastas:** uma tecla que abre um grupo de teclas e uma tecla "voltar", como no app da Elgato.
- Multi-ação com condição: se o app X está aberto, faz Y.
- Giratórios e faixa de toque (A7).
- Perfis prontos para baixar e compartilhar.

### 15. Documentação (5 → 10)
- Guia de usuário com capturas: instalar, primeira tecla, páginas, perfis, OBS, perguntas frequentes.
- Vídeo de 1 minuto.
- Guias "Adicionar um aparelho" e "Fazer um plugin", testados por alguém de fora.

### 16. Comunidade (1 → 8)
- Site de uma página, lançamento em r/streamdeck, r/linux, fórum do Omarchy e Discord, e um canal de ajuda.
- Modelos de issue já existem; acrescentar "Meu aparelho" com o `--probe` anexado.
- O 10 seria uma comunidade que mantém aparelhos e plugins sem você. Isso leva anos.

### 17. Licenças e marca (7 → 10)
- `cargo deny` confere as licenças das 494 crates; o `THIRD_PARTY_NOTICES.md` é gerado automaticamente.
- Pesquisar "Keyforge" como marca (INPI e USPTO) **antes** do lançamento; trocar depois custa caro.
- Renomear o repositório no GitHub (só você pode).

---

## Parte 3: a ordem (degraus com portões)

Um degrau só começa quando o **portão** do anterior está cumprido.

| Degrau | Nota | Conteúdo | Portão |
|---|---|---|---|
| **1. Confiança** | 6 | `--selftest`, checklist 21/21, Windows 1 a 10, janela opaca, teste instável consertado | 21/21 no Linux, 10/10 no Windows |
| **2. Alicerce** | 6,5 | `ActionKind`, protocolo JSON entre os processos, config sem 3x5, teste de camadas, `cargo deny`, cobertura, `egui_kittest`, U0 | Cobertura ≥ 60%; capturas de referência no CI |
| **3. Fundação visual** | 7 | U1, U2, U3; senhas no chaveiro (entra com as Preferências novas) | Capturas aprovadas por você; nenhuma janela crua |
| **4. Editor bom** | 7,5 | U4, U5, U6; tipos de dados aleatórios no protocolo e na config; teste de 24 h | Uma semana de uso seu sem queixa nova; 24 h sem vazamento |
| **5. Qualquer aparelho** | 8 | A4 resto, A5, A7 (Elgato), A6, giratórios | Um Elgato provado por alguém de fora |
| **6. Fácil de ter** | 8,5 | Instaladores assinados, Flatpak/AUR/deb/rpm, atualização, U7, guia de usuário, marca pesquisada, desempenho medido | Pessoa de fora instala e usa em < 5 min |
| **7. Mais que os outros** | 9 | A8 plugins, 10 a 15 integrações, pastas, multi-ação condicional, espanhol e alemão, U8, leitor de tela | Plugin feito por terceiros |
| **8. Provado** | 9,5 | U9 com 3 pessoas, lançamento, um mês de correções | SUS ≥ 85, primeira tecla < 30 s, nenhum "tecla não responde" aberto, 3 fabricantes provados |

**Tamanho honesto:** os degraus 1 a 4 são umas 6 a 10 sessões longas de trabalho meu, com você testando
entre elas. Os degraus 5 a 8 dependem de coisas fora do código (aparelhos, dinheiro, pessoas) e levam meses,
não sessões.

## O que depende de quem

- **Só de mim:** degraus 2, 3 e 4 inteiros; o código de 5, 6 e 7.
- **De você:** apertar as teclas (1), um Windows (1), usar por uma semana (4), decidir sobre o certificado (6),
  renomear o repositório (6).
- **De outras pessoas:** aparelhos e voluntários (5), testadores (8), comunidade (8).
