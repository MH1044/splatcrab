% covers: 5 - which of a path function returns its file, checked by name only because the absolute path is machine-dependent
disp(~isempty(strfind(which('addone'), 'addone.m')))
