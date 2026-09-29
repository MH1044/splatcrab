% covers: Scope - run runs a script file in the calling workspace and the current folder is back afterwards
p = pwd; run envscript; disp(y); disp(strcmp(pwd, p))
