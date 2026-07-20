#!/usr/bin/env bash
set -euo pipefail

serial="${1:-RX2Y800FTYY}"
apk="${2:-android/app/build/outputs/apk/personal/app-personal.apk}"
endpoint="${AUXSCREEN_ENDPOINT:-}"
token="${AUXSCREEN_TOKEN:-}"

adb -s "$serial" get-state | grep -qx device
adb -s "$serial" install -r -t "$apk"
adb -s "$serial" logcat -c

args=(-n io.github.lamggm.auxscreen.personal/io.github.lamggm.auxscreen.MainActivity)
if [[ -n "$endpoint" && -n "$token" ]]; then
  args+=(--es endpoint "$endpoint" --es token "$token")
fi
adb -s "$serial" shell am start -S "${args[@]}"
sleep 5

adb -s "$serial" shell dumpsys package io.github.lamggm.auxscreen.personal \
  | grep -E 'versionName|versionCode' | head -2
if adb -s "$serial" logcat -d -t 500 | grep -E 'FATAL EXCEPTION|AndroidRuntime.*Process: io.github.lamggm.auxscreen'; then
  echo "AuxScreen crashed during device smoke test" >&2
  exit 1
fi
echo "device smoke test passed: $serial"
