# Guia de Contribuição — Keyforge (AKP153E Controller)

Obrigado por se interessar em contribuir com o Keyforge! Este guia explica como participar do projeto de forma organizada e profissional.

## 📋 Índice

- [Como contribuir](#como-contribuir)
- [Padrões de código](#padrões-de-código)
- [Workflow de PR](#workflow-de-pull-request)
- [Segurança](#segurança)
- [Releases](#releases)
- [Contato](#contato)

---

## 🚀 Como contribuir

### 1. Clone o repositório

```bash
git clone https://github.com/warleyvic24/akp153e-controller.git
cd akp153e-controller
```

### 2. Crie uma branch para sua feature

```bash
git checkout -b feature/sua-feature
```

### 3. Faça suas alterações

Siga os padrões de código abaixo. Teste localmente antes de commitar:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo deny check
```

### 4. Commit e push

```bash
git add .
git commit -m "feat: descrição clara da sua alteração"
git push origin sua-branch
```

### 5. Abra um Pull Request

Abra um PR no GitHub. Use o template de PR disponível em `.github/PULL_REQUEST_TEMPLATE.md`.

---

## 📐 Padrões de código

### Rust

- **Formato:** `cargo fmt` deve passar sem alterações
- **Lint:** `cargo clippy --all-targets -- -D warnings` deve passar com zero warnings
- **Testes:** `cargo test` deve passar com 0 falhas
- **Dependências:** `cargo deny check` deve passar (advisories, bans, licenses, sources OK)
- **Nomes de funções/variáveis:** em português do Brasil (ex.: `enviar_imagem`, `ler_config`)
- **Comentários:** em português, explicando o "porquê", não o "o quê"
- **Unsafe:** todo bloco `unsafe` deve ter comentário `// SAFETY:` explicando a soundness
- **Mutex:** usar `crate::sync::lock()` em vez de `.lock().unwrap()` diretamente

### Testes

- Todo novo recurso deve ter pelo menos um teste unitário
- Testes de performance devem usar limites folgados em debug e rígidos em release
- Testes de integração CLI devem usar portas/diretórios temporários

---

## 🔀 Workflow de Pull Request

1. Abra o PR para a branch `linux-port`
2. Um mantenedor fará code review
3. Os checks do GitHub Actions devem passar (CI Linux + Windows)
4. Após aprovação, o PR será feito merge via squash

### Critérios de aprovação

- [ ] Todos os testes passam
- [ ] `cargo clippy` sem warnings
- [ ] `cargo fmt` limpo
- [ ] Documentação atualizada (se aplicável)
- [ ] CHANGELOG atualizado (se for mudança visível ao usuário)

---

## 🔒 Segurança

Relate vulnerabilidades de segurança por email privado, **não** abra issues públicas para questões de segurança.

Veja `SECURITY.md` para detalhes sobre o modelo de segurança e como reportar problemas.

---

## 📦 Releases

Releases são criados automaticamente pelo GitHub Actions quando uma tag `v*.*.*` é feita na branch `linux-port`.

### Processo de release

1. Atualize o `CHANGELOG.md` com as mudanças
2. Crie uma tag: `git tag -a v0.2.0 -m "release v0.2.0 - descrição"`
3. Push da tag: `git push origin v0.2.0`
4. O GitHub Actions builda Windows (.exe + .msi) e publica a release automaticamente

---

## 📞 Contato

- Issues: https://github.com/warleyvic24/akp153e-controller/issues
- Email: warleyvic@example.com

---

*Obrigado por contribuir com o Keyforge!*
