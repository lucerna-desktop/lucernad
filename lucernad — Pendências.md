# lucernad — Pendências

O que falta fazer no `lucernad`. Cada item vira um PR; ao terminar, ele é marcado aqui. Os concluídos ficam como histórico.

Até 2026-11-04, estes itens ficavam nas Pendências do [`lucerna-shell`](https://github.com/lucerna-desktop/lucerna-shell), junto com os do shell; as sobras do modo jogo vêm da seção "Pendências da revisão final" do [spec do modo jogo](docs/superpowers/specs/2026-10-27-modo-jogo-design.md). Cada repositório do ecossistema tem o seu arquivo de pendências na raiz.

## Módulos

Da seção "Separação da lógica" das Pendências do shell (decisão de 2026-10-18; ver também [`docs/proposta.md`](docs/proposta.md)). Em ordem de prioridade:

1. [x] **Espectro:** captura do PipeWire e FFT próprias no lugar do `cava` (feito em 2026-10-20: módulo `spectrum` do `lucernad`; a batida fica para depois).
2. [x] **Sistema:** CPU, memória, rede, discos, temperaturas e GPU no lugar do parse em `services/SystemStats.qml`, do `gpu-status.sh`, do `drm-fdinfo.sh`, do `df` e do `lsblk`; NVIDIA pela NVML, sem `nvidia-smi` (feito em 2026-10-20: módulo `system` do `lucernad`).
3. [x] **Área de transferência:** observador do Wayland, histórico com deduplicação e imagens (SQLite), no lugar do `wl-paste --watch`, do base64 decodificado em JS e do `clipboard.json` (feito em 2026-10-21: módulo `clipboard` do `lucernad`).
4. [x] **Energia e brilho:** histórico do UPower e perfis de energia por D-Bus (hoje `busctl`), brilho interno e DDC/CI por biblioteca (hoje `brightnessctl` e `ddcutil`). (feito em 2026-10-22: módulo `power` do `lucernad`)
5. [x] **Rede externa:** letras (LRCLIB) e clima (Open-Meteo), com cache em disco (hoje sem cache). (feito em 2026-10-26: módulo `web` do `lucernad`)
6. [ ] **Launcher:** índice de apps com pontuação, frecência de verdade (hoje só contagem), busca de arquivos (hoje `fd`/`find`) e metadados de apps (hoje `app-info.sh`).
7. [ ] **Hyprland:** binds, entrada, keymap (hoje `xkbcli`) e monitores pelo socket IPC do Hyprland, no lugar do `hyprctl eval` e da geração de Lua.
8. [ ] **Luz noturna:** gama direto pelo protocolo `wlr-gamma-control`, sem o `hyprsunset`.

- [ ] **Ferramenta à parte:** `lucerna-video`, o conversor de papel de parede em vídeo (hoje `video-wallpaper.sh`, com `ffmpeg`), em lote, fora do daemon.
- [ ] Depois, se valer: captura de tela e gravação (hoje `grim` e `wf-recorder`).

Ligados a itens de outros repositórios:

- [x] **Bloqueio confiável pelo logind** (marco 1 do roteiro até o uso diário, nas Pendências do `lucerna-shell`): **decidido: fica no shell** (`services/Logind.qml`), sem módulo no `lucernad`. O shell escuta o `PrepareForSleep`, o `Lock`/`Unlock` e a tampa do logind direto pelo D-Bus e segura um *delay inhibitor* até a tela de bloqueio estar desenhada; com o shell morto, o logind dorme depois do prazo. Motivo: um só caminho, e o shell bloqueia sem o daemon (o `lucernad` continua sem depender do shell para dormir). Feito na base robusta do `lucerna-shell` (branch `feat/base-robusta`; PR do shell: https://github.com/lucerna-desktop/lucerna-shell/pull/71).
- [ ] **Migrar para o `connect_or_spawn` e o agendador do SDK** quando o `lucernad` passar a usar o `lucerna-service` por inteiro, com spec próprio (do "Trabalho seguinte" do plano do serviço sem systemd, lucerna-sdk#14).
- O **tema gerado a partir do papel de parede** (Material 3 a partir da imagem, guardado pelo hash do arquivo; num vídeo, por um quadro) é do `lucernad`, mas o projeto todo está no item 17 das Pendências do `lucerna-shell`, com spec próprio.
- O **módulo de discos** (aviso de pendrive, ejetar), sem depender do explorador, está na v3 do [spec do `lucerna-files`](https://github.com/lucerna-desktop/lucerna-files/blob/main/docs/superpowers/specs/2026-10-27-explorador-design.md).
- Os **comandos em inglês** (o `lucernad` tem subcomandos em português, como `restaurar-jogo`) seguem o padrão que vai ser desenhado no `lucerna-sdk`: ver as Pendências de lá.

## Pacote

- [ ] **Pacote do `lucernad`:** um PKGBUILD neste repositório. Hoje ele é instalado à mão pelos comandos do README e precisa ser recompilado e reinstalado a cada merge. O AUR e as outras distros estão no item 14 das Pendências do `lucerna-shell`.
- [ ] **Versões casadas com o shell:** o pacote do shell pede uma versão mínima do `lucernad` (hoje o casamento é o `dev/lucernad.rev` do shell), então os dois repositórios ganham tags e releases juntos; o `CHANGELOG` vem com elas. Parte do marco 6 do roteiro nas Pendências do `lucerna-shell`.

## Sobras do módulo de sistema

- [ ] **Uso da Intel na primeira amostra:** o fdinfo precisa de duas leituras, então ao abrir a aba Desempenho o uso da Intel fica "—" por ~2 s. Dá para tirar uma leitura curta extra no primeiro ciclo.
- O `lucernad status` sem a primeira amostra é do `status` do `lucerna-cli`: está nas Pendências do `lucerna-sdk`.

## Sobras do módulo de espectro

Baixo risco, adiadas na revisão final.

- [ ] **Captura, por dentro (`lucernad-spectrum/src/pipewire.rs`):**
  - quando um client sai, o grafo não é reenviado;
  - um nó novo só é ligado no próximo tique (até 100 ms);
  - um alvo que o WirePlumber recusa sempre é recriado a cada ~600 ms, sem log nem teto;
  - a sessão não espera a thread do PipeWire terminar;
  - a sonda que liga a captura logo que o nó aparece não confere o link player → sink.
- [ ] **Threads presas por sessão:** com o `/proc` travado para sempre, cada ciclo de assinatura deixa uma thread de casamento presa. O limite de um casamento em voo vale por sessão, não para o processo todo.
- [ ] **Casamento:**
  - o `/proc` é lido em duas vezes (`stat` e `exe`), então um PID reciclado no meio pode misturar processos;
  - um prazo do D-Bus estourado não refaz a conexão;
  - com `LUCERNA_TEST_HOST_BUS`, o PID do host é procurado no `/proc` do container.
- [ ] **Análise:**
  - se dois nós do mesmo player vierem com taxas diferentes, o analisador é recriado a cada amostra e o anel quase para;
  - com o player pausado e os nós ainda casados, as FFTs seguem rodando sem publicar;
  - a `magnitudes_db` cria o planejador da FFT a cada chamada.
- [ ] **"Capturando" depois de recarregar o shell:** o `spectrum.status` fica `capturing: true` por alguns segundos depois de um reload do Quickshell. A causa provável é o socket da geração antiga do shell ainda aberto; conferir com `ss -x` logo após um reload.

## Sobras do módulo da área de transferência

Baixo risco, adiadas na revisão final.

- [x] **Apagar de verdade (`lucernad-clipboard/src/store.rs`):**
  - o `secure_delete` (e o `secure-delete` do FTS) só protege dali em diante: o que foi apagado antes de ligá-lo fica nas páginas livres do banco até um `VACUUM`;
  - o corte pelo limite (`evict`) não passa pelo `forget`, então as páginas apagadas ficam no `-wal` até o checkpoint automático;
  - um `forget` que falha depois do `delete` devolve erro sem publicar a revisão nova.

  Resolvido na branch `fix/clipboard-sobras`: um banco sem a marca `cleaned` (tabela `meta`, não o `user_version`, para um `lucernad` mais velho seguir abrindo) tem o índice refeito (`rebuild`) e um `VACUUM`, uma vez; o corte pelo limite zera o `-wal` como o `remove` e o `clear`; o `forget` só vai ao log (também com o checkpoint ocupado), e a revisão nova sai.
- [x] **Item que some entre o `locate` e a leitura** (apagado no meio de um `copy`/`get`): sai como erro interno, não como o `1001` "O item <id> não existe mais.". Resolvido na branch `fix/clipboard-sobras`: o arquivo da imagem sumido na leitura vira o `1001` (o `get` lê tudo com o lock e não tinha a janela).
- [x] **`lucernad copiar` como primeiro cliente** (sem o shell nem outro assinante) cria um `history.db` vazio antes de achar o item. Resolvido na branch `fix/clipboard-sobras`: antes de qualquer `configure` e sem `history.db`, o `copy` e o `get` respondem o `1001` já, sem aplicar o padrão (o `list` segue aplicando).
- [x] **`O_NONBLOCK` no fd de quem cola:** a escrita da seleção põe o fd em não bloqueante, e isso fica na descrição de arquivo do outro app; o caminho `WouldBlock`/`POLLOUT` do envio não tem teste. Resolvido na branch `fix/clipboard-sobras`: as flags do fd voltam como estavam no fim do envio, deu certo ou não; testes da volta do modo e do pipe cheio que espera o `POLLOUT`.
- [x] **Miniatura reduzida e girada** (EXIF com rotação numa imagem grande): sem teste. Resolvido na branch `fix/clipboard-sobras`: teste com as orientações 6 e 8 numa imagem de 800 x 400 (dimensões e o canto que gira).
- [x] **Marcas de direção no `historico`** (U+202E, U+2066–2069): trocadas por U+FFFD na saída, como os caracteres de controle (resolvido na revisão final).

## Sobras do módulo de energia e brilho

Adiadas na revisão do módulo `power` (lucernad#14).

- [ ] Cada escrita na integrada faz 4 chamadas D-Bus ao logind (dá para usar um `GetAll` e guardar o `Type`/`Seat` da sessão).
- [ ] Um `GetHistory` que falha derruba o histórico inteiro (o último bom poderia ficar, com o erro ao lado).
- [ ] Os perfis ficam sem estado inicial se a primeira leitura falha (só voltam no próximo `PropertiesChanged` ou `NameOwnerChanged`).
- [ ] O `O_NOFOLLOW` do `TreeWriter` vale só no último componente do caminho.
- [ ] `EBUSY` no endereço 0x50 (EDID) sem tentar o `I2C_SLAVE_FORCE`.
- [ ] Docks DisplayLink (monitores sem barramento i2c de placa de vídeo) ficam de fora.

## Sobras do módulo de rede externa

Adiadas na revisão do módulo `web` (lucernad#17).

- [ ] **Vagalume não testado na máquina real:** a API (`api.vagalume.com.br`) respondeu 503 durante todo o desenvolvimento, inclusive a documentação e os termos. Quando voltar: conferir o formato da resposta (`type` `exact`/`aprox`), colar a chave nas Configurações do shell e testar uma faixa brasileira que a LRCLIB não tenha.
- [ ] Letra em erro não tem nova tentativa automática (só ao reabrir a aba, no `web.refresh` ou na volta da conexão).
- [ ] Com a chave do Vagalume, uma faixa instrumental é buscada de novo a cada vez que toca.
- [ ] A busca de cidade em andamento não é cancelada quando o modo offline liga, e há uma corrida curta entre a troca da conexão e a busca.
- [ ] O clima fica em `loading` ou com o erro de offline já superado enquanto não há assinante, e aparece um `loading` passageiro quando um `configure` troca a cidade e liga o offline juntos.
- [ ] A normalização faz as minúsculas depois de tirar os acentos (`İstanbul` vira `i stanbul`); o sufixo só é reconhecido depois de ` - ` com hífen comum (não ` – ` nem ` — `); os formatadores bidi (Cf) não são removidos do texto (o shell passa pelo `plain()`).
- [ ] O cache: a poda varre as duas pastas, mas só confere a do tipo gravado (um link na outra apagaria `.json` fora do cache), e a leitura segue link (um FIFO no lugar do arquivo trava a leitura).
- [ ] A conectividade: o `PropertiesChanged` não filtra pela interface do NetworkManager; há uma janela estreita no `watch()` em que um receptor novo fica num canal morto.
- [ ] Testes: o corpo lido em pedaços (resposta sem `Content-Length`) não tem teste; o `LocalServer` solto não fecha as conexões abertas; o `base_from_env` aceita porta com zero à esquerda; falta teste do `--lat`/`--lon` negativos na linha de comando; umidade e chance de chuva saem como `58.0` (o shell não se importa).

## Sobras do modo jogo

Ficaram de fora das correções da revisão final (2026-10-27, lucernad#19); nenhuma deixa os efeitos desligados no caso comum.

- [ ] **M2, reaplicação por cima da mudança do usuário no meio do modo:** um `game.configure` ou uma troca das janelas do tearing desliga de novo o que o usuário ligou durante o jogo. Ao sair, o resultado está certo. Ideia: guardar as opções "devolvidas ao usuário" e não replanejá-las até o modo sair.
- [ ] **M3, restauração pendente que pisa numa escolha posterior:** com a restauração em nova tentativa, se o usuário (ou um reload) põe a opção no mesmo valor que o modo jogo tinha posto, a nova tentativa a devolve ao `before`. É raro.
- [ ] **M9, `immediate` depois do reload:** no modo fresco, as janelas já marcadas não recebem o `immediate 1` de novo. Se o reload zera as propriedades das janelas (não conferido no 0.56.2), o tearing não volta nelas.
- [ ] **M10, horário das cópias adiadas:** as cópias do modo jogo entram no histórico com o horário da descarga (`now + i`), não o da cópia. A ordem fica certa, mas o "copiado há X" mente. Basta guardar o `now_ms()` no `defer`.
- [ ] **M7, resto:** uma falha `Unreachable` no `j/clients` (o socket sumiu) ainda vale como "sem gatilhos" e solta a trava "desligado na mão"; só os soluços (prazo, resposta estranha) mantêm os gatilhos de antes.
- [ ] **Publicar a lista "nunca" de fábrica** (um `game.defaults` ou o campo no `game.status`). Hoje o shell copia o `DEFAULT_NEVER` no `Config.gameNever` e sempre manda a lista dele no `game.configure`, porque o daemon não a publica (revisão final do shell, M2; o resto do item está nas Pendências do `lucerna-shell`).
