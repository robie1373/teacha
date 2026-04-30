/// Seed the database with starter cards if it is empty.
/// Called from both the daemon and the Tauri GUI on startup.
use crate::db::Database;

pub fn seed_if_empty(db: &Database) {
    if !db.get_all_cards().unwrap_or_default().is_empty() {
        return;
    }

    let cards: &[(&str, Option<&str>, &str, &str)] = &[
        // ── Nix / CLI ──────────────────────────────────────────────────────
        (
            "comma — run any binary without installing",
            Some(", ffmpeg -i input.mp4 output.webm"),
            "Uses nix-index-database to locate the package automatically. \
             Faster than `nix-shell -p pkg --run cmd` for one-off commands.",
            "nix,cli",
        ),
        (
            "nix shell — temporary environment with a package",
            Some("nix shell nixpkgs#ripgrep"),
            "Drops into a shell with the package on PATH. \
             Exit the shell to discard it. Combine packages: `nix shell nixpkgs#a nixpkgs#b`.",
            "nix,cli",
        ),
        (
            "nix run — run a package without installing",
            Some("nix run nixpkgs#cowsay -- hello"),
            "Runs the default app from a flake without entering a shell. \
             Arguments after `--` are passed to the program.",
            "nix,cli",
        ),
        (
            "nix flake show — inspect flake outputs",
            Some("nix flake show"),
            "Lists all outputs (packages, apps, devShells, nixosModules) \
             of the flake in the current directory.",
            "nix,cli",
        ),
        (
            "nix store gc — garbage collect unreachable store paths",
            Some("nix store gc"),
            "Deletes store paths not reachable from any GC root \
             (profiles, /run/current-system, etc). Run after removing generations.",
            "nix,cli",
        ),
        (
            "nix-locate — find which package provides a file",
            Some("nix-locate bin/htop"),
            "Searches the nix-index database. Run `nix-index` first to build it. \
             Works without installing the package.",
            "nix,cli",
        ),
        (
            "nh os switch — rebuild and switch NixOS config",
            Some("nh os switch ~/nixos-config"),
            "Wrapper around `nixos-rebuild switch`. Shows a diff of what changed \
             and asks for confirmation. Cleaner output than raw nixos-rebuild.",
            "nix,cli",
        ),
        (
            "nh os test — test config without making it the boot default",
            Some("nh os test ~/nixos-config"),
            "Activates the new config in the running system but does not \
             update the bootloader. Safe for trying risky changes.",
            "nix,cli",
        ),

        // ── Vim / Neovim ───────────────────────────────────────────────────
        (
            "G / gg — jump to end or start of file in vim",
            Some("G"),
            "G jumps to the last line. gg jumps to the first. \
             Prefix with a number to jump to that line: `42G`.",
            "vim",
        ),
        (
            "ci\" — change inside quotes in vim",
            Some("ci\""),
            "Deletes content inside the nearest double quotes and enters insert mode. \
             Works with any delimiter: ci' ci( ci[ ci{ ci<",
            "vim",
        ),
        (
            "* — search for word under cursor in vim",
            Some("*"),
            "Jumps to the next occurrence of the word under the cursor. \
             # searches backwards. n/N navigate forward/backward through matches.",
            "vim",
        ),
        (
            ". — repeat last change in vim",
            Some("."),
            "Replays the last insert, delete, or substitution. \
             One of vim's most powerful features for repetitive edits.",
            "vim",
        ),
        (
            "% — jump to matching bracket in vim",
            Some("%"),
            "Jumps between matching pairs: () [] {}. \
             Also works on #if/#endif and HTML tags with matchit.",
            "vim",
        ),
        (
            "zz — center current line in vim",
            Some("zz"),
            "Scrolls the view so the current line is in the middle of the screen. \
             zt puts it at the top, zb at the bottom.",
            "vim",
        ),
        (
            "Ctrl-v — visual block mode in vim",
            Some("Ctrl-v"),
            "Select a rectangular block of text. \
             I inserts before every selected line; A appends after. \
             Useful for adding/removing indentation or prefixes in bulk.",
            "vim",
        ),
        (
            "Ctrl-o / Ctrl-i — jump list navigation in vim",
            Some("Ctrl-o"),
            "Ctrl-o jumps back to the previous location in the jump list. \
             Ctrl-i jumps forward. Lets you retrace movement across files.",
            "vim",
        ),

        // ── Fish shell ─────────────────────────────────────────────────────
        (
            "Alt-. — insert last argument of previous command in fish",
            Some("Alt-."),
            "Inserts the last word of the previous command line. \
             Press repeatedly to cycle through last arguments of older commands.",
            "fish,cli",
        ),
        (
            "Ctrl-r — fuzzy history search in fish",
            Some("Ctrl-r"),
            "Opens pager-style history search. Type to filter. \
             Works best with fzf integration (`fzf --fish | source`).",
            "fish,cli",
        ),
        (
            "Ctrl-e — edit current command in $EDITOR",
            Some("Ctrl-e"),
            "Opens the current command line in your $EDITOR for multi-line editing. \
             Save and quit to execute. Essential for long pipelines.",
            "fish,cli",
        ),
        (
            "funced — edit a fish function interactively",
            Some("funced fish_prompt"),
            "Opens the function body in your $EDITOR. \
             After saving, `funcsave fish_prompt` persists it to ~/.config/fish/functions/.",
            "fish,cli",
        ),
        (
            "abbr — fish abbreviations expand as you type",
            Some("abbr --add gs git status"),
            "Unlike aliases, abbreviations expand in-place before execution — \
             the real command is visible in history. `abbr --list` shows all.",
            "fish,cli",
        ),

        // ── Linux tools ────────────────────────────────────────────────────
        (
            "fd — fast find alternative that respects .gitignore",
            Some("fd pattern"),
            "Faster than find, colorized output, respects .gitignore by default. \
             `fd -e rs` finds all .rs files. `fd -H` includes hidden files.",
            "cli,linux",
        ),
        (
            "rg — ripgrep, fast grep alternative",
            Some("rg pattern"),
            "Respects .gitignore, searches recursively by default, \
             shows context with -C 2. `rg -t rust pattern` limits to Rust files.",
            "cli,linux",
        ),
        (
            "bat — cat with syntax highlighting and line numbers",
            Some("bat file.rs"),
            "Drop-in cat replacement with syntax highlighting, line numbers, \
             and git diff markers. `bat --plain` for plain output.",
            "cli,linux",
        ),
        (
            "jq — JSON processor",
            Some("curl -s api/endpoint | jq '.data[] | .name'"),
            "Filters and transforms JSON. `.` is identity. \
             `.key` extracts a field. `.[]` iterates arrays. \
             `jq -r` outputs raw strings without quotes.",
            "cli,linux",
        ),
        (
            "fzf — fuzzy finder for anything",
            Some("git branch | fzf"),
            "Pipe any list into fzf for interactive fuzzy selection. \
             Ctrl-r history, Ctrl-t file picker, and Alt-c cd widget \
             available via `fzf --fish | source`.",
            "cli,linux",
        ),
        (
            "sd — intuitive sed alternative",
            Some("sd 'old pattern' 'new text' file.txt"),
            "Uses regex by default, no flag juggling for literal replacements. \
             `sd -p` previews changes without writing. \
             Much simpler syntax than sed for common substitutions.",
            "cli,linux",
        ),
        (
            "hyperfine — command-line benchmarking tool",
            Some("hyperfine 'rg pattern' 'grep -r pattern'"),
            "Runs each command multiple times, warms up caches, \
             and reports mean/stddev with a comparison table. \
             `--warmup 3` for cache-sensitive benchmarks.",
            "cli,linux",
        ),
        (
            "What does HTTP 201 mean?",
            None,
            "Created. The request succeeded and a new resource was created. \
             The Location header typically points to the new resource.",
            "http",
        ),
        (
            "What does HTTP 204 mean?",
            None,
            "No Content. Success, but no response body. \
             Common for DELETE and PUT requests that return nothing.",
            "http",
        ),

        // ── Vim intermediate ───────────────────────────────────────────────
        (
            "q: — open command-line history window in vim",
            Some("q:"),
            "Opens a buffer showing your : command history. \
             Navigate, edit, and press Enter to re-run any line. \
             q/ does the same for search history.",
            "vim",
        ),
        (
            ":g/pattern/d — delete all lines matching a pattern",
            Some(":g/TODO/d"),
            "The global command runs an Ex command on every matching line. \
             :g/pattern/y A yanks all matches into register a. \
             :v/pattern/d deletes lines that do NOT match.",
            "vim",
        ),
        (
            "gf — go to file under cursor in vim",
            Some("gf"),
            "Opens the filename under the cursor. \
             Ctrl-w f opens it in a split. \
             Works with relative paths; set path+= to extend the search.",
            "vim",
        ),
        (
            ":earlier / :later — time-travel undo in vim",
            Some(":earlier 5m"),
            "Moves the buffer to its state 5 minutes ago. \
             :later 30s moves forward 30 seconds. \
             Works independently of the linear undo tree.",
            "vim",
        ),
        (
            "ga — show Unicode codepoint of character under cursor",
            Some("ga"),
            "Displays decimal, hex, and octal values of the character. \
             Useful for identifying invisible or ambiguous Unicode characters.",
            "vim",
        ),
        (
            "Ctrl-a / Ctrl-x — increment / decrement number under cursor",
            Some("Ctrl-a"),
            "Ctrl-a increments the number under or after the cursor. \
             Ctrl-x decrements. Prefix with a count: 5 Ctrl-a adds 5. \
             Works on hex (0xff) and octal (077) too.",
            "vim",
        ),

        // ── wlr-which-key ──────────────────────────────────────────────────
        (
            "wlr-which-key — keybinding cheatsheet overlay for Wayland",
            Some("Super+/"),
            "Pops up a menu showing available keybindings for the current prefix. \
             Configured in TOML; triggered by a keybind in Hyprland/Niri. \
             Like which-key in Neovim but for your compositor.",
            "wayland,niri,hyprland,cli",
        ),

        // ── systemd / journalctl / process monitoring ──────────────────────
        (
            "journalctl -u — follow a specific unit's logs",
            Some("journalctl -u nginx.service -f"),
            "-u filters to one unit, -f follows live output like tail -f. \
             --since '10 min ago' limits to recent entries. \
             -p err shows only errors and above.",
            "systemd,linux",
        ),
        (
            "journalctl -b — logs from current boot only",
            Some("journalctl -b"),
            "-b 0 is current boot, -b -1 is the previous boot. \
             Combine with -u and -p: `journalctl -b -u sshd -p err`.",
            "systemd,linux",
        ),
        (
            "systemctl status — inspect a unit",
            Some("systemctl status sshd"),
            "Shows active/failed state, recent log lines, PID, and cgroup. \
             `--user` for user units. `systemctl list-units --failed` lists all failures.",
            "systemd,linux",
        ),
        (
            "systemd-analyze blame — find slow boot units",
            Some("systemd-analyze blame"),
            "Lists units sorted by startup time. \
             `systemd-analyze critical-chain` shows the bottleneck path. \
             `systemd-analyze plot > boot.svg` generates a full timeline.",
            "systemd,linux",
        ),
        (
            "journalctl --disk-usage / vacuum",
            Some("journalctl --disk-usage"),
            "Shows total journal size on disk. \
             `journalctl --vacuum-time=2weeks` deletes entries older than 2 weeks. \
             `--vacuum-size=500M` caps total size.",
            "systemd,linux",
        ),
        (
            "ss — socket statistics (modern netstat)",
            Some("ss -tlnp"),
            "-t TCP, -u UDP, -l listening only, -n no DNS resolution, -p show process. \
             Faster than netstat. `ss -s` gives a summary.",
            "linux,cli",
        ),
        (
            "ps aux vs ps -ef — what's the difference?",
            None,
            "Both list all processes. `ps aux` is BSD-style: a=all users, u=user-oriented, x=no-tty. \
             `ps -ef` is POSIX-style: e=all, f=full format. \
             Output is similar; aux shows %CPU/%MEM, -ef shows PPID.",
            "linux,cli",
        ),
        (
            "lsof -i — list processes using network connections",
            Some("lsof -i :8080"),
            "Lists all open files; -i filters to network connections. \
             -i :port shows what's on a specific port. \
             `lsof -p PID` shows all files opened by a process.",
            "linux,cli",
        ),
        (
            "strace — trace system calls of a process",
            Some("strace -p PID"),
            "Attaches to a running process and prints every syscall. \
             -e trace=openat,read filters to specific calls. \
             -c prints a summary count at the end. Slow on production — use with care.",
            "linux,cli",
        ),
    ];

    for (title, prompt, body, tags) in cards {
        let _ = db.add_card(title, *prompt, body, tags);
    }
}
