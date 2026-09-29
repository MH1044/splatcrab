% covers: 15 - dir of a folder returns a struct of its entries, without . and ..
d = dir('envdir'); disp(numel(d)); disp(d(1).name); disp(d(1).isdir)
