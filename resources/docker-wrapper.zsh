#!/bin/zsh -f
# Private PATH relay. Never inspect terminal output or poll Docker.
relay_dir=${0:A:h}
# Compare physical paths: macOS exposes /private/var through /var, and
# user PATH entries may contain additional symlinks or duplicate spellings.
filtered_path=()
for entry in "${path[@]}"; do
  [[ "${entry:A}" == "$relay_dir" ]] || filtered_path+=("$entry")
done
path=("${filtered_path[@]}")
real_docker=$(whence -p docker)
if [[ -z "$real_docker" ]]; then
  print -u2 'Atelier : exécutable Docker introuvable dans PATH.'
  exit 127
fi
# Preserve ordinary CLI behavior, redirection and background jobs.
if [[ -z "$ATELIER_SSH_TOKEN" || ! -t 0 || ! -t 1 ]]; then
  exec "$real_docker" "$@"
fi
args=("$@")
i=1
config_args=()
context=${DOCKER_CONTEXT:-}
endpoint=${DOCKER_HOST:-}
while (( i <= $#args )); do
  arg=$args[i]
  case "$arg" in
    --context|-c) (( i++ )); context=$args[i] ;;
    --context=*) context=${arg#*=} ;;
    --host|-H) (( i++ )); endpoint=$args[i]; context='' ;;
    --host=*) endpoint=${arg#*=}; context='' ;;
    --config) (( i++ )); config_args+=(--config "$args[i]") ;;
    --config=*) config_args+=("$arg") ;;
    --debug|-D|--tls|--tlsverify) ;;
    --tlscacert|--tlscert|--tlskey) (( i++ )) ;;
    --tlscacert=*|--tlscert=*|--tlskey=*) ;;
    exec) (( i++ )); break ;;
    *) exec "$real_docker" "$@" ;;
  esac
  (( i++ ))
done
interactive=0
container=''
while (( i <= $#args )); do
  arg=$args[i]
  case "$arg" in
    --) (( i++ )); container=$args[i]; break ;;
    --detach|--detach=true|-d|--help) exec "$real_docker" "$@" ;;
    --interactive|--tty|--interactive=true|--tty=true) interactive=1 ;;
    --env|--env-file|--user|--workdir|-e|-u|-w|--detach-keys) (( i++ )) ;;
    --env=*|--env-file=*|--user=*|--workdir=*|--detach-keys=*|--privileged|--privileged=*|--detach=false) ;;
    --*) exec "$real_docker" "$@" ;;
    -*)
      # Combined -it/-ti, including options with attached values. Unknown
      # syntax passes through untouched without making a badge claim.
      flags=${arg#-}
      while [[ -n "$flags" ]]; do
        flag=$flags[1]; flags=${flags[2,-1]}
        case "$flag" in
          i|t) interactive=1 ;;
          d) exec "$real_docker" "$@" ;;
          e|u|w) [[ -z "$flags" ]] && (( i++ )); flags='' ;;
          *) exec "$real_docker" "$@" ;;
        esac
      done ;;
    *) container=$arg; break ;;
  esac
  (( i++ ))
done
if [[ -z "$container" || $interactive != 1 ]]; then
  exec "$real_docker" "$@"
fi
# Context inspect reads local configuration only; no daemon connection.
# CLI --config must also apply to this metadata lookup.
if [[ -n "$context" || -z "$endpoint" ]]; then
  context_args=()
  [[ -n "$context" ]] && context_args+=("$context")
  endpoint=$("$real_docker" "${config_args[@]}" context inspect "${context_args[@]}" --format '{{.Endpoints.docker.Host}}' 2>/dev/null)
fi
# Keep the OSC payload single-line and delimiter-safe, even for invalid inputs.
endpoint=${endpoint//[[:cntrl:]|]/}
container=${container//[[:cntrl:]|]/}
endpoint=${endpoint[1,500]}
container=${container[1,200]}
# Keep the relay alive long enough to reset the badge after Ctrl-C.
trap ':' INT
trap 'exit 143' TERM
trap 'exit 129' HUP
function TRAPEXIT() {
  printf '\033]2;ATELIER_DOCKER:%s:local\007' "$ATELIER_SSH_TOKEN"
}
printf '\033]2;ATELIER_DOCKER:%s:exec:%s|%s\007' "$ATELIER_SSH_TOKEN" "$endpoint" "$container"
# Docker may itself call SSH for a remote context: its transport must not
# overwrite the independent machine/container badges.
ATELIER_SSH_TOKEN='' "$real_docker" "$@"
result=$?
exit $result
