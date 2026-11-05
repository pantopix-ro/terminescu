# Terminescu shell integration for bash 3.2 and later (D9, D35). Reports prompts, exit codes,
# the cwd and the command line, and loads the side tab's variables. Loaded after the rc files.

_terminescu_vars() {
  [ -n "$TERMINESCU_CLI" ] || return
  local out
  out=$("$TERMINESCU_CLI" var env 2>/dev/null) && eval "$out"
}

# Runs last in PROMPT_COMMAND; the first entry saved the command's exit code.
_terminescu_precmd() {
  if [ -n "$_terminescu_running" ]; then
    printf '\033]133;D;%s;src=terminescu\007' "$_terminescu_status"
    _terminescu_running=
  fi
  printf '\033]7;file://%s%s\007' "$HOSTNAME" "${PWD//%/%25}"
  _terminescu_vars
  printf '\033]133;A;src=terminescu\007'
  _terminescu_hist=$(builtin history 1)
  _terminescu_at_prompt=1
}

# The DEBUG trap fires before every simple command; the first one after a prompt is the line
# the person typed.
_terminescu_preexec() {
  [ -n "$_terminescu_at_prompt" ] || return
  [ -z "$COMP_LINE" ] || return
  _terminescu_at_prompt=
  # An empty line runs no command; the trap then fires for PROMPT_COMMAND itself.
  case "$BASH_COMMAND" in
    _terminescu_status=*) return ;;
  esac
  local cmd
  cmd=$(builtin history 1)
  if [ "$cmd" = "$_terminescu_hist" ]; then
    # Not added to history (a leading space under ignorespace): the first command will do.
    cmd=$BASH_COMMAND
  else
    cmd=${cmd#"${cmd%%[![:space:]]*}"}
    cmd=${cmd#*[[:space:]]}
    cmd=${cmd#"${cmd%%[![:space:]]*}"}
  fi
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

if [ -n "$TERMINESCU_CLI" ]; then
  _terminescu_bin=${TERMINESCU_CLI%/*}
  case ":$PATH:" in
    *":$_terminescu_bin:"*) ;;
    *) PATH="$_terminescu_bin:$PATH" ;;
  esac
  unset _terminescu_bin
  if [ "$TERMINESCU_EDITOR" = 1 ]; then
    export VISUAL="$(printf '%q' "$TERMINESCU_CLI") edit --wait"
    export EDITOR=$VISUAL
  fi
fi

# A DEBUG trap the person set stays theirs; the pane then reports no command lines.
if [ -z "$(trap -p DEBUG)" ]; then
  trap '_terminescu_preexec' DEBUG
fi
PROMPT_COMMAND="_terminescu_status=\$?${PROMPT_COMMAND:+; $PROMPT_COMMAND}; _terminescu_precmd"
