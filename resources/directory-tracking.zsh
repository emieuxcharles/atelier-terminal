# Local notifications only. The history, ranking and persistence live in Rust.
[[ -o interactive && -t 1 && -n $ATELIER_SSH_TOKEN ]] || return 0
# Nested shells and remote sessions must not overwrite the initial shell's cwd.
[[ -z $ATELIER_CWD_OWNER || $ATELIER_CWD_OWNER == $$ ]] || return 0
export ATELIER_CWD_OWNER=$$

function _atelier_report_cwd() {
  emulate -L zsh
  [[ -t 1 && $ZSH_SUBSHELL == 0 && $ATELIER_CWD_OWNER == $$ ]] || return 0
  local LC_ALL=C
  local physical=${PWD:A}
  [[ $_atelier_previous_cwd == "$physical" ]] && return 0
  typeset -g _atelier_previous_cwd=$physical
  local encoded='' byte i
  for (( i=1; i <= ${#physical}; i++ )); do
    builtin printf -v byte '%02x' "'${physical[i]}"
    encoded+=$byte
  done
  builtin printf '\033]2;ATELIER_CWD:%s:%s\007' "$ATELIER_SSH_TOKEN" "$encoded"
}

typeset -ga chpwd_functions precmd_functions
(( ${chpwd_functions[(Ie)_atelier_report_cwd]} )) || chpwd_functions+=(_atelier_report_cwd)
(( ${precmd_functions[(Ie)_atelier_report_cwd]} )) || precmd_functions+=(_atelier_report_cwd)
_atelier_report_cwd
