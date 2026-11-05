# Terminescu shell integration for zsh (D9). The daemon points ZDOTDIR here; this restores the
# person's ZDOTDIR, so zsh goes on to read their .zprofile, .zshrc and .zlogin as usual.
if [[ -n "${TERMINESCU_ZDOTDIR+x}" ]]; then
  ZDOTDIR=$TERMINESCU_ZDOTDIR
else
  unset ZDOTDIR
fi
unset TERMINESCU_ZDOTDIR
if [[ -f "${ZDOTDIR:-$HOME}/.zshenv" ]]; then
  source "${ZDOTDIR:-$HOME}/.zshenv"
fi
if [[ -o interactive && -n "$TERMINESCU_SHELL_DIR" ]]; then
  source "$TERMINESCU_SHELL_DIR/terminescu.zsh"
fi
