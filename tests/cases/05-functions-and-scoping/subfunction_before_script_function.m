% covers: 14 - inside helper.m, the running file's own subfunction twice comes before the script's local twice
% The script's twice returns -1 wherever the script calls it; helper(3)
% still doubles 3 through helper.m's own twice.
disp(twice(3))
disp(helper(3))
function y = twice(x)
y = -1;
end
