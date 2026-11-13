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

## O que já funciona

- **Cadastros:** contas correntes, cartões (com dia de fechamento e vencimento), pessoas e categorias.
- **Lançamentos**, em três tipos:
  - **Compra** à vista ou parcelada (até 120x). Pode ser sua, só de outra pessoa ("minha mãe usou meu cartão") ou dividida ("de R$ 100, R$ 50 é da minha irmã").
  - **Mensalidade:** gasto que se repete todo mês (ex.: assinatura do Claude), com dono, até ser cancelado.
  - **Entrada:** pix, salário, pagamentos e reembolsos recebidos. Pode indicar quem pagou.
- **Resumo do mês:** entradas, gastos e saldo; gastos por categoria, por pessoa e por tipo (à vista, parcelado, mensalidades); acerto acumulado com cada pessoa (quanto gastou, quanto já pagou, quanto falta).
- **Faturas:** próximas faturas de cada cartão (3 a 24 meses) com gráficos por tipo de gasto, por categoria e por pessoa, e o detalhe de cada mês por categoria.
- **Simulador:** "se eu comprar X por Y em Z vezes", mostra as faturas antes e depois, sem salvar nada.
- **Senha de acesso:** opcional, em Cadastros → Segurança. Com senha, o app abre bloqueado e tem o botão "Bloquear" no menu.

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

### Sobre a senha

- Só o hash da senha (Argon2) é guardado, nunca a senha em si.
- Com o app bloqueado, o núcleo em Rust recusa qualquer leitura ou gravação, mesmo que alguém mexa na interface.
- **Limite:** a senha bloqueia o app, não cifra o arquivo `financas.db`. Quem tiver acesso ao seu usuário do Windows e souber onde está o arquivo pode abri-lo com um leitor de SQLite. Cifrar o arquivo (SQLCipher) está previsto para uma próxima etapa.
- Não há recuperação de senha esquecida. Se acontecer, apague a linha `senha_hash` da tabela `config` com um leitor de SQLite.

## Estrutura

```
crates/core/          Núcleo em Rust, sem dependência do Tauri
  src/dominio.rs      Regras de fatura, parcelas, divisão e projeção
  src/repositorio.rs  SQLite: migrações, cadastros, lançamentos, resumo, simulação
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

4. Importação de extrato/fatura em CSV e OFX, com deduplicação.
5. Backup local pelo app.
6. Backup cifrado no Google Drive/OneDrive, importação de PDF, seguros e veículos.
7. Previsão com aprendizado de máquina.

## Licença

MIT
