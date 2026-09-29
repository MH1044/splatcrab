% covers: 5 - (Scope, call_nested) an error raised inside fzero's function reaches the script unchanged, exit 1
x = fzero(@(x) error('boom'), 1);
