#!/bin/zsh -f
# Only Atelier prepends this directory to its own shells' PATH.
# Delegate unchanged arguments and exit status to the original OpenSSH executable.
if [[ -z "$ATELIER_SSH_TOKEN" || -z "$ATELIER_REAL_SSH" ]]; then
  exec /usr/bin/ssh "$@"
fi
local_target=""
skip=0
for arg in "$@"; do
  if (( skip )); then skip=0; continue; fi
  case "$arg" in
    -G|-V|-Q|-O) exec "$ATELIER_REAL_SSH" "$@" ;;
    --) continue ;;
    -B|-b|-c|-D|-E|-e|-F|-I|-i|-J|-L|-l|-m|-o|-P|-p|-R|-S|-W|-w) skip=1 ;;
    -*) continue ;;
    *) local_target="$arg"; break ;;
  esac
done
# Piped output must not receive terminal escape sequences.
if [[ ! -t 1 || -z "$local_target" ]]; then
  exec "$ATELIER_REAL_SSH" "$@"
fi
function TRAPEXIT() {
  printf '\033]2;ATELIER_SSH:%s:local\007' "$ATELIER_SSH_TOKEN"
}
printf '\033]2;ATELIER_SSH:%s:ssh:%s\007' "$ATELIER_SSH_TOKEN" "$local_target"
"$ATELIER_REAL_SSH" "$@"
result=$?
exit $result
