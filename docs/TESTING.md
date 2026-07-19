# Gates de estabilidade da v0.1

Os testes automatizados evitam regressões do protocolo e das máquinas de estado;
eles não fingem substituir KDE, PipeWire, Wi-Fi e um decoder Android real.

## Automatizados

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python scripts/validate_schema.py
JAVA_HOME=/usr/lib/jvm/java-17-openjdk \
  ./android/gradlew -p android \
  :app:testDebugUnitTest :app:lintDebug :app:assembleDebug :app:assemblePersonal
```

## Gate 1 — diagnóstico

`auxscreen-host doctor` deve confirmar portal ScreenCast com `VIRTUAL`,
PipeWire, `pipewiresrc`, `webrtcbin`, `x264enc`, endereço LAN e portas livres.

## Gate 2 — portal e prévia

Execute `preview --source virtual`, marque somente o monitor virtual no diálogo
do KDE, confirme pelo botão do diálogo e mantenha a prévia por dez minutos. Mova
uma janela para o monitor virtual. Ao encerrar, cancelar ou enviar `SIGTERM`,
confirme que o processo termina e que monitor e sessão PipeWire desaparecem.

## Gate 3 — transporte isolado

Execute `serve --source test`, instale o APK `personal`, conecte com o token e
mantenha o teste por dez minutos. O log deve mostrar H.264 por hardware, 28–30
FPS, heartbeat e `client_stats` sem crescimento contínuo de memória. Use
`packets_received`/`packets_lost` para perda de rede; `frames_dropped` mede o
decoder/renderizador e não deve ser vendido como a mesma coisa.

## Gate 4 — corte real

Troque para `serve --source virtual` por 30 minutos. Critérios: média mínima de
28 FPS, perda inferior a 1%, RTT/jitter registrados, decoder H.264 de hardware e
RSS/PSS estabilizados depois do aquecimento. Interrompa a rede cinco vezes e
execute também token inválido, protocolo incorreto, segundo cliente, bloqueio do
tablet, rotação e encerramento abrupto do host.

Resultados físicos devem ser registrados em `docs/test-results/` com versões,
comandos, duração, métricas e falhas observadas. Um gate não executado é
`PENDENTE`, nunca `PASSOU POR TELEPATIA`.
