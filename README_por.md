<p align="center">
  <img src="images/EH3D_PHOENIX_CNC_BANNER.svg" alt="EH3D-PHOENIX-CNC banner" width="100%">
</p>

# <img src="images/brand/icon-64.png" alt="" width="40" height="40" align="absmiddle"> EH3D-PHOENIX-CNC
<p align="center"><a href="README.md">🇺🇸 English</a> | <a href="README_spa.md">🇪🇸 Español</a> | <a href="README_fra.md">🇫🇷 Français</a> | <a href="README_ita.md">🇮🇹 Italiano</a> | <a href="README_deu.md">🇩🇪 Deutsch</a> | <a href="README_zho.md">🇨🇳 中文</a> | <a href="README_jpn.md">🇯🇵 日本語</a> | 🇵🇹 <b>Português</b></p>

### A base partilhada dos programas de controlo de máquina do EH3D: a vista de um trabalho, os temas e os textos da interface em oito idiomas

<p align="center">
  <img src="https://img.shields.io/badge/License-GPL%203.0--or--later-blue.svg" alt="GPL 3.0 or later">
  <img src="https://img.shields.io/badge/Stack-Rust-DEA584.svg" alt="Rust">
  <img src="https://img.shields.io/badge/Languages-8-00E5FF.svg" alt="Languages">
  <img src="https://img.shields.io/badge/Version-0.0.9-0f6cbd.svg" alt="Version">
  <img src="https://img.shields.io/badge/Maturity-scaffolding-FFB020.svg" alt="Maturity">
  <img src="https://img.shields.io/badge/Tests-126-0f6cbd.svg" alt="Tests">
</p>

---

**Verificação de honestidade - o que funciona hoje:** um programa em Rust com uma janela (`phoenix-cnc`) que comanda uma máquina **simulada**: mostra a posição da máquina (peça e máquina), move-a à mão com um teclado de jog, põe o zero, faz a referência, aceita os comandos escritos pelo operador, executa um programa G-code linha a linha (com pausa, paragem e paragem de emergência), desenha a trajetória do programa na mesa com as linhas feitas marcadas e pára no que o controlador recusa. A janela fala 37 idiomas e tem onze temas. A interface de controladores (`eh3d-controller`, no EH3D-COMMON) tem o GRBL escrito contra uma linha simulada e lista as outras famílias como ainda sem controlador. 126 testes correm num computador. **Nada aqui foi executado numa máquina real**, só se abre uma porta série para GRBL e uma ligação de rede só ao enviar um programa ao servidor, e os arcos e os outros códigos o simulador nem os desenha nem os executa.

---

> **Estado de segurança:** este programa comanda apenas uma máquina simulada: não abre nenhuma porta série nem ligação de rede. Quando se ligar um controlador real, terá de se provar num simulador e depois numa máquina real antes de se dizer funcional, e os fins de curso e uma paragem de emergência continuam a ser da própria unidade de controlo da máquina; a paragem de emergência da janela não é um circuito de segurança.

## 📋 Num relance

| | |
|---|---|
| Versão | `0.0.9` |
| Maturidade | Estrutura inicial |
| Testes | 126 |
| Tecnologias | Rust |
| Em hardware real | Não: comanda apenas uma máquina simulada |

**Ir para:** [Visão geral](#-visão-geral) · [Arquitetura](#-arquitetura) · [Início rápido](#-início-rápido) · [Testes](#-testes) · [Estrutura](#-estrutura-do-repositório) · [Relacionados](#-projetos-relacionados)

## 🚦 Estado do projeto

Cada linha é uma parte deste repositório e a luz diz até onde chega hoje. Escreve-se com o que o repositório consegue mostrar (testes, compilações, execuções), não com o que está planeado.

🟢 Feito e verificado · 🟡 Funciona, com limites conhecidos · 🟠 Feito a meio ou ainda por verificar · 🔴 Por fazer ou a não funcionar

| Área | Estado | O que significa aqui |
|---|---|---|
| Biblioteca base | 🟡 | Um crate de Rust que reexporta `scene`, `texts` e `theme` do `eh3d-view` com o nome deste programa; precisa da pasta do EH3D-COMMON ao lado. |
| Cena e vistas | 🟡 | Escrita e testada no EH3D-COMMON; a janela deste programa desenha a trajetória de um programa na mesa e a posição da ferramenta (vista num computador). |
| Temas e textos | 🟡 | Onze temas e as palavras da interface em 37 idiomas, vistos na janela deste programa. |
| Testes | 🟡 | Passam 126 testes num computador (`cargo test --features gui`): a sessão (jog, zero, referência, um programa, pausa, alarme, paragem de emergência), a trajetória de um programa e a linha de comandos; o controlador simulado é testado no EH3D-COMMON. |
| Janela e jog | 🟡 | Funciona num computador com o controlador simulado: posição, teclado de jog, passos, avanço, referência, zero, desbloqueio, uma linha de comandos e um registo; sem máquina real. |
| Controlo de máquina | 🟠 | Só o simulador: um programa corre linha a linha com pausa, paragem e paragem de emergência; ainda não se liga nenhum controlador real a partir daqui. |
| Documentação | 🟡 | README em oito idiomas, as duas imagens, três programas de exemplo e o changelog. |

## 🎯 Visão geral

* A **janela** (`src/gui.rs`, `phoenix-cnc`): a posição da máquina (peça e máquina), um teclado de jog com os seus passos e avanço, referência, zero e desbloqueio, a paragem de emergência, uma linha de comandos com o seu registo e o programa com a sua trajetória na mesa.
* A **sessão** (`src/session.rs`): o que o programa faz com qualquer controlador de `eh3d-controller`: jog, zero, referência, enviar uma linha, executar um programa linha a linha, pausa, paragem e os alarmes; é testada com o controlador simulado.
* A **trajetória de um programa** (`src/gcode.rs`): os movimentos retos (`G0`, `G1`, absolutos ou relativos, em mm ou polegadas) desenhados antes de executar o programa; os arcos não se desenham.
* O **controlador simulado** (`eh3d_controller::sim`, no EH3D-COMMON): uma máquina em memória com limites de curso que dão alarme; as famílias de controladores do mercado estão listadas em `eh3d-controller`, o GRBL é a primeira escrita e as outras aparecem como ainda sem controlador.
* Os **temas e as palavras** de `eh3d-view`: Forge (o predefinido), Neon Arcade, Glass Pro, o cinzento clássico do Windows e um que segue o Windows, e a interface em inglês, espanhol, italiano, alemão, francês, português, chinês e japonês. O que será: um programa para furar, fresar e fazer CNC 2D e 3D sobre os motores do EH3D-DRILLPRO e do EH3D-ROUTEPRO; essa parte ainda não existe.
* Os **menus, as definições e os ficheiros**: os menus Ficheiro, Ver, Programa, Máquina e Ajuda com o que o PlanetCNC tem (abrir, recentes, guardar, editar, os zeros, as posições guardadas, o fuso e o refrigerante); uma janela de definições (unidades, idioma, onze temas, limites de curso, jog) que se recorda entre execuções; e as janelas Abrir e Guardar da suite (`eh3d-filedialog`: locais, ordenação, filtros, a extensão de um ficheiro guardado e a pergunta antes de substituir).
* **GRBL numa porta série:** compilado com `--features serial`, `phoenix-cnc --grbl PORTA[@BAUD]` comanda um controlador GRBL (115200 baud por omissão) em vez do simulador e `phoenix-cnc --ports` lista as portas série; uma compilação sem a funcionalidade, ou uma porta que não abre, di-lo e termina. Experimentado apenas contra um controlador guionizado, nunca contra um real.
* **Uma janela que segue o ecrã:** abre maximizada e, de 720p a 1080p e até ecrãs maiores e mais largos, tudo cresce ou encolhe com ela.
* **Continuar após uma falha:** enquanto um programa corre, a linha alcançada é guardada num ficheiro pequeno; se a janela, o computador ou a ligação cair, o arranque seguinte pergunta se continua a partir dessa linha ou se deixa. Nada é enviado ao controlador até o operador responder.
* **Enviado para o servidor EH3D:** com o endereço do servidor, o token deste programa e o id da máquina nas definições (secção Servidor da janela de definições, ou `server_address`, `server_token` e `server_machine` no ficheiro de definições), **Programa > Enviar para o servidor** envia o texto do programa como um trabalho de G-code; o servidor executa-o na sua máquina (um controlador GRBL na sua porta série, ou o seu simulador) e a janela acompanha-o: o estado, a percentagem e a linha alcançada na barra de topo, no painel e no registo. Iniciar, Pausa e Parar da janela chegam ao trabalho no servidor, fechar a janela não o pára, e um trabalho que parou é retomado a partir da linha a que o servidor chegou (o arranque seguinte da janela oferece-o, como faz com um programa do controlador): o servidor devolve os códigos em vigor e espera pelo operador antes do primeiro movimento. O fuso e o refrigerante só voltam a arrancar com `server_restore_outputs = yes`. Experimentado com um servidor de substituição; os testes do próprio servidor enviam os mesmos pedidos ao seu simulador. Nunca numa máquina.
* **Retomar após uma falha:** um programa que ficou a meio é primeiro devolvido ao seu estado: unidades e distâncias absolutas, um Z seguro (`resume_safe_z` no ficheiro de definições, 5 mm), o fuso com o tempo para atingir a velocidade (`resume_spindle_wait`, 3 s), o refrigerante, o ponto atingido e o estado modal; o ficheiro de recuperação guarda o SHA-256 do programa e não continua se o programa mudou.
* **As ajudas da janela:** um **resumo antes de iniciar um programa** (os movimentos, a distância de corte, o tempo às velocidades das definições, o alcance dos eixos e uma verificação face ao curso da máquina, com os avisos que a suite partilha: um programa minúsculo ou longe do canto, maior do que a mesa, arcos que a estimativa deixa de fora e cortes antes de qualquer avanço; **Iniciar mesmo assim**, ou uma recusa quando o programa sai do curso, que `reject_out_of_travel = no` transforma em aviso, e `confirm_start = no` desativa o resumo); **teclas do teclado** (Ctrl+O abrir, Ctrl+S guardar, Ctrl+E editar, F5 iniciar, Espaço pausa, Esc parar (com uma janela aberta o Esc fecha-a, e com o teclado numa caixa de texto o Espaço e o Esc ficam para a caixa), F1 lista-as; `shortcuts = no` desativa-as); uma **barra de ferramentas** com desenhos e os seus nomes; números que se alteram com menos e mais ou **escrevendo-os**; a trajetória **arrastada e ampliada de cima ou rodada e ampliada em 3D** com a ferramenta; uma **pré-visualização do ficheiro G-code** na janela Abrir; o **registo de uma falha** (`eh3d-crash.log`); e uma secção Segurança nas definições (`rapid`, `summary_tiny`, `summary_far` e `summary_margin` no ficheiro de definições).
* **Uma fila de programas e o curso do controlador:** vários ficheiros escolhidos na janela Abrir com Ctrl+clique, largados em conjunto ou dados na linha de comandos (`phoenix-cnc a.nc b.nc`) correm um a seguir ao outro, cada um depois do seu próprio resumo; a janela lista-os com o seu estado, deixa retirar, ignorar e reordenar os que esperam, e a fila pára quando um programa não termina. Com um controlador GRBL o programa lê `$130`, `$131` e `$132` (o maior curso de X, Y e Z) ao ligar e usa o menor entre isso e `travel_x`, `travel_y` e `travel_z` para o resumo e a recusa; se o controlador não responder ou não os der, usam-se as definições e o registo di-lo (`read_controller_travel = no` desativa a leitura).

## 🧩 Papel no EH3D

O EH3D-PHOENIX-CNC é o programa de controlo de máquina do EH3D, hoje a funcionar sobre uma máquina simulada. Depende por caminho do EH3D-COMMON (a pasta `EH3D-COMMON` tem de estar ao lado desta: a vista, os temas, as palavras e os controladores) e de nenhum outro repositório EH3D por agora.

## 🏛️ Arquitetura

<p align="center">
  <img src="images/architecture.svg" alt="Arquitetura do EH3D-PHOENIX-CNC: quem o vai usar, o que contém e em que se apoia" width="100%">
</p>

Como as partes se encaixam: quem o vai usar, o que este repositório contém e em que se apoia. A imagem é desenhada a partir de `docs/architecture.json`, verificada contra o código, e marca como futuro o que é futuro.

## 🖼️ Capturas de ecrã

<p align="center">
  <img src="images/screenshots/program-running.png" alt="a program running" width="100%">
</p>

<p align="center"><em>Um programa em curso na máquina simulada: as linhas feitas a verde, a linha atual realçada, a trajetória desenhada na mesa e a ferramenta onde está a máquina.</em></p>

<p align="center">
  <img src="images/screenshots/spiral-neon.png" alt="Midnight theme" width="100%">
</p>

<p align="center"><em>O tema Meia-noite, com um programa longo já terminado.</em></p>

<p align="center">
  <img src="images/screenshots/square-german.png" alt="German interface, light theme" width="100%">
</p>

<p align="center"><em>A interface em alemão com o tema claro Papel: cada palavra da janela segue o idioma.</em></p>

<p align="center">
  <img src="images/screenshots/settings-window.png" alt="settings window" width="100%">
</p>

<p align="center"><em>A janela de definições, com as secções à esquerda como a árvore do PlanetCNC: o idioma, os onze temas e o que se mostra.</em></p>

<p align="center">
  <img src="images/screenshots/open-window.png" alt="Open window" width="100%">
</p>

<p align="center"><em>A janela Abrir da suite: locais, recentes, a lista da pasta com datas e tamanhos, o nome e o tipo.</em></p>

## 🔒 Segurança e proteção

Este programa só abre uma porta série quando é arrancado para GRBL (`--grbl`, uma compilação com `--features serial`) e só abre uma ligação de rede ao servidor EH3D cujo endereço está nas definições, quando o operador lhe envia um programa; o token fica guardado como texto no ficheiro de definições. Não abre ficheiros de outras pessoas, exceto o ficheiro G-code que o operador escolhe (lido como texto, nunca executado por si), e não é um sistema de segurança. Ver [SECURITY.md](SECURITY.md).

## 🚀 Início rápido

Precisa de uma cadeia de ferramentas Rust estável e da pasta `EH3D-COMMON` ao lado desta.

```bash
cargo build --release --features gui
cargo test --features gui
cargo clippy --all-targets --features gui -- -D warnings
cargo fmt --all --check
python -I tools/ci_validate.py
cargo run --release --features gui -- examples/02_frame_and_pocket.nc
```

`build.bat` / `build.sh` compilam o programa, `test.bat` / `test.sh` executam os testes e `run.bat` / `run.sh` abrem a janela com um programa de exemplo. Os scripts esperam uma tecla no fim, a menos que `NOPAUSE` esteja definido.

## 🧪 Testes

`cargo test --features gui` executa 126 testes: a sessão (jog e zero, um programa linha a linha, pausa e continuar, uma linha recusada, a paragem de emergência e o alarme, o registo dos comandos, a referência), a trajetória de um programa e a linha de comandos; a vista e o controlador simulado são testados no EH3D-COMMON. A CI também executa `cargo fmt --check` e `cargo clippy` com avisos como erros, e `python -I tools/ci_validate.py` verifica o manifesto, as versões, o changelog e os oito README.

## 🏗️ Compilação e publicação

A versão está em `Cargo.toml` e em `eh3d.project.json`, e cada versão tem a sua entrada em [CHANGELOG.md](CHANGELOG.md). Ainda não há programa para empacotar, por isso não há binários prontos.

## 📂 Estrutura do repositório

<p align="center">
  <img src="images/repository-structure.svg" alt="Estrutura do repositório do EH3D-PHOENIX-CNC" width="100%">
</p>

<details>
<summary>Versão em texto da árvore</summary>

```text
EH3D-PHOENIX-CNC/
├── src/
│   ├── lib.rs                                   os módulos e a vista do eh3d-view reexportada com o nome deste programa
│   ├── gui.rs                                   a janela: posição, jog, programa, trajetória, comandos e registo
│   ├── session.rs                               o que se faz com um controlador: jog, zero, referência, um programa linha a linha, pausa, paragem, alarmes
│   ├── queue.rs                                 a fila de programas: ordem, estado, ignorar, retirar, reordenar, paragem se falha
│   ├── gcode.rs                                 os movimentos retos de um programa G-code, para desenhar a sua trajetória
│   ├── view.rs                                  o plano que se arrasta e amplia e a vista 3D que se roda, como números
│   ├── plan.rs                                  a verificação antes do arranque: tempo, alcance e curso dos movimentos
│   ├── cli.rs                                   a linha de comandos: o ficheiro, o idioma, o tema e as opções de imagem
│   ├── bin/phoenix-cnc.rs                       o programa que abre a janela
│   ├── server.rs                                a ligação ao servidor EH3D: enviar, acompanhar, pausar, parar
│   ├── settings.rs                              o que se recorda entre execuções
│   ├── gui/dialogs.rs                           menus, definições, Abrir, Guardar, editor
│   ├── gui/remote.rs                            o programa no servidor: Iniciar, Pausa, Parar e retomar
│   ├── gui/views.rs                             o centro da janela: a trajetória vista de cima ou em 3D, com a ferramenta
│   ├── gui/summary.rs                           a janela de resumo antes do arranque e a lista de teclas
│   ├── gui/numfield.rs                          um número definido com os botões menos e mais ou escrevendo-o
│   ├── gui/keys.rs                              as teclas do teclado e a tabela que os menus e a lista de teclas leem
│   ├── gui/queue.rs                             a fila na janela: ficheiros um a seguir ao outro, cada um após o seu resumo
│   └── gui/icons.rs                             os desenhos dos botões, traçados com linhas, e o botão da barra de ferramentas
├── examples/                                    três programas G-code de exemplo, de um quadrado a uma espiral longa
├── tests/                                       mais testes do G-code e das definições
├── tools/                                       verificação de CI, do README e ferramentas de imagens
├── docs/                                        as fontes das imagens de Arquitetura e Estrutura do repositório
├── .github/                                     fluxo de CI, modelos de issues e pull requests
├── images/                                      faixa, as duas imagens e as capturas de ecrã
├── build.sh build.bat                           compilam a biblioteca
├── run.sh run.bat                               avisam de que ainda não há programa e executam os testes
├── eh3d.project.json                            manifesto: versão, papel, maturidade e comando de compilação
├── Cargo.toml Cargo.lock                        o pacote Rust e as suas dependências fixadas
├── CHANGELOG.md                                 o que mudou em cada versão
├── SECURITY.md SUPPORT.md                       como comunicar uma vulnerabilidade e onde perguntar
└── CONTRIBUTING.md CODE_OF_CONDUCT.md LICENSE   como contribuir, o código de conduta e a licença
```

</details>

## 🗺️ Roteiro e limites conhecidos

**Limites conhecidos hoje**

* Nada aqui foi executado numa máquina real: o controlador é um simulador (ou GRBL numa porta série, nunca experimentado) e o servidor só é alcançado pelos testes.
* O simulador só aceita G-code básico (`G0`, `G1`, `G90`, `G91`, `G20`, `G21`, `G92`, `M3` a `M9`); os arcos, os ciclos fixos e a apalpação são recusados, e a trajetória não desenha arcos.
* Precisa da pasta `EH3D-COMMON` ao lado, porque depende dela por caminho.

**A seguir**

* Um controlador real: primeiro o GRBL por porta série, depois as outras famílias, depois o controlador do HYDRA-UMC.
* Os arcos e o resto do G-code no simulador e na trajetória.
* Uma vista 3D da trajetória com a altura dos cortes.
* Apalpação, trocador de ferramentas e os materiais da máquina.
* Juntar os motores do EH3D-DRILLPRO e do EH3D-ROUTEPRO para que um programa fure e frese.

## 🌍 Idiomas

O README e os textos existem em inglês, espanhol, francês, italiano, alemão, chinês, japonês e português europeu.

## 🔗 Projetos relacionados

* [EH3D-COMMON](https://github.com/JuanenRac/EH3D-COMMON): onde vivem a vista (`eh3d-view`) e o ZIP de trabalhos (`eh3d-archive`)
* [EH3D-DRILLPRO](https://github.com/JuanenRac/EH3D-DRILLPRO): o programa de furação cujo motor será juntado aqui
* [EH3D-ROUTEPRO](https://github.com/JuanenRac/EH3D-ROUTEPRO): o programa de fresagem cujo motor será juntado aqui

## 📚 Documentação

* [Registo de alterações](CHANGELOG.md)
* [Política de segurança](SECURITY.md) e [suporte](SUPPORT.md)
* A vista de um trabalho está documentada no EH3D-COMMON, onde vive.

## 🤝 Contribuir e comunidade

* Antes de um pull request execute `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` e `python -I tools/ci_validate.py`: tudo tem de passar. Ver [CONTRIBUTING.md](CONTRIBUTING.md) e o [código de conduta](CODE_OF_CONDUCT.md).
* O README existe em oito idiomas: uma alteração ao texto faz-se nos oito ficheiros, e as duas imagens são desenhadas a partir de `docs/architecture.json` e `docs/structure.json` com `tools/make_architecture.py` e `tools/make_structure.py`.
* Não descreva como a funcionar o que não correu numa máquina.
* Comunique uma vulnerabilidade em privado: [SECURITY.md](SECURITY.md). Perguntas, ideias e relatos: electrohobby3d@gmail.com ([SUPPORT.md](SUPPORT.md)).

## 👤 AUTOR

**JuanenRac (Electro Hobby 3D)** · electrohobby3d@gmail.com

## 📜 LICENÇA

Software: GPL-3.0-or-later. Ver [LICENSE](LICENSE). Os desenhos de hardware da suite usam CERN OHL-S.
