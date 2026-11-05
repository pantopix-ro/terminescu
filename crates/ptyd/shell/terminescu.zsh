# Terminescu shell integration for zsh (D9, D35). Reports prompts, exit codes, the cwd and the
# command line, and loads the side tab's variables. It hooks in at the first prompt, after every
# rc file, so its PATH entry and editor come last and win.

_terminescu_vars() {
  [[ -n "$TERMINESCU_CLI" ]] || return
  local out
  out=$("$TERMINESCU_CLI" var env 2>/dev/null) && eval "$out"
}

_terminescu_precmd() {
  local code=$?
  if [[ -n "$_terminescu_running" ]]; then
    printf '\033]133;D;%s;src=terminescu\007' "$code"
    _terminescu_running=
  fi
  printf '\033]7;file://%s%s\007' "$HOST" "${PWD//\%/%25}"
  _terminescu_vars
  printf '\033]133;A;src=terminescu\007'
}

_terminescu_preexec() {
  local cmd=$1
  cmd=${cmd//\\/\\\\}
  cmd=${cmd//;/\\x3b}
  cmd=${cmd//$'\n'/\\x0a}
  cmd=${cmd//$'\r'/\\x0d}
  cmd=${cmd//$'\a'/\\x07}
  cmd=${cmd//$'\e'/\\x1b}
  printf '\033]633;E;%s\007\033]133;C;src=terminescu\007' "$cmd"
  _terminescu_running=1
  # A variable set while this prompt waited reaches the command (D35).
  _terminescu_vars
}

_terminescu_init() {
  precmd_functions=(${precmd_functions:#_terminescu_init})
  if [[ -n "$TERMINESCU_CLI" ]]; then
    local bin=${TERMINESCU_CLI:h}
    path=("$bin" ${path:#$bin})
    if [[ "$TERMINESCU_EDITOR" == 1 ]]; then
      export VISUAL="${(q)TERMINESCU_CLI} edit --wait"
      export EDITOR=$VISUAL
    fi
  fi
  # First in line, so $? is still the command's.
  precmd_functions=(_terminescu_precmd $precmd_functions)
  preexec_functions=(_terminescu_preexec $preexec_functions)
  _terminescu_precmd
}

typeset -ga precmd_functions preexec_functions
precmd_functions+=(_terminescu_init)
