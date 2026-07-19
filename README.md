# AuxScreen

AuxScreen é um projeto open source para transformar dispositivos Android em monitores adicionais para computadores Linux.

O objetivo é oferecer uma segunda tela real, com área de trabalho estendida, baixa latência e suporte a toque, usando tecnologias nativas do ecossistema Linux e Android.

## v0.1.0-rc.1 — corte vertical estabilizado

- Host Linux com Wayland
- Compatibilidade inicial com KDE Plasma 6
- Cliente Android 11 ou superior
- Monitor virtual via XDG Desktop Portal
- Captura via PipeWire
- Transmissão H.264 por WebRTC
- Sinalização WebSocket autenticada por token temporário
- Cliente Android Kotlin/Compose em tela cheia
- Reconexão limitada, heartbeat e métricas WebRTC
- Variante pessoal restrita a IPv4 privado da LAN
- Sem dependência de serviços em nuvem

O primeiro marco entrega vídeo a 30 FPS. Toque, teclado, áudio, descoberta e
pareamento persistente ficam deliberadamente fora desta versão.

## Estrutura

- `linux/host`: binário Rust `auxscreen-host` (`doctor`, `preview`, `serve`)
- `android/app`: cliente Android nativo, minSdk 30 e targetSdk 36
- `protocol/schema`: contrato JSON versionado da sinalização
- `third_party/zbus-5.17.0`: dependência vendorizada com patch mínimo documentado

## Início rápido

```bash
cargo run --locked --bin auxscreen-host -- doctor
cargo run --locked --bin auxscreen-host -- serve \
  --source test \
  --listen 192.168.1.254:9898 \
  --ice-ip 192.168.1.254
```

As instruções completas de dependências, build, firewall e deploy estão em
[docs/BUILD_AND_DEPLOY.md](docs/BUILD_AND_DEPLOY.md).
Os gates reproduzíveis e critérios de aceite estão em
[docs/TESTING.md](docs/TESTING.md).

## Planejamento

O plano técnico, roadmap, arquitetura, riscos e critérios de sucesso estão documentados em [docs/PLANO_DESENVOLVIMENTO.md](docs/PLANO_DESENVOLVIMENTO.md).

## Segurança

DTLS-SRTP protege o vídeo WebRTC. O WebSocket sem TLS existe
somente nas variantes debug/pessoal e o cliente pessoal aceita apenas IPv4
privado. Esta versão não é uma release segura para distribuição pública.

## Licença

GPL-3.0-or-later. Consulte [LICENSE](LICENSE) e
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
