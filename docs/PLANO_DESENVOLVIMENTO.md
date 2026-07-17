# Plano de desenvolvimento do AuxScreen

## 1. Objetivo

Criar uma solução open source que transforme um dispositivo Android em um monitor adicional para computadores Linux, oferecendo área de trabalho estendida, baixa latência e entrada por toque, mouse e teclado.

O projeto deverá funcionar sem depender de serviços em nuvem e priorizar interfaces padronizadas do Linux, evitando integrações frágeis e específicas de um único compositor sempre que possível.

## 2. Arquitetura proposta

```text
Linux KDE Plasma / Wayland
          │
          ├── monitor físico
          │
          └── monitor virtual
                    │
               PipeWire
                    │
          codificação H.264
                    │
                WebRTC
                    │
             aplicativo Android
                    │
          MediaCodec + SurfaceView
```

### Host Linux

- Rust
- Tokio
- zbus para D-Bus
- GStreamer e bindings Rust
- PipeWire
- XDG Desktop Portal ScreenCast
- XDG Desktop Portal RemoteDesktop
- mDNS para descoberta local
- WebRTC para vídeo e canal de controle

### Cliente Android

- Kotlin
- Jetpack Compose para interface
- SurfaceView para vídeo
- MediaCodec para decodificação por hardware
- WebRTC nativo
- Coroutines
- DataStore
- mDNS

## 3. Escopo do MVP

### Host suportado inicialmente

- Arch Linux
- KDE Plasma 6
- Wayland
- PipeWire
- GPUs NVIDIA, AMD e Intel
- conexão por rede local

### Cliente suportado inicialmente

- Android 11 ou superior
- arquitetura ARM64
- tablets e celulares
- orientação horizontal e vertical

### Funcionalidades

- criar um monitor virtual;
- selecionar resolução e taxa de atualização;
- transmitir a 30 ou 60 FPS;
- usar H.264 com aceleração por hardware;
- conectar pela rede local;
- parear por código ou QR Code;
- enviar toque, mouse, rolagem e teclado;
- reconectar após perda temporária da rede;
- mostrar FPS, bitrate e latência;
- funcionar sem servidor externo.

### Fora do MVP

- acesso pela internet;
- áudio;
- clipboard;
- transferência de arquivos;
- caneta com pressão;
- conexão USB;
- HDR;
- múltiplos tablets;
- host Windows ou macOS;
- suporte completo ao X11.

## 4. Monitor virtual

### Caminho principal

Usar o portal `org.freedesktop.portal.ScreenCast`, solicitando uma fonte do tipo `VIRTUAL`.

Fluxo previsto:

1. criar uma sessão;
2. solicitar uma fonte virtual;
3. iniciar a sessão;
4. receber o identificador do nó PipeWire;
5. conectar o pipeline GStreamer ao nó;
6. codificar e transmitir o vídeo.

### Fallbacks

Caso o compositor não suporte uma saída virtual:

1. permitir transmitir um monitor existente;
2. informar claramente a incompatibilidade;
3. documentar o uso opcional de VKMS;
4. considerar backend X11 apenas após o MVP.

Protocolos privados do KDE não devem ser usados como caminho principal.

## 5. Entrada remota

Usar o portal `org.freedesktop.portal.RemoteDesktop` para:

- teclado;
- ponteiro absoluto;
- ponteiro relativo;
- touchscreen.

### Fallback

Para sistemas sem suporte adequado ao portal, considerar um helper mínimo baseado em `uinput`, com regra de `udev` e comunicação por socket Unix.

O caminho padrão não deverá exigir root.

## 6. Transporte

### Primeira implementação

Usar WebRTC:

- RTP para vídeo;
- H.264;
- RTCDataChannel para eventos de entrada e controle;
- DTLS-SRTP para criptografia;
- ICE limitado à rede local no MVP.

### Mensagens de controle

Tipos previstos:

```text
pointer_absolute
pointer_relative
mouse_button
mouse_scroll
key_down
key_up
touch_down
touch_move
touch_up
display_resize
request_keyframe
ping
pong
```

Coordenadas devem ser normalizadas entre `0.0` e `1.0`.

Exemplo:

```json
{
  "type": "pointer_absolute",
  "x": 0.42,
  "y": 0.73,
  "buttons": 1,
  "timestamp_us": 182736128
}
```

### Evolução futura

Caso as medições mostrem que WebRTC introduz buffering excessivo, avaliar:

- WebRTC apenas para pareamento e controle;
- transporte próprio por QUIC ou UDP;
- envio direto de frames H.264, HEVC ou AV1;
- retransmissão seletiva de quadros essenciais.

Nenhum protocolo próprio deverá ser criado antes de existirem medições que justifiquem isso.

## 7. Codificação de vídeo

### Ordem de preferência

1. NVIDIA NVENC;
2. VA-API para Intel e AMD;
3. V4L2 hardware encoder;
4. x264 por software.

### Codec inicial

H.264, por compatibilidade ampla e bom suporte de hardware no Android.

### Configuração inicial sugerida

```text
Resolução: 1920x1200
FPS: 60
Bitrate: 8 a 20 Mbps
GOP: 30 ou 60 frames
B-frames: 0
Modo: baixa latência
Controle: CBR ou VBR restrito
```

### Perfis

| Perfil | Resolução | FPS | Bitrate |
|---|---:|---:|---:|
| Econômico | 1280x800 | 30 | 4 Mbps |
| Equilibrado | 1920x1200 | 60 | 10 Mbps |
| Qualidade | 2560x1600 | 60 | 20 Mbps |

## 8. Modos de toque

### Toque absoluto

O ponto tocado no tablet corresponde à mesma posição no monitor virtual.

### Touchpad

O tablet funciona como um touchpad relativo.

### Gestos iniciais

- toque: clique esquerdo;
- toque longo: clique direito;
- dois dedos: rolagem;
- arrastar: mover ou arrastar;
- três dedos: abrir barra de ferramentas;
- botão opcional para teclado virtual.

### Futuro

Adicionar caneta com posição, pressão, inclinação e botões.

## 9. Estrutura do repositório

```text
AuxScreen/
├── README.md
├── LICENSE
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── SECURITY.md
├── ROADMAP.md
├── CHANGELOG.md
├── docs/
│   ├── PLANO_DESENVOLVIMENTO.md
│   ├── ARCHITECTURE.md
│   ├── PROTOCOL.md
│   ├── BUILDING_LINUX.md
│   ├── BUILDING_ANDROID.md
│   └── TROUBLESHOOTING.md
├── protocol/
│   ├── schema/
│   └── test-vectors/
├── linux/
│   ├── host/
│   ├── portal/
│   ├── capture/
│   ├── encoder/
│   ├── transport/
│   └── input/
├── android/
│   ├── app/
│   ├── decoder/
│   ├── transport/
│   └── input/
├── tools/
│   ├── latency-test/
│   └── network-test/
└── .github/
    ├── workflows/
    └── ISSUE_TEMPLATE/
```

### Módulos do host

```text
portal-manager
capture-session
video-encoder
webrtc-session
input-controller
device-discovery
pairing-manager
metrics
```

Cada módulo deverá ser substituível e testável isoladamente.

## 10. Roadmap por sprints

### Sprint 0: prova técnica

Objetivo:

- comprovar que o KDE cria um monitor virtual;
- acessar o stream por PipeWire;
- exibir os frames localmente.

Entregáveis:

- CLI inicial;
- sessão via portal;
- monitor virtual visível nas configurações do KDE;
- preview local;
- documentação dos pacotes necessários.

Critério de sucesso:

> Uma janela pode ser arrastada para uma tela virtual e essa tela pode ser capturada pelo programa.

O Android não deve ser iniciado antes dessa prova.

### Sprint 1: transmissão bruta

Objetivo:

- codificar o monitor virtual;
- transmitir pela rede;
- reproduzir no tablet.

Entregáveis:

- PipeWire para H.264;
- WebRTC Linux para Android;
- decoder Android;
- tela cheia;
- 30 FPS estáveis.

Critérios:

- conexão manual por IP;
- 30 minutos sem interrupção;
- sem crescimento contínuo de memória;
- atraso inferior a 150 ms.

### Sprint 2: baixa latência

Objetivo:

- reduzir buffering;
- usar encoder e decoder por hardware;
- atingir 60 FPS.

Entregáveis:

- detecção de NVENC, VA-API e software;
- MediaCodec em modo de baixa latência;
- B-frames desativados;
- solicitação de keyframe;
- métricas de encode, rede e decode.

Metas:

```text
Latência visual mediana: abaixo de 60 ms
Latência visual p95: abaixo de 100 ms
```

### Sprint 3: entrada remota

Objetivo:

- controlar o monitor virtual pelo tablet.

Entregáveis:

- ponteiro absoluto;
- clique;
- arraste;
- rolagem;
- teclado;
- mapeamento de rotação e escala;
- RemoteDesktop portal;
- fallback opcional por uinput.

### Sprint 4: descoberta e pareamento

Entregáveis:

- descoberta mDNS;
- lista de computadores;
- código de pareamento;
- QR Code;
- identidade persistente;
- autorização revogável;
- reconexão automática;
- nenhuma comunicação com nuvem.

### Sprint 5: experiência de uso

Entregáveis:

- interface do host;
- seletor de resolução;
- seletor de FPS;
- perfil de qualidade;
- modo touchpad;
- modo touchscreen;
- orientação automática;
- escala da interface;
- métricas visíveis;
- erros compreensíveis.

### Sprint 6: empacotamento

Linux:

- pacote Arch Linux;
- AppImage;
- Flatpak;
- pacote Debian posteriormente;
- serviço systemd de usuário.

Android:

- APK assinado;
- GitHub Releases;
- F-Droid;
- Play Store opcional.

### Sprint 7: compatibilidade GNOME

Testar:

- Fedora Workstation;
- Ubuntu;
- GNOME Wayland;
- diferentes versões do Mutter;
- criação e redimensionamento da saída virtual.

KDE e GNOME devem ser tratados como backends testados separadamente.

## 11. Testes

### Unitários

- serialização do protocolo;
- conversão de coordenadas;
- rotação;
- escala;
- autenticação;
- bitrate;
- reconexão.

### Integração

- criação e encerramento da sessão portal;
- perda de Wi-Fi;
- mudança de resolução;
- suspensão do tablet;
- bloqueio da tela;
- suspensão do host;
- mudança de orientação;
- encoder indisponível;
- decoder incompatível.

### Latência

Criar uma ferramenta dedicada:

```text
host gera mudança visual com timestamp
cliente apresenta o frame
cliente devolve confirmação
host calcula o tempo total
```

Para medição vidro a vidro, usar uma câmera de alta velocidade filmando monitor físico e tablet com contador ou flash sincronizado.

### Matriz mínima

| Host | Ambiente | GPU |
|---|---|---|
| Arch | KDE Plasma Wayland | NVIDIA |
| Fedora KDE | KDE Plasma Wayland | AMD |
| Fedora Workstation | GNOME Wayland | Intel |
| Ubuntu | GNOME Wayland | Intel ou AMD |

Android:

- Android 11;
- Android 13;
- Android 15;
- Android 16;
- tablet Samsung;
- tablet genérico;
- celular Motorola.

## 12. Segurança

Requisitos:

- todo tráfego criptografado;
- pareamento obrigatório;
- nenhuma porta exposta à internet por padrão;
- rejeição de dispositivos não autorizados;
- chave diferente por dispositivo;
- indicador visível de sessão ativa;
- revogação de dispositivos;
- descoberta limitada à rede local;
- nenhum registro de teclas;
- nenhum armazenamento do conteúdo da tela;
- tokens temporários por sessão;
- proteção contra repetição de mensagens.

## 13. Licença e governança

### Licença recomendada

- GPL-3.0-or-later para garantir que versões distribuídas continuem abertas;
- MPL-2.0 caso seja desejável permitir integração mais simples com produtos proprietários.

A decisão deve ser tomada antes da primeira contribuição externa relevante.

### Arquivos de governança

```text
LICENSE
CONTRIBUTING.md
CODE_OF_CONDUCT.md
SECURITY.md
ROADMAP.md
CHANGELOG.md
```

### Processo de contribuição

- issues pequenas e objetivas;
- Conventional Commits;
- pull requests revisadas;
- formatter obrigatório;
- testes obrigatórios;
- código específico de distribuição fora do núcleo;
- recursos experimentais protegidos por feature flags.

## 14. Metas de desempenho

### MVP aceitável

- 1920x1200;
- 60 FPS;
- H.264;
- latência mediana abaixo de 60 ms;
- latência p95 abaixo de 100 ms;
- menos de 1% de frames perdidos em Wi-Fi 5 ou superior;
- menos de 20% de CPU com encoder por hardware;
- reconexão em menos de 5 segundos;
- funcionamento contínuo por 4 horas;
- nenhum acesso root no caminho principal.

### Versão 1.0

- KDE e GNOME;
- aceleração NVIDIA, AMD e Intel;
- 2560x1600 a 60 FPS;
- toque e teclado;
- descoberta automática;
- Flatpak;
- F-Droid;
- protocolo versionado;
- documentação completa;
- telemetria desativada ou inexistente por padrão.

## 15. Roadmap posterior

### Versão 1.1

- clipboard;
- áudio;
- caneta e pressão;
- perfis de qualidade;
- modo somente touchpad;
- cursor local para reduzir latência percebida.

### Versão 1.2

- conexão USB;
- HEVC;
- AV1;
- múltiplos tablets;
- monitor virtual persistente;
- resolução dinâmica.

### Versão 2.0

- acesso remoto pela internet;
- relay opcional auto-hospedável;
- autenticação por chave pública;
- suporte experimental a Windows como host;
- transporte independente de WebRTC.

## 16. Primeiras issues

```text
#1 Inicializar workspace Rust
#2 Implementar cliente D-Bus para ScreenCast portal
#3 Solicitar fonte VIRTUAL
#4 Obter node ID do PipeWire
#5 Exibir stream PipeWire localmente
#6 Criar pipeline H.264 por software
#7 Criar projeto Android vazio
#8 Decodificar arquivo H.264 com MediaCodec
#9 Implementar sinalização WebRTC local
#10 Transmitir primeiro frame Linux para Android
#11 Adicionar métricas de FPS e latência
#12 Documentar ambiente Arch/KDE
```

## 17. Primeiro marco público

Publicar `v0.1.0` apenas quando existirem:

- monitor virtual funcional;
- vídeo Linux para Android;
- 30 FPS estáveis;
- instalação reproduzível;
- demonstração gravada;
- teste em ao menos uma segunda máquina.

Antes disso, usar versões `v0.0.x` ou manter o projeto explicitamente como protótipo.

## 18. Decisão inicial

O primeiro trabalho técnico será o Sprint 0 em Arch Linux, KDE Plasma 6 e Wayland. A prioridade absoluta é provar que uma saída virtual pode ser criada, capturada via PipeWire e exibida localmente.

Sem essa prova, qualquer aplicativo Android seria apenas um reprodutor de vídeo com ambições de monitor.