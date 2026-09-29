% covers: Scope - system returns the command's status and its output
[s, o] = system('echo hi'); disp(s); disp(strtrim(o))
[s, o] = system('exit 3'); disp(s)
