% covers: 9 - save refuses a struct array with no fields past the 1048576 elements load keeps to, so it never writes a file its own load refuses; one at the bound is saved and loaded back, exit 1
% The refused save writes nothing, which fopen's -1 shows before the error
% is raised again. The file is named after the case and deleted.
f = 'err_save_fieldless_struct_bound.mat';
s = struct(); s(1, 1048576) = struct();
save(f, 's');
clear s
load(f);
delete(f);
fprintf('%d %d\n', size(s));
s(1, 1048577) = struct();
try, save(f, 's'); catch e, disp(fopen(f)); rethrow(e); end
delete(f);
