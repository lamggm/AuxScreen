# Build, teste e deploy da v0.1.0-rc.1

## Dependências do host Arch Linux

```bash
sudo pacman -S --needed gst-plugin-pipewire gst-plugins-ugly android-tools android-udev jdk17-openjdk
```

O SDK Android deve conter `platforms;android-36`, `build-tools;36.0.0` e
`platform-tools`. Neste host ele foi preparado em `/home/lamggm/Android/Sdk`;
configure `ANDROID_HOME` para o diretório do SDK e use
`JAVA_HOME=/usr/lib/jvm/java-17-openjdk`.

## Host Linux

```bash
cargo test --workspace --locked
cargo run --locked --bin auxscreen-host -- doctor --listen 192.168.1.254:9898
cargo run --locked --bin auxscreen-host -- preview --source virtual
cargo run --locked --bin auxscreen-host -- serve \
  --source virtual \
  --listen 192.168.1.254:9898 \
  --ice-ip 192.168.1.254 \
  --ice-ports 9900-9910 \
  --encode-max-size 1920x1200 \
  --fps 30 \
  --bitrate-kbps 6000
```

Para isolar captura/portal da sinalização e do decoder, troque `--source
virtual` por `--source test`. Se PipeWire falhar ao negociar DMA-BUF, o host
reconstrói o pipeline uma vez com a ponte OpenGL; `--use-gl-fallback` força esse
caminho desde o início.

O portal do KDE pode solicitar confirmação. O monitor virtual pertence à sessão
do portal e deve desaparecer quando `auxscreen-host` termina.

## APK pessoal

```bash
JAVA_HOME=/usr/lib/jvm/java-17-openjdk \
  ./android/gradlew -p android \
  :app:testDebugUnitTest :app:lintDebug :app:assembleDebug :app:assemblePersonal

adb -s RX2Y800FTYY install -r -t \
  android/app/build/outputs/apk/personal/app-personal.apk
adb -s RX2Y800FTYY shell am start \
  -n io.github.lamggm.auxscreen.personal/io.github.lamggm.auxscreen.MainActivity
```

Informe no aplicativo o endpoint `ws://192.168.1.254:9898/v1/session` e o token
aleatório impresso pelo host. As variantes debug e `personal` permitem WebSocket
sem TLS, mas `personal` rejeita destinos fora de IPv4 privado/loopback. A
variante pública `release` continua exigindo WSS.

`personal` é não-debuggable, porém permanece sem R8: a combinação atual do
libwebrtc com Android 16 no SM-X400 aborta em `JNI_OnLoad` quando otimizada. A
variante pública continua minificada e deverá ser requalificada no dispositivo
antes de qualquer distribuição.

Por padrão o RC local usa a chave debug. Para uma assinatura pessoal persistente,
defina fora do repositório `AUXSCREEN_KEYSTORE`, `AUXSCREEN_STORE_PASSWORD`,
`AUXSCREEN_KEY_ALIAS` e `AUXSCREEN_KEY_PASSWORD`; o Gradle usa as quatro somente
quando todas estiverem presentes. Keystore e senhas jamais entram no Git.

Para um teste automatizado da variante pessoal:

```bash
adb -s RX2Y800FTYY shell am start -S \
  -n io.github.lamggm.auxscreen.personal/io.github.lamggm.auxscreen.MainActivity \
  --es endpoint ws://192.168.1.254:9898/v1/session \
  --es token TOKEN_TEMPORARIO
```

## Firewall

O Gate 3 deve ser tentado antes de alterar firewall. Se necessário, libere
somente TCP 9898 e UDP 9900–9910 na interface Wi-Fi usada pelo host. Não abra
essas portas globalmente em Docker, Tailscale ou interfaces públicas.

## Troubleshooting do portal KDE

- `portal approval timed out after 60 seconds`: desbloqueie a sessão gráfica e
  responda ao diálogo; o host não clica por você, porque automação cega de
  consentimento é apenas malware com documentação melhor.
- `portal returned no PipeWire stream`: encerre o host, confirme que
  `xdg-desktop-portal-kde` está ativo e tente uma nova sessão.
- `instead of a virtual monitor`: o compositor retornou outra fonte; cancele e
  selecione explicitamente a tela virtual.
- `not-negotiated`: o host tenta uma reconstrução com ponte OpenGL. Se ainda
  falhar, repita com `--use-gl-fallback` e preserve o log sanitizado.

## Rollback

1. Encerre `auxscreen-host`; isso fecha a sessão do portal e remove o monitor.
2. No tablet, desconecte no app ou execute
   `adb -s RX2Y800FTYY uninstall io.github.lamggm.auxscreen.personal`.
3. Para voltar a um APK pessoal anterior, instale-o com `adb install -r -t`.
   O único dado persistido é o endpoint; tokens nunca são gravados.
4. Nenhuma regra de firewall é criada pelo projeto. Se você abriu portas
   manualmente, reverta exatamente essa regra e somente essa interface.

## Limites deliberados

Esta versão não implementa toque, teclado, áudio, mDNS, pareamento persistente,
NVENC ou TLS. A reconexão tenta no máximo cinco vezes. Não apresente a variante
`personal` como release pública segura.
