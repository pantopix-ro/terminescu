# Terminescu shell integration for bash (D9). The daemon starts `bash --rcfile` on this file, so
# it reads the startup files a login shell would, then hooks in.
if [ -r /etc/profile ]; then
  . /etc/profile
fi
for _terminescu_rc in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
  if [ -r "$_terminescu_rc" ]; then
    . "$_terminescu_rc"
    break
  fi
done
unset _terminescu_rc
if [ -n "$TERMINESCU_SHELL_DIR" ]; then
  . "$TERMINESCU_SHELL_DIR/terminescu.bash"
fi
