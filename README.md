# Finanças Locais

App desktop de finanças pessoais, de código aberto, que roda localmente no Windows e no Linux. Responde três perguntas: quanto gastei no mês, quanto vão ser as próximas faturas e como uma compra nova mudaria essas faturas.

Os dados ficam só na sua máquina, num arquivo SQLite.

## Download

**[Baixar a versão mais recente](https://github.com/CapJul/Financas-locais/releases/latest)** e, em *Assets*, escolher o arquivo do seu sistema:

| Sistema | Arquivo | Como instalar |
| --- | --- | --- |
| Windows | `Financas.Locais_x.y.z_x64-setup.exe` | Dê dois cliques e siga o instalador |
| Windows (alternativa) | `Financas.Locais_x.y.z_x64_en-US.msi` | Dê dois cliques |
| Linux (Debian/Ubuntu) | `.deb` | `sudo apt install ./arquivo.deb` |
| Linux (qualquer distro) | `.AppImage` | `chmod +x arquivo.AppImage` e execute |

**Aviso do Windows:** como o instalador ainda não tem assinatura digital paga, o Windows pode mostrar "O Windows protegeu o computador". Clique em **Mais informações → Executar assim mesmo**. O código-fonte completo está neste repositório.

## O que já funciona (v0.3)

- **Cadastros:** contas correntes, cartões (com dia de fechamento e vencimento), pessoas e categorias.
- **Lançamentos**, em três tipos, todos editáveis:
  - **Compra** à vista ou parcelada (até 120x). Pode ser sua, só de outra pessoa ("minha mãe usou meu cartão") ou dividida ("de R$ 100, R$ 50 é da minha irmã").
  - **Mensalidade:** gasto que se repete todo mês (ex.: assinatura do Claude), com dono, até ser cancelado.
  - **Entrada:** pix, salário, pagamentos e reembolsos recebidos. Pode indicar quem pagou.
- **Importar extrato ou fatura** em OFX ou CSV (Banco do Brasil, PicPay e outros bancos com colunas de data, descrição e valor). Mostra uma prévia, sugere a categoria pelo histórico, desmarca linhas de saldo e pagamento de fatura, e não importa a mesma linha duas vezes.
- **Sugestão de mensalidades:** cobranças que se repetem em 3 meses ou mais aparecem para você cadastrar com um clique.
- **Resumo do mês:** entradas, gastos e saldo; gastos por categoria, pessoa e tipo; acerto acumulado com cada pessoa; aviso de seguro perto de vencer.
- **Faturas:** próximas faturas de cada cartão com gráficos por tipo de gasto, categoria e pessoa, vencimento de cada uma e marcação de **fatura paga** (com aviso de atrasada).
- **Simulador:** "se eu comprar X por Y em Z vezes", mostra a soma das faturas de **todos os cartões** antes e depois e quanto você vai pagar a mais, sem salvar nada.
- **Relatórios:** escolha o período e exporte em **PDF** (pela impressão do sistema), **página HTML** ou **planilha CSV** para o Excel, com gráficos de entradas × gastos, categorias, pessoas, próximas faturas, mensalidades, seguros e a lista de lançamentos.
- **Seguros e veículos:** seguro do carro, de saúde, de vida etc., com o pagamento mensal entrando nas faturas e aviso quando a vigência está acabando.
- **Senha com criptografia:** com senha, o arquivo dos dados é criptografado (SQLCipher, AES-256).
- **Backup:** manual, restauração e backup automático numa pasta do OneDrive ou Google Drive.

## Regras de cálculo

1. Compra feita **antes** do dia de fechamento entra na fatura do mês; **no dia ou depois**, na do mês seguinte. Conta corrente usa o mês da compra.
2. Parcelas somam exatamente o total; o centavo que sobra vai para a 1ª parcela (R$ 100 em 3x = 33,34 + 33,33 + 33,33).
3. A divisão entre pessoas precisa somar o total. Sem divisão, a compra é toda de "Eu".
4. Fatura do mês = parcelas daquele mês + mensalidades que caem nele. Mensalidade cobrada no dia do fechamento ou depois entra na fatura seguinte, como uma compra.
5. No Resumo, gastos contam pelo mês da fatura; entradas, pela data em que o dinheiro entrou.
6. O acerto com cada pessoa é acumulado: tudo o que ela gastou até a fatura do mês menos tudo o que pagou até o fim do mês.
7. Todo valor é guardado em centavos (inteiro), para não ter erro de arredondamento.

As regras estão em `crates/core/src/dominio.rs`, cobertas por testes.

## Rodar a partir do código (para desenvolver)

### Pré-requisitos no Windows

0. **Strawberry Perl** ([strawberryperl.com](https://strawberryperl.com)): a criptografia (SQLCipher + OpenSSL) é compilada junto e precisa de Perl. Só para quem compila; quem baixa o instalador não precisa.
1. **Microsoft C++ Build Tools**: instale o [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) e marque "Desenvolvimento para desktop com C++".
2. **Rust**: instale pelo [rustup](https://rustup.rs) (`rustup-init.exe`).
3. **Node.js LTS**: [nodejs.org](https://nodejs.org).
4. **WebView2**: já vem no Windows 10 e 11.

### Pré-requisitos no Linux (Debian/Ubuntu)

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
```

E o Node.js LTS.

### Comandos

```bash
npm install            # dependências da interface
npm run tauri dev      # abre o app em modo desenvolvimento (recarrega ao salvar)
npm run tauri build    # gera o instalador em target/release/bundle/
cargo test -p financas-core   # testes das regras e do banco
```

A primeira compilação do Rust demora alguns minutos; as seguintes são rápidas.

### Onde ficam os dados

- Windows: `%APPDATA%\io.github.financaslocais\financas.db`
- Linux: `~/.local/share/io.github.financaslocais/financas.db`

Para fazer backup manual, feche o app e copie esse arquivo.

### Sobre a senha e a criptografia

- Com senha definida (Cadastros → Segurança), o arquivo `financas.db` inteiro fica criptografado com SQLCipher (AES-256, chave derivada da senha). Sem a senha, nem o app nem um leitor de SQLite conseguem abrir.
- **Não há recuperação de senha.** Se esquecer, os dados não abrem. Por isso, mantenha o backup automático ligado e lembre a senha que o backup usa.
- "Bloquear" fecha o banco e tira a chave da memória até a senha ser digitada de novo.
- Quem vem da v0.1 com senha: ao abrir a v0.2 e digitar a senha, o arquivo é criptografado na hora.

### Backup

- **Automático:** em Cadastros → Backup, escolha uma pasta dentro do OneDrive ou do Google Drive. Ao abrir, o app salva uma cópia lá (no máximo uma por dia) e guarda as 10 mais recentes. O programa da nuvem sincroniza sozinho.
- Com senha, os backups saem criptografados com ela. Sem senha, saem abertos.
- **Restaurar:** escolha o arquivo e informe a senha que o app tinha quando o backup foi feito. Depois de restaurado, os dados ficam com a senha atual.

## Estrutura

```
crates/core/          Núcleo em Rust, sem dependência do Tauri
  src/dominio.rs      Regras de fatura, parcelas, divisão e projeção
  src/repositorio.rs  SQLite: migrações, cadastros, lançamentos, resumo, simulação
  src/repositorio/    Arquivo e criptografia, importação, sugestões, seguros
  src/extrato.rs      Leitura de OFX e CSV dos bancos
  src/modelos.rs      Estruturas trocadas com a interface
  src/competencia.rs  Tipo AAAA-MM (mês da fatura)
src-tauri/            App Tauri: expõe o núcleo como comandos
src/                  Interface React + TypeScript
  api.ts              Chamadas tipadas aos comandos
  telas/              Resumo, Lançamentos, Faturas, Simulador, Cadastros
.github/workflows/    CI (testes) e Release (instaladores por tag)
```

## Publicar uma nova versão

1. Atualize a versão (ex.: `0.2.0`) em `src-tauri/tauri.conf.json`, `package.json` e `Cargo.toml`.
2. Faça commit e push na `main`.

O workflow `Release` percebe a mudança de versão, compila para Windows e Linux (uns 15 minutos), cria a tag `v0.2.0` e publica a release com os instaladores. Também dá para rodar à mão em **Actions → Release → Run workflow**.

## Próximas etapas

- Importação de fatura em PDF e do relatório do Registrato (Banco Central).
- Previsão de gastos com aprendizado de máquina.

## Licença

MIT
