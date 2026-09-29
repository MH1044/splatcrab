% covers: 9 - save refuses a variable that is not there, a value with no MAT form, a value -ascii cannot write, nesting past its bound and an empty workspace, each a clean error that writes no file, exit 1
% The texts are SplatCrab's own. d nests 201 cells, one past the bound of
% 200. An empty workspace is refused because a MAT-file of a header alone is
% one load refuses (item 16), so save never writes one.
h = @sin;
c = {1};
d = 1;
for k = 1:201, d = {d}; end
try, save('err_save_refusals.mat', 'q'); catch e, disp(e.message); end
try, save('err_save_refusals.mat', 'h'); catch e, disp(e.message); end
try, save('err_save_refusals.txt', 'c', '-ascii'); catch e, disp(e.message); end
try, save('err_save_refusals.mat', 'd'); catch e, disp(e.message); end
clear
save('err_save_refusals.mat')
