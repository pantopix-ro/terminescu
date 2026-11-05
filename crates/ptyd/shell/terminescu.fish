# Terminescu shell integration for fish (D9, D35). The daemon puts its folder first on
# XDG_DATA_DIRS, so fish loads this from vendor_conf.d; it restores XDG_DATA_DIRS first.
if set -q TERMINESCU_XDG_DATA_DIRS
    if test -n "$TERMINESCU_XDG_DATA_DIRS"
        set -gx XDG_DATA_DIRS $TERMINESCU_XDG_DATA_DIRS
    else
        set -e XDG_DATA_DIRS
    end
    set -e TERMINESCU_XDG_DATA_DIRS
end

if status is-interactive
    function __terminescu_vars
        test -n "$TERMINESCU_CLI"; or return
        "$TERMINESCU_CLI" var env --fish 2>/dev/null | source
    end

    # Hooks in at the first prompt, after config.fish, so its PATH entry and editor win.
    function __terminescu_init --on-event fish_prompt
        functions -e __terminescu_init
        if test -n "$TERMINESCU_CLI"
            set -l bin (string replace -r '/[^/]*$' '' -- $TERMINESCU_CLI)
            set -l rest
            for p in $PATH
                test "$p" = "$bin"; or set -a rest $p
            end
            set -gx PATH $bin $rest
            if test "$TERMINESCU_EDITOR" = 1
                set -gx VISUAL (string escape -- $TERMINESCU_CLI)" edit --wait"
                set -gx EDITOR $VISUAL
            end
        end

        function __terminescu_prompt --on-event fish_prompt
            if set -q __terminescu_running
                printf '\e]133;D;%s;src=terminescu\a' $__terminescu_status
                set -e __terminescu_running
            end
            printf '\e]7;file://%s%s\a' (prompt_hostname) (string replace -a % %25 -- $PWD)
            __terminescu_vars
            printf '\e]133;A;src=terminescu\a'
        end

        function __terminescu_preexec --on-event fish_preexec
            set -l cmd (string split \n -- $argv[1])
            set cmd (string replace -a -- '\\' '\\\\' $cmd)
            set cmd (string replace -a -- ';' '\\x3b' $cmd)
            set cmd (string replace -a -- \r '\\x0d' $cmd)
            set cmd (string replace -a -- \e '\\x1b' $cmd)
            set cmd (string replace -a -- \a '\\x07' $cmd)
            set cmd (string join -- '\x0a' $cmd)
            printf '\e]633;E;%s\a\e]133;C;src=terminescu\a' "$cmd"
            set -g __terminescu_running 1
            # A variable set while this prompt waited reaches the command (D35).
            __terminescu_vars
        end

        function __terminescu_postexec --on-event fish_postexec
            set -g __terminescu_status $status
        end

        __terminescu_prompt
    end
end
